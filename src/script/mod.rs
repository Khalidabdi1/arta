//! Script execution module for Arta
//! 
//! Handles loading, validating, and executing .arta script files.

pub mod runner;
pub mod validator;

pub use runner::{ScriptRunner, ScriptResult, explain_script};
pub use validator::{validate_script, ScriptValidationError, ValidationOptions, ValidationSeverity, has_errors, has_warnings};
