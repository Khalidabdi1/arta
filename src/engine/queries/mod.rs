//! Query implementations

pub mod cpu;
pub mod memory;
pub mod disk;
pub mod network;
pub mod system;
pub mod battery;
pub mod process;

pub use cpu::{query_cpu, CpuInfo};
pub use memory::{query_memory, MemoryInfo};
pub use disk::{query_disk, DiskInfo};
pub use network::{query_network, NetworkInfo};
pub use system::{query_system, SystemInfo};
pub use battery::{query_battery, BatteryInfo};
pub use process::{query_processes, ProcessInfo};
