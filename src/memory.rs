//! Host memory usage and top RAM consumers (aligned with hudsse Memory page).

use std::ffi::OsStr;

use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

/// One process in the “top RAM” list (like hudsse `TopProcess`).
#[derive(Debug, Clone)]
pub struct TopProcess {
    pub pid: String,
    pub name: String,
    pub memory_usage: String,
    pub memory_percent: f64,
    pub memory_absolute: String,
}

#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_ram: String,
    pub free_ram: String,
    pub used_ram: String,
    /// 0–100, rounded (same idea as the Memory page bar).
    pub used_percent: u32,
    pub top_processes: Vec<TopProcess>,
}

/// Same style as hudsse `formatBytes` / `gpu` module.
fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let k = 1024u64;
    let sizes = ["B", "KB", "MB", "GB", "TB"];
    let i = (bytes as f64).log(k as f64).floor() as usize;
    let i = i.min(sizes.len() - 1);
    let value = bytes as f64 / (k.pow(i as u32) as f64);
    format!("{:.2} {}", value, sizes[i])
}

fn truncate_name(s: &str) -> String {
    let count = s.chars().count();
    if count > 30 {
        s.chars().take(30).collect::<String>() + "..."
    } else {
        s.to_string()
    }
}

fn process_label(name: &OsStr, cmd: &[std::ffi::OsString]) -> String {
    let s = name.to_string_lossy();
    if !s.is_empty() {
        return truncate_name(s.trim());
    }
    if let Some(first) = cmd.first() {
        return truncate_name(&first.to_string_lossy());
    }
    "?".to_string()
}

/// Snapshot RAM totals (Node `os.totalmem` / `os.freemem` style) and top 3 processes by RSS.
pub fn gather_memory_info() -> MemoryInfo {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::everything())
            .with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let total = sys.total_memory();
    let free = sys.free_memory();
    let used = total.saturating_sub(free);

    let used_percent = if total > 0 {
        ((used as f64 / total as f64) * 100.0).round() as u32
    } else {
        0
    };

    let mut procs: Vec<_> = sys.processes().values().collect();
    procs.sort_by(|a, b| b.memory().cmp(&a.memory()));

    let top_processes: Vec<TopProcess> = procs
        .into_iter()
        .take(3)
        .map(|p| {
            let mem = p.memory();
            let pct = if total > 0 {
                (mem as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            TopProcess {
                pid: p.pid().to_string(),
                name: process_label(p.name(), p.cmd()),
                memory_usage: format!("{:.1}%", pct),
                memory_percent: (pct * 100.0).round() / 100.0,
                memory_absolute: format_bytes(mem),
            }
        })
        .collect();

    MemoryInfo {
        total_ram: format_bytes(total),
        free_ram: format_bytes(free),
        used_ram: format_bytes(used),
        used_percent,
        top_processes,
    }
}
