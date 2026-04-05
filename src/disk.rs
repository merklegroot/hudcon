//! Disk usage and physical drives (aligned with hudsse Disk page: `df`/`lsblk` / `wmic`).

use std::process::Command;

use sysinfo::Disks;

/// One mounted filesystem (hudsse `DiskInfo`).
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub mount: String,
    pub total: String,
    pub used: String,
    pub available: String,
    pub used_percent: u32,
    pub filesystem: String,
}

/// One physical drive (hudsse `PhysicalDisk`).
#[derive(Debug, Clone)]
pub struct PhysicalDisk {
    pub device: String,
    pub size: String,
    pub model: String,
    pub disk_type: String,
}

#[derive(Debug, Clone)]
pub struct DiskGatherResult {
    pub disks: Vec<DiskInfo>,
    pub physical_disks: Vec<PhysicalDisk>,
}

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

fn mount_included(mount: &str) -> bool {
    if mount == "none" {
        return false;
    }
    #[cfg(windows)]
    {
        return mount.contains(':') || mount.starts_with("\\\\");
    }
    #[cfg(not(windows))]
    {
        mount.starts_with('/') || mount == "/"
    }
}

/// Mount points and usage via sysinfo (cross-platform; skips tmpfs-style FS like hudsse’s `df` filter).
pub fn gather_mounts() -> Vec<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut v = Vec::new();
    for d in disks.list() {
        let total = d.total_space();
        if total == 0 {
            continue;
        }
        let fs = d.file_system().to_string_lossy().to_string();
        let fs_lower = fs.to_lowercase();
        if fs_lower.contains("tmpfs") || fs_lower.contains("devtmpfs") {
            continue;
        }
        let mount = d.mount_point().to_string_lossy().to_string();
        if !mount_included(&mount) {
            continue;
        }
        let avail = d.available_space();
        let used = total.saturating_sub(avail);
        let used_percent = if total > 0 {
            ((used as f64 / total as f64) * 100.0).round() as u32
        } else {
            0
        };
        v.push(DiskInfo {
            mount,
            total: format_bytes(total),
            used: format_bytes(used),
            available: format_bytes(avail),
            used_percent,
            filesystem: if fs.is_empty() {
                "Unknown".to_string()
            } else {
                fs
            },
        });
    }
    v.sort_by(|a, b| a.mount.cmp(&b.mount));
    v
}

/// `lsblk -d -o NAME,SIZE,MODEL,ROTA` (hudsse `parsePhysicalDisks` for Linux).
pub fn parse_lsblk_physical(output: &str) -> Vec<PhysicalDisk> {
    let mut out = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let device = parts[0];
        let size = parts[1].to_string();
        let rota = parts[parts.len() - 1];
        let model = if parts.len() > 3 {
            parts[2..parts.len() - 1].join(" ")
        } else {
            String::new()
        };
        let model = if model.trim().is_empty() {
            "Unknown".to_string()
        } else {
            model.trim().to_string()
        };
        if device.contains("loop") || device.contains("ram") {
            continue;
        }
        let disk_type = if rota == "1" {
            "HDD"
        } else {
            "SSD"
        };
        out.push(PhysicalDisk {
            device: format!("/dev/{device}"),
            size,
            model,
            disk_type: disk_type.to_string(),
        });
    }
    out
}

/// `wmic diskdrive get size,model,caption /format:csv` (hudsse Windows path).
pub fn parse_wmic_diskdrive(output: &str) -> Vec<PhysicalDisk> {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() <= 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for line in lines.iter().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 4 {
            continue;
        }
        let caption = parts[1].trim();
        let model = parts[2].trim();
        let size_bytes: u64 = parts[3].trim().parse().unwrap_or(0);
        if caption.is_empty() {
            continue;
        }
        out.push(PhysicalDisk {
            device: caption.to_string(),
            size: format_bytes(size_bytes),
            model: if model.is_empty() {
                "Unknown".to_string()
            } else {
                model.to_string()
            },
            disk_type: "Unknown".to_string(),
        });
    }
    out
}

#[cfg(target_os = "linux")]
fn physical_disks_platform() -> Vec<PhysicalDisk> {
    let Ok(out) = Command::new("sh")
        .arg("-c")
        .arg("lsblk -d -o NAME,SIZE,MODEL,ROTA -n 2>/dev/null || true")
        .output()
    else {
        return Vec::new();
    };
    parse_lsblk_physical(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(target_os = "windows")]
fn physical_disks_platform() -> Vec<PhysicalDisk> {
    let Ok(out) = Command::new("wmic")
        .args(["diskdrive", "get", "size,model,caption", "/format:csv"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_wmic_diskdrive(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn physical_disks_platform() -> Vec<PhysicalDisk> {
    Vec::new()
}

pub fn gather_disk_info() -> DiskGatherResult {
    DiskGatherResult {
        disks: gather_mounts(),
        physical_disks: physical_disks_platform(),
    }
}
