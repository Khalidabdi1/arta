//! LIFE monitoring module for Arta
//!
//! Provides continuous monitoring of system resources with reactive updates.

use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{ArtaError, Result};
use crate::parser::{LifeTarget, Command};
use crate::engine::{execute_command_with_context, ExecutionContext, ResultData};
use crate::engine::queries::*;
use crate::context::Context;
use crate::output::{format_output, OutputFormat};

/// State for tracking changes in monitored resources
#[derive(Debug, Clone)]
pub enum MonitorState {
    Battery { percentage: f32, charging: bool },
    Memory { used: u64, total: u64 },
    Cpu { usage: f32 },
    Disk { used: u64, total: u64 },
    Network { bytes_sent: u64, bytes_recv: u64 },
    Processes { count: usize },
}

impl MonitorState {
    /// Check if state has changed significantly from another state
    pub fn has_changed(&self, other: &MonitorState) -> bool {
        match (self, other) {
            (MonitorState::Battery { percentage: p1, charging: c1 }, 
             MonitorState::Battery { percentage: p2, charging: c2 }) => {
                c1 != c2 || (p1 - p2).abs() >= 1.0
            }
            (MonitorState::Memory { used: u1, .. }, MonitorState::Memory { used: u2, .. }) => {
                // Consider changed if difference > 1%
                let diff = if *u1 > *u2 { u1 - u2 } else { u2 - u1 };
                diff > (*u1 / 100)
            }
            (MonitorState::Cpu { usage: u1 }, MonitorState::Cpu { usage: u2 }) => {
                (u1 - u2).abs() >= 1.0
            }
            (MonitorState::Disk { used: u1, .. }, MonitorState::Disk { used: u2, .. }) => {
                let diff = if *u1 > *u2 { u1 - u2 } else { u2 - u1 };
                diff > (*u1 / 100)
            }
            (MonitorState::Network { bytes_sent: s1, bytes_recv: r1 }, 
             MonitorState::Network { bytes_sent: s2, bytes_recv: r2 }) => {
                s1 != s2 || r1 != r2
            }
            (MonitorState::Processes { count: c1 }, MonitorState::Processes { count: c2 }) => {
                c1 != c2
            }
            _ => true, // Different types always considered changed
        }
    }
}

/// Live monitor that continuously watches system resources
pub struct LiveMonitor {
    target: LifeTarget,
    interval: Duration,
    running: Arc<AtomicBool>,
}

impl LiveMonitor {
    /// Create a new live monitor
    pub fn new(target: LifeTarget, interval_secs: u64, _exec_ctx: ExecutionContext) -> Self {
        Self {
            target,
            interval: Duration::from_secs(interval_secs),
            running: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Start monitoring with a callback for each update
    pub fn start<F>(&self, mut on_update: F) -> Result<()>
    where
        F: FnMut(&MonitorState) -> Result<()>,
    {
        self.running.store(true, Ordering::SeqCst);
        
        let mut last_state: Option<MonitorState> = None;
        
        while self.running.load(Ordering::SeqCst) {
            let current_state = self.get_current_state()?;
            
            // Only trigger callback if state has changed
            let should_update = match &last_state {
                None => true,
                Some(prev) => current_state.has_changed(prev),
            };
            
            if should_update {
                on_update(&current_state)?;
                last_state = Some(current_state);
            }
            
            std::thread::sleep(self.interval);
        }
        
        Ok(())
    }
    
    /// Stop the monitor
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
    
    /// Get the current state of the monitored resource
    fn get_current_state(&self) -> Result<MonitorState> {
        match self.target {
            LifeTarget::Battery => {
                let info = query_battery(&crate::parser::FieldList::All)?;
                if let Some(battery) = info.batteries.first() {
                    Ok(MonitorState::Battery {
                        percentage: battery.percentage,
                        charging: battery.state.to_lowercase().contains("charging"),
                    })
                } else {
                    Ok(MonitorState::Battery { percentage: 100.0, charging: false })
                }
            }
            LifeTarget::Memory => {
                let info = query_memory(&crate::parser::FieldList::All)?;
                Ok(MonitorState::Memory {
                    used: info.used,
                    total: info.total,
                })
            }
            LifeTarget::Cpu => {
                let info = query_cpu(&crate::parser::FieldList::All)?;
                Ok(MonitorState::Cpu { usage: info.usage })
            }
            LifeTarget::Disk => {
                let info = query_disk(&crate::parser::FieldList::All, None)?;
                let (used, total) = info.disks.first()
                    .map(|d| (d.used, d.total))
                    .unwrap_or((0, 0));
                Ok(MonitorState::Disk { used, total })
            }
            LifeTarget::Network => {
                let info = query_network(&crate::parser::FieldList::All)?;
                let (sent, recv) = info.interfaces.iter()
                    .fold((0, 0), |(s, r), iface| (s + iface.transmitted, r + iface.received));
                Ok(MonitorState::Network { bytes_sent: sent, bytes_recv: recv })
            }
            LifeTarget::Processes => {
                let procs = query_processes(&crate::parser::FieldList::All, None)?;
                Ok(MonitorState::Processes { count: procs.len() })
            }
        }
    }
    
    /// Check if the monitor is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Run a LIFE monitoring block from a script
pub fn run_life_block(
    target: LifeTarget,
    body: &[Command],
    exec_ctx: &ExecutionContext,
    context: &mut Context,
    interval_secs: u64,
) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    // Set up Ctrl+C handler
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).map_err(|e| ArtaError::ExecutionError(format!("Failed to set Ctrl+C handler: {}", e)))?;
    
    let interval = Duration::from_secs(interval_secs);
    let mut last_state: Option<MonitorState> = None;
    
    println!("Starting LIFE monitor for {}... (Press Ctrl+C to stop)", target);
    
    let monitor = LiveMonitor::new(target, interval_secs, exec_ctx.clone());
    
    while running.load(Ordering::SeqCst) {
        let current_state = monitor.get_current_state()?;
        
        // Only execute body if state has changed
        let should_execute = match &last_state {
            None => true,
            Some(prev) => current_state.has_changed(prev),
        };
        
        if should_execute {
            // Execute each command in the body
            for cmd in body {
                let result = execute_command_with_context(cmd, exec_ctx, context)?;
                
                // Print output for non-empty results
                match &result.data {
                    ResultData::Empty => {}
                    _ => {
                        println!("{}", format_output(&result, &exec_ctx.output_format));
                    }
                }
            }
            
            last_state = Some(current_state);
        }
        
        std::thread::sleep(interval);
    }
    
    println!("\nLIFE monitor stopped.");
    Ok(())
}

/// Simple CLI monitoring command (arta life battery)
pub fn run_simple_monitor(
    target_str: &str,
    interval_secs: u64,
    output_format: &OutputFormat,
) -> Result<()> {
    let target = match target_str.to_lowercase().as_str() {
        "battery" => LifeTarget::Battery,
        "memory" => LifeTarget::Memory,
        "cpu" => LifeTarget::Cpu,
        "disk" => LifeTarget::Disk,
        "network" => LifeTarget::Network,
        "processes" => LifeTarget::Processes,
        _ => return Err(ArtaError::InvalidTarget(target_str.to_string())),
    };
    
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    // Set up Ctrl+C handler
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).map_err(|e| ArtaError::ExecutionError(format!("Failed to set Ctrl+C handler: {}", e)))?;
    
    let interval = Duration::from_secs(interval_secs);
    
    println!("Monitoring {}... (Press Ctrl+C to stop)\n", target);
    
    let exec_ctx = ExecutionContext::default();
    let monitor = LiveMonitor::new(target, interval_secs, exec_ctx);
    let mut last_state: Option<MonitorState> = None;
    
    while running.load(Ordering::SeqCst) {
        let current_state = monitor.get_current_state()?;
        
        // Print state on change
        let should_print = match &last_state {
            None => true,
            Some(prev) => current_state.has_changed(prev),
        };
        
        if should_print {
            print_state(&current_state, output_format);
            last_state = Some(current_state);
        }
        
        std::thread::sleep(interval);
    }
    
    println!("\nMonitoring stopped.");
    Ok(())
}

fn print_state(state: &MonitorState, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            let json = match state {
                MonitorState::Battery { percentage, charging } => {
                    serde_json::json!({
                        "type": "battery",
                        "percentage": percentage,
                        "charging": charging,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })
                }
                MonitorState::Memory { used, total } => {
                    serde_json::json!({
                        "type": "memory",
                        "used": used,
                        "total": total,
                        "used_percent": (*used as f64 / *total as f64) * 100.0,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })
                }
                MonitorState::Cpu { usage } => {
                    serde_json::json!({
                        "type": "cpu",
                        "usage": usage,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })
                }
                MonitorState::Disk { used, total } => {
                    serde_json::json!({
                        "type": "disk",
                        "used": used,
                        "total": total,
                        "used_percent": (*used as f64 / *total as f64) * 100.0,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })
                }
                MonitorState::Network { bytes_sent, bytes_recv } => {
                    serde_json::json!({
                        "type": "network",
                        "bytes_sent": bytes_sent,
                        "bytes_recv": bytes_recv,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })
                }
                MonitorState::Processes { count } => {
                    serde_json::json!({
                        "type": "processes",
                        "count": count,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })
                }
            };
            println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
        }
        OutputFormat::Human => {
            let time = chrono::Local::now().format("%H:%M:%S");
            match state {
                MonitorState::Battery { percentage, charging } => {
                    let status = if *charging { "Charging" } else { "Discharging" };
                    println!("[{}] Battery: {:.0}% ({})", time, percentage, status);
                }
                MonitorState::Memory { used, total } => {
                    let used_gb = *used as f64 / (1024.0 * 1024.0 * 1024.0);
                    let total_gb = *total as f64 / (1024.0 * 1024.0 * 1024.0);
                    let percent = (*used as f64 / *total as f64) * 100.0;
                    println!("[{}] Memory: {:.1} GB / {:.1} GB ({:.1}%)", time, used_gb, total_gb, percent);
                }
                MonitorState::Cpu { usage } => {
                    println!("[{}] CPU: {:.1}%", time, usage);
                }
                MonitorState::Disk { used, total } => {
                    let used_gb = *used as f64 / (1024.0 * 1024.0 * 1024.0);
                    let total_gb = *total as f64 / (1024.0 * 1024.0 * 1024.0);
                    let percent = (*used as f64 / *total as f64) * 100.0;
                    println!("[{}] Disk: {:.1} GB / {:.1} GB ({:.1}%)", time, used_gb, total_gb, percent);
                }
                MonitorState::Network { bytes_sent, bytes_recv } => {
                    let sent_mb = *bytes_sent as f64 / (1024.0 * 1024.0);
                    let recv_mb = *bytes_recv as f64 / (1024.0 * 1024.0);
                    println!("[{}] Network: Sent {:.1} MB, Recv {:.1} MB", time, sent_mb, recv_mb);
                }
                MonitorState::Processes { count } => {
                    println!("[{}] Processes: {}", time, count);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_monitor_state_battery_change() {
        let s1 = MonitorState::Battery { percentage: 80.0, charging: false };
        let s2 = MonitorState::Battery { percentage: 80.5, charging: false };
        let s3 = MonitorState::Battery { percentage: 82.0, charging: false };
        let s4 = MonitorState::Battery { percentage: 80.0, charging: true };
        
        assert!(!s1.has_changed(&s2)); // Less than 1% difference
        assert!(s1.has_changed(&s3));  // 2% difference
        assert!(s1.has_changed(&s4));  // Charging state changed
    }
    
    #[test]
    fn test_monitor_state_cpu_change() {
        let s1 = MonitorState::Cpu { usage: 50.0 };
        let s2 = MonitorState::Cpu { usage: 50.5 };
        let s3 = MonitorState::Cpu { usage: 52.0 };
        
        assert!(!s1.has_changed(&s2)); // Less than 1% difference
        assert!(s1.has_changed(&s3));  // 2% difference
    }
}
