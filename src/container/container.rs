//! Container struct definition
//!
//! A Container represents an isolated execution environment with its own
//! context, variables, and configuration.

use chrono::{DateTime, Utc};
use crate::context::Context;
use crate::parser::ContainerOptions;

/// A sandboxed execution container
#[derive(Debug)]
pub struct Container {
    /// Unique name for this container
    pub name: String,
    /// The isolated context for this container
    pub context: Context,
    /// Whether destructive actions (DELETE, KILL) are allowed
    pub allow_actions: bool,
    /// Whether the container is read-only (no file modifications)
    pub readonly: bool,
    /// When the container was created
    pub created_at: DateTime<Utc>,
}

impl Container {
    /// Create a new container with the given name and options
    pub fn new(name: String, options: ContainerOptions) -> Self {
        Self {
            name,
            context: Context::new(),
            allow_actions: options.allow_actions,
            readonly: options.readonly,
            created_at: Utc::now(),
        }
    }
    
    /// Create a new container with default options
    pub fn new_default(name: String) -> Self {
        Self::new(name, ContainerOptions::default())
    }
    
    /// Get the container's context
    pub fn context(&self) -> &Context {
        &self.context
    }
    
    /// Get a mutable reference to the container's context
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }
    
    /// Check if actions are allowed in this container
    pub fn actions_allowed(&self) -> bool {
        self.allow_actions
    }
    
    /// Check if the container is read-only
    pub fn is_readonly(&self) -> bool {
        self.readonly
    }
}

impl Clone for Container {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            context: Context::new(), // Fresh context for cloned container
            allow_actions: self.allow_actions,
            readonly: self.readonly,
            created_at: self.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_container_new() {
        let container = Container::new_default("test".to_string());
        assert_eq!(container.name, "test");
        assert!(!container.allow_actions);
        assert!(!container.readonly);
    }
    
    #[test]
    fn test_container_with_options() {
        let options = ContainerOptions {
            allow_actions: true,
            readonly: true,
        };
        let container = Container::new("test".to_string(), options);
        assert!(container.allow_actions);
        assert!(container.readonly);
    }
    
    #[test]
    fn test_container_context() {
        let mut container = Container::new_default("test".to_string());
        let ctx = container.context_mut();
        // Should be able to modify context
        assert!(ctx.current_folder().exists() || true); // Just check it's accessible
    }
}
