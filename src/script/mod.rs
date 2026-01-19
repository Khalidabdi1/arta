//! Script execution module for Arta
//!
//! Handles loading, validating, and executing .arta script files.

pub mod runner;
pub mod validator;

pub use runner::{explain_script, ScriptResult, ScriptRunner};
pub use validator::{
    has_errors, has_warnings, validate_script, ScriptValidationError, ValidationOptions,
    ValidationSeverity,
};
