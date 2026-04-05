//! CPU snapshot shared by the CLI and Tauri (same logic as the original `main.rs` paths).

use serde::Serialize;
use sysinfo::{CpuRefreshKind, RefreshKind, System};

use crate::lscpu;

/// Hardware max frequency in MHz (e.g. cpufreq `cpuinfo_max_freq`), if known.
pub fn advertised_max_cpu_mhz() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        linux_max_cpu_freq_mhz()
    }
    #[cfg(target_os = "macos")]
    {
        macos_max_cpu_freq_mhz()
    }
    #[cfg(target_os = "windows")]
    {
        windows_max_cpu_freq_mhz()
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn linux_max_cpu_freq_mhz() -> Option<u64> {
    use std::fs;
    use std::path::Path;

    let mut max_khz = 0u64;
    let entries = fs::read_dir("/sys/devices/system/cpu").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(rest) = name.strip_prefix("cpu") else {
            continue;
        };
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let path = entry.path().join("cpufreq/cpuinfo_max_freq");
        if !Path::new(&path).exists() {
            continue;
        }
        let s = fs::read_to_string(&path).ok()?;
        let khz = s.trim().parse::<u64>().ok()?;
        max_khz = max_khz.max(khz);
    }
    (max_khz > 0).then_some(max_khz / 1000)
}

#[cfg(target_os = "macos")]
fn macos_max_cpu_freq_mhz() -> Option<u64> {
    use std::process::Command;

    let out = Command::new("sysctl").args(["-n", "hw.cpufrequency_max"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let hz: u64 = s.trim().parse().ok()?;
    (hz > 0).then_some(hz / 1_000_000)
}

#[cfg(target_os = "windows")]
fn windows_max_cpu_freq_mhz() -> Option<u64> {
    use std::process::Command;

    let out = Command::new("wmic")
        .args(["cpu", "get", "MaxClockSpeed", "/value"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("MaxClockSpeed=") {
            return rest.trim().parse().ok();
        }
    }
    None
}

/// Raw `lscpu` stdout on Linux; `None` elsewhere or on failure.
pub fn try_lscpu_output() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        try_lscpu_output_linux()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn try_lscpu_output_linux() -> Option<String> {
    use std::process::Command;
    let out = Command::new("lscpu").output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// Serializable CPU view: either `lscpu`-backed (Linux) or sysinfo fallback.
#[derive(Debug, Clone, Serialize)]
pub struct CpuSnapshot {
    /// When set (Linux + successful parse), primary fields match the CLI `lscpu` path.
    pub lscpu: Option<lscpu::LscpuInfo>,
    pub current_mhz: u64,
    pub max_advertised_mhz: Option<u64>,
    /// Sysinfo fallback when `lscpu` is not used.
    pub vendor: Option<String>,
    pub cpu_model: Option<String>,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
}

pub fn gather_cpu_info() -> CpuSnapshot {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    );
    sys.refresh_cpu_all();

    let max_advertised = advertised_max_cpu_mhz();
    let logical = sys.cpus().len();
    let physical = sys.physical_core_count();

    #[cfg(target_os = "linux")]
    {
        if let Some(raw) = try_lscpu_output() {
            if let Some(info) = lscpu::parse_lscpu(&raw) {
                let current_mhz = sys
                    .cpus()
                    .first()
                    .map(|c| c.frequency())
                    .unwrap_or(0);
                return CpuSnapshot {
                    lscpu: Some(info),
                    current_mhz,
                    max_advertised_mhz: max_advertised,
                    vendor: None,
                    cpu_model: None,
                    physical_cores: physical,
                    logical_cores: logical,
                };
            }
        }
    }

    if let Some(cpu) = sys.cpus().first() {
        CpuSnapshot {
            lscpu: None,
            current_mhz: cpu.frequency(),
            max_advertised_mhz: max_advertised,
            vendor: Some(cpu.vendor_id().to_string()),
            cpu_model: Some(cpu.brand().trim().to_string()),
            physical_cores: physical,
            logical_cores: logical,
        }
    } else {
        CpuSnapshot {
            lscpu: None,
            current_mhz: 0,
            max_advertised_mhz: max_advertised,
            vendor: None,
            cpu_model: None,
            physical_cores: physical,
            logical_cores: logical,
        }
    }
}
