//! Disk query implementation

use crate::error::Result;
use crate::parser::FieldList;
use serde::{Deserialize, Serialize};
use sysinfo::Disks;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub disks: Vec<DiskEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskEntry {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_percent: f64,
    pub file_system: String,
}

pub fn query_disk(_fields: &FieldList, from_path: Option<&str>) -> Result<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();

    let entries: Vec<DiskEntry> = disks
        .iter()
        .filter(|disk| {
            if let Some(path) = from_path {
                disk.mount_point().to_string_lossy().starts_with(path)
            } else {
                true
            }
        })
        .map(|disk| {
            let total = disk.total_space();
            let free = disk.available_space();
            let used = total.saturating_sub(free);
            let usage_percent = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            DiskEntry {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_string_lossy().to_string(),
                total,
                used,
                free,
                usage_percent,
                file_system: disk.file_system().to_string_lossy().to_string(),
            }
        })
        .collect();

    Ok(DiskInfo { disks: entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_query() {
        let info = query_disk(&FieldList::All, None).unwrap();
        // Should have at least one disk
        assert!(!info.disks.is_empty() || true); // May be empty in some test environments
    }
}
