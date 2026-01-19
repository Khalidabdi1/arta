//! Context management for Arta
//!
//! The context system maintains stateful information across commands,
//! such as the current working directory and file being inspected.

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::error::{ArtaError, Result};

/// Represents the current execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Stack of folder contexts (for nested ENTER FOLDER)
    folder_stack: Vec<PathBuf>,
    
    /// Currently focused file (for content inspection)
    current_file: Option<PathBuf>,
    
    /// User-defined variables
    variables: HashMap<String, VariableValue>,
    
    /// History of entered paths
    history: Vec<ContextHistoryEntry>,
}

/// Variable value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableValue {
    String(String),
    Number(f64),
    Size(u64),
    Boolean(bool),
    Path(PathBuf),
}

impl std::fmt::Display for VariableValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariableValue::String(s) => write!(f, "\"{}\"", s),
            VariableValue::Number(n) => write!(f, "{}", n),
            VariableValue::Size(s) => write!(f, "{}", bytesize::ByteSize(*s)),
            VariableValue::Boolean(b) => write!(f, "{}", b),
            VariableValue::Path(p) => write!(f, "{}", p.display()),
        }
    }
}

/// History entry for context changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextHistoryEntry {
    pub action: String,
    pub path: Option<PathBuf>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            folder_stack: vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))],
            current_file: None,
            variables: HashMap::new(),
            history: Vec::new(),
        }
    }
}

impl Context {
    /// Create a new context
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Get the current working directory
    pub fn current_folder(&self) -> &Path {
        self.folder_stack.last()
            .map(|p| p.as_path())
            .unwrap_or(Path::new("/"))
    }
    
    /// Get the current file being inspected
    pub fn current_file(&self) -> Option<&Path> {
        self.current_file.as_deref()
    }
    
    /// Get the folder stack depth
    pub fn folder_depth(&self) -> usize {
        self.folder_stack.len()
    }
    
    /// Enter a folder context
    pub fn enter_folder(&mut self, path: &str) -> Result<()> {
        let path = self.resolve_path(path)?;
        
        if !path.exists() {
            return Err(ArtaError::PathNotFound(path.to_string_lossy().to_string()));
        }
        
        if !path.is_dir() {
            return Err(ArtaError::ExecutionError(
                format!("'{}' is not a directory", path.display())
            ));
        }
        
        // Canonicalize the path
        let canonical = path.canonicalize()
            .map_err(|e| ArtaError::IoError(e))?;
        
        self.folder_stack.push(canonical.clone());
        self.current_file = None; // Clear file context when entering folder
        
        self.history.push(ContextHistoryEntry {
            action: "ENTER FOLDER".to_string(),
            path: Some(canonical),
            timestamp: chrono::Utc::now(),
        });
        
        Ok(())
    }
    
    /// Enter a file context for content inspection
    pub fn enter_file(&mut self, path: &str) -> Result<()> {
        let path = self.resolve_path(path)?;
        
        if !path.exists() {
            return Err(ArtaError::PathNotFound(path.to_string_lossy().to_string()));
        }
        
        if !path.is_file() {
            return Err(ArtaError::ExecutionError(
                format!("'{}' is not a file", path.display())
            ));
        }
        
        let canonical = path.canonicalize()
            .map_err(|e| ArtaError::IoError(e))?;
        
        self.current_file = Some(canonical.clone());
        
        self.history.push(ContextHistoryEntry {
            action: "ENTER FILE".to_string(),
            path: Some(canonical),
            timestamp: chrono::Utc::now(),
        });
        
        Ok(())
    }
    
    /// Exit the current context (pop folder stack or clear file)
    pub fn exit_context(&mut self) -> Result<()> {
        // First, clear file context if set
        if self.current_file.is_some() {
            self.current_file = None;
            self.history.push(ContextHistoryEntry {
                action: "EXIT FILE".to_string(),
                path: None,
                timestamp: chrono::Utc::now(),
            });
            return Ok(());
        }
        
        // Then, pop folder stack if we have more than the root
        if self.folder_stack.len() > 1 {
            let exited = self.folder_stack.pop();
            self.history.push(ContextHistoryEntry {
                action: "EXIT FOLDER".to_string(),
                path: exited,
                timestamp: chrono::Utc::now(),
            });
            return Ok(());
        }
        
        Err(ArtaError::ExecutionError(
            "Already at root context, cannot exit further".to_string()
        ))
    }
    
    /// Reset context to initial state
    pub fn reset(&mut self) {
        let initial_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        self.folder_stack = vec![initial_dir];
        self.current_file = None;
        
        self.history.push(ContextHistoryEntry {
            action: "RESET CONTEXT".to_string(),
            path: None,
            timestamp: chrono::Utc::now(),
        });
    }
    
    /// Resolve a path relative to current context
    pub fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        let path = Path::new(path);
        
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(self.current_folder().join(path))
        }
    }
    
    /// Set a variable
    pub fn set_variable(&mut self, name: String, value: VariableValue) {
        self.variables.insert(name, value);
    }
    
    /// Get a variable
    pub fn get_variable(&self, name: &str) -> Option<&VariableValue> {
        self.variables.get(name)
    }
    
    /// Get all variables
    pub fn variables(&self) -> &HashMap<String, VariableValue> {
        &self.variables
    }
    
    /// Get context history
    pub fn history(&self) -> &[ContextHistoryEntry] {
        &self.history
    }
    
    /// Format context for display
    pub fn display(&self) -> String {
        let mut output = String::new();
        
        output.push_str("Current Context\n");
        output.push_str("---------------\n");
        output.push_str(&format!("Folder: {}\n", self.current_folder().display()));
        
        if let Some(file) = &self.current_file {
            output.push_str(&format!("File:   {}\n", file.display()));
        }
        
        output.push_str(&format!("Depth:  {}\n", self.folder_stack.len()));
        
        if !self.variables.is_empty() {
            output.push_str("\nVariables:\n");
            for (name, value) in &self.variables {
                output.push_str(&format!("  {} = {}\n", name, value));
            }
        }
        
        output
    }
    
    /// Get a short prompt string showing current context
    pub fn prompt(&self) -> String {
        let folder = self.current_folder();
        let folder_str = if let Some(home) = dirs::home_dir() {
            if folder.starts_with(&home) {
                folder.strip_prefix(&home)
                    .map(|p| format!("~/{}", p.display()))
                    .unwrap_or_else(|_| folder.display().to_string())
            } else {
                folder.display().to_string()
            }
        } else {
            folder.display().to_string()
        };
        
        if let Some(file) = &self.current_file {
            format!("{}:{}", folder_str, file.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default())
        } else {
            folder_str
        }
    }
}

// Helper for home directory - simple fallback if dirs crate not available
mod dirs {
    use std::path::PathBuf;
    
    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    
    #[test]
    fn test_context_default() {
        let ctx = Context::default();
        assert!(ctx.current_folder().exists());
        assert!(ctx.current_file().is_none());
        assert_eq!(ctx.folder_depth(), 1);
    }
    
    #[test]
    fn test_enter_folder() {
        let temp_dir = TempDir::new().unwrap();
        let mut ctx = Context::new();
        
        ctx.enter_folder(temp_dir.path().to_str().unwrap()).unwrap();
        assert_eq!(ctx.folder_depth(), 2);
        assert_eq!(ctx.current_folder(), temp_dir.path().canonicalize().unwrap());
    }
    
    #[test]
    fn test_enter_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        File::create(&file_path).unwrap();
        
        let mut ctx = Context::new();
        ctx.enter_file(file_path.to_str().unwrap()).unwrap();
        
        assert!(ctx.current_file().is_some());
        assert_eq!(ctx.current_file().unwrap(), file_path.canonicalize().unwrap());
    }
    
    #[test]
    fn test_exit_context() {
        let temp_dir = TempDir::new().unwrap();
        let mut ctx = Context::new();
        
        ctx.enter_folder(temp_dir.path().to_str().unwrap()).unwrap();
        assert_eq!(ctx.folder_depth(), 2);
        
        ctx.exit_context().unwrap();
        assert_eq!(ctx.folder_depth(), 1);
    }
    
    #[test]
    fn test_exit_file_before_folder() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        File::create(&file_path).unwrap();
        
        let mut ctx = Context::new();
        ctx.enter_folder(temp_dir.path().to_str().unwrap()).unwrap();
        ctx.enter_file(file_path.to_str().unwrap()).unwrap();
        
        assert!(ctx.current_file().is_some());
        
        // First exit clears file
        ctx.exit_context().unwrap();
        assert!(ctx.current_file().is_none());
        assert_eq!(ctx.folder_depth(), 2);
        
        // Second exit pops folder
        ctx.exit_context().unwrap();
        assert_eq!(ctx.folder_depth(), 1);
    }
    
    #[test]
    fn test_reset_context() {
        let temp_dir = TempDir::new().unwrap();
        let mut ctx = Context::new();
        
        ctx.enter_folder(temp_dir.path().to_str().unwrap()).unwrap();
        ctx.enter_folder(temp_dir.path().to_str().unwrap()).unwrap();
        
        ctx.reset();
        assert_eq!(ctx.folder_depth(), 1);
        assert!(ctx.current_file().is_none());
    }
    
    #[test]
    fn test_variables() {
        let mut ctx = Context::new();
        
        ctx.set_variable("test_path".to_string(), VariableValue::Path(PathBuf::from("/tmp")));
        ctx.set_variable("threshold".to_string(), VariableValue::Number(80.0));
        
        assert!(ctx.get_variable("test_path").is_some());
        assert!(ctx.get_variable("threshold").is_some());
        assert!(ctx.get_variable("nonexistent").is_none());
    }
    
    #[test]
    fn test_resolve_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        let mut ctx = Context::new();
        
        ctx.enter_folder(temp_dir.path().to_str().unwrap()).unwrap();
        
        let resolved = ctx.resolve_path("subdir").unwrap();
        assert!(resolved.starts_with(temp_dir.path().canonicalize().unwrap()));
    }
}
