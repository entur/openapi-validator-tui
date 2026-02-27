use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Cooperative cancellation token backed by an `AtomicBool`.
#[derive(Debug, Clone)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Signal cancellation. Idempotent.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Describes a container invocation. The `args` field is the full argument list
/// passed to `docker` (the pipeline layer is responsible for assembling it).
pub struct ContainerCommand {
    pub args: Vec<String>,
    pub timeout: Duration,
    pub log_path: Option<PathBuf>,
}

/// Outcome of a container run.
#[derive(Debug)]
pub struct ContainerResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub log: String,
    pub cancelled: bool,
    pub timed_out: bool,
}

/// Streamed output from a running container.
#[derive(Debug)]
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
    Done(ContainerResult),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_starts_uncancelled() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_token_transitions_once() {
        let token = CancelToken::new();
        token.cancel();
        assert!(token.is_cancelled());
        // Idempotent — calling again is fine.
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn cancel_token_is_visible_across_clones() {
        let a = CancelToken::new();
        let b = a.clone();
        a.cancel();
        assert!(b.is_cancelled());
    }
}
