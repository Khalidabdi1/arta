//! Arta - A SQL-like DSL for querying and managing system state
//!
//! Arta provides a familiar SQL-like syntax to query system information
//! and perform controlled system actions.
//!
//! # Example
//!
//! ```no_run
//! use arta::{parse_command, execute_command, ExecutionContext, OutputFormat, format_output};
//!
//! let cmd = parse_command("SELECT CPU *").unwrap();
//! let ctx = ExecutionContext::default();
//! let result = execute_command(&cmd, &ctx).unwrap();
//! println!("{}", format_output(&result, &OutputFormat::Human));
//! ```

pub mod parser;
pub mod engine;
pub mod security;
pub mod output;
pub mod error;
pub mod cli;
pub mod context;
pub mod script;
pub mod life;
pub mod container;

#[cfg(feature = "repl")]
pub mod repl;

pub use parser::{parse_command, parse_script, Command, Script};
pub use engine::{execute_command, execute_command_with_context, ExecutionContext};
pub use error::{ArtaError, Result};
pub use output::{OutputFormat, format_output};
pub use context::Context;
pub use script::{ScriptRunner, ScriptResult, validate_script};
pub use container::{Container, ContainerManager};
