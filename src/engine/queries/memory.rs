//! Memory query implementation

use crate::error::Result;
use crate::parser::FieldList;
use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub available: u64,
    pub usage_percent: f64,
}

pub fn query_memory(_fields: &FieldList) -> Result<MemoryInfo> {
    let mut sys = System::new_all();
    sys.refresh_memory();

    let total = sys.total_memory();
    let used = sys.used_memory();
    let free = sys.free_memory();
    let available = sys.available_memory();

    let usage_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    Ok(MemoryInfo {
        total,
        used,
        free,
        available,
        usage_percent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_query() {
        let info = query_memory(&FieldList::All).unwrap();
        assert!(info.total > 0);
    }
}
