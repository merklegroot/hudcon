//! Machine-level info aligned with the hudsse app Machine page (hostname, IP, DMI, distro, kernel).

use std::env;
use std::fs;
use std::net::IpAddr;
use std::path::Path;

use serde::Serialize;
use sysinfo::{CpuRefreshKind, Networks, RefreshKind, System};

use crate::cpu;
use crate::lscpu;

/// Friendly OS name (matches hudsse `platformUtil.getFriendlyPlatformName`).
pub fn friendly_os_type() -> &'static str {
    match std::env::consts::OS {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        "freebsd" => "FreeBSD",
        "openbsd" => "OpenBSD",
        "netbsd" => "NetBSD",
        "dragonfly" => "DragonFly",
        "solaris" => "SunOS",
        "android" => "Android",
        "aix" => "AIX",
        _ => "Unknown",
    }
}

/// Cloud/hosting environment label (matches hudsse `virtualizationUtil.getVirtualizationFromEnv`).
pub fn virtualization_env_label() -> &'static str {
    if env::var("VERCEL").ok().as_deref() == Some("1") || env_var_nonempty("VERCEL_URL") {
        return "Vercel";
    }
    if env_var_nonempty("AWS_LAMBDA_FUNCTION_NAME") {
        return "AWS Lambda";
    }
    if env_var_nonempty("AZURE_FUNCTIONS_WORKER_RUNTIME") {
        return "Azure Functions";
    }
    if env_var_nonempty("GOOGLE_CLOUD_PROJECT") || env_var_nonempty("GCP_PROJECT") {
        return "Google Cloud Platform";
    }
    if env_var_nonempty("HEROKU_APP_NAME") {
        return "Heroku";
    }
    if env_var_nonempty("RAILWAY_ENVIRONMENT") {
        return "Railway";
    }
    if env_var_nonempty("NETLIFY") {
        return "Netlify";
    }
    if env_var_nonempty("RENDER") {
        return "Render";
    }
    if env_var_nonempty("FLY_APP_NAME") {
        return "Fly.io";
    }
    if env_var_nonempty("DIGITAL_OCEAN_APP_ID") {
        return "DigitalOcean";
    }
    if env_var_nonempty("LINODE_APP_ID") {
        return "Linode";
    }
    if env_var_nonempty("VULTR_APP_ID") {
        return "Vultr";
    }
    "Physical Machine"
}

fn env_var_nonempty(name: &str) -> bool {
    env::var(name).ok().filter(|s| !s.trim().is_empty()).is_some()
}

/// Non-loopback IPv4 addresses (space-separated), similar to `hostname -I` on Linux.
pub fn local_ip_addresses() -> String {
    let networks = Networks::new_with_refreshed_list();
    let mut addrs: Vec<String> = networks
        .iter()
        .flat_map(|(_iface, data)| data.ip_networks())
        .filter_map(|ipn| match ipn.addr {
            IpAddr::V4(v4) if !v4.is_loopback() => Some(v4.to_string()),
            _ => None,
        })
        .collect();
    addrs.sort();
    addrs.dedup();
    if addrs.is_empty() {
        "n/a".to_string()
    } else {
        addrs.join(" ")
    }
}

#[cfg(target_os = "linux")]
fn read_dmi_trimmed(rel: &str) -> Option<String> {
    let path = format!("/sys/class/dmi/id/{rel}");
    fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(not(target_os = "linux"))]
fn read_dmi_trimmed(_rel: &str) -> Option<String> {
    None
}

pub fn machine_model() -> String {
    #[cfg(target_os = "linux")]
    {
        read_dmi_trimmed("product_name").unwrap_or_else(|| "n/a".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        macos_sysctl_trimmed("hw.model").unwrap_or_else(|| "n/a".to_string())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        "n/a".to_string()
    }
}

pub fn motherboard_name() -> String {
    #[cfg(target_os = "linux")]
    {
        read_dmi_trimmed("board_name").unwrap_or_else(|| "n/a".to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        "n/a".to_string()
    }
}

#[cfg(target_os = "macos")]
fn macos_sysctl_trimmed(name: &str) -> Option<String> {
    use std::process::Command;
    let out = Command::new("sysctl").args(["-n", name]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Distro flavor: follows the main branches of hudsse `detect_distro_flavor.sh` where practical.
pub fn distro_flavor() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Some(f) = linux_distro_flavor_deterministic() {
            return f;
        }
        let base = pretty_name_from_os_release().unwrap_or_default();
        if base.is_empty() {
            return System::long_os_version().unwrap_or_else(|| "n/a".to_string());
        }
        let base_lower = base.to_lowercase();
        if base_lower.contains("ubuntu") {
            if let Ok(de) = env::var("XDG_CURRENT_DESKTOP") {
                let d = de.to_lowercase();
                if d.contains("kde") || d.contains("plasma") || d.contains("kwin") {
                    return "Kubuntu".to_string();
                }
                if d.contains("gnome") {
                    return "Ubuntu".to_string();
                }
                if d.contains("xfce") {
                    return "Xubuntu".to_string();
                }
                if d.contains("mate") {
                    return "Ubuntu MATE".to_string();
                }
                if d.contains("lxqt") {
                    return "Lubuntu".to_string();
                }
                if d.contains("budgie") {
                    return "Ubuntu Budgie".to_string();
                }
                if d.contains("cinnamon") {
                    return "Ubuntu Cinnamon".to_string();
                }
                return format!("Ubuntu ({de})");
            }
        }
        base
    }
    #[cfg(not(target_os = "linux"))]
    {
        System::long_os_version()
            .or_else(System::os_version)
            .unwrap_or_else(|| "n/a".to_string())
    }
}

#[cfg(target_os = "linux")]
fn linux_distro_flavor_deterministic() -> Option<String> {
    for path in [
        "/etc/xdg/kcm-about-distrorc",
        "/usr/share/kubuntu-default-settings/kf5-settings/kcm-about-distrorc",
    ] {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("Name=") {
                    let name = rest.trim().trim_matches('"');
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    if Path::new("/usr/share/kubuntu-default-settings").exists() {
        return Some("Kubuntu".to_string());
    }
    if Path::new("/usr/share/xubuntu-default-settings").exists() {
        return Some("Xubuntu".to_string());
    }
    if Path::new("/usr/share/ubuntu-mate-default-settings").exists() {
        return Some("Ubuntu MATE".to_string());
    }
    if Path::new("/usr/share/lubuntu-default-settings").exists() {
        return Some("Lubuntu".to_string());
    }
    if Path::new("/usr/share/ubuntu-budgie-default-settings").exists() {
        return Some("Ubuntu Budgie".to_string());
    }
    None
}

#[cfg(target_os = "linux")]
fn pretty_name_from_os_release() -> Option<String> {
    let s = fs::read_to_string("/etc/os-release").ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
            let v = rest.trim().trim_matches('"');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn kernel_version_string() -> String {
    System::kernel_version().unwrap_or_else(|| "n/a".to_string())
}

pub fn host_name_string() -> String {
    System::host_name().unwrap_or_else(|| "n/a".to_string())
}

/// CPU model string (lscpu model name on Linux when available, else first CPU brand).
pub fn cpu_model_string(lscpu_raw: Option<&str>, sys: &System) -> String {
    if let Some(raw) = lscpu_raw {
        if let Some(info) = lscpu::parse_lscpu(raw) {
            if let Some(ref m) = info.model {
                return m.clone();
            }
        }
    }
    sys.cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "n/a".to_string())
}

/// Refresh CPU list for [`cpu_model_string`].
pub fn system_for_cpu_model() -> System {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    );
    sys.refresh_cpu_all();
    sys
}

/// Flat machine snapshot for IPC (same fields as the CLI machine view).
#[derive(Debug, Clone, Serialize)]
pub struct MachineInfo {
    pub os: String,
    pub virtualization: String,
    pub host_name: String,
    pub local_ip: String,
    pub machine_model: String,
    pub cpu_model: String,
    pub distro_flavor: String,
    pub kernel_version: String,
    pub motherboard: String,
}

pub fn gather_machine_info() -> MachineInfo {
    let sys = system_for_cpu_model();
    let lscpu_raw = cpu::try_lscpu_output();
    MachineInfo {
        os: friendly_os_type().to_string(),
        virtualization: virtualization_env_label().to_string(),
        host_name: host_name_string(),
        local_ip: local_ip_addresses(),
        machine_model: machine_model(),
        cpu_model: cpu_model_string(lscpu_raw.as_deref(), &sys),
        distro_flavor: distro_flavor(),
        kernel_version: kernel_version_string(),
        motherboard: motherboard_name(),
    }
}
