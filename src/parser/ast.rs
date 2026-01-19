//! Abstract Syntax Tree definitions for Arta DSL

use serde::{Deserialize, Serialize};

/// Top-level command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Query(QueryCommand),
    Action(ActionCommand),
    Context(ContextCommand),
    Let(LetStatement),
    For(ForLoop),
    If(IfStatement),
    Life(LifeMonitor),
    Print(PrintCommand),
    Container(ContainerCommand),
    Explain(Box<Command>),
}

/// A script is a sequence of commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub statements: Vec<Command>,
}

// ============================================================================
// LIFE Monitoring
// ============================================================================

/// LIFE monitoring block for continuous observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeMonitor {
    /// What to monitor (BATTERY, CPU, MEMORY, etc.)
    pub target: LifeTarget,
    /// Commands to execute when changes are detected
    pub body: Vec<Command>,
}

/// Targets that can be monitored with LIFE
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeTarget {
    Battery,
    Memory,
    Cpu,
    Disk,
    Network,
    Processes,
}

impl std::fmt::Display for LifeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifeTarget::Battery => write!(f, "BATTERY"),
            LifeTarget::Memory => write!(f, "MEMORY"),
            LifeTarget::Cpu => write!(f, "CPU"),
            LifeTarget::Disk => write!(f, "DISK"),
            LifeTarget::Network => write!(f, "NETWORK"),
            LifeTarget::Processes => write!(f, "PROCESSES"),
        }
    }
}

// ============================================================================
// PRINT Command
// ============================================================================

/// PRINT command for outputting values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintCommand {
    pub expressions: Vec<PrintExpr>,
}

/// Expression in a PRINT command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrintExpr {
    /// Query a specific field (e.g., BATTERY LEVEL)
    QueryField { target: QueryTarget, field: String },
    /// A literal string
    String(String),
    /// A variable reference
    Variable(String),
}

// ============================================================================
// Control Flow
// ============================================================================

/// FOR loop that iterates over query results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForLoop {
    /// The iterator variable name (e.g., "file" in "FOR file IN ...")
    pub iterator_var: String,
    /// The source query to iterate over
    pub source_query: QueryCommand,
    /// The body of commands to execute for each iteration
    pub body: Vec<Command>,
}

/// IF conditional statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfStatement {
    /// The condition to evaluate
    pub condition: IfCondition,
    /// Commands to execute if condition is true
    pub then_body: Vec<Command>,
    /// Commands to execute if condition is false (optional)
    pub else_body: Option<Vec<Command>>,
}

/// Condition for IF statement - based on query result comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfCondition {
    /// The query target (CPU, MEMORY, etc.)
    pub target: QueryTarget,
    /// The field to compare
    pub field: String,
    /// The comparison operator
    pub operator: CompareOp,
    /// The value to compare against
    pub value: Value,
}

// ============================================================================
// LET Statement
// ============================================================================

/// Variable assignment statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetStatement {
    pub name: String,
    pub value: LetValue,
}

/// Value types that can be assigned to variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LetValue {
    String(String),
    Number(f64),
    Size(u64),
    Boolean(bool),
    Path(String),
}

// ============================================================================
// Context Commands
// ============================================================================

/// Context navigation commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextCommand {
    EnterFolder(String),
    EnterFile(String),
    Exit,
    Reset,
    Show(ShowTarget),
}

/// What to show with SHOW command
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShowTarget {
    Context,
    Variables,
    History,
}

impl std::fmt::Display for ShowTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShowTarget::Context => write!(f, "CONTEXT"),
            ShowTarget::Variables => write!(f, "VARIABLES"),
            ShowTarget::History => write!(f, "HISTORY"),
        }
    }
}

// ============================================================================
// Query Commands
// ============================================================================

/// Query command for reading system state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCommand {
    pub target: QueryTarget,
    pub fields: FieldList,
    pub from_path: Option<String>,
    pub where_clause: Option<WhereClause>,
}

/// Available query targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryTarget {
    Cpu,
    Memory,
    Disk,
    Network,
    System,
    Battery,
    Process,
    Files,
    Content,
}

impl std::fmt::Display for QueryTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryTarget::Cpu => write!(f, "CPU"),
            QueryTarget::Memory => write!(f, "MEMORY"),
            QueryTarget::Disk => write!(f, "DISK"),
            QueryTarget::Network => write!(f, "NETWORK"),
            QueryTarget::System => write!(f, "SYSTEM"),
            QueryTarget::Battery => write!(f, "BATTERY"),
            QueryTarget::Process => write!(f, "PROCESS"),
            QueryTarget::Files => write!(f, "FILES"),
            QueryTarget::Content => write!(f, "CONTENT"),
        }
    }
}

/// Field selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldList {
    All,
    Fields(Vec<String>),
}

/// WHERE clause for filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereClause {
    pub conditions: Vec<ConditionExpr>,
}

/// Condition expression with optional logical operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionExpr {
    pub condition: Condition,
    pub next: Option<(LogicalOp, Box<ConditionExpr>)>,
}

/// Single condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: CompareOp,
    pub value: Value,
}

/// Logical operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicalOp {
    And,
    Or,
}

/// Comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Like,
    Contains,
    Matches,
}

impl std::fmt::Display for CompareOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompareOp::Equal => write!(f, "="),
            CompareOp::NotEqual => write!(f, "!="),
            CompareOp::GreaterThan => write!(f, ">"),
            CompareOp::GreaterThanOrEqual => write!(f, ">="),
            CompareOp::LessThan => write!(f, "<"),
            CompareOp::LessThanOrEqual => write!(f, "<="),
            CompareOp::Like => write!(f, "LIKE"),
            CompareOp::Contains => write!(f, "CONTAINS"),
            CompareOp::Matches => write!(f, "MATCHES"),
        }
    }
}

// ============================================================================
// Values
// ============================================================================

/// Value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Number(f64),
    Size(u64), // Size in bytes
    Boolean(bool),
    Identifier(String), // For variable references
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Number(n) => write!(f, "{}", n),
            Value::Size(s) => write!(f, "{}", bytesize::ByteSize(*s)),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Identifier(id) => write!(f, "{}", id),
        }
    }
}

// ============================================================================
// Action Commands
// ============================================================================

/// Action commands that modify system state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionCommand {
    DeleteFiles(DeleteFilesCommand),
    KillProcess(KillProcessCommand),
}

/// DELETE FILES command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFilesCommand {
    pub path: String,
    pub where_clause: Option<WhereClause>,
}

/// KILL PROCESS command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillProcessCommand {
    pub where_clause: WhereClause,
}

// ============================================================================
// Container Commands
// ============================================================================

/// Container management commands for sandboxed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerCommand {
    /// Create a new container with optional initialization body
    Create(CreateContainer),
    /// Switch to a different container
    Switch(String),
    /// List all containers
    List,
    /// Destroy a container
    Destroy(String),
    /// Export a container to a script file
    Export(ExportContainer),
}

/// CREATE CONTAINER command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateContainer {
    /// Container name
    pub name: String,
    /// Container options (allow_actions, readonly)
    pub options: ContainerOptions,
    /// Initialization commands to run in the container
    pub body: Vec<Command>,
}

/// Options for container creation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerOptions {
    /// Whether to allow action commands (DELETE, KILL) in this container
    pub allow_actions: bool,
    /// Whether the container is read-only (no file modifications)
    pub readonly: bool,
}

/// EXPORT CONTAINER command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportContainer {
    /// Name of the container to export
    pub name: String,
    /// Path to export the container script to
    pub path: String,
}
