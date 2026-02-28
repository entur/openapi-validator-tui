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
