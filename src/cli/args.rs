//! CLI argument parsing

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "arta")]
#[command(author, version, about = "Query your system with SQL-like commands", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: SubCommand,
    
    /// Enable dry-run mode (show what would happen without executing)
    #[arg(long, global = true)]
    pub dry_run: bool,
    
    /// Allow destructive actions (DELETE, KILL)
    #[arg(long, global = true)]
    pub allow_actions: bool,
    
    /// Output format as JSON
    #[arg(long, global = true)]
    pub json: bool,
    
    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum SubCommand {
    /// Execute a single query
    Query {
        /// The SQL-like query to execute
        query: String,
    },
    
    /// Run an Arta script file (.arta)
    Run {
        /// Path to the .arta script file
        file: PathBuf,
        
        /// Script arguments in the form key=value
        #[arg(long = "arg", value_name = "KEY=VALUE")]
        args: Vec<String>,
        
        /// Run the script in a specific container
        #[arg(long)]
        container: Option<String>,
    },
    
    /// Start live monitoring mode
    Life {
        /// What to monitor (battery, cpu, memory, disk, network, processes)
        target: String,
        
        /// Polling interval in seconds (default: 1)
        #[arg(long, short, default_value = "1")]
        interval: u64,
    },
    
    /// Explain a script or query without executing
    Explain {
        /// Query string or path to .arta script file
        input: String,
    },
    
    /// Start interactive REPL mode
    Repl {
        /// Start REPL in a specific container
        #[arg(long)]
        container: Option<String>,
    },
    
    /// List all containers
    Containers,
}
