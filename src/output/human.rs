//! Human-readable output formatting

use crate::engine::executor::{ExecutionResult, ResultData};
use bytesize::ByteSize;

pub fn format_human(result: &ExecutionResult) -> String {
    match &result.data {
        ResultData::Cpu(info) => {
            format!(
                "CPU Information\n\
                 ---------------\n\
                 Cores:     {}\n\
                 Usage:     {:.1}%\n\
                 Brand:     {}\n\
                 Frequency: {} MHz",
                info.cores, info.usage, info.brand, info.frequency
            )
        }
        ResultData::Memory(info) => {
            format!(
                "Memory Information\n\
                 ------------------\n\
                 Total:     {}\n\
                 Used:      {}\n\
                 Free:      {}\n\
                 Available: {}\n\
                 Usage:     {:.1}%",
                ByteSize(info.total),
                ByteSize(info.used),
                ByteSize(info.free),
                ByteSize(info.available),
                info.usage_percent
            )
        }
        ResultData::Disk(info) => {
            let mut output = String::from("Disk Information\n----------------\n");
            for disk in &info.disks {
                output.push_str(&format!(
                    "\n{} ({})\n  Total: {} | Used: {} | Free: {} | Usage: {:.1}%\n",
                    disk.mount_point,
                    disk.file_system,
                    ByteSize(disk.total),
                    ByteSize(disk.used),
                    ByteSize(disk.free),
                    disk.usage_percent
                ));
            }
            output
        }
        ResultData::Network(info) => {
            let mut output = String::from("Network Interfaces\n------------------\n");
            for iface in &info.interfaces {
                output.push_str(&format!(
                    "\n{}\n  Received: {} | Transmitted: {}\n",
                    iface.name,
                    ByteSize(iface.received),
                    ByteSize(iface.transmitted)
                ));
            }
            output
        }
        ResultData::System(info) => {
            let uptime_hours = info.uptime / 3600;
            let uptime_mins = (info.uptime % 3600) / 60;
            format!(
                "System Information\n\
                 ------------------\n\
                 Hostname:       {}\n\
                 OS:             {} {}\n\
                 Kernel:         {}\n\
                 Uptime:         {}h {}m",
                info.hostname,
                info.os_name,
                info.os_version,
                info.kernel_version,
                uptime_hours,
                uptime_mins
            )
        }
        ResultData::Battery(info) => {
            if info.batteries.is_empty() {
                return "No batteries found".to_string();
            }
            let mut output = String::from("Battery Information\n-------------------\n");
            for (i, battery) in info.batteries.iter().enumerate() {
                output.push_str(&format!(
                    "\nBattery {}\n  State: {} | Charge: {:.1}%",
                    i + 1,
                    battery.state,
                    battery.percentage
                ));
                if let Some(ref time) = battery.time_to_empty {
                    output.push_str(&format!(" | Time to empty: {}", time));
                }
                if let Some(ref time) = battery.time_to_full {
                    output.push_str(&format!(" | Time to full: {}", time));
                }
                output.push('\n');
            }
            output
        }
        ResultData::Processes(processes) => {
            if processes.is_empty() {
                return "No matching processes found".to_string();
            }
            let mut output = String::from("Processes\n---------\n");
            output.push_str(&format!(
                "{:<8} {:<20} {:>8} {:>12}\n",
                "PID", "NAME", "CPU%", "MEMORY"
            ));
            output.push_str(&"-".repeat(52));
            output.push('\n');
            for proc in processes.iter().take(20) {
                output.push_str(&format!(
                    "{:<8} {:<20} {:>7.1}% {:>12}\n",
                    proc.pid,
                    truncate(&proc.name, 20),
                    proc.cpu,
                    ByteSize(proc.memory)
                ));
            }
            if processes.len() > 20 {
                output.push_str(&format!(
                    "\n... and {} more processes\n",
                    processes.len() - 20
                ));
            }
            output
        }
        ResultData::Files(files) => {
            if files.is_empty() {
                return "No files found".to_string();
            }
            let mut output = String::from("Files\n-----\n");
            output.push_str(&format!(
                "{:<30} {:>12} {:<20}\n",
                "NAME", "SIZE", "MODIFIED"
            ));
            output.push_str(&"-".repeat(64));
            output.push('\n');
            for file in files.iter().take(50) {
                let name = if file.is_dir {
                    format!("{}/", file.name)
                } else {
                    file.name.clone()
                };
                output.push_str(&format!(
                    "{:<30} {:>12} {:<20}\n",
                    truncate(&name, 30),
                    if file.is_dir {
                        "-".to_string()
                    } else {
                        ByteSize(file.size).to_string()
                    },
                    file.modified.as_deref().unwrap_or("-")
                ));
            }
            if files.len() > 50 {
                output.push_str(&format!("\n... and {} more files\n", files.len() - 50));
            }
            output
        }
        ResultData::Content(content) => {
            let mut output = format!(
                "File: {}\nSize: {} | Lines: {}\n{}\n",
                content.file_path,
                ByteSize(content.file_size),
                content.total_lines,
                "-".repeat(60)
            );
            for line in &content.lines {
                output.push_str(line);
                output.push('\n');
            }
            if content.lines.len() < content.total_lines {
                output.push_str(&format!(
                    "\n... {} more lines\n",
                    content.total_lines - content.lines.len()
                ));
            }
            output
        }
        ResultData::ActionResult(action) => {
            let mut output = format!(
                "{} Result\n{}\n",
                action.action_type,
                "-".repeat(action.action_type.len() + 7)
            );
            if action.dry_run {
                output.push_str("[DRY RUN] No changes were made\n\n");
            }
            output.push_str(&format!("Affected: {} items\n\n", action.affected_count));
            for detail in &action.details {
                output.push_str(&format!("  {}\n", detail));
            }
            output
        }
        ResultData::ContextInfo(info) => {
            let mut output = String::new();

            if !info.current_folder.is_empty() {
                output.push_str("Current Context\n");
                output.push_str("---------------\n");
                output.push_str(&format!("Folder: {}\n", info.current_folder));
                if let Some(ref file) = info.current_file {
                    output.push_str(&format!("File:   {}\n", file));
                }
                output.push_str(&format!("Depth:  {}\n", info.folder_depth));
            }

            if !info.variables.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str("Variables\n");
                output.push_str("---------\n");
                for (name, value) in &info.variables {
                    output.push_str(&format!("  {} = {}\n", name, value));
                }
            }

            if !info.history.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str("History\n");
                output.push_str("-------\n");
                for entry in &info.history {
                    output.push_str(&format!("  {}\n", entry));
                }
            }

            if output.is_empty() {
                output = "No context information".to_string();
            }

            output
        }
        ResultData::Explanation(explanation) => explanation.clone(),
        ResultData::Message(msg) => msg.clone(),
        ResultData::Multiple(results) => {
            let mut output = String::new();
            for (i, res) in results.iter().enumerate() {
                if i > 0 {
                    output.push_str("\n---\n\n");
                }
                output.push_str(&format_human(res));
            }
            output
        }
        ResultData::Empty => "".to_string(),
        ResultData::ContainerResult(info) => {
            let mut output = format!("Container: {}\n", info.operation);
            output.push_str(&"-".repeat(info.operation.len() + 11));
            output.push('\n');

            if let Some(ref name) = info.container_name {
                output.push_str(&format!("Name: {}\n", name));
            }

            output.push_str(&format!("{}\n", info.message));

            if let Some(ref containers) = info.containers {
                output.push('\n');
                output.push_str(&format!(
                    "{:<20} {:>12} {:>12} {:>8}\n",
                    "NAME", "ALLOW_ACTIONS", "READONLY", "ACTIVE"
                ));
                output.push_str(&"-".repeat(56));
                output.push('\n');
                for container in containers {
                    output.push_str(&format!(
                        "{:<20} {:>12} {:>12} {:>8}\n",
                        container.name,
                        if container.allow_actions { "yes" } else { "no" },
                        if container.readonly { "yes" } else { "no" },
                        if container.is_active { "*" } else { "" }
                    ));
                }
            }

            output
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
