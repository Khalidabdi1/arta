//! Arta CLI - Query your system with SQL-like commands

use clap::Parser;
use arta::{parse_command, parse_script, execute_command, ExecutionContext, OutputFormat, format_output};
use arta::cli::Args;
use arta::script::{ScriptRunner, validate_script, ValidationOptions, ValidationSeverity, has_errors, explain_script};
use arta::container::ContainerManager;

fn main() {
    let args = Args::parse();
    
    if let Err(e) = run(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run(args: Args) -> arta::Result<()> {
    match args.command {
        arta::cli::SubCommand::Query { query } => {
            let cmd = parse_command(&query)?;
            let ctx = ExecutionContext {
                dry_run: args.dry_run,
                allow_actions: args.allow_actions,
                output_format: if args.json { OutputFormat::Json } else { OutputFormat::Human },
                verbose: args.verbose,
            };
            let result = execute_command(&cmd, &ctx)?;
            println!("{}", format_output(&result, &ctx.output_format));
            Ok(())
        }
        
        arta::cli::SubCommand::Run { file, args: script_args, container } => {
            let ctx = ExecutionContext {
                dry_run: args.dry_run,
                allow_actions: args.allow_actions,
                output_format: if args.json { OutputFormat::Json } else { OutputFormat::Human },
                verbose: args.verbose,
            };
            
            // Read and parse the script first for validation
            let content = std::fs::read_to_string(&file)
                .map_err(|e| arta::ArtaError::IoError(e))?;
            let script = parse_script(&content)?;
            
            // Validate the script
            let validation_opts = ValidationOptions {
                allow_actions: args.allow_actions,
                allow_life_actions: false,
                max_nesting_depth: 10,
            };
            let validation_errors = validate_script(&script, &validation_opts);
            
            // Print warnings
            for err in validation_errors.iter().filter(|e| e.severity == ValidationSeverity::Warning) {
                eprintln!("Warning: {}", err);
            }
            
            // Abort on errors
            if has_errors(&validation_errors) {
                for err in validation_errors.iter().filter(|e| e.severity == ValidationSeverity::Error) {
                    eprintln!("Error: {}", err);
                }
                return Err(arta::ArtaError::ExecutionError(
                    "Script validation failed. Fix errors or use --allow-actions if needed.".to_string()
                ));
            }
            
            // Log container if specified
            if let Some(ref container_name) = container {
                if args.verbose {
                    println!("Running in container: {}", container_name);
                }
            }
            
            // Run the script
            let mut runner = ScriptRunner::new(ctx).with_args(script_args);
            let result = runner.run_file(&file)?;
            
            if !result.success {
                if let Some(err) = result.error {
                    return Err(arta::ArtaError::ExecutionError(err));
                }
            }
            
            if args.verbose {
                println!("\n--- Script completed: {} statements executed ---", result.statements_executed);
            }
            
            Ok(())
        }
        
        arta::cli::SubCommand::Life { target, interval } => {
            let output_format = if args.json { OutputFormat::Json } else { OutputFormat::Human };
            arta::life::run_simple_monitor(&target, interval, &output_format)
        }
        
        arta::cli::SubCommand::Explain { input } => {
            // Check if input is a file path or a query
            let path = std::path::Path::new(&input);
            
            if path.exists() && path.extension().map_or(false, |e| e == "arta") {
                // It's a script file
                let content = std::fs::read_to_string(path)
                    .map_err(|e| arta::ArtaError::IoError(e))?;
                let script = parse_script(&content)?;
                
                println!("Script: {}", path.display());
                println!("Statements: {}\n", script.statements.len());
                
                for explanation in explain_script(&script).iter() {
                    println!("{}", explanation);
                }
                
                // Also show validation results
                let validation_opts = ValidationOptions {
                    allow_actions: true, // Show all issues
                    allow_life_actions: true,
                    max_nesting_depth: 10,
                };
                let validation_errors = validate_script(&script, &validation_opts);
                
                if !validation_errors.is_empty() {
                    println!("\nValidation Notes:");
                    for err in &validation_errors {
                        println!("  - {}", err);
                    }
                }
            } else {
                // It's a query
                let cmd = parse_command(&input)?;
                let ctx = ExecutionContext {
                    dry_run: true,
                    allow_actions: false,
                    output_format: OutputFormat::Human,
                    verbose: args.verbose,
                };
                let result = execute_command(&arta::parser::Command::Explain(Box::new(cmd)), &ctx)?;
                println!("{}", format_output(&result, &ctx.output_format));
            }
            
            Ok(())
        }
        
        arta::cli::SubCommand::Containers => {
            let manager = ContainerManager::new();
            println!("Containers:");
            println!("-----------");
            for name in manager.list() {
                let container = manager.get(name).unwrap();
                let active = if manager.active_name() == name { " (active)" } else { "" };
                println!("  {} - actions: {}, readonly: {}{}", 
                    name, 
                    if container.allow_actions { "yes" } else { "no" },
                    if container.readonly { "yes" } else { "no" },
                    active
                );
            }
            Ok(())
        }
        
        #[cfg(feature = "repl")]
        arta::cli::SubCommand::Repl { container } => {
            if let Some(ref container_name) = container {
                println!("Starting REPL in container: {}", container_name);
            }
            arta::repl::run_repl()
        }
        #[cfg(not(feature = "repl"))]
        arta::cli::SubCommand::Repl { .. } => {
            eprintln!("REPL support not enabled. Rebuild with --features repl");
            std::process::exit(1);
        }
    }
}
