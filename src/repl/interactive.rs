//! Interactive REPL implementation

use crate::error::Result;
use crate::{parse_command, ExecutionContext, OutputFormat, format_output, Context};
use crate::engine::executor::execute_command_with_context;
use crate::container::ContainerManager;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

pub fn run_repl() -> Result<()> {
    let mut rl = DefaultEditor::new()
        .map_err(|e| crate::error::ArtaError::ExecutionError(e.to_string()))?;
    
    println!("Arta v{} - Interactive Mode", env!("CARGO_PKG_VERSION"));
    println!("Type 'help' for commands, 'exit' to quit\n");
    
    let exec_ctx = ExecutionContext {
        dry_run: false,
        allow_actions: false,
        output_format: OutputFormat::Human,
        verbose: false,
    };
    
    // Create container manager for multi-container support
    let mut container_manager = ContainerManager::new();
    
    // Buffer for multi-line input (for control flow blocks)
    let mut input_buffer = String::new();
    let mut block_depth = 0;
    
    loop {
        // Get current container and context
        let container = container_manager.active();
        let container_name = container_manager.active_name();
        
        // Create prompt based on whether we're in a multi-line block
        let prompt = if block_depth > 0 {
            format!("{}...> ", "  ".repeat(block_depth))
        } else {
            // Show container name if not default
            let container_prefix = if container_name != "default" {
                format!("[{}] ", container_name)
            } else {
                String::new()
            };
            format!("arta {}[{}]> ", container_prefix, container.context().prompt())
        };
        
        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                let line = line.trim();
                
                // Handle empty lines
                if line.is_empty() {
                    if block_depth == 0 {
                        continue;
                    }
                    // In a block, empty line is allowed
                    input_buffer.push(' ');
                    continue;
                }
                
                // If we're not in a block, check for built-in REPL commands
                if block_depth == 0 {
                    match line.to_lowercase().as_str() {
                        "exit" | "quit" | "q" => {
                            println!("Goodbye!");
                            break;
                        }
                        "help" | "?" => {
                            print_help();
                            continue;
                        }
                        "clear" | "cls" => {
                            print!("\x1B[2J\x1B[1;1H");
                            continue;
                        }
                        "pwd" => {
                            let container = container_manager.active();
                            println!("{}\n", container.context().current_folder().display());
                            continue;
                        }
                        "containers" => {
                            println!("Containers:");
                            for name in container_manager.list() {
                                let c = container_manager.get(name).unwrap();
                                let active = if container_manager.active_name() == name { " (active)" } else { "" };
                                println!("  {} - actions: {}, readonly: {}{}", 
                                    name, 
                                    if c.allow_actions { "yes" } else { "no" },
                                    if c.readonly { "yes" } else { "no" },
                                    active
                                );
                            }
                            println!();
                            continue;
                        }
                        _ => {}
                    }
                }
                
                let _ = rl.add_history_entry(line);
                
                // Handle shortcuts (only when not in a block)
                let line_to_process = if block_depth == 0 {
                    expand_shortcuts(line)
                } else {
                    line.to_string()
                };
                
                // Update block depth based on keywords
                let upper = line_to_process.to_uppercase();
                
                // Count opening keywords (DO, THEN)
                if upper.contains(" DO") || upper.contains(" DO ") || upper.ends_with(" DO") {
                    block_depth += 1;
                }
                if upper.contains(" THEN") || upper.contains(" THEN ") || upper.ends_with(" THEN") {
                    block_depth += 1;
                }
                
                // Count closing keywords (END FOR, END IF, END CONTAINER, END LIFE)
                if upper.contains("END FOR") {
                    block_depth = block_depth.saturating_sub(1);
                }
                if upper.contains("END IF") {
                    block_depth = block_depth.saturating_sub(1);
                }
                if upper.contains("END CONTAINER") {
                    block_depth = block_depth.saturating_sub(1);
                }
                if upper.contains("END LIFE") {
                    block_depth = block_depth.saturating_sub(1);
                }
                
                // Add to buffer
                if !input_buffer.is_empty() {
                    input_buffer.push(' ');
                }
                input_buffer.push_str(&line_to_process);
                
                // If block is complete, execute
                if block_depth == 0 && !input_buffer.is_empty() {
                    let command_str = std::mem::take(&mut input_buffer);
                    
                    match parse_command(&command_str) {
                        Ok(cmd) => {
                            // Handle container-specific commands
                            if let crate::parser::Command::Container(ref container_cmd) = cmd {
                                match container_cmd {
                                    crate::parser::ContainerCommand::Switch(name) => {
                                        match container_manager.switch(name) {
                                            Ok(()) => println!("Switched to container '{}'\n", name),
                                            Err(e) => eprintln!("Error: {}\n", e),
                                        }
                                        continue;
                                    }
                                    crate::parser::ContainerCommand::Create(create) => {
                                        match container_manager.create(&create.name, create.options.clone()) {
                                            Ok(container) => {
                                                // Execute initialization body in the new container
                                                for body_cmd in &create.body {
                                                    if let Err(e) = execute_command_with_context(body_cmd, &exec_ctx, container.context_mut()) {
                                                        eprintln!("Error in container initialization: {}\n", e);
                                                    }
                                                }
                                                println!("Container '{}' created with {} initialization commands\n", create.name, create.body.len());
                                            }
                                            Err(e) => eprintln!("Error: {}\n", e),
                                        }
                                        continue;
                                    }
                                    crate::parser::ContainerCommand::Destroy(name) => {
                                        match container_manager.destroy(name) {
                                            Ok(()) => println!("Container '{}' destroyed\n", name),
                                            Err(e) => eprintln!("Error: {}\n", e),
                                        }
                                        continue;
                                    }
                                    crate::parser::ContainerCommand::List => {
                                        println!("Containers:");
                                        for name in container_manager.list() {
                                            let c = container_manager.get(name).unwrap();
                                            let active = if container_manager.active_name() == name { " (active)" } else { "" };
                                            println!("  {} - actions: {}, readonly: {}{}", 
                                                name, 
                                                if c.allow_actions { "yes" } else { "no" },
                                                if c.readonly { "yes" } else { "no" },
                                                active
                                            );
                                        }
                                        println!();
                                        continue;
                                    }
                                    crate::parser::ContainerCommand::Export(export) => {
                                        let path = std::path::Path::new(&export.path);
                                        match container_manager.export(&export.name, path) {
                                            Ok(()) => println!("Container '{}' exported to '{}'\n", export.name, export.path),
                                            Err(e) => eprintln!("Error: {}\n", e),
                                        }
                                        continue;
                                    }
                                }
                            }
                            
                            // Execute regular commands in active container's context
                            let container = container_manager.active_mut();
                            match execute_command_with_context(&cmd, &exec_ctx, container.context_mut()) {
                                Ok(result) => {
                                    let output = format_output(&result, &exec_ctx.output_format);
                                    if !output.is_empty() {
                                        println!("{}\n", output);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error: {}\n", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Parse error: {}\n", e);
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C - cancel current input
                if block_depth > 0 {
                    println!("^C (input cancelled)");
                    input_buffer.clear();
                    block_depth = 0;
                } else {
                    println!("^C");
                }
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }
    
    Ok(())
}

/// Expand common shortcuts to full commands
fn expand_shortcuts(input: &str) -> String {
    let lower = input.to_lowercase();
    
    // Common shortcuts
    if lower.starts_with("cd ") {
        return format!("ENTER FOLDER {}", &input[3..]);
    }
    if lower == "cd" || lower == ".." {
        return "EXIT".to_string();
    }
    if lower.starts_with("ls") {
        if lower == "ls" {
            return "SELECT FILES *".to_string();
        } else if lower.starts_with("ls ") {
            return format!("SELECT FILES * FROM {}", &input[3..]);
        }
    }
    if lower.starts_with("cat ") {
        return format!("ENTER FILE {}", &input[4..]);
    }
    if lower == "cat" {
        return "SELECT CONTENT *".to_string();
    }
    if lower == "vars" || lower == "variables" {
        return "SHOW VARIABLES".to_string();
    }
    if lower == "ctx" || lower == "context" {
        return "SHOW CONTEXT".to_string();
    }
    
    input.to_string()
}

fn print_help() {
    println!(r#"
Arta Commands
=============

VARIABLES:
  LET name = "value"              - Set string variable
  LET count = 42                  - Set number variable
  LET path = /tmp                 - Set path variable
  LET size = 100MB                - Set size variable
  LET flag = true                 - Set boolean variable

CONTROL FLOW:
  FOR file IN SELECT FILES * FROM /path DO
      <commands>
  END FOR

  IF SELECT MEMORY usage > 80 THEN
      <commands>
  ELSE
      <commands>
  END IF

CONTAINERS:
  CREATE CONTAINER "name" DO      - Create a new container
      <commands>
  END CONTAINER

  CREATE CONTAINER "name" WITH ALLOW ACTIONS DO
      <commands>                  - Create container with actions enabled
  END CONTAINER

  SWITCH CONTAINER "name"         - Switch to a different container
  LIST CONTAINERS                 - List all containers
  DESTROY CONTAINER "name"        - Destroy a container
  EXPORT CONTAINER "name" TO /path - Export container to script file

CONTEXT NAVIGATION:
  ENTER FOLDER /path              - Change to directory
  ENTER FILE /path                - Select file for inspection
  EXIT                            - Go back (exit file, then folder)
  RESET                           - Reset to initial context
  SHOW CONTEXT                    - Show current context
  SHOW VARIABLES                  - Show defined variables
  SHOW HISTORY                    - Show navigation history

QUERIES (read-only):
  SELECT CPU *                    - Show CPU information
  SELECT MEMORY *                 - Show memory usage
  SELECT DISK * FROM /            - Show disk information
  SELECT NETWORK *                - Show network interfaces
  SELECT SYSTEM *                 - Show system information  
  SELECT BATTERY *                - Show battery status
  SELECT PROCESS * WHERE cpu > 10 - Show processes with high CPU
  SELECT FILES * FROM /path       - List files in directory
  SELECT FILES * FROM my_var      - List files using variable
  SELECT CONTENT *                - Show content of current file
  SELECT CONTENT * FROM /path     - Show content of specific file

ACTIONS (require --allow-actions at startup):
  DELETE FILES FROM /path WHERE size > 100MB
  KILL PROCESS WHERE name = "process"

OTHER:
  EXPLAIN <command>               - Show what a command would do

SHORTCUTS:
  cd /path                        - Same as ENTER FOLDER /path
  cd or ..                        - Same as EXIT
  ls                              - Same as SELECT FILES *
  ls /path                        - Same as SELECT FILES * FROM /path
  cat /path                       - Same as ENTER FILE /path
  cat                             - Same as SELECT CONTENT *
  vars                            - Same as SHOW VARIABLES
  ctx                             - Same as SHOW CONTEXT

REPL Commands:
  help, ?                         - Show this help
  pwd                             - Show current folder
  containers                      - List all containers
  clear, cls                      - Clear screen
  exit, quit, q                   - Exit REPL

Note: FOR, IF, CONTAINER, and LIFE blocks can be entered across multiple lines.
      The REPL will wait for the corresponding END keyword before executing.
"#);
}
