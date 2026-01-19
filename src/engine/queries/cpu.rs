//! CPU query implementation

use crate::error::Result;
use crate::parser::FieldList;
use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub cores: usize,
    pub usage: f32,
    pub brand: String,
    pub frequency: u64,
}

pub fn query_cpu(_fields: &FieldList) -> Result<CpuInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Give CPU time to collect usage data
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_all();

    let cpus = sys.cpus();
    let usage: f32 = if !cpus.is_empty() {
        cpus.iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / cpus.len() as f32
    } else {
        0.0
    };

    let brand = cpus
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let frequency = cpus.first().map(|cpu| cpu.frequency()).unwrap_or(0);

    Ok(CpuInfo {
        cores: cpus.len(),
        usage,
        brand,
        frequency,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_query() {
        let info = query_cpu(&FieldList::All).unwrap();
        assert!(info.cores > 0);
    }
}
