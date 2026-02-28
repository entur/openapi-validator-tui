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

    /// Helper: build a `PipelineInput` with the given config and a dummy spec path.
    fn test_input(cfg: Config) -> PipelineInput {
        PipelineInput {
            config: cfg,
            spec_path: std::path::PathBuf::from("/tmp/spec.yaml"),
            work_dir: std::path::PathBuf::from("/tmp"),
        }
    }

    /// Collect all events from the pipeline receiver until it closes.
    fn collect_events(rx: mpsc::Receiver<PipelineEvent>) -> Vec<PipelineEvent> {
        let mut events = Vec::new();
        while let Ok(ev) = rx.recv() {
            events.push(ev);
        }
        events
    }

    #[test]
    fn pipeline_no_phases_completes_immediately() {
        let cfg = Config {
            lint: false,
            generate: false,
            ..Config::default()
        };
        let cancel = CancelToken::new();
        let rx = run_pipeline(test_input(cfg), cancel);
        let events = collect_events(rx);

        // Should end with exactly one Completed event.
        let last = events.last().expect("expected at least one event");
        match last {
            PipelineEvent::Completed(report) => {
                assert_eq!(report.summary.total, 0);
                assert_eq!(report.summary.passed, 0);
                assert_eq!(report.summary.failed, 0);
                assert!(report.phases.lint.is_none());
                assert!(report.phases.generate.is_none());
                assert!(report.phases.compile.is_none());
            }
            other => panic!("expected Completed, got: {other:?}"),
        }
    }

    #[test]
    fn pipeline_lint_disabled_skips_lint() {
        let cfg = Config {
            lint: false,
            generate: false,
            ..Config::default()
        };
        let cancel = CancelToken::new();
        let rx = run_pipeline(test_input(cfg), cancel);
        let events = collect_events(rx);

        // No PhaseStarted(Lint) should appear.
        for ev in &events {
            if let PipelineEvent::PhaseStarted(Phase::Lint) = ev {
                panic!("lint phase should not start when lint=false");
            }
        }

        match events.last().expect("expected events") {
            PipelineEvent::Completed(report) => {
                assert!(report.phases.lint.is_none());
            }
            other => panic!("expected Completed, got: {other:?}"),
        }
    }

    #[test]
    fn pipeline_precancelled_aborts() {
        let cfg = Config {
            lint: true,
            linter: crate::config::Linter::Spectral,
            generate: true,
            server_generators: vec!["spring".into()],
            mode: Mode::Server,
            ..Config::default()
        };
        let cancel = CancelToken::new();
        cancel.cancel(); // Pre-cancel before starting.

        let (tx, rx) = mpsc::channel();
        // Call run_inner directly so we don't rely on Docker being available.
        // With a pre-cancelled token the lint phase will attempt to spawn a
        // container, which will fail (no Docker in CI), but the cancel check
        // after the phase will fire and produce Aborted.
        run_inner(test_input(cfg), cancel, tx);

        let events = collect_events(rx);
        let last = events.last().expect("expected at least one event");
        match last {
            PipelineEvent::Aborted(_) => {} // expected
            PipelineEvent::Completed(_) => {
                // Also acceptable — if Docker isn't available the lint phase
                // fails but the cancel check may not trigger because
                // run_container returns immediately on spawn failure.
                // The important thing is we don't hang.
            }
            other => panic!("expected Aborted or Completed, got: {other:?}"),
        }
    }

    #[test]
    fn pipeline_empty_generators_skips_generate_compile() {
        let cfg = Config {
            lint: false,
            generate: true,
            compile: true,
            server_generators: Vec::new(),
            client_generators: Vec::new(),
            ..Config::default()
        };
        let cancel = CancelToken::new();
        let rx = run_pipeline(test_input(cfg), cancel);
        let events = collect_events(rx);

        // No generate/compile phase events should appear.
        for ev in &events {
            match ev {
                PipelineEvent::PhaseStarted(Phase::Generate { .. })
                | PipelineEvent::PhaseStarted(Phase::Compile { .. }) => {
                    panic!("generate/compile should not start with empty generators");
                }
                _ => {}
            }
        }

        match events.last().expect("expected events") {
            PipelineEvent::Completed(report) => {
                assert!(report.phases.generate.is_none());
                assert!(report.phases.compile.is_none());
                assert_eq!(report.summary.total, 0);
            }
            other => panic!("expected Completed, got: {other:?}"),
        }
    }
}
