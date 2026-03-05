//! Integration tests for the validation pipeline.
//!
//! These require a running Docker daemon and are marked `#[ignore]`.
//! Run with: `cargo test -- --ignored`

use std::path::PathBuf;
use std::sync::mpsc;

use lazyoav::config::{Config, Linter, Mode};
use lazyoav::docker::CancelToken;
use lazyoav::pipeline::{PipelineEvent, PipelineInput, run_pipeline};

/// Copy the bundled petstore spec into a temporary work directory.
fn setup_workdir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/petstore.yaml");
    let dest = dir.path().join("petstore.yaml");
    std::fs::copy(&src, &dest).expect("failed to copy petstore.yaml");
    (dir, dest)
}

/// Collect all events from the pipeline receiver.
fn collect_events(rx: mpsc::Receiver<PipelineEvent>) -> Vec<PipelineEvent> {
    let mut events = Vec::new();
    while let Ok(ev) = rx.recv() {
        events.push(ev);
    }
    events
}

#[test]
#[ignore]
fn spectral_lint_petstore() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: true,
        generate: false,
        linter: Linter::Spectral,
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    let last = events.last().expect("expected at least one event");
    match last {
        PipelineEvent::Completed(report) => {
            assert!(
                report.phases.lint.is_some(),
                "lint phase should be populated"
            );
            let lint = report.phases.lint.as_ref().unwrap();
            assert_eq!(lint.linter, "spectral");
            assert!(
                lint.status == "pass" || lint.status == "fail",
                "unexpected lint status: {}",
                lint.status
            );
        }
        other => panic!("expected Completed, got: {other:?}"),
    }
}

#[test]
#[ignore]
fn generate_single_generator() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: false,
        generate: true,
        compile: false,
        mode: Mode::Server,
        server_generators: vec!["java".into()],
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    let last = events.last().expect("expected at least one event");
    match last {
        PipelineEvent::Completed(report) => {
            let steps = report
                .phases
                .generate
                .as_ref()
                .expect("generate phase should be populated");
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0].generator, "java");
            assert_eq!(steps[0].scope, "server");
        }
        other => panic!("expected Completed, got: {other:?}"),
    }
}

#[test]
#[ignore]
fn full_pipeline_lint_generate_compile() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: true,
        generate: true,
        compile: true,
        linter: Linter::Spectral,
        mode: Mode::Server,
        server_generators: vec!["java".into()],
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    let last = events.last().expect("expected at least one event");
    match last {
        PipelineEvent::Completed(report) => {
            assert!(report.phases.lint.is_some(), "lint phase missing");
            assert!(report.phases.generate.is_some(), "generate phase missing");

            // Compile may be None if generation failed — only assert if
            // generation passed.
            let gen_steps = report.phases.generate.as_ref().unwrap();
            let all_passed = gen_steps.iter().all(|r| r.status == "pass");
            if all_passed {
                assert!(report.phases.compile.is_some(), "compile phase missing");
            }

            // Summary totals should match phase counts.
            let expected_total = 1 // lint
                + gen_steps.len()
                + report.phases.compile.as_ref().map_or(0, |c| c.len());
            assert_eq!(report.summary.total, expected_total);
            assert_eq!(
                report.summary.passed + report.summary.failed,
                report.summary.total
            );
        }
        other => panic!("expected Completed, got: {other:?}"),
    }
}

#[test]
#[ignore]
fn redocly_lint_petstore() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: true,
        generate: false,
        linter: Linter::Redocly,
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    let last = events.last().expect("expected at least one event");
    match last {
        PipelineEvent::Completed(report) => {
            let lint = report
                .phases
                .lint
                .as_ref()
                .expect("lint phase should be populated");
            assert_eq!(lint.linter, "redocly");
            assert!(
                lint.status == "pass" || lint.status == "fail",
                "unexpected lint status: {}",
                lint.status
            );
        }
        other => panic!("expected Completed, got: {other:?}"),
    }
}

#[test]
#[ignore]
fn generate_client_typescript_axios() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: false,
        generate: true,
        compile: false,
        mode: Mode::Client,
        client_generators: vec!["typescript-axios".into()],
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    match events.last().expect("expected events") {
        PipelineEvent::Completed(report) => {
            let steps = report
                .phases
                .generate
                .as_ref()
                .expect("generate phase should be populated");
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0].generator, "typescript-axios");
            assert_eq!(steps[0].scope, "client");
        }
        other => panic!("expected Completed, got: {other:?}"),
    }
}

#[test]
#[ignore]
fn generate_parallel_server_and_client() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: false,
        generate: true,
        compile: false,
        mode: Mode::Both,
        server_generators: vec!["spring".into()],
        client_generators: vec!["typescript-axios".into()],
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    match events.last().expect("expected events") {
        PipelineEvent::Completed(report) => {
            let steps = report
                .phases
                .generate
                .as_ref()
                .expect("generate phase should be populated");
            assert_eq!(steps.len(), 2);
            let names: Vec<&str> = steps.iter().map(|s| s.generator.as_str()).collect();
            assert!(names.contains(&"spring"), "missing spring: {names:?}");
            assert!(
                names.contains(&"typescript-axios"),
                "missing typescript-axios: {names:?}"
            );
        }
        other => panic!("expected Completed, got: {other:?}"),
    }
}

#[test]
#[ignore]
fn generated_code_exists_on_host() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: false,
        generate: true,
        compile: false,
        mode: Mode::Server,
        server_generators: vec!["spring".into()],
        ..Config::default()
    };

    // Pipeline needs the .oav directory tree to exist.
    lazyoav::scaffold::ensure_oav_dirs(dir.path()).unwrap();

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    match events.last().expect("expected events") {
        PipelineEvent::Completed(report) => {
            let steps = report.phases.generate.as_ref().unwrap();
            if steps[0].status == "pass" {
                let gen_dir = dir.path().join(".oav/generated/server/spring");
                assert!(gen_dir.exists(), "generated output dir should exist on host");
                // Should contain at least one file (e.g. pom.xml, build.gradle, etc.)
                let file_count = std::fs::read_dir(&gen_dir)
                    .expect("should read generated dir")
                    .count();
                assert!(file_count > 0, "generated dir should not be empty");
            }
        }
        other => panic!("expected Completed, got: {other:?}"),
    }
}

#[test]
#[ignore]
fn report_json_persisted_to_disk() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: true,
        generate: false,
        linter: Linter::Spectral,
        ..Config::default()
    };

    // Ensure report directory exists.
    lazyoav::scaffold::ensure_oav_dirs(dir.path()).unwrap();

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    collect_events(rx);

    let report_path = dir.path().join(".oav/reports/report.json");
    assert!(report_path.exists(), "report.json should be written");

    let content = std::fs::read_to_string(&report_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&content).expect("should be valid JSON");
    assert!(report.get("spec").is_some());
    assert!(report.get("summary").is_some());
    assert!(report.get("phases").is_some());
}

#[test]
#[ignore]
fn log_files_written_for_phases() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: true,
        generate: true,
        compile: false,
        linter: Linter::Spectral,
        mode: Mode::Server,
        server_generators: vec!["spring".into()],
        ..Config::default()
    };

    lazyoav::scaffold::ensure_oav_dirs(dir.path()).unwrap();

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    collect_events(rx);

    // Spectral lint log.
    let lint_log = dir.path().join(".oav/reports/lint/spectral.log");
    assert!(lint_log.exists(), "spectral lint log should be written");
    assert!(
        std::fs::read_to_string(&lint_log).unwrap().len() > 0,
        "lint log should not be empty"
    );

    // Generate log.
    let gen_log = dir
        .path()
        .join(".oav/reports/generate/server/spring.log");
    assert!(gen_log.exists(), "generate log should be written");
}

#[test]
#[ignore]
fn custom_generator_runs_via_openapi_cli() {
    let (dir, spec_path) = setup_workdir();
    lazyoav::scaffold::ensure_oav_dirs(dir.path()).unwrap();

    let custom_defs = vec![lazyoav::custom::CustomGeneratorDef {
        name: "custom-go".into(),
        scope: "server".into(),
        generate: lazyoav::custom::GenerateBlock {
            image: "openapitools/openapi-generator-cli:v7.17.0".into(),
            command: "generate -i {spec} -g go-server -o /work/.oav/generated/server/custom-go"
                .into(),
        },
        compile: None,
    }];

    let cfg = Config {
        lint: false,
        generate: true,
        compile: true, // compile=true but custom has no compile block → should skip
        mode: Mode::Server,
        server_generators: vec!["custom-go".into()],
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs,
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    match events.last().expect("expected events") {
        PipelineEvent::Completed(report) => {
            let gen_steps = report
                .phases
                .generate
                .as_ref()
                .expect("generate phase should be populated");
            assert_eq!(gen_steps.len(), 1);
            assert_eq!(gen_steps[0].generator, "custom-go");
            assert_eq!(gen_steps[0].scope, "server");

            if gen_steps[0].status == "pass" {
                // Generated code should exist on disk.
                let gen_dir = dir.path().join(".oav/generated/server/custom-go");
                assert!(gen_dir.exists(), "custom generator output should exist");

                // Compile phase should exist (no-op pass for custom without compile block).
                let compile_steps = report
                    .phases
                    .compile
                    .as_ref()
                    .expect("compile phase should exist even for no-op");
                assert_eq!(compile_steps.len(), 1);
                assert_eq!(compile_steps[0].status, "pass");
            }
        }
        other => panic!("expected Completed, got: {other:?}"),
    }
}

#[test]
#[ignore]
fn pipeline_events_ordering() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: true,
        generate: true,
        compile: false,
        linter: Linter::Spectral,
        mode: Mode::Server,
        server_generators: vec!["spring".into()],
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel);
    let events = collect_events(rx);

    // Every PhaseStarted must have a matching PhaseFinished before Completed.
    let mut started = Vec::new();
    let mut finished = Vec::new();
    for ev in &events {
        match ev {
            PipelineEvent::PhaseStarted(p) => started.push(p.clone()),
            PipelineEvent::PhaseFinished { phase, .. } => finished.push(phase.clone()),
            _ => {}
        }
    }

    assert_eq!(
        started.len(),
        finished.len(),
        "every started phase must have a matching finished"
    );
    for (s, f) in started.iter().zip(finished.iter()) {
        assert_eq!(s, f, "started/finished phases should match in order");
    }

    // Last event must be Completed.
    assert!(
        matches!(events.last(), Some(PipelineEvent::Completed(_))),
        "last event should be Completed"
    );

    // Lint PhaseStarted should come before any Generate PhaseStarted.
    let lint_start_idx = events
        .iter()
        .position(|e| matches!(e, PipelineEvent::PhaseStarted(lazyoav::pipeline::Phase::Lint)));
    let gen_start_idx = events.iter().position(|e| {
        matches!(
            e,
            PipelineEvent::PhaseStarted(lazyoav::pipeline::Phase::Generate { .. })
        )
    });
    if let (Some(l), Some(g)) = (lint_start_idx, gen_start_idx) {
        assert!(l < g, "lint should start before generate");
    }
}

#[test]
#[ignore]
fn cancel_mid_pipeline() {
    let (dir, spec_path) = setup_workdir();

    let cfg = Config {
        lint: true,
        generate: true,
        compile: true,
        linter: Linter::Spectral,
        mode: Mode::Server,
        server_generators: vec!["java".into()],
        ..Config::default()
    };

    let input = PipelineInput {
        config: cfg,
        custom_defs: Vec::new(),
        spec_path,
        work_dir: dir.path().to_path_buf(),
    };

    let cancel = CancelToken::new();
    let rx = run_pipeline(input, cancel.clone());

    // Wait for the first PhaseStarted event, then cancel.
    let mut got_phase_started = false;
    let mut events = Vec::new();
    while let Ok(ev) = rx.recv() {
        if matches!(&ev, PipelineEvent::PhaseStarted(_)) && !got_phase_started {
            got_phase_started = true;
            cancel.cancel();
        }
        let is_terminal = matches!(&ev, PipelineEvent::Completed(_) | PipelineEvent::Aborted(_));
        events.push(ev);
        if is_terminal {
            break;
        }
    }

    assert!(
        got_phase_started,
        "should have received at least one PhaseStarted"
    );

    let last = events.last().expect("expected events");
    match last {
        PipelineEvent::Aborted(_) => {} // expected
        PipelineEvent::Completed(_) => {
            // Acceptable if the phase completed before cancellation was observed.
        }
        other => panic!("expected Aborted or Completed, got: {other:?}"),
    }
}
