//! Process query implementation

use crate::error::Result;
use crate::parser::{FieldList, WhereClause, CompareOp, Value};
use serde::{Serialize, Deserialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
    pub status: String,
    pub user: Option<String>,
}

pub fn query_processes(_fields: &FieldList, where_clause: Option<&WhereClause>) -> Result<Vec<ProcessInfo>> {
    let mut sys = System::new_all();
    sys.refresh_all();
    
    // Give it time to collect CPU usage
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_all();
    
    let mut processes: Vec<ProcessInfo> = sys.processes()
        .iter()
        .map(|(pid, process)| {
            ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string(),
                cpu: process.cpu_usage(),
                memory: process.memory(),
                status: format!("{:?}", process.status()),
                user: process.user_id().map(|u| format!("{:?}", u)),
            }
        })
        .collect();
    
    // Apply WHERE clause filtering
    if let Some(where_clause) = where_clause {
        processes = processes.into_iter()
            .filter(|p| matches_where_clause(p, where_clause))
            .collect();
    }
    
    // Sort by CPU usage descending
    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    
    Ok(processes)
}

fn matches_where_clause(process: &ProcessInfo, where_clause: &WhereClause) -> bool {
    for condition_expr in &where_clause.conditions {
        if !matches_condition(process, &condition_expr.condition) {
            return false;
        }
    }
    true
}

fn matches_condition(process: &ProcessInfo, condition: &crate::parser::Condition) -> bool {
    let field = condition.field.to_lowercase();
    
    match field.as_str() {
        "pid" => {
            if let Value::Number(n) = &condition.value {
                compare_numbers(process.pid as f64, *n, &condition.operator)
            } else {
                false
            }
        }
        "name" => {
            if let Value::String(s) = &condition.value {
                compare_strings(&process.name, s, &condition.operator)
            } else {
                false
            }
        }
        "cpu" => {
            if let Value::Number(n) = &condition.value {
                compare_numbers(process.cpu as f64, *n, &condition.operator)
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
            compare_numbers(process.memory as f64, target as f64, &condition.operator)
        }
        _ => true, // Unknown field - don't filter
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
        CompareOp::Equal => left == right,
        CompareOp::NotEqual => left != right,
        CompareOp::Like => {
            let pattern = right.replace('%', ".*");
            regex::Regex::new(&format!("^{}$", pattern))
                .map(|r| r.is_match(left))
                .unwrap_or(false)
        }
        CompareOp::Contains => left.contains(right),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_process_query() {
        let processes = query_processes(&FieldList::All, None).unwrap();
        assert!(!processes.is_empty());
    }
    
    #[test]
    fn test_compare_numbers() {
        assert!(compare_numbers(10.0, 5.0, &CompareOp::GreaterThan));
        assert!(compare_numbers(5.0, 10.0, &CompareOp::LessThan));
        assert!(compare_numbers(5.0, 5.0, &CompareOp::Equal));
    }
    
    #[test]
    fn test_compare_strings() {
        assert!(compare_strings("hello", "hello", &CompareOp::Equal));
        assert!(compare_strings("hello world", "world", &CompareOp::Contains));
    }
}
