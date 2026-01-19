//! System query implementation

use crate::error::Result;
use crate::parser::FieldList;
use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime: u64,
}

pub fn query_system(_fields: &FieldList) -> Result<SystemInfo> {
    Ok(SystemInfo {
        hostname: System::host_name().unwrap_or_else(|| "Unknown".to_string()),
        os_name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
        uptime: System::uptime(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_query() {
        let info = query_system(&FieldList::All).unwrap();
        assert!(!info.hostname.is_empty() || info.hostname == "Unknown");
    }
}
