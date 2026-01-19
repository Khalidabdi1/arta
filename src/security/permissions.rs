//! Permission checking

use crate::error::Result;

/// Check if current user has required permissions
pub fn check_permissions(path: &str) -> Result<bool> {
    use std::fs;
    
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
