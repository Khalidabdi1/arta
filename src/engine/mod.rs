//! Execution engine for Arta commands

pub mod actions;
pub mod executor;
pub mod queries;

pub use executor::{
    execute_command, execute_command_with_context, ExecutionContext, ExecutionResult, ResultData,
};
