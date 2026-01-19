//! Error types for Arta

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArtaError {
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Execution error: {0}")]
    ExecutionError(String),
    
    #[error("Security error: {0}")]
    SecurityError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Actions not enabled. Use --allow-actions flag to enable system modifications")]
    ActionsDisabled,
    
    #[error("Invalid query target: {0}")]
    InvalidTarget(String),
    
    #[error("Invalid field: {0}")]
    InvalidField(String),
    
    #[error("Path not found: {0}")]
    PathNotFound(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

pub type Result<T> = std::result::Result<T, ArtaError>;
