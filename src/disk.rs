//! Disk usage and physical drives (aligned with hudsse Disk page: `df`/`lsblk` / `wmic` / macOS `diskutil`).

use std::process::Command;

use serde::Serialize;
use sysinfo::Disks;

/// One mounted filesystem (hudsse `DiskInfo`).
#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub mount: String,
    pub total: String,
    pub used: String,
    pub available: String,
    pub used_percent: u32,
    pub filesystem: String,
}

/// One physical drive (hudsse `PhysicalDisk`).
#[derive(Debug, Clone, Serialize)]
pub struct PhysicalDisk {
    pub device: String,
    pub size: String,
    pub model: String,
    pub disk_type: String,
}

#[derive(Debug, Clone, Serialize)]
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

/// `diskutil list` — physical disks are lines like `/dev/disk0 (internal, physical):`.
fn parse_diskutil_list_physical(output: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = output.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.starts_with("/dev/disk") || !trimmed.contains("physical") {
            continue;
        }
        let dev_path = trimmed.split_whitespace().next().unwrap_or("");
        if !dev_path.starts_with("/dev/disk") {
            continue;
        }
        let disk_id = dev_path.trim_start_matches("/dev/");
        let mut size_str: Option<String> = None;
        while let Some(&next) = lines.peek() {
            let t = next.trim();
            if t.starts_with("/dev/disk") {
                break;
            }
            lines.next();
            if let Some((id, sz)) = parse_diskutil_partition_line(t) {
                if id == disk_id {
                    size_str = Some(sz);
                    break;
                }
            }
        }
        out.push((
            dev_path.to_string(),
            size_str.unwrap_or_else(|| "Unknown".to_string()),
        ));
    }
    out
}

/// Parses a `diskutil list` partition row; returns (IDENTIFIER, SIZE) when the line ends with
/// `… <num> <unit> diskN` (e.g. GUID_partition_scheme … 500.3 GB disk0).
fn parse_diskutil_partition_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let first = line.chars().next()?;
    if !first.is_ascii_digit() {
        return None;
    }
    let mut parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    let id = parts.pop()?.to_string();
    if !id.starts_with("disk") {
        return None;
    }
    let unit = parts.pop()?;
    if !matches!(unit, "B" | "KB" | "MB" | "GB" | "TB") {
        return None;
    }
    let num = parts.pop()?;
    Some((id, format!("{num} {unit}")))
}

#[cfg(target_os = "macos")]
fn diskutil_info_details(disk_id: &str) -> (String, String) {
    let Ok(out) = Command::new("diskutil").args(["info", disk_id]).output() else {
        return ("Unknown".to_string(), "Unknown".to_string());
    };
    if !out.status.success() {
        return ("Unknown".to_string(), "Unknown".to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut model = "Unknown".to_string();
    let mut disk_type = "Unknown".to_string();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Device / Media Name:") {
            let m = rest.trim();
            if !m.is_empty() {
                model = m.to_string();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("Solid State:") {
            let v = rest.trim().to_lowercase();
            disk_type = if v == "yes" {
                "SSD".to_string()
            } else if v == "no" {
                "HDD".to_string()
            } else {
                "Unknown".to_string()
            };
        }
    }
    (model, disk_type)
}

#[cfg(target_os = "macos")]
fn physical_disks_platform() -> Vec<PhysicalDisk> {
    let Ok(out) = Command::new("diskutil").arg("list").output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    parse_diskutil_list_physical(&text)
        .into_iter()
        .map(|(device, size)| {
            let disk_id = device
                .strip_prefix("/dev/")
                .unwrap_or(device.as_str())
                .to_string();
            let (model, disk_type) = diskutil_info_details(&disk_id);
            PhysicalDisk {
                device,
                size,
                model,
                disk_type,
            }
        })
        .collect()
}

#[cfg(all(
    not(target_os = "linux"),
    not(target_os = "windows"),
    not(target_os = "macos")
))]
fn physical_disks_platform() -> Vec<PhysicalDisk> {
    Vec::new()
}

pub fn gather_disk_info() -> DiskGatherResult {
    DiskGatherResult {
        disks: gather_mounts(),
        physical_disks: physical_disks_platform(),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_diskutil_list_physical, parse_diskutil_partition_line};

    #[test]
    fn diskutil_partition_line_parses_scheme_row() {
        let line = "0:      GUID_partition_scheme                        500.3 GB   disk0";
        let (id, sz) = parse_diskutil_partition_line(line).unwrap();
        assert_eq!(id, "disk0");
        assert_eq!(sz, "500.3 GB");
    }

    #[test]
    fn diskutil_list_finds_physical_disk_and_size() {
        let sample = r"/dev/disk0 (internal, physical):
   #:                       TYPE NAME                    SIZE       IDENTIFIER
   0:      GUID_partition_scheme                        500.3 GB   disk0
   1:                        EFI EFI                     314.2 MB   disk0s1

/dev/disk1 (synthesized):
   0:      APFS Volume VM                         20.5 KB   disk1s4
";
        let v = parse_diskutil_list_physical(sample);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "/dev/disk0");
        assert_eq!(v[0].1, "500.3 GB");
    }
}
