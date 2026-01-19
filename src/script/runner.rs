//! Script runner for executing .arta files

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::context::{Context, VariableValue};
use crate::engine::{execute_command_with_context, ExecutionContext, ExecutionResult, ResultData};
use crate::error::{ArtaError, Result};
use crate::output::{format_output, OutputFormat};
use crate::parser::{parse_script, Command, Script};

/// Result of script execution
#[derive(Debug)]
pub struct ScriptResult {
    /// All results from executed statements
    pub results: Vec<ExecutionResult>,
    /// Total statements executed
    pub statements_executed: usize,
    /// Whether the script completed successfully
    pub success: bool,
    /// Error message if script failed
    pub error: Option<String>,
}

/// Script runner that manages script execution
pub struct ScriptRunner {
    /// Execution context (dry_run, allow_actions, etc.)
    exec_ctx: ExecutionContext,
    /// Runtime context (folder stack, variables, etc.)
    context: Context,
    /// Script arguments passed via --arg
    script_args: HashMap<String, String>,
}

impl ScriptRunner {
    /// Create a new script runner
    pub fn new(exec_ctx: ExecutionContext) -> Self {
        Self {
            exec_ctx,
            context: Context::new(),
            script_args: HashMap::new(),
        }
    }

    /// Set script arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        for arg in args {
            if let Some((key, value)) = arg.split_once('=') {
                self.script_args.insert(key.to_string(), value.to_string());
            }
        }
        self
    }

    /// Load and run a script file
    pub fn run_file(&mut self, path: &Path) -> Result<ScriptResult> {
        // Validate file extension
        if path.extension().is_none_or(|e| e != "arta") {
            return Err(ArtaError::ExecutionError(format!(
                "Script file must have .arta extension: {}",
                path.display()
            )));
        }

        // Read script content
        let content = fs::read_to_string(path).map_err(ArtaError::IoError)?;

        // Parse the script
        let script = parse_script(&content)?;

        // Inject script arguments as variables
        self.inject_script_args();

        // Execute the script
        self.run_script(&script)
    }

    /// Run a parsed script
    pub fn run_script(&mut self, script: &Script) -> Result<ScriptResult> {
        let mut results = Vec::new();
        let mut statements_executed = 0;

        for cmd in &script.statements {
            match execute_command_with_context(cmd, &self.exec_ctx, &mut self.context) {
                Ok(result) => {
                    statements_executed += 1;

                    // Print output for non-empty results
                    match &result.data {
                        ResultData::Empty => {}
                        ResultData::Message(msg) if self.exec_ctx.verbose => {
                            println!("{}", msg);
                        }
                        _ => {
                            println!("{}", format_output(&result, &self.exec_ctx.output_format));
                        }
                    }

                    results.push(result);
                }
                Err(e) => {
                    return Ok(ScriptResult {
                        results,
                        statements_executed,
                        success: false,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(ScriptResult {
            results,
            statements_executed,
            success: true,
            error: None,
        })
    }

    /// Inject script arguments as context variables
    fn inject_script_args(&mut self) {
        for (key, value) in &self.script_args {
            // Try to parse as number, size, or boolean; fallback to string
            let var_value = if let Ok(n) = value.parse::<f64>() {
                VariableValue::Number(n)
            } else if value.to_lowercase() == "true" {
                VariableValue::Boolean(true)
            } else if value.to_lowercase() == "false" {
                VariableValue::Boolean(false)
            } else if value.starts_with('/') {
                VariableValue::Path(std::path::PathBuf::from(value))
            } else {
                VariableValue::String(value.clone())
            };

            self.context.set_variable(key.clone(), var_value);
        }
    }

    /// Get the output format
    pub fn output_format(&self) -> &OutputFormat {
        &self.exec_ctx.output_format
    }
}

/// Explain a script without executing
pub fn explain_script(script: &Script) -> Vec<String> {
    let mut explanations = Vec::new();

    for (i, cmd) in script.statements.iter().enumerate() {
        explanations.push(format!("{}. {}", i + 1, explain_command(cmd)));
    }

    explanations
}

fn explain_command(cmd: &Command) -> String {
    match cmd {
        Command::Query(q) => {
            format!(
                "SELECT {} {} {}{}",
                q.target,
                match &q.fields {
                    crate::parser::FieldList::All => "*".to_string(),
                    crate::parser::FieldList::Fields(f) => f.join(", "),
                },
                q.from_path
                    .as_ref()
                    .map(|p| format!("FROM {} ", p))
                    .unwrap_or_default(),
                q.where_clause
                    .as_ref()
                    .map(|_| "with filtering")
                    .unwrap_or("")
            )
        }
        Command::Action(a) => match a {
            crate::parser::ActionCommand::DeleteFiles(d) => {
                format!(
                    "DELETE FILES FROM {} {}",
                    d.path,
                    d.where_clause
                        .as_ref()
                        .map(|_| "with filtering")
                        .unwrap_or("")
                )
            }
            crate::parser::ActionCommand::KillProcess(_) => {
                "KILL PROCESS with filtering".to_string()
            }
        },
        Command::Context(c) => match c {
            crate::parser::ContextCommand::EnterFolder(p) => format!("ENTER FOLDER {}", p),
            crate::parser::ContextCommand::EnterFile(p) => format!("ENTER FILE {}", p),
            crate::parser::ContextCommand::Exit => "EXIT".to_string(),
            crate::parser::ContextCommand::Reset => "RESET".to_string(),
            crate::parser::ContextCommand::Show(t) => format!("SHOW {}", t),
        },
        Command::Let(l) => format!("LET {} = {:?}", l.name, l.value),
        Command::For(f) => {
            format!(
                "FOR {} IN {} ({} statements)",
                f.iterator_var,
                f.source_query.target,
                f.body.len()
            )
        }
        Command::If(i) => {
            format!(
                "IF {} {} {} {} ({} then, {} else)",
                i.condition.target,
                i.condition.field,
                i.condition.operator,
                i.condition.value,
                i.then_body.len(),
                i.else_body.as_ref().map(|e| e.len()).unwrap_or(0)
            )
        }
        Command::Life(l) => {
            format!("LIFE MONITOR {} ({} statements)", l.target, l.body.len())
        }
        Command::Print(p) => {
            format!("PRINT ({} expressions)", p.expressions.len())
        }
        Command::Container(c) => match c {
            crate::parser::ContainerCommand::Create(create) => {
                format!(
                    "CREATE CONTAINER \"{}\" ({} statements)",
                    create.name,
                    create.body.len()
                )
            }
            crate::parser::ContainerCommand::Switch(name) => {
                format!("SWITCH CONTAINER \"{}\"", name)
            }
            crate::parser::ContainerCommand::List => "LIST CONTAINERS".to_string(),
            crate::parser::ContainerCommand::Destroy(name) => {
                format!("DESTROY CONTAINER \"{}\"", name)
            }
            crate::parser::ContainerCommand::Export(e) => {
                format!("EXPORT CONTAINER \"{}\" TO \"{}\"", e.name, e.path)
            }
        },
        Command::Explain(inner) => format!("EXPLAIN {}", explain_command(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_runner_new() {
        let runner = ScriptRunner::new(ExecutionContext::default());
        assert!(runner.script_args.is_empty());
    }

    #[test]
    fn test_script_runner_with_args() {
        let runner = ScriptRunner::new(ExecutionContext::default())
            .with_args(vec!["path=/tmp".to_string(), "threshold=80".to_string()]);
        assert_eq!(runner.script_args.get("path"), Some(&"/tmp".to_string()));
        assert_eq!(runner.script_args.get("threshold"), Some(&"80".to_string()));
    }

    #[test]
    fn test_explain_script() {
        let script = parse_script("SELECT CPU *; SELECT MEMORY *").unwrap();
        let explanations = explain_script(&script);
        assert_eq!(explanations.len(), 2);
        assert!(explanations[0].contains("CPU"));
        assert!(explanations[1].contains("MEMORY"));
    }
}
