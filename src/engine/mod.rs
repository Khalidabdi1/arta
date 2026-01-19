//! Execution engine for Arta commands

pub mod executor;
pub mod queries;
pub mod actions;

pub use executor::{execute_command, execute_command_with_context, ExecutionContext, ExecutionResult, ResultData};
