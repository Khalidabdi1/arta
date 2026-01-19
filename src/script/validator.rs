//! Script validation for Arta
//!
//! Validates scripts before execution for safety and correctness.

use crate::parser::{ActionCommand, Command, Script};

/// Errors that can occur during script validation
#[derive(Debug, Clone)]
pub struct ScriptValidationError {
    pub line: Option<usize>,
    pub message: String,
    pub severity: ValidationSeverity,
}

/// Severity level for validation issues
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

impl std::fmt::Display for ScriptValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.severity {
            ValidationSeverity::Error => "ERROR",
            ValidationSeverity::Warning => "WARNING",
        };
        if let Some(line) = self.line {
            write!(f, "{} (line {}): {}", prefix, line, self.message)
        } else {
            write!(f, "{}: {}", prefix, self.message)
        }
    }
}

/// Validation options
#[derive(Debug, Clone)]
pub struct ValidationOptions {
    /// Whether actions are allowed in the script
    pub allow_actions: bool,
    /// Whether LIFE blocks can contain actions
    pub allow_life_actions: bool,
    /// Maximum nesting depth for control flow
    pub max_nesting_depth: usize,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            allow_actions: false,
            allow_life_actions: false,
            max_nesting_depth: 10,
        }
    }
}

/// Validate a script for safety and correctness
pub fn validate_script(script: &Script, options: &ValidationOptions) -> Vec<ScriptValidationError> {
    let mut errors = Vec::new();

    for (i, cmd) in script.statements.iter().enumerate() {
        validate_command(cmd, options, &mut errors, i + 1, 0);
    }

    errors
}

fn validate_command(
    cmd: &Command,
    options: &ValidationOptions,
    errors: &mut Vec<ScriptValidationError>,
    line: usize,
    depth: usize,
) {
    // Check nesting depth
    if depth > options.max_nesting_depth {
        errors.push(ScriptValidationError {
            line: Some(line),
            message: format!(
                "Maximum nesting depth ({}) exceeded",
                options.max_nesting_depth
            ),
            severity: ValidationSeverity::Error,
        });
        return;
    }

    match cmd {
        Command::Action(action) => {
            if !options.allow_actions {
                let action_name = match action {
                    ActionCommand::DeleteFiles(_) => "DELETE FILES",
                    ActionCommand::KillProcess(_) => "KILL PROCESS",
                };
                errors.push(ScriptValidationError {
                    line: Some(line),
                    message: format!(
                        "{} action found. Use --allow-actions to enable destructive actions",
                        action_name
                    ),
                    severity: ValidationSeverity::Error,
                });
            }

            // Check for dangerous patterns
            if let ActionCommand::DeleteFiles(d) = action {
                if d.where_clause.is_none() {
                    errors.push(ScriptValidationError {
                        line: Some(line),
                        message: "DELETE FILES without WHERE clause will delete ALL files!"
                            .to_string(),
                        severity: ValidationSeverity::Warning,
                    });
                }

                // Warn about dangerous paths
                let dangerous_paths = ["/", "/bin", "/etc", "/usr", "/var", "/home"];
                if dangerous_paths.contains(&d.path.as_str()) {
                    errors.push(ScriptValidationError {
                        line: Some(line),
                        message: format!("DELETE FILES targeting system path: {}", d.path),
                        severity: ValidationSeverity::Warning,
                    });
                }
            }
        }

        Command::For(f) => {
            // Validate body
            for body_cmd in &f.body {
                validate_command(body_cmd, options, errors, line, depth + 1);
            }
        }

        Command::If(i) => {
            // Validate then body
            for body_cmd in &i.then_body {
                validate_command(body_cmd, options, errors, line, depth + 1);
            }

            // Validate else body
            if let Some(else_body) = &i.else_body {
                for body_cmd in else_body {
                    validate_command(body_cmd, options, errors, line, depth + 1);
                }
            }
        }

        Command::Life(l) => {
            // LIFE blocks should not contain destructive actions by default
            for body_cmd in &l.body {
                if let Command::Action(_) = body_cmd {
                    if !options.allow_life_actions {
                        errors.push(ScriptValidationError {
                            line: Some(line),
                            message: "LIFE blocks cannot contain destructive actions by default"
                                .to_string(),
                            severity: ValidationSeverity::Error,
                        });
                    }
                }
                validate_command(body_cmd, options, errors, line, depth + 1);
            }
        }

        Command::Container(crate::parser::ContainerCommand::Create(create)) => {
            // Validate container body
            for body_cmd in &create.body {
                validate_command(body_cmd, options, errors, line, depth + 1);
            }

            // Check for actions in container without allow_actions
            if !create.options.allow_actions {
                for body_cmd in &create.body {
                    if let Command::Action(action) = body_cmd {
                        let action_name = match action {
                            ActionCommand::DeleteFiles(_) => "DELETE FILES",
                            ActionCommand::KillProcess(_) => "KILL PROCESS",
                        };
                        errors.push(ScriptValidationError {
                            line: Some(line),
                            message: format!(
                                "{} action in container '{}' without ALLOW ACTIONS option",
                                action_name, create.name
                            ),
                            severity: ValidationSeverity::Warning,
                        });
                    }
                }
            }
        }

        Command::Container(_) => {
            // Other container commands (Switch, List, Destroy, Export) are safe
        }

        // Other commands are safe
        _ => {}
    }
}

/// Check if a script has any validation errors (not just warnings)
pub fn has_errors(errors: &[ScriptValidationError]) -> bool {
    errors
        .iter()
        .any(|e| e.severity == ValidationSeverity::Error)
}

/// Check if a script has any validation warnings
pub fn has_warnings(errors: &[ScriptValidationError]) -> bool {
    errors
        .iter()
        .any(|e| e.severity == ValidationSeverity::Warning)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_script;

    #[test]
    fn test_validate_safe_script() {
        let script = parse_script("SELECT CPU *; SELECT MEMORY *").unwrap();
        let errors = validate_script(&script, &ValidationOptions::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_action_without_permission() {
        let script = parse_script("DELETE FILES FROM /tmp WHERE size > 100MB").unwrap();
        let errors = validate_script(&script, &ValidationOptions::default());
        assert!(has_errors(&errors));
    }

    #[test]
    fn test_validate_action_with_permission() {
        let script = parse_script("DELETE FILES FROM /tmp WHERE size > 100MB").unwrap();
        let options = ValidationOptions {
            allow_actions: true,
            ..Default::default()
        };
        let errors = validate_script(&script, &options);
        assert!(!has_errors(&errors));
    }

    #[test]
    fn test_validate_delete_without_where() {
        let script = parse_script("DELETE FILES FROM /tmp").unwrap();
        let options = ValidationOptions {
            allow_actions: true,
            ..Default::default()
        };
        let errors = validate_script(&script, &options);
        assert!(has_warnings(&errors));
    }

    #[test]
    fn test_validate_dangerous_path() {
        let script = parse_script("DELETE FILES FROM / WHERE name = \"temp\"").unwrap();
        let options = ValidationOptions {
            allow_actions: true,
            ..Default::default()
        };
        let errors = validate_script(&script, &options);
        assert!(has_warnings(&errors));
    }
}
