//! Query implementations

pub mod battery;
pub mod cpu;
pub mod disk;
pub mod memory;
pub mod network;
pub mod process;
pub mod system;

pub use battery::{query_battery, BatteryInfo};
pub use cpu::{query_cpu, CpuInfo};
pub use disk::{query_disk, DiskInfo};
pub use memory::{query_memory, MemoryInfo};
pub use network::{query_network, NetworkInfo};
pub use process::{query_processes, ProcessInfo};
pub use system::{query_system, SystemInfo};
