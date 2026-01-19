//! File deletion action

use crate::error::{ArtaError, Result};
use crate::parser::{WhereClause, CompareOp, Value};
use crate::engine::actions::ActionResult;
use std::fs;
use std::path::Path;

const MAX_FILES_PER_OPERATION: usize = 100;

pub fn delete_files(path: &str, where_clause: Option<&WhereClause>, dry_run: bool) -> Result<ActionResult> {
    let base_path = Path::new(path);
    
    if !base_path.exists() {
        return Err(ArtaError::PathNotFound(path.to_string()));
    }
    
    if !base_path.is_dir() {
        return Err(ArtaError::ExecutionError(format!("{} is not a directory", path)));
    }
    
    // Security check: require WHERE clause
    if where_clause.is_none() {
        return Err(ArtaError::SecurityError(
            "DELETE without WHERE clause is too dangerous. Add a WHERE clause to filter files.".to_string()
        ));
    }
    
    let mut matched_files: Vec<FileInfo> = Vec::new();
    
    // Scan directory (non-recursive for safety)
    for entry in fs::read_dir(base_path)
        .map_err(|e| ArtaError::IoError(e))?
    {
        let entry = entry.map_err(|e| ArtaError::IoError(e))?;
        let file_path = entry.path();
        
        if file_path.is_file() {
            let metadata = fs::metadata(&file_path)
                .map_err(|e| ArtaError::IoError(e))?;
            
            let file_info = FileInfo {
                path: file_path.to_string_lossy().to_string(),
                name: file_path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                size: metadata.len(),
                extension: file_path.extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_default(),
            };
            
            if let Some(wc) = where_clause {
                if matches_file_where_clause(&file_info, wc) {
                    matched_files.push(file_info);
                }
            }
        }
    }
    
    // Safety limit
    if matched_files.len() > MAX_FILES_PER_OPERATION {
        return Err(ArtaError::SecurityError(format!(
            "Too many files to delete ({} > {}). Please use a more specific WHERE clause.",
            matched_files.len(),
            MAX_FILES_PER_OPERATION
        )));
    }
    
    let mut details = Vec::new();
    let mut deleted_count = 0;
    
    for file in &matched_files {
        if dry_run {
            details.push(format!("Would delete: {} ({} bytes)", file.path, file.size));
        } else {
            match fs::remove_file(&file.path) {
                Ok(_) => {
                    details.push(format!("Deleted: {}", file.path));
                    deleted_count += 1;
                }
                Err(e) => {
                    details.push(format!("Failed to delete {}: {}", file.path, e));
                }
            }
        }
    }
    
    Ok(ActionResult {
        action_type: "DELETE FILES".to_string(),
        affected_count: if dry_run { matched_files.len() } else { deleted_count },
        dry_run,
        details,
    })
}

#[derive(Debug)]
struct FileInfo {
    path: String,
    name: String,
    size: u64,
    extension: String,
}

fn matches_file_where_clause(file: &FileInfo, where_clause: &WhereClause) -> bool {
    for condition_expr in &where_clause.conditions {
        if !matches_file_condition(file, &condition_expr.condition) {
            return false;
        }
    }
    true
}

fn matches_file_condition(file: &FileInfo, condition: &crate::parser::Condition) -> bool {
    let field = condition.field.to_lowercase();
    
    match field.as_str() {
        "size" => {
            let target = match &condition.value {
                Value::Number(n) => *n as u64,
                Value::Size(s) => *s,
                _ => return false,
            };
            compare_numbers(file.size as f64, target as f64, &condition.operator)
        }
        "name" => {
            if let Value::String(s) = &condition.value {
                compare_strings(&file.name, s, &condition.operator)
            } else {
                false
            }
        }
        "extension" | "ext" => {
            if let Value::String(s) = &condition.value {
                compare_strings(&file.extension, s, &condition.operator)
            } else {
                false
            }
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
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;
    
    #[test]
    fn test_delete_files_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "test content").unwrap();
        
        // Create WHERE clause for size > 0
        let where_clause = WhereClause {
            conditions: vec![crate::parser::ConditionExpr {
                condition: crate::parser::Condition {
                    field: "size".to_string(),
                    operator: CompareOp::GreaterThan,
                    value: Value::Number(0.0),
                },
                next: None,
            }],
        };
        
        let result = delete_files(
            temp_dir.path().to_str().unwrap(),
            Some(&where_clause),
            true  // dry_run
        ).unwrap();
        
        assert!(result.dry_run);
        assert_eq!(result.affected_count, 1);
        
        // File should still exist
        assert!(file_path.exists());
    }
    
    #[test]
    fn test_delete_requires_where_clause() {
        let temp_dir = TempDir::new().unwrap();
        
        let result = delete_files(
            temp_dir.path().to_str().unwrap(),
            None,  // No WHERE clause
            false
        );
        
        assert!(result.is_err());
    }
}
