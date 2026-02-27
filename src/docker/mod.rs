// Docker orchestration — container management, streaming output, cancellation.

pub mod engine;
pub mod run;
pub mod types;

pub use engine::{ensure_available, user_args};
pub use run::spawn;
pub use types::{CancelToken, ContainerCommand, ContainerResult, OutputLine};
