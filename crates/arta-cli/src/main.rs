//! # arta (L4 CLI)
//!
//! The `arta` binary. Wraps the human-facing workflow and the agent API over
//! the [`arta_agent::AgentRepo`] object store.
//!
//! Human verbs mirror familiar git UX (`init`, `status`, `snapshot`, `log`);
//! the `agent` subcommand exposes the richer intent/confidence/task API for
//! programmatic use. All command logic lives in [`app`]; `main` only parses
//! arguments, dispatches, and renders errors at the top level.

#![forbid(unsafe_code)]

mod app;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// arta — agent-native version control.
#[derive(Parser)]
#[command(name = "arta", version, about = "agent-native version control")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new arta repository in the current directory.
    Init {
        /// Directory to initialise (defaults to the current directory).
        path: Option<PathBuf>,
    },
    /// Show HEAD, the active task, and object counts.
    Status,
    /// Snapshot the working tree and record a commit.
    Snapshot {
        /// Why this change was made.
        #[arg(short, long)]
        intent: String,
        /// Confidence in the change, 0.0 (guessing) to 1.0 (certain).
        #[arg(short, long, default_value_t = 1.0)]
        confidence: f32,
        /// Optional longer-form reasoning to attach.
        #[arg(short, long)]
        reasoning: Option<String>,
    },
    /// Show commit history, newest first.
    Log {
        /// Include each commit's intent (and reasoning) in the output.
        #[arg(long)]
        show_intent: bool,
        /// Emit machine-readable JSON instead of the human format.
        #[arg(long)]
        json: bool,
    },
    /// Agent-facing commands: intent commits, checkpoints, rollback, tasks.
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
}

#[derive(Subcommand)]
enum AgentCommand {
    /// Record an intent-aware commit of the working tree.
    Commit {
        /// Why this change was made.
        #[arg(short, long)]
        intent: String,
        /// Confidence in the change, 0.0 to 1.0.
        #[arg(short, long)]
        confidence: f32,
        /// Optional longer-form reasoning to attach.
        #[arg(short, long)]
        reasoning: Option<String>,
    },
    /// Save a named rollback point at the current HEAD.
    Checkpoint {
        /// A human-readable reason for the checkpoint.
        #[arg(short, long)]
        reason: String,
    },
    /// Move HEAD back to a matching past commit.
    Rollback {
        /// Roll back to the most recent commit matching this intent.
        #[arg(long, value_name = "QUERY", conflicts_with = "to_confidence")]
        to_intent: Option<String>,
        /// Roll back to the most recent commit at or above this confidence.
        #[arg(long, value_name = "MIN", conflicts_with = "to_intent")]
        to_confidence: Option<f32>,
    },
    /// Machine-readable commit log (JSON).
    Log,
    /// Manage the task graph.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}

#[derive(Subcommand)]
enum TaskCommand {
    /// Open a task and make it active.
    Open {
        /// What the task is meant to accomplish.
        description: String,
        /// The parent task's UUID, for a sub-task.
        #[arg(long)]
        parent: Option<String>,
    },
    /// Complete a task by its UUID.
    Complete {
        /// The task's UUID.
        id: String,
    },
    /// List all tasks.
    List,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatch a parsed command to the matching [`app`] handler.
///
/// The working directory is resolved once here; handlers walk up from it to
/// find the repository root (except `init`, which creates one).
fn run(cli: Cli) -> app::Result<String> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match cli.command {
        Command::Init { path } => {
            let root = path.unwrap_or_else(|| cwd.clone());
            app::init(&root)
        }
        Command::Status => app::status(&cwd),
        Command::Snapshot {
            intent,
            confidence,
            reasoning,
        } => app::snapshot(&cwd, &intent, confidence, reasoning.as_deref()),
        Command::Log { show_intent, json } => app::log(&cwd, json, show_intent),
        Command::Agent { command } => run_agent(command, &cwd),
    }
}

/// Dispatch the `agent` subcommands.
fn run_agent(command: AgentCommand, cwd: &std::path::Path) -> app::Result<String> {
    match command {
        AgentCommand::Commit {
            intent,
            confidence,
            reasoning,
        } => app::snapshot(cwd, &intent, confidence, reasoning.as_deref()),
        AgentCommand::Checkpoint { reason } => app::checkpoint(cwd, &reason),
        AgentCommand::Rollback {
            to_intent,
            to_confidence,
        } => match (to_intent, to_confidence) {
            (Some(query), _) => app::rollback_to_intent(cwd, &query),
            (_, Some(min)) => app::rollback_to_confidence(cwd, min),
            (None, None) => Err(app::CliError::NoRollbackTarget(
                "specify --to-intent <QUERY> or --to-confidence <MIN>".to_string(),
            )),
        },
        AgentCommand::Log => app::log(cwd, true, false),
        AgentCommand::Task { command } => match command {
            TaskCommand::Open { description, parent } => {
                app::task_open(cwd, &description, parent.as_deref())
            }
            TaskCommand::Complete { id } => app::task_complete(cwd, &id),
            TaskCommand::List => app::task_list(cwd),
        },
    }
}
