//! Action implementations (system modifications)

pub mod files;
pub mod process;

pub use files::delete_files;
pub use process::kill_processes;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action_type: String,
    pub affected_count: usize,
    pub dry_run: bool,
    pub details: Vec<String>,
}
