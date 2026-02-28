use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Mirrors the CLI's ValidateReport JSON structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidateReport {
    pub spec: String,
    pub mode: String,
    pub phases: Phases,
    pub summary: Summary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Phases {
    pub lint: Option<LintResult>,
    pub generate: Option<Vec<StepResult>>,
    pub compile: Option<Vec<StepResult>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LintResult {
    pub linter: String,
    pub status: String,
    pub log: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepResult {
    pub generator: String,
    pub scope: String,
    pub status: String,
    pub log: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

/// Input to the validation pipeline.
pub struct PipelineInput {
    pub config: Config,
    pub spec_path: PathBuf,
    pub work_dir: PathBuf,
}

/// Identifies which pipeline phase is running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Lint,
    Generate { generator: String, scope: String },
    Compile { generator: String, scope: String },
}

/// Events emitted by the pipeline orchestrator.
#[derive(Debug)]
#[allow(dead_code)]
pub enum PipelineEvent {
    PhaseStarted(Phase),
    Log { phase: Phase, line: String },
    PhaseFinished { phase: Phase, success: bool },
    Completed(ValidateReport),
    Aborted(String),
}
