//! Container module for sandboxed execution environments
//!
//! Containers provide isolated execution contexts with their own:
//! - Context state (variables, folder stack, current file)
//! - LIFE monitoring loops
//! - Configurable permissions (allow_actions, readonly)

mod manager;
mod types;

pub use manager::ContainerManager;
pub use types::Container;
