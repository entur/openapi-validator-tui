use std::sync::mpsc::{self, Receiver, Sender};

use crate::config::Linter;
use crate::docker::{self, CancelToken, OutputLine};

use super::commands::{
    build_generator_list, compile_command, generator_command, redocly_command, spectral_command,
};
use super::types::{
    LintResult, Phase, Phases, PipelineEvent, PipelineInput, StepResult, Summary, ValidateReport,
};

/// Launch the validation pipeline on a background thread.
///
/// Returns a receiver that streams `PipelineEvent` values. The final event
/// is always either `Completed` or `Aborted`.
pub fn run_pipeline(input: PipelineInput, cancel: CancelToken) -> Receiver<PipelineEvent> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        run_inner(input, cancel, tx);
    });
    rx
}

fn run_inner(input: PipelineInput, cancel: CancelToken, tx: Sender<PipelineEvent>) {
    let cfg = &input.config;
    let mut phases = Phases::default();
    let mut total: usize = 0;
    let mut passed: usize = 0;
    let mut failed: usize = 0;

    // ── Lint ──────────────────────────────────────────────────────────
    if cfg.lint && cfg.linter != Linter::None {
        let phase = Phase::Lint;
        let _ = tx.send(PipelineEvent::PhaseStarted(phase.clone()));

        let cmd = match cfg.linter {
            Linter::Spectral => spectral_command(cfg, &input.spec_path, &input.work_dir),
            Linter::Redocly => redocly_command(cfg, &input.spec_path, &input.work_dir),
            Linter::None => unreachable!(),
        };

        let outcome = run_container(cmd, &cancel, &phase, &tx);
        total += 1;

        let lint_success = outcome.success;
        if lint_success {
            passed += 1;
        } else {
            failed += 1;
        }

        phases.lint = Some(LintResult {
            linter: cfg.linter.as_str().to_string(),
            status: if lint_success { "pass" } else { "fail" }.to_string(),
            log: outcome.log,
        });

        let _ = tx.send(PipelineEvent::PhaseFinished {
            phase: phase.clone(),
            success: lint_success,
        });

        if cancel.is_cancelled() {
            let _ = tx.send(PipelineEvent::Aborted("Cancelled by user".into()));
            return;
        }
    }

    // ── Generate ─────────────────────────────────────────────────────
    let generators = build_generator_list(cfg);

    if cfg.generate && !generators.is_empty() {
        let gen_results =
            run_steps_parallel(&generators, cfg, &input, &cancel, &tx, StepKind::Generate);

        if cancel.is_cancelled() {
            let _ = tx.send(PipelineEvent::Aborted("Cancelled by user".into()));
            return;
        }

        let all_passed = gen_results.iter().all(|r| r.status == "pass");
        for r in &gen_results {
            total += 1;
            if r.status == "pass" {
                passed += 1;
            } else {
                failed += 1;
            }
        }
        phases.generate = Some(gen_results);

        // ── Compile (only if all generators passed) ──────────────────
        if cfg.compile && all_passed {
            let compile_results =
                run_steps_parallel(&generators, cfg, &input, &cancel, &tx, StepKind::Compile);

            if cancel.is_cancelled() {
                let _ = tx.send(PipelineEvent::Aborted("Cancelled by user".into()));
                return;
            }

            for r in &compile_results {
                total += 1;
                if r.status == "pass" {
                    passed += 1;
                } else {
                    failed += 1;
                }
            }
            phases.compile = Some(compile_results);
        }
    }

    let report = ValidateReport {
        spec: input
            .spec_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        mode: cfg.mode.as_str().to_string(),
        phases,
        summary: Summary {
            total,
            passed,
            failed,
        },
    };

    let _ = tx.send(PipelineEvent::Completed(report));
}

#[derive(Clone, Copy)]
enum StepKind {
    Generate,
    Compile,
}

/// Run a set of generator/compile steps with bounded parallelism.
fn run_steps_parallel(
    generators: &[(String, String)],
    cfg: &crate::config::Config,
    input: &PipelineInput,
    cancel: &CancelToken,
    tx: &Sender<PipelineEvent>,
    kind: StepKind,
) -> Vec<StepResult> {
    let jobs = cfg.jobs.resolve();
    let mut results = Vec::with_capacity(generators.len());

    for chunk in generators.chunks(jobs) {
        if cancel.is_cancelled() {
            break;
        }

        let handles: Vec<_> = chunk
            .iter()
            .map(|(gen_name, scope)| {
                let phase = match kind {
                    StepKind::Generate => Phase::Generate {
                        generator: gen_name.clone(),
                        scope: scope.clone(),
                    },
                    StepKind::Compile => Phase::Compile {
                        generator: gen_name.clone(),
                        scope: scope.clone(),
                    },
                };

                let cmd = match kind {
                    StepKind::Generate => {
                        generator_command(cfg, &input.spec_path, &input.work_dir, gen_name, scope)
                    }
                    StepKind::Compile => compile_command(cfg, &input.work_dir, gen_name, scope),
                };

                let cancel = cancel.clone();
                let tx = tx.clone();
                let phase_clone = phase.clone();
                let gen_name = gen_name.clone();
                let scope = scope.clone();

                std::thread::spawn(move || {
                    let _ = tx.send(PipelineEvent::PhaseStarted(phase_clone.clone()));
                    let outcome = run_container(cmd, &cancel, &phase_clone, &tx);
                    let success = outcome.success;
                    let _ = tx.send(PipelineEvent::PhaseFinished {
                        phase: phase_clone,
                        success,
                    });
                    StepResult {
                        generator: gen_name,
                        scope,
                        status: if success { "pass" } else { "fail" }.to_string(),
                        log: outcome.log,
                    }
                })
            })
            .collect();

        for handle in handles {
            if let Ok(result) = handle.join() {
                results.push(result);
            }
        }
    }

    results
}

struct ContainerOutcome {
    success: bool,
    log: String,
}

/// Run a single container, draining its output channel and forwarding
/// lines as `PipelineEvent::Log`.
fn run_container(
    cmd: docker::ContainerCommand,
    cancel: &CancelToken,
    phase: &Phase,
    tx: &Sender<PipelineEvent>,
) -> ContainerOutcome {
    let container_rx = match docker::spawn(cmd, cancel.clone()) {
        Ok(rx) => rx,
        Err(e) => {
            return ContainerOutcome {
                success: false,
                log: format!("Failed to spawn container: {e}"),
            };
        }
    };

    let mut log = String::new();
    let mut success = false;

    for line in container_rx {
        match line {
            OutputLine::Stdout(s) | OutputLine::Stderr(s) => {
                let _ = tx.send(PipelineEvent::Log {
                    phase: phase.clone(),
                    line: s.clone(),
                });
                log.push_str(&s);
                log.push('\n');
            }
            OutputLine::Done(result) => {
                success = result.success;
                if result.cancelled {
                    success = false;
                }
                // Prefer the container's accumulated log if our line-by-line
                // accumulation missed anything.
                if log.is_empty() {
                    log = result.log;
                }
                break;
            }
        }
    }

    ContainerOutcome { success, log }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Mode};

    #[test]
    fn build_generator_list_determines_step_count() {
        let cfg = Config {
            mode: Mode::Both,
            server_generators: vec!["spring".into(), "go-server".into()],
            client_generators: vec!["typescript-axios".into()],
            ..Config::default()
        };
        let pairs = build_generator_list(&cfg);
        assert_eq!(pairs.len(), 3);
    }

    #[test]
    fn report_assembly_with_empty_phases() {
        let report = ValidateReport {
            spec: "test.yaml".into(),
            mode: "server".into(),
            phases: Phases::default(),
            summary: Summary {
                total: 0,
                passed: 0,
                failed: 0,
            },
        };
        assert_eq!(report.summary.total, 0);
        assert!(report.phases.lint.is_none());
        assert!(report.phases.generate.is_none());
        assert!(report.phases.compile.is_none());
    }

    #[test]
    fn report_assembly_with_lint_only() {
        let report = ValidateReport {
            spec: "test.yaml".into(),
            mode: "server".into(),
            phases: Phases {
                lint: Some(LintResult {
                    linter: "spectral".into(),
                    status: "pass".into(),
                    log: "all good".into(),
                }),
                generate: None,
                compile: None,
            },
            summary: Summary {
                total: 1,
                passed: 1,
                failed: 0,
            },
        };
        assert_eq!(report.summary.total, 1);
        assert!(report.phases.lint.is_some());
    }

    #[test]
    fn step_result_status_values() {
        let pass = StepResult {
            generator: "spring".into(),
            scope: "server".into(),
            status: "pass".into(),
            log: String::new(),
        };
        let fail = StepResult {
            generator: "go".into(),
            scope: "client".into(),
            status: "fail".into(),
            log: "compile error".into(),
        };
        assert_eq!(pass.status, "pass");
        assert_eq!(fail.status, "fail");
    }

    #[test]
    fn phase_enum_equality() {
        let a = Phase::Generate {
            generator: "spring".into(),
            scope: "server".into(),
        };
        let b = Phase::Generate {
            generator: "spring".into(),
            scope: "server".into(),
        };
        assert_eq!(a, b);

        let c = Phase::Compile {
            generator: "spring".into(),
            scope: "server".into(),
        };
        assert_ne!(a, c);
    }
}
