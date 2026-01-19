//! Container manager for managing multiple containers
//!
//! The ContainerManager handles creating, switching between, and destroying
//! containers. It always maintains a "default" container.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::types::Container;
use crate::error::{ArtaError, Result};
use crate::parser::ContainerOptions;

/// Default container name
pub const DEFAULT_CONTAINER: &str = "default";

/// Manages multiple containers and tracks the active one
#[derive(Debug)]
pub struct ContainerManager {
    /// All containers, keyed by name
    containers: HashMap<String, Container>,
    /// Name of the currently active container
    active: String,
}

impl ContainerManager {
    /// Create a new ContainerManager with a default container
    pub fn new() -> Self {
        let mut containers = HashMap::new();
        containers.insert(
            DEFAULT_CONTAINER.to_string(),
            Container::new_default(DEFAULT_CONTAINER.to_string()),
        );

        Self {
            containers,
            active: DEFAULT_CONTAINER.to_string(),
        }
    }

    /// Create a new container
    pub fn create(&mut self, name: &str, options: ContainerOptions) -> Result<&mut Container> {
        if self.containers.contains_key(name) {
            return Err(ArtaError::ExecutionError(format!(
                "Container '{}' already exists",
                name
            )));
        }

        let container = Container::new(name.to_string(), options);
        self.containers.insert(name.to_string(), container);

        Ok(self.containers.get_mut(name).unwrap())
    }

    /// Switch to a different container
    pub fn switch(&mut self, name: &str) -> Result<()> {
        if !self.containers.contains_key(name) {
            return Err(ArtaError::ExecutionError(format!(
                "Container '{}' does not exist",
                name
            )));
        }

        self.active = name.to_string();
        Ok(())
    }

    /// Destroy a container (cannot destroy the default container)
    pub fn destroy(&mut self, name: &str) -> Result<()> {
        if name == DEFAULT_CONTAINER {
            return Err(ArtaError::ExecutionError(
                "Cannot destroy the default container".to_string(),
            ));
        }

        if !self.containers.contains_key(name) {
            return Err(ArtaError::ExecutionError(format!(
                "Container '{}' does not exist",
                name
            )));
        }

        // If destroying the active container, switch back to default
        if self.active == name {
            self.active = DEFAULT_CONTAINER.to_string();
        }

        self.containers.remove(name);
        Ok(())
    }

    /// List all container names
    pub fn list(&self) -> Vec<&str> {
        self.containers.keys().map(|s| s.as_str()).collect()
    }

    /// Get the active container
    pub fn active(&self) -> &Container {
        self.containers.get(&self.active).unwrap()
    }

    /// Get a mutable reference to the active container
    pub fn active_mut(&mut self) -> &mut Container {
        self.containers.get_mut(&self.active).unwrap()
    }

    /// Get the name of the active container
    pub fn active_name(&self) -> &str {
        &self.active
    }

    /// Get a container by name
    pub fn get(&self, name: &str) -> Option<&Container> {
        self.containers.get(name)
    }

    /// Get a mutable reference to a container by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Container> {
        self.containers.get_mut(name)
    }

    /// Check if a container exists
    pub fn exists(&self, name: &str) -> bool {
        self.containers.contains_key(name)
    }

    /// Export a container to a script file
    pub fn export(&self, name: &str, path: &Path) -> Result<()> {
        let container = self.containers.get(name).ok_or_else(|| {
            ArtaError::ExecutionError(format!("Container '{}' does not exist", name))
        })?;

        // Generate script content
        let mut script = String::new();
        script.push_str(&format!("-- Exported container: {}\n", container.name));
        script.push_str(&format!("-- Created: {}\n", container.created_at));
        script.push_str(&format!("-- Allow actions: {}\n", container.allow_actions));
        script.push_str(&format!("-- Readonly: {}\n\n", container.readonly));

        // Export variables
        for (key, value) in container.context.variables() {
            script.push_str(&format!("LET {} = {};\n", key, value));
        }

        // Export current folder context
        let current_folder = container.context.current_folder();
        script.push_str(&format!(
            "\nENTER FOLDER \"{}\";\n",
            current_folder.display()
        ));

        // Write to file
        fs::write(path, script).map_err(ArtaError::IoError)?;

        Ok(())
    }

    /// Get the number of containers
    pub fn count(&self) -> usize {
        self.containers.len()
    }
}

impl Default for ContainerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_new() {
        let manager = ContainerManager::new();
        assert_eq!(manager.count(), 1);
        assert!(manager.exists(DEFAULT_CONTAINER));
        assert_eq!(manager.active_name(), DEFAULT_CONTAINER);
    }

    #[test]
    fn test_manager_create() {
        let mut manager = ContainerManager::new();
        manager.create("test", ContainerOptions::default()).unwrap();
        assert_eq!(manager.count(), 2);
        assert!(manager.exists("test"));
    }

    #[test]
    fn test_manager_create_duplicate() {
        let mut manager = ContainerManager::new();
        manager.create("test", ContainerOptions::default()).unwrap();
        let result = manager.create("test", ContainerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_manager_switch() {
        let mut manager = ContainerManager::new();
        manager.create("test", ContainerOptions::default()).unwrap();
        manager.switch("test").unwrap();
        assert_eq!(manager.active_name(), "test");
    }

    #[test]
    fn test_manager_switch_nonexistent() {
        let mut manager = ContainerManager::new();
        let result = manager.switch("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_manager_destroy() {
        let mut manager = ContainerManager::new();
        manager.create("test", ContainerOptions::default()).unwrap();
        manager.destroy("test").unwrap();
        assert_eq!(manager.count(), 1);
        assert!(!manager.exists("test"));
    }

    #[test]
    fn test_manager_destroy_default() {
        let mut manager = ContainerManager::new();
        let result = manager.destroy(DEFAULT_CONTAINER);
        assert!(result.is_err());
    }

    #[test]
    fn test_manager_destroy_active() {
        let mut manager = ContainerManager::new();
        manager.create("test", ContainerOptions::default()).unwrap();
        manager.switch("test").unwrap();
        manager.destroy("test").unwrap();
        // Should switch back to default
        assert_eq!(manager.active_name(), DEFAULT_CONTAINER);
    }

    #[test]
    fn test_manager_list() {
        let mut manager = ContainerManager::new();
        manager
            .create("test1", ContainerOptions::default())
            .unwrap();
        manager
            .create("test2", ContainerOptions::default())
            .unwrap();
        let list = manager.list();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&"default"));
        assert!(list.contains(&"test1"));
        assert!(list.contains(&"test2"));
    }
}
