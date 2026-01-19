//! Process kill action

use crate::engine::actions::ActionResult;
use crate::error::{ArtaError, Result};
use crate::parser::{CompareOp, Value, WhereClause};
use sysinfo::{Pid, Signal, System};

const MAX_PROCESSES_PER_OPERATION: usize = 10;

pub fn kill_processes(where_clause: &WhereClause, dry_run: bool) -> Result<ActionResult> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut matched_processes: Vec<ProcessMatch> = Vec::new();

    for (pid, process) in sys.processes() {
        let proc_info = ProcessMatch {
            pid: pid.as_u32(),
            name: process.name().to_string(),
            cpu: process.cpu_usage(),
            memory: process.memory(),
        };

        if matches_process_where_clause(&proc_info, where_clause) {
            // Don't allow killing system-critical processes
            if is_protected_process(&proc_info.name) {
                continue;
            }
            matched_processes.push(proc_info);
        }
    }

    // Safety limit
    if matched_processes.len() > MAX_PROCESSES_PER_OPERATION {
        return Err(ArtaError::SecurityError(format!(
            "Too many processes to kill ({} > {}). Please use a more specific WHERE clause.",
            matched_processes.len(),
            MAX_PROCESSES_PER_OPERATION
        )));
    }

    let mut details = Vec::new();
    let mut killed_count = 0;

    for proc in &matched_processes {
        if dry_run {
            details.push(format!("Would kill: {} (PID {})", proc.name, proc.pid));
        } else {
            // Re-get the process from a fresh system snapshot
            let mut fresh_sys = System::new_all();
            fresh_sys.refresh_all();

            if let Some(process) = fresh_sys.process(Pid::from_u32(proc.pid)) {
                if process.kill_with(Signal::Term).unwrap_or(false) {
                    details.push(format!("Killed: {} (PID {})", proc.name, proc.pid));
                    killed_count += 1;
                } else {
                    details.push(format!("Failed to kill: {} (PID {})", proc.name, proc.pid));
                }
            } else {
                details.push(format!(
                    "Process no longer exists: {} (PID {})",
                    proc.name, proc.pid
                ));
            }
        }
    }

    if matched_processes.is_empty() {
        details.push("No matching processes found".to_string());
    }

    Ok(ActionResult {
        action_type: "KILL PROCESS".to_string(),
        affected_count: if dry_run {
            matched_processes.len()
        } else {
            killed_count
        },
        dry_run,
        details,
    })
}

#[derive(Debug)]
struct ProcessMatch {
    pid: u32,
    name: String,
    cpu: f32,
    memory: u64,
}

fn is_protected_process(name: &str) -> bool {
    let protected = [
        "init",
        "systemd",
        "kernel",
        "launchd",
        "WindowServer",
        "loginwindow",
        "kernel_task",
        "syslogd",
        "notifyd",
    ];
    protected
        .iter()
        .any(|p| name.to_lowercase().contains(&p.to_lowercase()))
}

fn matches_process_where_clause(proc: &ProcessMatch, where_clause: &WhereClause) -> bool {
    for condition_expr in &where_clause.conditions {
        if !matches_process_condition(proc, &condition_expr.condition) {
            return false;
        }
    }
    true
}

fn matches_process_condition(proc: &ProcessMatch, condition: &crate::parser::Condition) -> bool {
    let field = condition.field.to_lowercase();

    match field.as_str() {
        "pid" => {
            if let Value::Number(n) = &condition.value {
                compare_numbers(proc.pid as f64, *n, &condition.operator)
            } else {
                false
            }
        }
        "name" => {
            if let Value::String(s) = &condition.value {
                compare_strings(&proc.name, s, &condition.operator)
            } else {
                false
            }
        }
        "cpu" => {
            if let Value::Number(n) = &condition.value {
                compare_numbers(proc.cpu as f64, *n, &condition.operator)
            } else {
                false
            }
        }
        "memory" => {
            let target = match &condition.value {
                Value::Number(n) => *n as u64,
                Value::Size(s) => *s,
                _ => return false,
            };
            compare_numbers(proc.memory as f64, target as f64, &condition.operator)
        }
        _ => true,
    }
}

fn compare_numbers(left: f64, right: f64, op: &CompareOp) -> bool {
    match op {
        CompareOp::Equal => (left - right).abs() < f64::EPSILON,
        CompareOp::NotEqual => (left - right).abs() >= f64::EPSILON,
        CompareOp::GreaterThan => left > right,
        CompareOp::GreaterThanOrEqual => left >= right,
        CompareOp::LessThan => left < right,
        CompareOp::LessThanOrEqual => left <= right,
        _ => false,
    }
}

fn compare_strings(left: &str, right: &str, op: &CompareOp) -> bool {
    match op {
        CompareOp::Equal => left.eq_ignore_ascii_case(right),
        CompareOp::NotEqual => !left.eq_ignore_ascii_case(right),
        CompareOp::Like => {
            let pattern = right.replace('%', ".*");
            regex::Regex::new(&format!("(?i)^{}$", pattern))
                .map(|r| r.is_match(left))
                .unwrap_or(false)
        }
        CompareOp::Contains => left.to_lowercase().contains(&right.to_lowercase()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_processes_with_filter() {
        // Create a WHERE clause that likely won't match anything
        let where_clause = WhereClause {
            conditions: vec![crate::parser::ConditionExpr {
                condition: crate::parser::Condition {
                    field: "name".to_string(),
                    operator: CompareOp::Equal,
                    value: Value::String("nonexistent_process_12345".to_string()),
                },
                next: None,
            }],
        };

        let result = kill_processes(&where_clause, true).unwrap();
        assert!(result.dry_run);
        assert_eq!(result.affected_count, 0);
    }

    #[test]
    fn test_kill_dry_run_no_matches() {
        let where_clause = WhereClause {
            conditions: vec![crate::parser::ConditionExpr {
                condition: crate::parser::Condition {
                    field: "pid".to_string(),
                    operator: CompareOp::Equal,
                    value: Value::Number(999999.0),
                },
                next: None,
            }],
        };

        let result = kill_processes(&where_clause, true).unwrap();
        assert_eq!(result.affected_count, 0);
    }

    #[test]
    fn test_protected_processes() {
        assert!(is_protected_process("systemd"));
        assert!(is_protected_process("init"));
        assert!(is_protected_process("launchd"));
        assert!(!is_protected_process("node"));
        assert!(!is_protected_process("python"));
    }
}
