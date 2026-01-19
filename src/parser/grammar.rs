//! Pest grammar parser for Arta DSL

use pest::Parser;
use pest_derive::Parser;

use crate::error::{ArtaError, Result};
use crate::parser::ast::*;

#[derive(Parser)]
#[grammar = "../grammar/arta.pest"]
pub struct ArtaParser;

/// Parse a command string into an AST
pub fn parse_command(input: &str) -> Result<Command> {
    let pairs = ArtaParser::parse(Rule::command, input)
        .map_err(|e| ArtaError::ParseError(e.to_string()))?;

    let pair = pairs
        .into_iter()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Empty input".to_string()))?;

    parse_command_inner(pair)
}

/// Parse a script (multiple statements) into an AST
pub fn parse_script(input: &str) -> Result<Script> {
    let pairs =
        ArtaParser::parse(Rule::script, input).map_err(|e| ArtaError::ParseError(e.to_string()))?;

    let pair = pairs
        .into_iter()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Empty script".to_string()))?;

    let mut statements = Vec::new();

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::statement {
            statements.push(parse_statement(inner)?);
        }
    }

    Ok(Script { statements })
}

fn parse_command_inner(pair: pest::iterators::Pair<Rule>) -> Result<Command> {
    // command -> statement
    let statement = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected statement".to_string()))?;

    parse_statement(statement)
}

fn parse_statement(pair: pest::iterators::Pair<Rule>) -> Result<Command> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected statement content".to_string()))?;

    match inner.as_rule() {
        Rule::container_cmd => Ok(Command::Container(parse_container_cmd(inner)?)),
        Rule::life_cmd => Ok(Command::Life(parse_life_cmd(inner)?)),
        Rule::for_cmd => Ok(Command::For(parse_for_cmd(inner)?)),
        Rule::if_cmd => Ok(Command::If(parse_if_cmd(inner)?)),
        Rule::simple_cmd => parse_simple_cmd(inner),
        _ => Err(ArtaError::ParseError(format!(
            "Unexpected rule in statement: {:?}",
            inner.as_rule()
        ))),
    }
}

fn parse_simple_cmd(pair: pest::iterators::Pair<Rule>) -> Result<Command> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected simple command".to_string()))?;

    match inner.as_rule() {
        Rule::print_cmd => Ok(Command::Print(parse_print_cmd(inner)?)),
        Rule::explain_cmd => {
            let inner_cmd = inner.into_inner().next().ok_or_else(|| {
                ArtaError::ParseError("Expected command after EXPLAIN".to_string())
            })?;
            let cmd = match inner_cmd.as_rule() {
                Rule::query_cmd => Command::Query(parse_query_cmd(inner_cmd)?),
                Rule::action_cmd => Command::Action(parse_action_cmd(inner_cmd)?),
                _ => return Err(ArtaError::ParseError("Invalid EXPLAIN target".to_string())),
            };
            Ok(Command::Explain(Box::new(cmd)))
        }
        Rule::let_cmd => Ok(Command::Let(parse_let_cmd(inner)?)),
        Rule::context_cmd => Ok(Command::Context(parse_context_cmd(inner)?)),
        Rule::query_cmd => Ok(Command::Query(parse_query_cmd(inner)?)),
        Rule::action_cmd => Ok(Command::Action(parse_action_cmd(inner)?)),
        _ => Err(ArtaError::ParseError(format!(
            "Unexpected rule: {:?}",
            inner.as_rule()
        ))),
    }
}

// ============================================================================
// LIFE Monitoring Parsing
// ============================================================================

fn parse_life_cmd(pair: pest::iterators::Pair<Rule>) -> Result<LifeMonitor> {
    let mut inner = pair.into_inner();

    // Parse life target
    let target_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected target in LIFE".to_string()))?;
    let target = parse_life_target(target_pair)?;

    // Parse statement block (body)
    let block_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected statement block in LIFE".to_string()))?;
    let body = parse_statement_block(block_pair)?;

    Ok(LifeMonitor { target, body })
}

fn parse_life_target(pair: pest::iterators::Pair<Rule>) -> Result<LifeTarget> {
    let target_str = pair.as_str().to_uppercase();
    match target_str.as_str() {
        "BATTERY" => Ok(LifeTarget::Battery),
        "MEMORY" => Ok(LifeTarget::Memory),
        "CPU" => Ok(LifeTarget::Cpu),
        "DISK" => Ok(LifeTarget::Disk),
        "NETWORK" => Ok(LifeTarget::Network),
        "PROCESSES" => Ok(LifeTarget::Processes),
        _ => Err(ArtaError::ParseError(format!(
            "Unknown LIFE target: {}",
            target_str
        ))),
    }
}

// ============================================================================
// Container Command Parsing
// ============================================================================

fn parse_container_cmd(pair: pest::iterators::Pair<Rule>) -> Result<ContainerCommand> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected container command".to_string()))?;

    match inner.as_rule() {
        Rule::create_container => parse_create_container(inner),
        Rule::switch_container => parse_switch_container(inner),
        Rule::list_containers => Ok(ContainerCommand::List),
        Rule::destroy_container => parse_destroy_container(inner),
        Rule::export_container => parse_export_container(inner),
        _ => Err(ArtaError::ParseError(format!(
            "Unknown container command: {:?}",
            inner.as_rule()
        ))),
    }
}

fn parse_create_container(pair: pest::iterators::Pair<Rule>) -> Result<ContainerCommand> {
    let mut inner = pair.into_inner();

    // Parse container name
    let name_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected container name".to_string()))?;
    let name = parse_container_name(name_pair)?;

    // Parse options and body
    let mut options = ContainerOptions::default();
    let mut body = Vec::new();

    for item in inner {
        match item.as_rule() {
            Rule::container_options => {
                options = parse_container_options(item)?;
            }
            Rule::statement_block => {
                body = parse_statement_block(item)?;
            }
            _ => {}
        }
    }

    Ok(ContainerCommand::Create(CreateContainer {
        name,
        options,
        body,
    }))
}

fn parse_container_name(pair: pest::iterators::Pair<Rule>) -> Result<String> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected container name value".to_string()))?;

    match inner.as_rule() {
        Rule::string_value => {
            let s = inner.as_str();
            Ok(s[1..s.len() - 1].to_string())
        }
        Rule::identifier => Ok(inner.as_str().to_string()),
        _ => Err(ArtaError::ParseError("Invalid container name".to_string())),
    }
}

fn parse_container_options(pair: pest::iterators::Pair<Rule>) -> Result<ContainerOptions> {
    let mut options = ContainerOptions::default();

    for item in pair.into_inner() {
        if item.as_rule() == Rule::container_option {
            let opt_inner = item.into_inner().next();
            if let Some(opt) = opt_inner {
                match opt.as_rule() {
                    Rule::allow_actions_opt => options.allow_actions = true,
                    Rule::readonly_opt => options.readonly = true,
                    _ => {}
                }
            }
        }
    }

    Ok(options)
}

fn parse_switch_container(pair: pest::iterators::Pair<Rule>) -> Result<ContainerCommand> {
    let name_pair = pair.into_inner().next().ok_or_else(|| {
        ArtaError::ParseError("Expected container name after SWITCH CONTAINER".to_string())
    })?;
    let name = parse_container_name(name_pair)?;
    Ok(ContainerCommand::Switch(name))
}

fn parse_destroy_container(pair: pest::iterators::Pair<Rule>) -> Result<ContainerCommand> {
    let name_pair = pair.into_inner().next().ok_or_else(|| {
        ArtaError::ParseError("Expected container name after DESTROY CONTAINER".to_string())
    })?;
    let name = parse_container_name(name_pair)?;
    Ok(ContainerCommand::Destroy(name))
}

fn parse_export_container(pair: pest::iterators::Pair<Rule>) -> Result<ContainerCommand> {
    let mut inner = pair.into_inner();

    let name_pair = inner.next().ok_or_else(|| {
        ArtaError::ParseError("Expected container name after EXPORT CONTAINER".to_string())
    })?;
    let name = parse_container_name(name_pair)?;

    let path_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected path after TO".to_string()))?;
    let path = parse_path_value(path_pair)?;

    Ok(ContainerCommand::Export(ExportContainer { name, path }))
}

// ============================================================================
// PRINT Command Parsing
// ============================================================================

fn parse_print_cmd(pair: pest::iterators::Pair<Rule>) -> Result<PrintCommand> {
    let mut expressions = Vec::new();

    for expr_pair in pair.into_inner() {
        if expr_pair.as_rule() == Rule::print_expr {
            expressions.push(parse_print_expr(expr_pair)?);
        }
    }

    Ok(PrintCommand { expressions })
}

fn parse_print_expr(pair: pest::iterators::Pair<Rule>) -> Result<PrintExpr> {
    let mut inner = pair.into_inner();

    let first = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected print expression".to_string()))?;

    match first.as_rule() {
        Rule::query_target => {
            // This is QueryTarget followed by field
            let target = parse_query_target(first)?;
            let field = inner
                .next()
                .ok_or_else(|| {
                    ArtaError::ParseError("Expected field after query target in PRINT".to_string())
                })?
                .as_str()
                .to_string();
            Ok(PrintExpr::QueryField { target, field })
        }
        Rule::string_value => {
            let s = first.as_str();
            Ok(PrintExpr::String(s[1..s.len() - 1].to_string()))
        }
        Rule::identifier => Ok(PrintExpr::Variable(first.as_str().to_string())),
        _ => Err(ArtaError::ParseError(format!(
            "Invalid print expression: {:?}",
            first.as_rule()
        ))),
    }
}

// ============================================================================
// Control Flow Parsing
// ============================================================================

fn parse_for_cmd(pair: pest::iterators::Pair<Rule>) -> Result<ForLoop> {
    let mut inner = pair.into_inner();

    // Parse iterator variable
    let iter_var = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected iterator variable in FOR".to_string()))?
        .as_str()
        .to_string();

    // Parse source query
    let query_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected query in FOR".to_string()))?;
    let source_query = parse_query_cmd(query_pair)?;

    // Parse statement block (body)
    let block_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected statement block in FOR".to_string()))?;
    let body = parse_statement_block(block_pair)?;

    Ok(ForLoop {
        iterator_var: iter_var,
        source_query,
        body,
    })
}

fn parse_if_cmd(pair: pest::iterators::Pair<Rule>) -> Result<IfStatement> {
    let mut inner = pair.into_inner();

    // Parse condition
    let condition_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected condition in IF".to_string()))?;
    let condition = parse_if_condition(condition_pair)?;

    // Parse THEN block
    let then_block = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected THEN block in IF".to_string()))?;
    let then_body = parse_statement_block(then_block)?;

    // Parse optional ELSE block
    let else_body = if let Some(else_pair) = inner.next() {
        if else_pair.as_rule() == Rule::else_clause {
            let else_block = else_pair.into_inner().next().ok_or_else(|| {
                ArtaError::ParseError("Expected statement block in ELSE".to_string())
            })?;
            Some(parse_statement_block(else_block)?)
        } else {
            None
        }
    } else {
        None
    };

    Ok(IfStatement {
        condition,
        then_body,
        else_body,
    })
}

fn parse_if_condition(pair: pest::iterators::Pair<Rule>) -> Result<IfCondition> {
    let mut inner = pair.into_inner();

    // Parse query target
    let target_pair = inner.next().ok_or_else(|| {
        ArtaError::ParseError("Expected query target in IF condition".to_string())
    })?;
    let target = parse_query_target(target_pair)?;

    // Parse field
    let field = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected field in IF condition".to_string()))?
        .as_str()
        .to_string();

    // Parse operator
    let op_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected operator in IF condition".to_string()))?;
    let operator = parse_compare_op(op_pair)?;

    // Parse value
    let value_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected value in IF condition".to_string()))?;
    let value = parse_value(value_pair)?;

    Ok(IfCondition {
        target,
        field,
        operator,
        value,
    })
}

fn parse_statement_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Command>> {
    let mut commands = Vec::new();

    for stmt_pair in pair.into_inner() {
        if stmt_pair.as_rule() == Rule::statement {
            commands.push(parse_statement(stmt_pair)?);
        }
    }

    Ok(commands)
}

// ============================================================================
// Context Command Parsing
// ============================================================================

fn parse_context_cmd(pair: pest::iterators::Pair<Rule>) -> Result<ContextCommand> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected context command".to_string()))?;

    match inner.as_rule() {
        Rule::enter_cmd => parse_enter_cmd(inner),
        Rule::exit_cmd => Ok(ContextCommand::Exit),
        Rule::reset_cmd => Ok(ContextCommand::Reset),
        Rule::show_cmd => parse_show_cmd(inner),
        _ => Err(ArtaError::ParseError(format!(
            "Unknown context command: {:?}",
            inner.as_rule()
        ))),
    }
}

fn parse_enter_cmd(pair: pest::iterators::Pair<Rule>) -> Result<ContextCommand> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected FOLDER or FILE after ENTER".to_string()))?;

    match inner.as_rule() {
        Rule::enter_folder => {
            let path_pair = inner.into_inner().next().ok_or_else(|| {
                ArtaError::ParseError("Expected path after ENTER FOLDER".to_string())
            })?;
            let path = parse_path_value(path_pair)?;
            Ok(ContextCommand::EnterFolder(path))
        }
        Rule::enter_file => {
            let path_pair = inner.into_inner().next().ok_or_else(|| {
                ArtaError::ParseError("Expected path after ENTER FILE".to_string())
            })?;
            let path = parse_path_value(path_pair)?;
            Ok(ContextCommand::EnterFile(path))
        }
        _ => Err(ArtaError::ParseError("Invalid ENTER command".to_string())),
    }
}

fn parse_show_cmd(pair: pest::iterators::Pair<Rule>) -> Result<ContextCommand> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected target after SHOW".to_string()))?;

    let target_str = inner.as_str().to_uppercase();
    let target = match target_str.as_str() {
        "CONTEXT" => ShowTarget::Context,
        "VARIABLES" => ShowTarget::Variables,
        "HISTORY" => ShowTarget::History,
        _ => {
            return Err(ArtaError::ParseError(format!(
                "Unknown SHOW target: {}",
                target_str
            )))
        }
    };

    Ok(ContextCommand::Show(target))
}

// ============================================================================
// LET Command Parsing
// ============================================================================

fn parse_let_cmd(pair: pest::iterators::Pair<Rule>) -> Result<LetStatement> {
    let mut inner = pair.into_inner();

    let name_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected variable name in LET".to_string()))?;
    let name = name_pair.as_str().to_string();

    let value_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected value in LET".to_string()))?;
    let value = parse_let_value(value_pair)?;

    Ok(LetStatement { name, value })
}

fn parse_let_value(pair: pest::iterators::Pair<Rule>) -> Result<LetValue> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected value in LET".to_string()))?;

    match inner.as_rule() {
        Rule::path_value => {
            // Parse path_value which can be string_value, bare_path, or identifier
            let path_inner = inner
                .into_inner()
                .next()
                .ok_or_else(|| ArtaError::ParseError("Expected path value".to_string()))?;
            match path_inner.as_rule() {
                Rule::string_value => {
                    let s = path_inner.as_str();
                    Ok(LetValue::Path(s[1..s.len() - 1].to_string()))
                }
                Rule::bare_path => Ok(LetValue::Path(path_inner.as_str().to_string())),
                Rule::identifier => Ok(LetValue::String(path_inner.as_str().to_string())),
                _ => Err(ArtaError::ParseError("Invalid path value".to_string())),
            }
        }
        Rule::size_value => {
            let s = inner.as_str();
            let bytes = parse_size_value(s)?;
            Ok(LetValue::Size(bytes))
        }
        Rule::number => {
            let n: f64 = inner
                .as_str()
                .parse()
                .map_err(|_| ArtaError::ParseError("Invalid number in LET".to_string()))?;
            Ok(LetValue::Number(n))
        }
        Rule::boolean => {
            let b = inner.as_str().to_uppercase() == "TRUE";
            Ok(LetValue::Boolean(b))
        }
        Rule::string_value => {
            let s = inner.as_str();
            let content = &s[1..s.len() - 1];
            // Treat strings that look like paths as paths
            if content.starts_with('/') || content.starts_with("~/") {
                Ok(LetValue::Path(content.to_string()))
            } else {
                Ok(LetValue::String(content.to_string()))
            }
        }
        _ => Err(ArtaError::ParseError(format!(
            "Invalid LET value type: {:?}",
            inner.as_rule()
        ))),
    }
}

// ============================================================================
// Query Command Parsing
// ============================================================================

fn parse_query_cmd(pair: pest::iterators::Pair<Rule>) -> Result<QueryCommand> {
    let mut inner = pair.into_inner();

    let target = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected query target".to_string()))?;
    let target = parse_query_target(target)?;

    let fields = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected field list".to_string()))?;
    let fields = parse_field_list(fields)?;

    let mut from_path = None;
    let mut where_clause = None;

    for item in inner {
        match item.as_rule() {
            Rule::from_clause => {
                from_path = Some(parse_from_clause(item)?);
            }
            Rule::where_clause => {
                where_clause = Some(parse_where_clause(item)?);
            }
            _ => {}
        }
    }

    Ok(QueryCommand {
        target,
        fields,
        from_path,
        where_clause,
    })
}

fn parse_query_target(pair: pest::iterators::Pair<Rule>) -> Result<QueryTarget> {
    let target_str = pair.as_str().to_uppercase();
    match target_str.as_str() {
        "CPU" => Ok(QueryTarget::Cpu),
        "MEMORY" => Ok(QueryTarget::Memory),
        "DISK" => Ok(QueryTarget::Disk),
        "NETWORK" => Ok(QueryTarget::Network),
        "SYSTEM" => Ok(QueryTarget::System),
        "BATTERY" => Ok(QueryTarget::Battery),
        "PROCESS" | "PROCESSES" => Ok(QueryTarget::Process),
        "FILES" => Ok(QueryTarget::Files),
        "CONTENT" => Ok(QueryTarget::Content),
        _ => Err(ArtaError::InvalidTarget(target_str)),
    }
}

fn parse_field_list(pair: pest::iterators::Pair<Rule>) -> Result<FieldList> {
    let mut inner_iter = pair.into_inner();
    let first = inner_iter
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected field list content".to_string()))?;

    match first.as_rule() {
        Rule::star => Ok(FieldList::All),
        Rule::field => {
            let mut fields = vec![first.as_str().to_string()];
            // Collect remaining fields
            for item in inner_iter {
                if item.as_rule() == Rule::field {
                    fields.push(item.as_str().to_string());
                }
            }
            Ok(FieldList::Fields(fields))
        }
        _ => {
            // Fallback - shouldn't normally reach here
            Ok(FieldList::All)
        }
    }
}

fn parse_from_clause(pair: pest::iterators::Pair<Rule>) -> Result<String> {
    let path_pair = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected path in FROM clause".to_string()))?;
    parse_path_value(path_pair)
}

fn parse_path_value(pair: pest::iterators::Pair<Rule>) -> Result<String> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected path value".to_string()))?;

    match inner.as_rule() {
        Rule::string_value => {
            let s = inner.as_str();
            // Remove quotes
            Ok(s[1..s.len() - 1].to_string())
        }
        Rule::bare_path => Ok(inner.as_str().to_string()),
        Rule::identifier => Ok(inner.as_str().to_string()),
        _ => Err(ArtaError::ParseError("Invalid path value".to_string())),
    }
}

// ============================================================================
// WHERE Clause Parsing
// ============================================================================

fn parse_where_clause(pair: pest::iterators::Pair<Rule>) -> Result<WhereClause> {
    let condition_expr = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected condition expression".to_string()))?;

    let conditions = parse_condition_expr(condition_expr)?;
    Ok(WhereClause {
        conditions: vec![conditions],
    })
}

fn parse_condition_expr(pair: pest::iterators::Pair<Rule>) -> Result<ConditionExpr> {
    let mut inner = pair.into_inner();

    let first_condition = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected condition".to_string()))?;
    let condition = parse_condition(first_condition)?;

    let mut next = None;

    while let Some(op_pair) = inner.next() {
        let logical_op = match op_pair.as_rule() {
            Rule::and_op => LogicalOp::And,
            Rule::or_op => LogicalOp::Or,
            _ => continue,
        };

        if let Some(next_cond) = inner.next() {
            let next_condition = parse_condition(next_cond)?;
            next = Some((
                logical_op,
                Box::new(ConditionExpr {
                    condition: next_condition,
                    next: None,
                }),
            ));
        }
    }

    Ok(ConditionExpr { condition, next })
}

fn parse_condition(pair: pest::iterators::Pair<Rule>) -> Result<Condition> {
    let mut inner = pair.into_inner();

    let field = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected field in condition".to_string()))?
        .as_str()
        .to_string();

    let op_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected operator in condition".to_string()))?;
    let operator = parse_compare_op(op_pair)?;

    let value_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected value in condition".to_string()))?;
    let value = parse_value(value_pair)?;

    Ok(Condition {
        field,
        operator,
        value,
    })
}

fn parse_compare_op(pair: pest::iterators::Pair<Rule>) -> Result<CompareOp> {
    let op_str = pair.as_str().to_uppercase();
    match op_str.as_str() {
        "=" => Ok(CompareOp::Equal),
        "!=" => Ok(CompareOp::NotEqual),
        ">" => Ok(CompareOp::GreaterThan),
        ">=" => Ok(CompareOp::GreaterThanOrEqual),
        "<" => Ok(CompareOp::LessThan),
        "<=" => Ok(CompareOp::LessThanOrEqual),
        "LIKE" => Ok(CompareOp::Like),
        "CONTAINS" => Ok(CompareOp::Contains),
        "MATCHES" => Ok(CompareOp::Matches),
        _ => Err(ArtaError::ParseError(format!(
            "Unknown operator: {}",
            op_str
        ))),
    }
}

fn parse_value(pair: pest::iterators::Pair<Rule>) -> Result<Value> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected value".to_string()))?;

    match inner.as_rule() {
        Rule::string_value => {
            let s = inner.as_str();
            Ok(Value::String(s[1..s.len() - 1].to_string()))
        }
        Rule::number => {
            let n: f64 = inner
                .as_str()
                .parse()
                .map_err(|_| ArtaError::ParseError("Invalid number".to_string()))?;
            Ok(Value::Number(n))
        }
        Rule::size_value => {
            let s = inner.as_str();
            let bytes = parse_size_value(s)?;
            Ok(Value::Size(bytes))
        }
        Rule::boolean => {
            let b = inner.as_str().to_uppercase() == "TRUE";
            Ok(Value::Boolean(b))
        }
        Rule::identifier => Ok(Value::Identifier(inner.as_str().to_string())),
        _ => Err(ArtaError::ParseError("Invalid value type".to_string())),
    }
}

fn parse_size_value(s: &str) -> Result<u64> {
    let s_upper = s.to_uppercase();

    let (num_str, multiplier) = if s_upper.ends_with("TB") {
        (&s[..s.len() - 2], 1024u64 * 1024 * 1024 * 1024)
    } else if s_upper.ends_with("GB") {
        (&s[..s.len() - 2], 1024u64 * 1024 * 1024)
    } else if s_upper.ends_with("MB") {
        (&s[..s.len() - 2], 1024u64 * 1024)
    } else if s_upper.ends_with("KB") {
        (&s[..s.len() - 2], 1024u64)
    } else if s_upper.ends_with("B") {
        (&s[..s.len() - 1], 1u64)
    } else {
        return Err(ArtaError::ParseError(format!("Invalid size unit: {}", s)));
    };

    let num: f64 = num_str
        .parse()
        .map_err(|_| ArtaError::ParseError(format!("Invalid size number: {}", num_str)))?;

    Ok((num * multiplier as f64) as u64)
}

// ============================================================================
// Action Command Parsing
// ============================================================================

fn parse_action_cmd(pair: pest::iterators::Pair<Rule>) -> Result<ActionCommand> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected action command".to_string()))?;

    match inner.as_rule() {
        Rule::delete_cmd => Ok(ActionCommand::DeleteFiles(parse_delete_cmd(inner)?)),
        Rule::kill_cmd => Ok(ActionCommand::KillProcess(parse_kill_cmd(inner)?)),
        _ => Err(ArtaError::ParseError("Unknown action command".to_string())),
    }
}

fn parse_delete_cmd(pair: pest::iterators::Pair<Rule>) -> Result<DeleteFilesCommand> {
    let mut inner = pair.into_inner();

    let path_pair = inner
        .next()
        .ok_or_else(|| ArtaError::ParseError("Expected path in DELETE command".to_string()))?;
    let path = parse_path_value(path_pair)?;

    let where_clause = inner.next().map(|p| parse_where_clause(p)).transpose()?;

    Ok(DeleteFilesCommand { path, where_clause })
}

fn parse_kill_cmd(pair: pest::iterators::Pair<Rule>) -> Result<KillProcessCommand> {
    let where_pair = pair.into_inner().next().ok_or_else(|| {
        ArtaError::ParseError("Expected WHERE clause in KILL command".to_string())
    })?;

    let where_clause = parse_where_clause(where_pair)?;

    Ok(KillProcessCommand { where_clause })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_query() {
        let cmd = parse_command("SELECT CPU *").unwrap();
        match cmd {
            Command::Query(q) => {
                assert_eq!(q.target, QueryTarget::Cpu);
                assert!(matches!(q.fields, FieldList::All));
            }
            _ => panic!("Expected Query command"),
        }
    }

    #[test]
    fn test_parse_memory_query_with_fields() {
        let cmd = parse_command("SELECT MEMORY total, used, free").unwrap();
        match cmd {
            Command::Query(q) => {
                assert_eq!(q.target, QueryTarget::Memory);
            }
            _ => panic!("Expected Query command"),
        }
    }

    #[test]
    fn test_parse_disk_query_with_from() {
        let cmd = parse_command("SELECT DISK * FROM /").unwrap();
        match cmd {
            Command::Query(q) => {
                assert_eq!(q.target, QueryTarget::Disk);
                assert_eq!(q.from_path, Some("/".to_string()));
            }
            _ => panic!("Expected Query command"),
        }
    }

    #[test]
    fn test_parse_process_query_with_where() {
        let cmd = parse_command("SELECT PROCESS * WHERE cpu > 10").unwrap();
        match cmd {
            Command::Query(q) => {
                assert_eq!(q.target, QueryTarget::Process);
                assert!(q.where_clause.is_some());
            }
            _ => panic!("Expected Query command"),
        }
    }

    #[test]
    fn test_parse_delete_command() {
        let cmd = parse_command("DELETE FILES FROM /tmp WHERE size > 100MB").unwrap();
        match cmd {
            Command::Action(ActionCommand::DeleteFiles(d)) => {
                assert_eq!(d.path, "/tmp");
                assert!(d.where_clause.is_some());
            }
            _ => panic!("Expected DeleteFiles command"),
        }
    }

    #[test]
    fn test_parse_kill_command() {
        let cmd = parse_command("KILL PROCESS WHERE name = \"node\"").unwrap();
        match cmd {
            Command::Action(ActionCommand::KillProcess(k)) => {
                assert!(k.where_clause.conditions.len() > 0);
            }
            _ => panic!("Expected KillProcess command"),
        }
    }

    #[test]
    fn test_parse_explain() {
        let cmd = parse_command("EXPLAIN SELECT CPU *").unwrap();
        match cmd {
            Command::Explain(inner) => match *inner {
                Command::Query(q) => assert_eq!(q.target, QueryTarget::Cpu),
                _ => panic!("Expected Query inside Explain"),
            },
            _ => panic!("Expected Explain command"),
        }
    }

    #[test]
    fn test_parse_size_values() {
        assert_eq!(parse_size_value("100MB").unwrap(), 100 * 1024 * 1024);
        assert_eq!(parse_size_value("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size_value("500KB").unwrap(), 500 * 1024);
    }

    #[test]
    fn test_case_insensitivity() {
        assert!(parse_command("select cpu *").is_ok());
        assert!(parse_command("SELECT CPU *").is_ok());
        assert!(parse_command("Select Cpu *").is_ok());
    }

    #[test]
    fn test_trailing_semicolon() {
        assert!(parse_command("SELECT CPU *;").is_ok());
        assert!(parse_command("SELECT CPU *").is_ok());
    }

    // Context command tests
    #[test]
    fn test_parse_enter_folder() {
        let cmd = parse_command("ENTER FOLDER /tmp").unwrap();
        match cmd {
            Command::Context(ContextCommand::EnterFolder(path)) => {
                assert_eq!(path, "/tmp");
            }
            _ => panic!("Expected EnterFolder command"),
        }
    }

    #[test]
    fn test_parse_enter_file() {
        let cmd = parse_command("ENTER FILE /etc/passwd").unwrap();
        match cmd {
            Command::Context(ContextCommand::EnterFile(path)) => {
                assert_eq!(path, "/etc/passwd");
            }
            _ => panic!("Expected EnterFile command"),
        }
    }

    #[test]
    fn test_parse_exit_context() {
        let cmd = parse_command("EXIT CONTEXT").unwrap();
        assert!(matches!(cmd, Command::Context(ContextCommand::Exit)));

        let cmd2 = parse_command("EXIT").unwrap();
        assert!(matches!(cmd2, Command::Context(ContextCommand::Exit)));
    }

    #[test]
    fn test_parse_reset_context() {
        let cmd = parse_command("RESET CONTEXT").unwrap();
        assert!(matches!(cmd, Command::Context(ContextCommand::Reset)));

        let cmd2 = parse_command("RESET").unwrap();
        assert!(matches!(cmd2, Command::Context(ContextCommand::Reset)));
    }

    #[test]
    fn test_parse_show_context() {
        let cmd = parse_command("SHOW CONTEXT").unwrap();
        match cmd {
            Command::Context(ContextCommand::Show(target)) => {
                assert_eq!(target, ShowTarget::Context);
            }
            _ => panic!("Expected Show Context command"),
        }
    }

    #[test]
    fn test_parse_show_variables() {
        let cmd = parse_command("SHOW VARIABLES").unwrap();
        match cmd {
            Command::Context(ContextCommand::Show(target)) => {
                assert_eq!(target, ShowTarget::Variables);
            }
            _ => panic!("Expected Show Variables command"),
        }
    }

    #[test]
    fn test_parse_show_history() {
        let cmd = parse_command("SHOW HISTORY").unwrap();
        match cmd {
            Command::Context(ContextCommand::Show(target)) => {
                assert_eq!(target, ShowTarget::History);
            }
            _ => panic!("Expected Show History command"),
        }
    }

    #[test]
    fn test_parse_content_query() {
        let cmd = parse_command("SELECT CONTENT *").unwrap();
        match cmd {
            Command::Query(q) => {
                assert_eq!(q.target, QueryTarget::Content);
            }
            _ => panic!("Expected Query command"),
        }
    }

    #[test]
    fn test_parse_files_query() {
        let cmd = parse_command("SELECT FILES * FROM /tmp").unwrap();
        match cmd {
            Command::Query(q) => {
                assert_eq!(q.target, QueryTarget::Files);
                assert_eq!(q.from_path, Some("/tmp".to_string()));
            }
            _ => panic!("Expected Query command"),
        }
    }

    // LET command tests
    #[test]
    fn test_parse_let_string() {
        let cmd = parse_command("LET name = \"hello\"").unwrap();
        match cmd {
            Command::Let(l) => {
                assert_eq!(l.name, "name");
                match l.value {
                    LetValue::String(s) => assert_eq!(s, "hello"),
                    _ => panic!("Expected String value"),
                }
            }
            _ => panic!("Expected Let command"),
        }
    }

    #[test]
    fn test_parse_let_number() {
        let cmd = parse_command("LET threshold = 80").unwrap();
        match cmd {
            Command::Let(l) => {
                assert_eq!(l.name, "threshold");
                match l.value {
                    LetValue::Number(n) => assert!((n - 80.0).abs() < 0.001),
                    _ => panic!("Expected Number value"),
                }
            }
            _ => panic!("Expected Let command"),
        }
    }

    #[test]
    fn test_parse_let_path() {
        let cmd = parse_command("LET my_path = /tmp").unwrap();
        match cmd {
            Command::Let(l) => {
                assert_eq!(l.name, "my_path");
                match l.value {
                    LetValue::Path(p) => assert_eq!(p, "/tmp"),
                    _ => panic!("Expected Path value"),
                }
            }
            _ => panic!("Expected Let command"),
        }
    }

    #[test]
    fn test_parse_let_size() {
        let cmd = parse_command("LET max_size = 100MB").unwrap();
        match cmd {
            Command::Let(l) => {
                assert_eq!(l.name, "max_size");
                match l.value {
                    LetValue::Size(s) => assert_eq!(s, 100 * 1024 * 1024),
                    _ => panic!("Expected Size value"),
                }
            }
            _ => panic!("Expected Let command"),
        }
    }

    #[test]
    fn test_parse_let_boolean() {
        let cmd = parse_command("LET enabled = true").unwrap();
        match cmd {
            Command::Let(l) => {
                assert_eq!(l.name, "enabled");
                match l.value {
                    LetValue::Boolean(b) => assert!(b),
                    _ => panic!("Expected Boolean value"),
                }
            }
            _ => panic!("Expected Let command"),
        }
    }

    #[test]
    fn test_parse_let_quoted_path() {
        let cmd = parse_command("LET my_path = \"/tmp/my folder\"").unwrap();
        match cmd {
            Command::Let(l) => {
                assert_eq!(l.name, "my_path");
                match l.value {
                    LetValue::Path(p) => assert_eq!(p, "/tmp/my folder"),
                    _ => panic!("Expected Path value"),
                }
            }
            _ => panic!("Expected Let command"),
        }
    }

    // FOR loop tests
    #[test]
    fn test_parse_for_loop_basic() {
        let cmd = parse_command(
            "FOR file IN SELECT FILES * FROM /tmp DO SELECT CONTENT * FROM file END FOR",
        )
        .unwrap();
        match cmd {
            Command::For(f) => {
                assert_eq!(f.iterator_var, "file");
                assert_eq!(f.source_query.target, QueryTarget::Files);
                assert_eq!(f.body.len(), 1);
            }
            _ => panic!("Expected For command"),
        }
    }

    #[test]
    fn test_parse_for_loop_with_where() {
        let cmd = parse_command("FOR f IN SELECT FILES * FROM /tmp WHERE extension = \"log\" DO SELECT CONTENT * END FOR").unwrap();
        match cmd {
            Command::For(f) => {
                assert_eq!(f.iterator_var, "f");
                assert!(f.source_query.where_clause.is_some());
            }
            _ => panic!("Expected For command"),
        }
    }

    #[test]
    fn test_parse_for_loop_multiple_body_statements() {
        let cmd = parse_command(
            "FOR file IN SELECT FILES * FROM /tmp DO LET x = 1; SELECT CPU * END FOR",
        )
        .unwrap();
        match cmd {
            Command::For(f) => {
                assert_eq!(f.body.len(), 2);
            }
            _ => panic!("Expected For command"),
        }
    }

    #[test]
    fn test_parse_for_loop_processes() {
        let cmd =
            parse_command("FOR proc IN SELECT PROCESS * WHERE cpu > 10 DO SELECT MEMORY * END FOR")
                .unwrap();
        match cmd {
            Command::For(f) => {
                assert_eq!(f.iterator_var, "proc");
                assert_eq!(f.source_query.target, QueryTarget::Process);
            }
            _ => panic!("Expected For command"),
        }
    }

    // IF statement tests
    #[test]
    fn test_parse_if_basic() {
        let cmd = parse_command("IF SELECT MEMORY used_percent > 80 THEN SELECT PROCESS * END IF")
            .unwrap();
        match cmd {
            Command::If(i) => {
                assert_eq!(i.condition.target, QueryTarget::Memory);
                assert_eq!(i.condition.field, "used_percent");
                assert_eq!(i.condition.operator, CompareOp::GreaterThan);
                assert_eq!(i.then_body.len(), 1);
                assert!(i.else_body.is_none());
            }
            _ => panic!("Expected If command"),
        }
    }

    #[test]
    fn test_parse_if_with_else() {
        let cmd = parse_command(
            "IF SELECT CPU usage > 90 THEN SELECT PROCESS * ELSE SELECT SYSTEM * END IF",
        )
        .unwrap();
        match cmd {
            Command::If(i) => {
                assert_eq!(i.condition.target, QueryTarget::Cpu);
                assert_eq!(i.then_body.len(), 1);
                assert!(i.else_body.is_some());
                assert_eq!(i.else_body.unwrap().len(), 1);
            }
            _ => panic!("Expected If command"),
        }
    }

    #[test]
    fn test_parse_if_multiple_statements() {
        let cmd = parse_command("IF SELECT DISK used_percent > 90 THEN LET warning = true; SELECT FILES * FROM /tmp END IF").unwrap();
        match cmd {
            Command::If(i) => {
                assert_eq!(i.then_body.len(), 2);
            }
            _ => panic!("Expected If command"),
        }
    }

    // Nested control flow tests
    #[test]
    fn test_parse_nested_if_in_for() {
        let cmd = parse_command("FOR file IN SELECT FILES * FROM /tmp DO IF SELECT MEMORY used_percent > 80 THEN SELECT CPU * END IF END FOR").unwrap();
        match cmd {
            Command::For(f) => {
                assert_eq!(f.body.len(), 1);
                match &f.body[0] {
                    Command::If(i) => {
                        assert_eq!(i.condition.target, QueryTarget::Memory);
                    }
                    _ => panic!("Expected nested If command"),
                }
            }
            _ => panic!("Expected For command"),
        }
    }

    #[test]
    fn test_parse_nested_for_in_for() {
        let cmd = parse_command("FOR dir IN SELECT FILES * FROM /tmp DO FOR file IN SELECT FILES * FROM dir DO SELECT CONTENT * END FOR END FOR").unwrap();
        match cmd {
            Command::For(outer) => {
                assert_eq!(outer.iterator_var, "dir");
                assert_eq!(outer.body.len(), 1);
                match &outer.body[0] {
                    Command::For(inner) => {
                        assert_eq!(inner.iterator_var, "file");
                    }
                    _ => panic!("Expected nested For command"),
                }
            }
            _ => panic!("Expected For command"),
        }
    }

    // LIFE monitoring tests
    #[test]
    fn test_parse_life_battery() {
        let cmd = parse_command("LIFE MONITOR BATTERY DO PRINT BATTERY level END LIFE").unwrap();
        match cmd {
            Command::Life(l) => {
                assert_eq!(l.target, LifeTarget::Battery);
                assert_eq!(l.body.len(), 1);
            }
            _ => panic!("Expected Life command"),
        }
    }

    #[test]
    fn test_parse_life_memory() {
        let cmd = parse_command("LIFE MONITOR MEMORY DO SELECT MEMORY * END LIFE").unwrap();
        match cmd {
            Command::Life(l) => {
                assert_eq!(l.target, LifeTarget::Memory);
            }
            _ => panic!("Expected Life command"),
        }
    }

    #[test]
    fn test_parse_life_cpu_multiple_statements() {
        let cmd = parse_command(
            "LIFE MONITOR CPU DO PRINT CPU usage; SELECT PROCESS * WHERE cpu > 50 END LIFE",
        )
        .unwrap();
        match cmd {
            Command::Life(l) => {
                assert_eq!(l.target, LifeTarget::Cpu);
                assert_eq!(l.body.len(), 2);
            }
            _ => panic!("Expected Life command"),
        }
    }

    // PRINT command tests
    #[test]
    fn test_parse_print_string() {
        let cmd = parse_command("PRINT \"Hello World\"").unwrap();
        match cmd {
            Command::Print(p) => {
                assert_eq!(p.expressions.len(), 1);
                match &p.expressions[0] {
                    PrintExpr::String(s) => assert_eq!(s, "Hello World"),
                    _ => panic!("Expected String expression"),
                }
            }
            _ => panic!("Expected Print command"),
        }
    }

    #[test]
    fn test_parse_print_query_field() {
        let cmd = parse_command("PRINT BATTERY level").unwrap();
        match cmd {
            Command::Print(p) => {
                assert_eq!(p.expressions.len(), 1);
                match &p.expressions[0] {
                    PrintExpr::QueryField { target, field } => {
                        assert_eq!(*target, QueryTarget::Battery);
                        assert_eq!(field, "level");
                    }
                    _ => panic!("Expected QueryField expression"),
                }
            }
            _ => panic!("Expected Print command"),
        }
    }

    #[test]
    fn test_parse_print_multiple() {
        let cmd = parse_command("PRINT BATTERY level, \"status:\", BATTERY state").unwrap();
        match cmd {
            Command::Print(p) => {
                assert_eq!(p.expressions.len(), 3);
            }
            _ => panic!("Expected Print command"),
        }
    }

    // Script parsing tests
    #[test]
    fn test_parse_script_single_statement() {
        let script = parse_script("SELECT CPU *").unwrap();
        assert_eq!(script.statements.len(), 1);
    }

    #[test]
    fn test_parse_script_multiple_statements() {
        let script = parse_script("SELECT CPU *; SELECT MEMORY *; SELECT BATTERY *").unwrap();
        assert_eq!(script.statements.len(), 3);
    }

    #[test]
    fn test_parse_script_with_comments() {
        let script = parse_script(
            r#"
            -- This is a comment
            SELECT CPU *;
            # Another comment style
            SELECT MEMORY *;
            // Yet another comment
            SELECT BATTERY *
        "#,
        )
        .unwrap();
        assert_eq!(script.statements.len(), 3);
    }

    #[test]
    fn test_parse_script_with_variables_and_loops() {
        let script = parse_script(
            r#"
            LET threshold = 80;
            IF SELECT MEMORY usage > threshold THEN
                SELECT PROCESS * WHERE memory > 100MB
            END IF;
            FOR file IN SELECT FILES * FROM /tmp DO
                SELECT CONTENT *
            END FOR
        "#,
        )
        .unwrap();
        assert_eq!(script.statements.len(), 3);
    }

    // Container command tests
    #[test]
    fn test_parse_create_container_basic() {
        let cmd =
            parse_command("CREATE CONTAINER \"sandbox\" DO SELECT CPU * END CONTAINER").unwrap();
        match cmd {
            Command::Container(ContainerCommand::Create(c)) => {
                assert_eq!(c.name, "sandbox");
                assert_eq!(c.body.len(), 1);
                assert!(!c.options.allow_actions);
                assert!(!c.options.readonly);
            }
            _ => panic!("Expected Create Container command"),
        }
    }

    #[test]
    fn test_parse_create_container_with_options() {
        let cmd = parse_command("CREATE CONTAINER \"sandbox\" WITH ALLOW ACTIONS, READONLY DO SELECT CPU * END CONTAINER").unwrap();
        match cmd {
            Command::Container(ContainerCommand::Create(c)) => {
                assert_eq!(c.name, "sandbox");
                assert!(c.options.allow_actions);
                assert!(c.options.readonly);
            }
            _ => panic!("Expected Create Container command"),
        }
    }

    #[test]
    fn test_parse_create_container_identifier_name() {
        let cmd =
            parse_command("CREATE CONTAINER mycontainer DO SELECT CPU * END CONTAINER").unwrap();
        match cmd {
            Command::Container(ContainerCommand::Create(c)) => {
                assert_eq!(c.name, "mycontainer");
            }
            _ => panic!("Expected Create Container command"),
        }
    }

    #[test]
    fn test_parse_create_container_multiple_statements() {
        let cmd = parse_command(
            "CREATE CONTAINER \"test\" DO LET x = 1; SELECT CPU *; SELECT MEMORY * END CONTAINER",
        )
        .unwrap();
        match cmd {
            Command::Container(ContainerCommand::Create(c)) => {
                assert_eq!(c.body.len(), 3);
            }
            _ => panic!("Expected Create Container command"),
        }
    }

    #[test]
    fn test_parse_switch_container() {
        let cmd = parse_command("SWITCH CONTAINER \"sandbox\"").unwrap();
        match cmd {
            Command::Container(ContainerCommand::Switch(name)) => {
                assert_eq!(name, "sandbox");
            }
            _ => panic!("Expected Switch Container command"),
        }
    }

    #[test]
    fn test_parse_list_containers() {
        let cmd = parse_command("LIST CONTAINERS").unwrap();
        match cmd {
            Command::Container(ContainerCommand::List) => {}
            _ => panic!("Expected List Containers command"),
        }
    }

    #[test]
    fn test_parse_destroy_container() {
        let cmd = parse_command("DESTROY CONTAINER \"sandbox\"").unwrap();
        match cmd {
            Command::Container(ContainerCommand::Destroy(name)) => {
                assert_eq!(name, "sandbox");
            }
            _ => panic!("Expected Destroy Container command"),
        }
    }

    #[test]
    fn test_parse_export_container() {
        let cmd = parse_command("EXPORT CONTAINER \"sandbox\" TO /tmp/sandbox.arta").unwrap();
        match cmd {
            Command::Container(ContainerCommand::Export(e)) => {
                assert_eq!(e.name, "sandbox");
                assert_eq!(e.path, "/tmp/sandbox.arta");
            }
            _ => panic!("Expected Export Container command"),
        }
    }

    #[test]
    fn test_parse_container_with_life() {
        let cmd = parse_command("CREATE CONTAINER \"monitor\" DO LIFE MONITOR BATTERY DO PRINT BATTERY level END LIFE END CONTAINER").unwrap();
        match cmd {
            Command::Container(ContainerCommand::Create(c)) => {
                assert_eq!(c.name, "monitor");
                assert_eq!(c.body.len(), 1);
                match &c.body[0] {
                    Command::Life(l) => {
                        assert_eq!(l.target, LifeTarget::Battery);
                    }
                    _ => panic!("Expected Life command inside container"),
                }
            }
            _ => panic!("Expected Create Container command"),
        }
    }
}
