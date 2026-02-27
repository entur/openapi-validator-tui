mod discovery;
mod parser;
mod types;

pub use discovery::{discover_spec, normalize_spec_path};
pub use parser::parse_spec;
pub use types::{ContextWindow, SourceSpan, SpecIndex};
