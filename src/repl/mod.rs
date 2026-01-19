//! REPL module for interactive mode

#[cfg(feature = "repl")]
pub mod interactive;

#[cfg(feature = "repl")]
pub use interactive::run_repl;

#[cfg(not(feature = "repl"))]
pub fn run_repl() -> crate::error::Result<()> {
    Err(crate::error::ArtaError::ExecutionError(
        "REPL not enabled. Rebuild with --features repl".to_string(),
    ))
}
