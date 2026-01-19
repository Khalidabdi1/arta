//! Battery query implementation

use crate::error::Result;
use crate::parser::FieldList;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    pub batteries: Vec<BatteryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryEntry {
    pub state: String,
    pub percentage: f32,
    pub time_to_empty: Option<String>,
    pub time_to_full: Option<String>,
}

pub fn query_battery(_fields: &FieldList) -> Result<BatteryInfo> {
    let manager = battery::Manager::new()
        .map_err(|e| crate::error::ArtaError::ExecutionError(e.to_string()))?;

    let batteries: Vec<BatteryEntry> = manager
        .batteries()
        .map_err(|e| crate::error::ArtaError::ExecutionError(e.to_string()))?
        .filter_map(|b| b.ok())
        .map(|battery| {
            use battery::State;

            let state = match battery.state() {
                State::Charging => "Charging",
                State::Discharging => "Discharging",
                State::Full => "Full",
                State::Empty => "Empty",
                _ => "Unknown",
            }
            .to_string();

            let percentage = battery.state_of_charge().value * 100.0;

            let time_to_empty = battery
                .time_to_empty()
                .map(|t| format_duration(t.value as u64));

            let time_to_full = battery
                .time_to_full()
                .map(|t| format_duration(t.value as u64));

            BatteryEntry {
                state,
                percentage,
                time_to_empty,
                time_to_full,
            }
        })
        .collect();

    Ok(BatteryInfo { batteries })
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_battery_query() {
        // Battery query should not fail even without batteries
        let result = query_battery(&FieldList::All);
        assert!(result.is_ok());
    }
}
