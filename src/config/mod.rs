mod loader;
mod types;

pub use loader::{load, validate};
pub use types::{Config, Jobs, Linter, Mode};
