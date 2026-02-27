use serde::Deserialize;

/// Mirrors the CLI's ValidateReport JSON structure.
#[derive(Debug, Deserialize)]
pub struct ValidateReport {
    pub spec: String,
    pub mode: String,
    pub phases: Phases,
    pub summary: Summary,
}

#[derive(Debug, Deserialize)]
pub struct Phases {
    pub lint: Option<LintResult>,
    pub generate: Option<Vec<StepResult>>,
    pub compile: Option<Vec<StepResult>>,
}

#[derive(Debug, Deserialize)]
pub struct LintResult {
    pub linter: String,
    pub status: String,
    pub log: String,
}

#[derive(Debug, Deserialize)]
pub struct StepResult {
    pub generator: String,
    pub scope: String,
    pub status: String,
    pub log: String,
}

#[derive(Debug, Deserialize)]
pub struct Summary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}
