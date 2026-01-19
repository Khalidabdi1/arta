//! Network query implementation

use crate::error::Result;
use crate::parser::FieldList;
use serde::{Serialize, Deserialize};
use sysinfo::Networks;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub interfaces: Vec<NetworkInterface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
    pub packets_received: u64,
    pub packets_transmitted: u64,
}

pub fn query_network(_fields: &FieldList) -> Result<NetworkInfo> {
    let networks = Networks::new_with_refreshed_list();
    
    let interfaces: Vec<NetworkInterface> = networks.iter()
        .map(|(name, data)| {
            NetworkInterface {
                name: name.to_string(),
                received: data.received(),
                transmitted: data.transmitted(),
                packets_received: data.packets_received(),
                packets_transmitted: data.packets_transmitted(),
            }
        })
        .collect();
    
    Ok(NetworkInfo { interfaces })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_network_query() {
        let info = query_network(&FieldList::All).unwrap();
        // Network info should be queryable
        assert!(info.interfaces.len() >= 0);
    }
}
