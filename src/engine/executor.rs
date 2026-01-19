//! Command executor

use crate::error::{ArtaError, Result};
use crate::parser::{Command, QueryCommand, ActionCommand, ContextCommand, ShowTarget, QueryTarget, LetStatement, LetValue, ForLoop, IfStatement, IfCondition, CompareOp, Value, LifeMonitor, PrintCommand, PrintExpr, ContainerCommand};
use crate::output::OutputFormat;
use crate::engine::queries::*;
use crate::engine::actions::*;
use crate::context::Context;

/// Execution context containing runtime configuration
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub dry_run: bool,
    pub allow_actions: bool,
    pub output_format: OutputFormat,
    pub verbose: bool,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            dry_run: false,
            allow_actions: false,
            output_format: OutputFormat::Human,
            verbose: false,
        }
    }
}

/// Result of command execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub data: ResultData,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ResultData {
    Cpu(CpuInfo),
    Memory(MemoryInfo),
    Disk(DiskInfo),
    Network(NetworkInfo),
    System(SystemInfo),
    Battery(BatteryInfo),
    Processes(Vec<ProcessInfo>),
    Files(Vec<FileEntry>),
    Content(ContentInfo),
    ActionResult(ActionResult),
    ContextInfo(ContextInfo),
    Explanation(String),
    Message(String),
    /// Container operation result
    ContainerResult(ContainerResultInfo),
    /// Multiple results from loop execution
    Multiple(Vec<ExecutionResult>),
    /// Empty result (e.g., IF condition was false with no ELSE)
    Empty,
}

/// Information about current context
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextInfo {
    pub current_folder: String,
    pub current_file: Option<String>,
    pub folder_depth: usize,
    pub variables: Vec<(String, String)>,
    pub history: Vec<String>,
}

/// Result of container operations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContainerResultInfo {
    pub operation: String,
    pub container_name: Option<String>,
    pub containers: Option<Vec<ContainerInfo>>,
    pub message: String,
}

/// Information about a container
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    pub allow_actions: bool,
    pub readonly: bool,
    pub is_active: bool,
}

/// File entry for FILES query
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified: Option<String>,
    pub extension: Option<String>,
}

/// Content information for CONTENT query
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContentInfo {
    pub file_path: String,
    pub lines: Vec<String>,
    pub total_lines: usize,
    pub file_size: u64,
}

/// Execute a parsed command (stateless - for single queries)
pub fn execute_command(cmd: &Command, ctx: &ExecutionContext) -> Result<ExecutionResult> {
    let mut context = Context::new();
    execute_command_with_context(cmd, ctx, &mut context)
}

/// Execute a parsed command with a stateful context
pub fn execute_command_with_context(
    cmd: &Command, 
    ctx: &ExecutionContext,
    context: &mut Context
) -> Result<ExecutionResult> {
    match cmd {
        Command::Query(query) => execute_query(query, ctx, context),
        Command::Action(action) => execute_action(action, ctx, context),
        Command::Context(context_cmd) => execute_context_command(context_cmd, context),
        Command::Let(let_stmt) => execute_let(let_stmt, context),
        Command::For(for_loop) => execute_for_loop(for_loop, ctx, context),
        Command::If(if_stmt) => execute_if(if_stmt, ctx, context),
        Command::Life(life_monitor) => execute_life(life_monitor, ctx, context),
        Command::Print(print_cmd) => execute_print(print_cmd, context),
        Command::Container(container_cmd) => execute_container_cmd(container_cmd, ctx, context),
        Command::Explain(inner) => execute_explain(inner, ctx),
    }
}

fn execute_query(query: &QueryCommand, _ctx: &ExecutionContext, context: &Context) -> Result<ExecutionResult> {
    let data = match query.target {
        QueryTarget::Cpu => ResultData::Cpu(query_cpu(&query.fields)?),
        QueryTarget::Memory => ResultData::Memory(query_memory(&query.fields)?),
        QueryTarget::Disk => ResultData::Disk(query_disk(&query.fields, query.from_path.as_deref())?),
        QueryTarget::Network => ResultData::Network(query_network(&query.fields)?),
        QueryTarget::System => ResultData::System(query_system(&query.fields)?),
        QueryTarget::Battery => ResultData::Battery(query_battery(&query.fields)?),
        QueryTarget::Process => ResultData::Processes(query_processes(&query.fields, query.where_clause.as_ref())?),
        QueryTarget::Files => {
            let path = query.from_path.as_deref()
                .map(|p| {
                    let resolved = resolve_variable_in_string(p, context);
                    context.resolve_path(&resolved)
                })
                .transpose()?
                .unwrap_or_else(|| context.current_folder().to_path_buf());
            ResultData::Files(query_files(&path, query.where_clause.as_ref())?)
        }
        QueryTarget::Content => {
            let file_path = if let Some(ref path) = query.from_path {
                let resolved = resolve_variable_in_string(path, context);
                context.resolve_path(&resolved)?
            } else if let Some(file) = context.current_file() {
                file.to_path_buf()
            } else {
                return Err(ArtaError::ExecutionError(
                    "No file in context. Use 'ENTER FILE <path>' or 'SELECT CONTENT * FROM <path>'".to_string()
                ));
            };
            ResultData::Content(query_content(&file_path, query.where_clause.as_ref())?)
        }
    };
    
    Ok(ExecutionResult { data, message: None })
}

fn execute_action(action: &ActionCommand, ctx: &ExecutionContext, context: &Context) -> Result<ExecutionResult> {
    if !ctx.allow_actions && !ctx.dry_run {
        return Err(ArtaError::ActionsDisabled);
    }
    
    let result = match action {
        ActionCommand::DeleteFiles(cmd) => {
            let resolved_path = resolve_variable_in_string(&cmd.path, context);
            let path = context.resolve_path(&resolved_path)?;
            delete_files(path.to_str().unwrap_or(&cmd.path), cmd.where_clause.as_ref(), ctx.dry_run)?
        }
        ActionCommand::KillProcess(cmd) => {
            kill_processes(&cmd.where_clause, ctx.dry_run)?
        }
    };
    
    Ok(ExecutionResult {
        data: ResultData::ActionResult(result),
        message: None,
    })
}

fn execute_context_command(cmd: &ContextCommand, context: &mut Context) -> Result<ExecutionResult> {
    match cmd {
        ContextCommand::EnterFolder(path) => {
            let resolved_path = resolve_variable_in_string(path, context);
            context.enter_folder(&resolved_path)?;
            Ok(ExecutionResult {
                data: ResultData::Message(format!("Entered folder: {}", context.current_folder().display())),
                message: None,
            })
        }
        ContextCommand::EnterFile(path) => {
            let resolved_path = resolve_variable_in_string(path, context);
            context.enter_file(&resolved_path)?;
            Ok(ExecutionResult {
                data: ResultData::Message(format!("Entered file: {}", context.current_file().unwrap().display())),
                message: None,
            })
        }
        ContextCommand::Exit => {
            context.exit_context()?;
            Ok(ExecutionResult {
                data: ResultData::Message(format!("Exited to: {}", context.current_folder().display())),
                message: None,
            })
        }
        ContextCommand::Reset => {
            context.reset();
            Ok(ExecutionResult {
                data: ResultData::Message("Context reset to initial state".to_string()),
                message: None,
            })
        }
        ContextCommand::Show(target) => {
            let info = match target {
                ShowTarget::Context => {
                    ContextInfo {
                        current_folder: context.current_folder().display().to_string(),
                        current_file: context.current_file().map(|p| p.display().to_string()),
                        folder_depth: context.folder_depth(),
                        variables: context.variables()
                            .iter()
                            .map(|(k, v)| (k.clone(), v.to_string()))
                            .collect(),
                        history: Vec::new(),
                    }
                }
                ShowTarget::Variables => {
                    ContextInfo {
                        current_folder: String::new(),
                        current_file: None,
                        folder_depth: 0,
                        variables: context.variables()
                            .iter()
                            .map(|(k, v)| (k.clone(), v.to_string()))
                            .collect(),
                        history: Vec::new(),
                    }
                }
                ShowTarget::History => {
                    ContextInfo {
                        current_folder: String::new(),
                        current_file: None,
                        folder_depth: 0,
                        variables: Vec::new(),
                        history: context.history()
                            .iter()
                            .map(|h| format!("{}: {} {:?}", 
                                h.timestamp.format("%H:%M:%S"),
                                h.action,
                                h.path.as_ref().map(|p| p.display().to_string())
                            ))
                            .collect(),
                    }
                }
            };
            Ok(ExecutionResult {
                data: ResultData::ContextInfo(info),
                message: None,
            })
        }
    }
}

fn execute_let(let_stmt: &LetStatement, context: &mut Context) -> Result<ExecutionResult> {
    use crate::context::VariableValue;
    
    let value = match &let_stmt.value {
        LetValue::String(s) => VariableValue::String(s.clone()),
        LetValue::Number(n) => VariableValue::Number(*n),
        LetValue::Size(s) => VariableValue::Size(*s),
        LetValue::Boolean(b) => VariableValue::Boolean(*b),
        LetValue::Path(p) => VariableValue::Path(std::path::PathBuf::from(p)),
    };
    
    let display_value = value.to_string();
    context.set_variable(let_stmt.name.clone(), value);
    
    Ok(ExecutionResult {
        data: ResultData::Message(format!("Variable '{}' set to {}", let_stmt.name, display_value)),
        message: None,
    })
}

fn execute_for_loop(for_loop: &ForLoop, ctx: &ExecutionContext, context: &mut Context) -> Result<ExecutionResult> {
    use crate::context::VariableValue;
    
    // Execute the source query to get items to iterate over
    let source_result = execute_query(&for_loop.source_query, ctx, context)?;
    
    let mut results = Vec::new();
    
    // Determine what we're iterating over based on the query result
    match source_result.data {
        ResultData::Files(files) => {
            for file in files {
                // Bind the iterator variable to the current file
                // Create a struct-like variable with fields: name, path, size, extension
                context.set_variable(
                    for_loop.iterator_var.clone(),
                    VariableValue::Path(std::path::PathBuf::from(&file.path))
                );
                
                // Also set field accessors like file.name, file.size, etc.
                context.set_variable(
                    format!("{}.name", for_loop.iterator_var),
                    VariableValue::String(file.name.clone())
                );
                context.set_variable(
                    format!("{}.path", for_loop.iterator_var),
                    VariableValue::Path(std::path::PathBuf::from(&file.path))
                );
                context.set_variable(
                    format!("{}.size", for_loop.iterator_var),
                    VariableValue::Size(file.size)
                );
                if let Some(ext) = &file.extension {
                    context.set_variable(
                        format!("{}.extension", for_loop.iterator_var),
                        VariableValue::String(ext.clone())
                    );
                }
                context.set_variable(
                    format!("{}.is_dir", for_loop.iterator_var),
                    VariableValue::Boolean(file.is_dir)
                );
                
                // Execute each command in the body
                for cmd in &for_loop.body {
                    let result = execute_command_with_context(cmd, ctx, context)?;
                    results.push(result);
                }
            }
        }
        ResultData::Processes(processes) => {
            for proc in processes {
                // Bind the iterator variable to the current process
                context.set_variable(
                    for_loop.iterator_var.clone(),
                    VariableValue::String(proc.name.clone())
                );
                
                // Set field accessors
                context.set_variable(
                    format!("{}.name", for_loop.iterator_var),
                    VariableValue::String(proc.name.clone())
                );
                context.set_variable(
                    format!("{}.pid", for_loop.iterator_var),
                    VariableValue::Number(proc.pid as f64)
                );
                context.set_variable(
                    format!("{}.cpu", for_loop.iterator_var),
                    VariableValue::Number(proc.cpu as f64)
                );
                context.set_variable(
                    format!("{}.memory", for_loop.iterator_var),
                    VariableValue::Size(proc.memory)
                );
                
                // Execute each command in the body
                for cmd in &for_loop.body {
                    let result = execute_command_with_context(cmd, ctx, context)?;
                    results.push(result);
                }
            }
        }
        _ => {
            return Err(ArtaError::ExecutionError(
                "FOR loop source must be a FILES or PROCESS query".to_string()
            ));
        }
    }
    
    // Clean up iterator variables (optional, but good practice)
    // Note: We don't have a remove_variable method, so they persist until context reset
    
    if results.is_empty() {
        Ok(ExecutionResult {
            data: ResultData::Message("FOR loop completed (no items)".to_string()),
            message: None,
        })
    } else {
        Ok(ExecutionResult {
            data: ResultData::Multiple(results),
            message: Some(format!("FOR loop completed")),
        })
    }
}

fn execute_if(if_stmt: &IfStatement, ctx: &ExecutionContext, context: &mut Context) -> Result<ExecutionResult> {
    // Evaluate the condition
    let condition_met = evaluate_if_condition(&if_stmt.condition, context)?;
    
    if condition_met {
        // Execute THEN body
        let mut results = Vec::new();
        for cmd in &if_stmt.then_body {
            let result = execute_command_with_context(cmd, ctx, context)?;
            results.push(result);
        }
        
        if results.len() == 1 {
            Ok(results.into_iter().next().unwrap())
        } else {
            Ok(ExecutionResult {
                data: ResultData::Multiple(results),
                message: None,
            })
        }
    } else if let Some(else_body) = &if_stmt.else_body {
        // Execute ELSE body
        let mut results = Vec::new();
        for cmd in else_body {
            let result = execute_command_with_context(cmd, ctx, context)?;
            results.push(result);
        }
        
        if results.len() == 1 {
            Ok(results.into_iter().next().unwrap())
        } else {
            Ok(ExecutionResult {
                data: ResultData::Multiple(results),
                message: None,
            })
        }
    } else {
        // No ELSE and condition was false
        Ok(ExecutionResult {
            data: ResultData::Empty,
            message: Some("IF condition was false".to_string()),
        })
    }
}

fn evaluate_if_condition(condition: &IfCondition, context: &Context) -> Result<bool> {
    // Execute a query to get the current value
    // For now, we'll get the system info and compare the field
    
    match condition.target {
        QueryTarget::Memory => {
            let info = query_memory(&crate::parser::FieldList::All)?;
            let field_value = get_memory_field_value(&info, &condition.field)?;
            compare_values(field_value, &condition.operator, &condition.value, context)
        }
        QueryTarget::Cpu => {
            let info = query_cpu(&crate::parser::FieldList::All)?;
            let field_value = get_cpu_field_value(&info, &condition.field)?;
            compare_values(field_value, &condition.operator, &condition.value, context)
        }
        QueryTarget::Disk => {
            let info = query_disk(&crate::parser::FieldList::All, None)?;
            let field_value = get_disk_field_value(&info, &condition.field)?;
            compare_values(field_value, &condition.operator, &condition.value, context)
        }
        QueryTarget::Battery => {
            let info = query_battery(&crate::parser::FieldList::All)?;
            let field_value = get_battery_field_value(&info, &condition.field)?;
            compare_values(field_value, &condition.operator, &condition.value, context)
        }
        _ => {
            Err(ArtaError::ExecutionError(
                format!("IF condition not supported for {} queries yet", condition.target)
            ))
        }
    }
}

fn get_memory_field_value(info: &MemoryInfo, field: &str) -> Result<f64> {
    match field.to_lowercase().as_str() {
        "total" | "total_bytes" => Ok(info.total as f64),
        "used" | "used_bytes" => Ok(info.used as f64),
        "free" | "free_bytes" => Ok(info.free as f64),
        "available" | "available_bytes" => Ok(info.available as f64),
        "used_percent" | "percent" | "usage" | "usage_percent" => Ok(info.usage_percent),
        _ => Err(ArtaError::ExecutionError(format!("Unknown MEMORY field: {}", field))),
    }
}

fn get_cpu_field_value(info: &CpuInfo, field: &str) -> Result<f64> {
    match field.to_lowercase().as_str() {
        "usage" | "percent" | "used_percent" | "usage_percent" => Ok(info.usage as f64),
        "cores" | "core_count" => Ok(info.cores as f64),
        "frequency" | "frequency_mhz" => Ok(info.frequency as f64),
        _ => Err(ArtaError::ExecutionError(format!("Unknown CPU field: {}", field))),
    }
}

fn get_disk_field_value(info: &DiskInfo, field: &str) -> Result<f64> {
    // Use first disk if available
    if let Some(disk) = info.disks.first() {
        match field.to_lowercase().as_str() {
            "total" | "total_bytes" => Ok(disk.total as f64),
            "used" | "used_bytes" => Ok(disk.used as f64),
            "free" | "free_bytes" | "available" | "available_bytes" => Ok(disk.free as f64),
            "used_percent" | "percent" | "usage" => Ok(disk.usage_percent),
            _ => Err(ArtaError::ExecutionError(format!("Unknown DISK field: {}", field))),
        }
    } else {
        Err(ArtaError::ExecutionError("No disks found".to_string()))
    }
}

fn get_battery_field_value(info: &BatteryInfo, field: &str) -> Result<f64> {
    if let Some(battery) = info.batteries.first() {
        match field.to_lowercase().as_str() {
            "percent" | "charge" | "level" | "charge_percent" | "percentage" => Ok(battery.percentage as f64),
            _ => Err(ArtaError::ExecutionError(format!("Unknown BATTERY field: {}", field))),
        }
    } else {
        // No battery, return 100 (assume desktop/always powered)
        Ok(100.0)
    }
}

fn compare_values(actual: f64, operator: &CompareOp, expected: &Value, context: &Context) -> Result<bool> {
    let expected_num = match expected {
        Value::Number(n) => *n,
        Value::Size(s) => *s as f64,
        Value::Identifier(id) => {
            // Try to resolve variable
            if let Some(var_value) = context.get_variable(id) {
                match var_value {
                    crate::context::VariableValue::Number(n) => *n,
                    crate::context::VariableValue::Size(s) => *s as f64,
                    _ => return Err(ArtaError::ExecutionError(
                        format!("Variable '{}' is not a number", id)
                    )),
                }
            } else {
                return Err(ArtaError::ExecutionError(format!("Unknown variable: {}", id)));
            }
        }
        _ => return Err(ArtaError::ExecutionError(
            "IF condition value must be a number or size".to_string()
        )),
    };
    
    Ok(match operator {
        CompareOp::GreaterThan => actual > expected_num,
        CompareOp::GreaterThanOrEqual => actual >= expected_num,
        CompareOp::LessThan => actual < expected_num,
        CompareOp::LessThanOrEqual => actual <= expected_num,
        CompareOp::Equal => (actual - expected_num).abs() < 0.001,
        CompareOp::NotEqual => (actual - expected_num).abs() >= 0.001,
        _ => return Err(ArtaError::ExecutionError(
            "IF condition only supports numeric comparisons".to_string()
        )),
    })
}

/// Resolve variable references in a string (e.g., path references)
fn resolve_variable_in_string(input: &str, context: &Context) -> String {
    // Check if the entire input is a variable name
    if let Some(var_value) = context.get_variable(input) {
        return match var_value {
            crate::context::VariableValue::String(s) => s.clone(),
            crate::context::VariableValue::Path(p) => p.display().to_string(),
            other => other.to_string(),
        };
    }
    
    // Otherwise return as-is (we can add ${var} syntax later)
    input.to_string()
}

fn execute_life(life: &LifeMonitor, ctx: &ExecutionContext, context: &mut Context) -> Result<ExecutionResult> {
    // For LIFE monitoring in script context, we run synchronously
    // The actual continuous monitoring is handled by the life module
    crate::life::run_life_block(life.target, &life.body, ctx, context, 1)?;
    
    Ok(ExecutionResult {
        data: ResultData::Message("LIFE monitoring completed".to_string()),
        message: None,
    })
}

fn execute_print(print_cmd: &PrintCommand, context: &Context) -> Result<ExecutionResult> {
    let mut output_parts = Vec::new();
    
    for expr in &print_cmd.expressions {
        let value = match expr {
            PrintExpr::String(s) => s.clone(),
            PrintExpr::Variable(name) => {
                if let Some(var) = context.get_variable(name) {
                    var.to_string()
                } else {
                    format!("<undefined: {}>", name)
                }
            }
            PrintExpr::QueryField { target, field } => {
                // Query the target and extract the field
                get_query_field_value(*target, field)?
            }
        };
        output_parts.push(value);
    }
    
    let output = output_parts.join(" ");
    
    Ok(ExecutionResult {
        data: ResultData::Message(output),
        message: None,
    })
}

fn execute_container_cmd(
    cmd: &ContainerCommand,
    ctx: &ExecutionContext,
    context: &mut Context,
) -> Result<ExecutionResult> {
    match cmd {
        ContainerCommand::Create(create) => {
            // For now, we execute the body in the current context
            // Full container isolation will be added with the container module
            let mut results = Vec::new();
            for body_cmd in &create.body {
                let result = execute_command_with_context(body_cmd, ctx, context)?;
                results.push(result);
            }
            
            Ok(ExecutionResult {
                data: ResultData::ContainerResult(ContainerResultInfo {
                    operation: "CREATE".to_string(),
                    container_name: Some(create.name.clone()),
                    containers: None,
                    message: format!("Container '{}' created with {} initialization commands", 
                        create.name, create.body.len()),
                }),
                message: None,
            })
        }
        ContainerCommand::Switch(name) => {
            Ok(ExecutionResult {
                data: ResultData::ContainerResult(ContainerResultInfo {
                    operation: "SWITCH".to_string(),
                    container_name: Some(name.clone()),
                    containers: None,
                    message: format!("Switched to container '{}'", name),
                }),
                message: None,
            })
        }
        ContainerCommand::List => {
            Ok(ExecutionResult {
                data: ResultData::ContainerResult(ContainerResultInfo {
                    operation: "LIST".to_string(),
                    container_name: None,
                    containers: Some(vec![
                        ContainerInfo {
                            name: "default".to_string(),
                            allow_actions: ctx.allow_actions,
                            readonly: false,
                            is_active: true,
                        }
                    ]),
                    message: "Container list".to_string(),
                }),
                message: None,
            })
        }
        ContainerCommand::Destroy(name) => {
            if name == "default" {
                return Err(ArtaError::ExecutionError(
                    "Cannot destroy the default container".to_string()
                ));
            }
            Ok(ExecutionResult {
                data: ResultData::ContainerResult(ContainerResultInfo {
                    operation: "DESTROY".to_string(),
                    container_name: Some(name.clone()),
                    containers: None,
                    message: format!("Container '{}' destroyed", name),
                }),
                message: None,
            })
        }
        ContainerCommand::Export(export) => {
            Ok(ExecutionResult {
                data: ResultData::ContainerResult(ContainerResultInfo {
                    operation: "EXPORT".to_string(),
                    container_name: Some(export.name.clone()),
                    containers: None,
                    message: format!("Container '{}' exported to '{}'", export.name, export.path),
                }),
                message: None,
            })
        }
    }
}

fn get_query_field_value(target: QueryTarget, field: &str) -> Result<String> {
    match target {
        QueryTarget::Battery => {
            let info = query_battery(&crate::parser::FieldList::All)?;
            if let Some(battery) = info.batteries.first() {
                match field.to_lowercase().as_str() {
                    "level" | "percent" | "percentage" | "charge" => Ok(format!("{}%", battery.percentage as u32)),
                    "state" | "status" => Ok(battery.state.clone()),
                    "time_to_empty" | "remaining" => Ok(battery.time_to_empty.clone().unwrap_or_else(|| "N/A".to_string())),
                    "time_to_full" => Ok(battery.time_to_full.clone().unwrap_or_else(|| "N/A".to_string())),
                    _ => Err(ArtaError::ExecutionError(format!("Unknown BATTERY field: {}", field))),
                }
            } else {
                Ok("No battery".to_string())
            }
        }
        QueryTarget::Memory => {
            let info = query_memory(&crate::parser::FieldList::All)?;
            match field.to_lowercase().as_str() {
                "total" => Ok(bytesize::ByteSize(info.total).to_string()),
                "used" => Ok(bytesize::ByteSize(info.used).to_string()),
                "free" => Ok(bytesize::ByteSize(info.free).to_string()),
                "available" => Ok(bytesize::ByteSize(info.available).to_string()),
                "usage" | "percent" | "used_percent" => Ok(format!("{:.1}%", info.usage_percent)),
                _ => Err(ArtaError::ExecutionError(format!("Unknown MEMORY field: {}", field))),
            }
        }
        QueryTarget::Cpu => {
            let info = query_cpu(&crate::parser::FieldList::All)?;
            match field.to_lowercase().as_str() {
                "usage" | "percent" => Ok(format!("{:.1}%", info.usage)),
                "cores" => Ok(info.cores.to_string()),
                "frequency" | "frequency_mhz" => Ok(format!("{} MHz", info.frequency)),
                "name" | "brand" => Ok(info.brand.clone()),
                _ => Err(ArtaError::ExecutionError(format!("Unknown CPU field: {}", field))),
            }
        }
        QueryTarget::Disk => {
            let info = query_disk(&crate::parser::FieldList::All, None)?;
            if let Some(disk) = info.disks.first() {
                match field.to_lowercase().as_str() {
                    "total" => Ok(bytesize::ByteSize(disk.total).to_string()),
                    "used" => Ok(bytesize::ByteSize(disk.used).to_string()),
                    "free" | "available" => Ok(bytesize::ByteSize(disk.free).to_string()),
                    "usage" | "percent" | "used_percent" => Ok(format!("{:.1}%", disk.usage_percent)),
                    "name" | "mount" | "mount_point" => Ok(disk.mount_point.clone()),
                    _ => Err(ArtaError::ExecutionError(format!("Unknown DISK field: {}", field))),
                }
            } else {
                Ok("No disks".to_string())
            }
        }
        QueryTarget::System => {
            let info = query_system(&crate::parser::FieldList::All)?;
            match field.to_lowercase().as_str() {
                "hostname" | "name" => Ok(info.hostname.clone()),
                "os" | "os_name" => Ok(info.os_name.clone()),
                "os_version" | "version" => Ok(info.os_version.clone()),
                "kernel" | "kernel_version" => Ok(info.kernel_version.clone()),
                "uptime" | "uptime_secs" => Ok(format!("{} seconds", info.uptime)),
                _ => Err(ArtaError::ExecutionError(format!("Unknown SYSTEM field: {}", field))),
            }
        }
        QueryTarget::Network => {
            let info = query_network(&crate::parser::FieldList::All)?;
            if let Some(iface) = info.interfaces.first() {
                match field.to_lowercase().as_str() {
                    "name" => Ok(iface.name.clone()),
                    "sent" | "bytes_sent" | "transmitted" => Ok(bytesize::ByteSize(iface.transmitted).to_string()),
                    "recv" | "received" | "bytes_recv" => Ok(bytesize::ByteSize(iface.received).to_string()),
                    _ => Err(ArtaError::ExecutionError(format!("Unknown NETWORK field: {}", field))),
                }
            } else {
                Ok("No network interfaces".to_string())
            }
        }
        _ => Err(ArtaError::ExecutionError(format!("PRINT not supported for {} queries", target))),
    }
}

fn execute_explain(cmd: &Command, _ctx: &ExecutionContext) -> Result<ExecutionResult> {
    let explanation = match cmd {
        Command::Query(q) => {
            format!(
                "EXPLAIN: Would query {} with fields {:?}{}{}",
                q.target,
                q.fields,
                q.from_path.as_ref().map(|p| format!(" from path '{}'", p)).unwrap_or_default(),
                q.where_clause.as_ref().map(|_| " with filtering").unwrap_or_default()
            )
        }
        Command::Action(ActionCommand::DeleteFiles(d)) => {
            format!(
                "EXPLAIN: Would delete files from '{}' {}",
                d.path,
                d.where_clause.as_ref().map(|_| "with filtering").unwrap_or("(all files - DANGEROUS!)")
            )
        }
        Command::Action(ActionCommand::KillProcess(_)) => {
            "EXPLAIN: Would kill processes matching filter criteria".to_string()
        }
        Command::Context(c) => {
            match c {
                ContextCommand::EnterFolder(p) => format!("EXPLAIN: Would enter folder '{}'", p),
                ContextCommand::EnterFile(p) => format!("EXPLAIN: Would enter file '{}'", p),
                ContextCommand::Exit => "EXPLAIN: Would exit current context".to_string(),
                ContextCommand::Reset => "EXPLAIN: Would reset context to initial state".to_string(),
                ContextCommand::Show(t) => format!("EXPLAIN: Would show {}", t),
            }
        }
        Command::Let(l) => {
            format!("EXPLAIN: Would set variable '{}' to {:?}", l.name, l.value)
        }
        Command::For(f) => {
            format!(
                "EXPLAIN: Would iterate '{}' over {} query{} and execute {} statement(s)",
                f.iterator_var,
                f.source_query.target,
                f.source_query.where_clause.as_ref().map(|_| " with filtering").unwrap_or(""),
                f.body.len()
            )
        }
        Command::If(i) => {
            format!(
                "EXPLAIN: Would check IF {} {} {} {} THEN execute {} statement(s){}",
                i.condition.target,
                i.condition.field,
                i.condition.operator,
                i.condition.value,
                i.then_body.len(),
                i.else_body.as_ref().map(|e| format!(" ELSE execute {} statement(s)", e.len())).unwrap_or_default()
            )
        }
        Command::Life(l) => {
            format!(
                "EXPLAIN: Would start LIFE monitoring for {} and execute {} statement(s) on changes",
                l.target,
                l.body.len()
            )
        }
        Command::Print(p) => {
            format!(
                "EXPLAIN: Would print {} expression(s)",
                p.expressions.len()
            )
        }
        Command::Container(c) => {
            match c {
                ContainerCommand::Create(create) => format!(
                    "EXPLAIN: Would create container '{}' with {} initialization statement(s){}{}",
                    create.name,
                    create.body.len(),
                    if create.options.allow_actions { " [ALLOW ACTIONS]" } else { "" },
                    if create.options.readonly { " [READONLY]" } else { "" }
                ),
                ContainerCommand::Switch(name) => format!("EXPLAIN: Would switch to container '{}'", name),
                ContainerCommand::List => "EXPLAIN: Would list all containers".to_string(),
                ContainerCommand::Destroy(name) => format!("EXPLAIN: Would destroy container '{}'", name),
                ContainerCommand::Export(e) => format!("EXPLAIN: Would export container '{}' to '{}'", e.name, e.path),
            }
        }
        Command::Explain(_) => "EXPLAIN: Nested EXPLAIN not supported".to_string(),
    };
    
    Ok(ExecutionResult {
        data: ResultData::Explanation(explanation),
        message: None,
    })
}

// Query helpers for new targets

fn query_files(path: &std::path::Path, where_clause: Option<&crate::parser::WhereClause>) -> Result<Vec<FileEntry>> {
    use std::fs;
    
    if !path.exists() {
        return Err(ArtaError::PathNotFound(path.display().to_string()));
    }
    
    if !path.is_dir() {
        return Err(ArtaError::ExecutionError(format!("'{}' is not a directory", path.display())));
    }
    
    let mut entries = Vec::new();
    
    for entry in fs::read_dir(path).map_err(|e| ArtaError::IoError(e))? {
        let entry = entry.map_err(|e| ArtaError::IoError(e))?;
        let metadata = entry.metadata().map_err(|e| ArtaError::IoError(e))?;
        let file_path = entry.path();
        
        let modified = metadata.modified()
            .ok()
            .map(|t| {
                chrono::DateTime::<chrono::Utc>::from(t)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            });
        
        let file_entry = FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: file_path.display().to_string(),
            size: metadata.len(),
            is_dir: metadata.is_dir(),
            modified,
            extension: file_path.extension().map(|e| e.to_string_lossy().to_string()),
        };
        
        // Apply filtering if WHERE clause exists
        if let Some(wc) = where_clause {
            if matches_file_filter(&file_entry, wc) {
                entries.push(file_entry);
            }
        } else {
            entries.push(file_entry);
        }
    }
    
    // Sort by name
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(entries)
}

fn matches_file_filter(_entry: &FileEntry, _where_clause: &crate::parser::WhereClause) -> bool {
    // TODO: Implement proper WHERE filtering for files
    // For now, accept all
    true
}

fn query_content(path: &std::path::Path, where_clause: Option<&crate::parser::WhereClause>) -> Result<ContentInfo> {
    use std::fs;
    use std::io::{BufRead, BufReader};
    
    if !path.exists() {
        return Err(ArtaError::PathNotFound(path.display().to_string()));
    }
    
    if !path.is_file() {
        return Err(ArtaError::ExecutionError(format!("'{}' is not a file", path.display())));
    }
    
    let metadata = fs::metadata(path).map_err(|e| ArtaError::IoError(e))?;
    let file = fs::File::open(path).map_err(|e| ArtaError::IoError(e))?;
    let reader = BufReader::new(file);
    
    let mut lines: Vec<String> = Vec::new();
    let mut total_lines = 0;
    
    // Check for pattern filter in WHERE clause
    let pattern = where_clause.and_then(|wc| {
        wc.conditions.first().and_then(|c| {
            if c.condition.field.to_lowercase() == "content" || 
               c.condition.field.to_lowercase() == "line" {
                match &c.condition.value {
                    crate::parser::Value::String(s) => Some(s.clone()),
                    _ => None,
                }
            } else {
                None
            }
        })
    });
    
    for (i, line_result) in reader.lines().enumerate() {
        let line = line_result.map_err(|e| ArtaError::IoError(e))?;
        total_lines = i + 1;
        
        if let Some(ref pat) = pattern {
            if line.contains(pat) {
                lines.push(format!("{:>4}: {}", i + 1, line));
            }
        } else {
            // Limit to first 100 lines if no filter
            if lines.len() < 100 {
                lines.push(line);
            }
        }
    }
    
    Ok(ContentInfo {
        file_path: path.display().to_string(),
        lines,
        total_lines,
        file_size: metadata.len(),
    })
}
