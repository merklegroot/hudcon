//! GPU / graphics info aligned with [hudsse](https://github.com/merklegroot/hudsse) `parseGpuInfo` and the GPU page.

use serde::Serialize;

#[cfg(target_os = "linux")]
use std::collections::HashSet;
#[cfg(target_os = "linux")]
use std::fs;
use std::process::Command;

/// One graphics adapter (matches hudsse `GpuResult`).
#[derive(Debug, Clone)]
pub struct GpuCard {
    pub index: usize,
    pub name: String,
    pub bus: String,
    pub revision: String,
    pub driver: String,
    pub memory_total: Option<String>,
    pub memory_used: Option<String>,
    pub memory_free: Option<String>,
    pub utilization: Option<u32>,
    pub temperature: Option<u32>,
    /// PCI boot-VGA device (typical firmware primary display output).
    pub primary_display: bool,
    /// Best match for the current session’s OpenGL renderer (when known).
    pub opengl_active: bool,
}

#[derive(Debug, Default, Clone)]
pub struct GpuInfo {
    pub gpus: Vec<GpuCard>,
    pub opengl_renderer: Option<String>,
}

pub fn gather_gpu_info() -> GpuInfo {
    #[cfg(target_os = "linux")]
    return gather_gpu_info_linux();
    #[cfg(target_os = "macos")]
    return gather_gpu_info_macos();
    #[cfg(target_os = "windows")]
    return gather_gpu_info_windows();
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )))]
    GpuInfo::default()
}

#[cfg(target_os = "linux")]
fn gather_gpu_info_linux() -> GpuInfo {
    let raw = linux_gpu_command_output();
    let mut info = parse_gpu_info(&raw);
    info.opengl_renderer = linux_opengl_renderer();
    linux_apply_activity_hints(&mut info);
    info
}

#[cfg(target_os = "linux")]
fn linux_gpu_command_output() -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg(
            "nvidia-smi --query-gpu=index,name,pci.bus_id,memory.total,memory.used,memory.free,utilization.gpu,temperature.gpu,driver_version --format=csv,noheader,nounits 2>/dev/null || nvidia-smi --query-gpu=index,name,memory.total,memory.used,memory.free,utilization.gpu,temperature.gpu,driver_version --format=csv,noheader,nounits 2>/dev/null || lspci -v | grep -A 20 -i \"vga\\|3d\\|display\"",
        )
        .output();
    match out {
        Ok(o) if o.status.success() || !o.stdout.is_empty() => String::from_utf8_lossy(&o.stdout).into_owned(),
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

#[cfg(target_os = "linux")]
fn linux_opengl_renderer() -> Option<String> {
    let out = Command::new("sh")
        .arg("-c")
        .arg("glxinfo 2>/dev/null | grep -i \"OpenGL renderer\" 2>/dev/null || true")
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    parse_opengl_renderer(&s).filter(|r| !r.is_empty())
}

#[cfg(target_os = "macos")]
fn gather_gpu_info_macos() -> GpuInfo {
    let out = Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output();
    let text = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => String::new(),
    };
    GpuInfo {
        gpus: parse_macos_system_profiler(&text),
        opengl_renderer: None,
    }
}

#[cfg(target_os = "macos")]
fn parse_macos_system_profiler(text: &str) -> Vec<GpuCard> {
    let mut gpus = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Chipset Model:") {
            let name = rest.trim().to_string();
            if name.is_empty() {
                continue;
            }
            let idx = gpus.len();
            gpus.push(GpuCard {
                index: idx,
                name,
                bus: "n/a".to_string(),
                revision: "n/a".to_string(),
                driver: "n/a".to_string(),
                memory_total: None,
                memory_used: None,
                memory_free: None,
                utilization: None,
                temperature: None,
                primary_display: gpus.is_empty(),
                opengl_active: false,
            });
        }
    }
    gpus
}

#[cfg(target_os = "windows")]
fn gather_gpu_info_windows() -> GpuInfo {
    let out = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Get-WmiObject -Class Win32_VideoController | ForEach-Object { Write-Output \"$($_.Index),$($_.Name),$($_.AdapterRAM),0,0,$($_.AdapterRAM),0,0,$($_.DriverVersion)\" }",
        ])
        .output();
    let raw = match out {
        Ok(o) if o.status.success() || !o.stdout.is_empty() => String::from_utf8_lossy(&o.stdout).into_owned(),
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => String::new(),
    };
    parse_gpu_info(&raw)
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

/// Parses combined `nvidia-smi` CSV or `lspci -v` grep output (same rules as hudsse `parseGpuInfo`).
pub fn parse_gpu_info(output: &str) -> GpuInfo {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return GpuInfo::default();
    }
    // Same heuristic as hudsse: CSV with `index, name, digits...` on a line.
    if trimmed.contains(',') && trimmed.lines().any(looks_like_nvidia_csv_line) {
        let gpus = parse_nvidia_smi_csv(trimmed);
        return GpuInfo {
            gpus,
            opengl_renderer: None,
        };
    }
    GpuInfo {
        gpus: parse_lspci_sections(trimmed),
        opengl_renderer: None,
    }
}

fn looks_like_nvidia_csv_line(line: &str) -> bool {
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    if parts.len() < 8 {
        return false;
    }
    if parts[0].parse::<u32>().is_err() {
        return false;
    }
    // `pci.bus_id` (contains ':') or legacy column that is memory size in MiB.
    parts[2].parse::<u64>().is_ok() || parts[2].contains(':')
}

fn parse_nvidia_smi_csv(trimmed: &str) -> Vec<GpuCard> {
    let mut gpus = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        if parts.len() < 8 {
            continue;
        }
        let has_pci = parts.len() >= 9 && parts[2].contains(':');
        let (mem_off, driver_idx) = if has_pci {
            (3usize, 8usize)
        } else {
            (2usize, 7usize)
        };
        let index = parts[0].parse::<usize>().unwrap_or(gpus.len());
        let name = parts[1].to_string();
        let bus = if has_pci {
            normalize_pci_bus_id(parts[2])
        } else {
            "Unknown".to_string()
        };
        let memory_total_mb: u64 = parts.get(mem_off).and_then(|s| s.parse().ok()).unwrap_or(0);
        let memory_used_mb: u64 = parts
            .get(mem_off + 1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let memory_free_mb: u64 = parts
            .get(mem_off + 2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let utilization = parts
            .get(mem_off + 3)
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&u| u > 0);
        let temperature = parts
            .get(mem_off + 4)
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&t| t > 0);
        let driver = parts
            .get(driver_idx)
            .unwrap_or(&"Unknown")
            .to_string();

        gpus.push(GpuCard {
            index,
            name,
            bus,
            revision: "Unknown".to_string(),
            driver,
            memory_total: (memory_total_mb > 0).then(|| format_bytes(memory_total_mb * 1024 * 1024)),
            memory_used: (memory_used_mb > 0).then(|| format_bytes(memory_used_mb * 1024 * 1024)),
            memory_free: (memory_free_mb > 0).then(|| format_bytes(memory_free_mb * 1024 * 1024)),
            utilization,
            temperature,
            primary_display: false,
            opengl_active: false,
        });
    }
    gpus
}

/// Normalizes `00000000:01:00.0` / `0000:01:00.0` to `01:00.0` for comparison with lspci / sysfs.
fn normalize_pci_bus_id(raw: &str) -> String {
    let s = raw.trim();
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        n if n >= 3 => format!("{}:{}", parts[n - 2], parts[n - 1]),
        2 => s.to_string(),
        _ => s.to_string(),
    }
}

#[cfg(target_os = "linux")]
fn linux_apply_activity_hints(info: &mut GpuInfo) {
    let boot = linux_boot_vga_bus_ids();
    let gl_idx = info
        .opengl_renderer
        .as_deref()
        .and_then(|r| best_opengl_gpu_index(&info.gpus, r));
    for (i, g) in info.gpus.iter_mut().enumerate() {
        g.primary_display = bus_is_boot_vga(&g.bus, &boot);
        g.opengl_active = gl_idx == Some(i);
    }
}

#[cfg(target_os = "linux")]
fn bus_is_boot_vga(bus: &str, boot: &HashSet<String>) -> bool {
    if bus == "Unknown" || bus == "n/a" {
        return false;
    }
    let n = normalize_pci_bus_id(bus);
    boot.contains(&n)
}

#[cfg(target_os = "linux")]
fn linux_boot_vga_bus_ids() -> HashSet<String> {
    let mut set = HashSet::new();
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return set;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let path = e.path();
        let boot = path.join("device/boot_vga");
        let Ok(v) = fs::read_to_string(&boot) else {
            continue;
        };
        if v.trim() != "1" {
            continue;
        }
        let Ok(link) = fs::read_link(path.join("device")) else {
            continue;
        };
        let fname = link.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if let Some(b) = bus_from_pci_sysname(fname) {
            set.insert(b);
        }
    }
    set
}

#[cfg(target_os = "linux")]
fn bus_from_pci_sysname(fname: &str) -> Option<String> {
    let parts: Vec<&str> = fname.split(':').collect();
    if parts.len() >= 3 {
        return Some(format!("{}:{}", parts[parts.len() - 2], parts[parts.len() - 1]));
    }
    None
}

fn best_opengl_gpu_index(gpus: &[GpuCard], renderer: &str) -> Option<usize> {
    let r = renderer.to_lowercase();
    let mut best: Option<(usize, u32)> = None;
    for (i, g) in gpus.iter().enumerate() {
        let score = opengl_match_score(&g.name, &r);
        if score > 0 {
            let replace = best.map_or(true, |(_, s)| score > s);
            if replace {
                best = Some((i, score));
            }
        }
    }
    best.map(|(i, _)| i)
}

fn opengl_match_score(gpu_name: &str, renderer_lower: &str) -> u32 {
    let g = gpu_name.to_lowercase();
    if g.len() >= 7 && renderer_lower.contains(g.as_str()) {
        return 100;
    }
    for w in g.split_whitespace() {
        if w.len() >= 6 && renderer_lower.contains(w) {
            return 90;
        }
    }
    let mut score = 0u32;
    for token in g.split_whitespace() {
        if token.len() >= 3 && token.chars().any(|c| c.is_ascii_digit()) {
            if renderer_lower.contains(token) {
                score += 25;
            }
        }
    }
    if (g.contains("nvidia") || g.contains("geforce")) && renderer_lower.contains("nvidia") {
        score += 35;
    }
    if g.contains("intel") && (renderer_lower.contains("intel") || renderer_lower.contains("mesa")) {
        score += 35;
    }
    if (g.contains("amd") || g.contains("radeon"))
        && (renderer_lower.contains("amd") || renderer_lower.contains("radeon"))
    {
        score += 35;
    }
    score
}

impl GpuCard {
    /// Short label for whether this adapter is the boot-VGA / OpenGL device.
    pub fn active_for_display(&self) -> &'static str {
        match (self.opengl_active, self.primary_display) {
            (true, true) => "OpenGL (this session) + primary display (boot VGA)",
            (true, false) => "OpenGL rendering (this session)",
            (false, true) => "Primary display (boot VGA)",
            (false, false) => "n/a",
        }
    }
}

/// Serializable GPU card (includes [`GpuCard::active_for_display`] as a string).
#[derive(Debug, Clone, Serialize)]
pub struct GpuCardDto {
    pub index: usize,
    pub name: String,
    pub bus: String,
    pub revision: String,
    pub driver: String,
    pub memory_total: Option<String>,
    pub memory_used: Option<String>,
    pub memory_free: Option<String>,
    pub utilization: Option<u32>,
    pub temperature: Option<u32>,
    pub primary_display: bool,
    pub opengl_active: bool,
    pub active_for_display: String,
}

impl From<&GpuCard> for GpuCardDto {
    fn from(c: &GpuCard) -> Self {
        GpuCardDto {
            index: c.index,
            name: c.name.clone(),
            bus: c.bus.clone(),
            revision: c.revision.clone(),
            driver: c.driver.clone(),
            memory_total: c.memory_total.clone(),
            memory_used: c.memory_used.clone(),
            memory_free: c.memory_free.clone(),
            utilization: c.utilization,
            temperature: c.temperature,
            primary_display: c.primary_display,
            opengl_active: c.opengl_active,
            active_for_display: c.active_for_display().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuInfoDto {
    pub gpus: Vec<GpuCardDto>,
    pub opengl_renderer: Option<String>,
}

pub fn gather_gpu_info_dto() -> GpuInfoDto {
    let g = gather_gpu_info();
    GpuInfoDto {
        gpus: g.gpus.iter().map(GpuCardDto::from).collect(),
        opengl_renderer: g.opengl_renderer,
    }
}

fn parse_lspci_sections(output: &str) -> Vec<GpuCard> {
    let sections: Vec<&str> = if output.contains("--\n") {
        output.split("--\n").collect()
    } else {
        vec![output]
    };
    let mut gpus = Vec::new();
    for section in sections {
        if let Some(gpu) = parse_lspci_section(section, gpus.len()) {
            gpus.push(gpu);
        }
    }
    gpus
}

fn parse_lspci_section(section: &str, next_index: usize) -> Option<GpuCard> {
    let lines: Vec<String> = section
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }

    let mut first_line = String::new();
    for (j, line) in lines.iter().enumerate() {
        if is_gpu_controller_line(line) {
            first_line.clone_from(line);
            if j + 1 < lines.len() {
                let next = &lines[j + 1];
                if !next.contains(':')
                    && !next.to_lowercase().contains("subsystem")
                    && !next.to_lowercase().contains("flags")
                {
                    first_line.push(' ');
                    first_line.push_str(next.trim());
                }
            }
            break;
        }
    }
    if first_line.is_empty() {
        return None;
    }

    let bus_id = first_line
        .split_whitespace()
        .next()
        .filter(|s| {
            let b = s.as_bytes();
            b.len() >= 7
                && b[2] == b':'
                && b[5] == b'.'
                && s.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        })
        .unwrap_or("Unknown")
        .to_string();

    let device_name = extract_lspci_device_name(&first_line);
    let revision = extract_lspci_revision(&first_line);

    let mut driver = "Unknown".to_string();
    for line in &lines {
        if let Some(idx) = line.to_lowercase().find("kernel driver in use:") {
            let rest = &line[idx + "kernel driver in use:".len()..];
            driver = rest.trim().to_string();
            break;
        }
    }

    Some(GpuCard {
        index: next_index,
        name: device_name,
        bus: bus_id,
        revision,
        driver,
        memory_total: None,
        memory_used: None,
        memory_free: None,
        utilization: None,
        temperature: None,
        primary_display: false,
        opengl_active: false,
    })
}

fn extract_lspci_device_name(first_line: &str) -> String {
    let lower = first_line.to_lowercase();
    for p in [
        "vga compatible controller:",
        "3d controller:",
        "display controller:",
    ] {
        if let Some(idx) = lower.find(p) {
            let rest = first_line[idx + p.len()..].trim();
            return strip_lspci_name_suffixes(rest);
        }
    }
    "Unknown GPU".to_string()
}

fn strip_lspci_name_suffixes(s: &str) -> String {
    let lower = s.to_lowercase();
    let end = lower
        .find("(rev ")
        .or_else(|| lower.find("(prog-if "))
        .unwrap_or(s.len());
    s[..end].trim().to_string()
}

fn extract_lspci_revision(first_line: &str) -> String {
    let lower = first_line.to_lowercase();
    if let Some(i) = lower.find("(rev ") {
        let tail = &first_line[i..];
        if let Some(close) = tail.find(')') {
            return tail[5..close].trim().to_string();
        }
    }
    "Unknown".to_string()
}

fn is_gpu_controller_line(line: &str) -> bool {
    let l = line.to_lowercase();
    l.contains("vga compatible controller:")
        || l.contains("3d controller:")
        || l.contains("display controller:")
}

fn parse_opengl_renderer(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }
    for line in trimmed.lines() {
        let l = line.trim();
        let lower = l.to_lowercase();
        if let Some(idx) = lower.find("opengl renderer string:") {
            let rest = &l[idx + "opengl renderer string:".len()..];
            return Some(rest.trim().to_string());
        }
        if let Some(idx) = lower.find("opengl renderer:") {
            let rest = &l[idx + "opengl renderer:".len()..];
            return Some(rest.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nvidia_csv_line() {
        let s = "0, NVIDIA GeForce RTX 4090, 24564, 100, 24464, 5, 42, 550.54.15";
        let info = parse_gpu_info(s);
        assert_eq!(info.gpus.len(), 1);
        assert_eq!(info.gpus[0].name, "NVIDIA GeForce RTX 4090");
        assert_eq!(info.gpus[0].driver, "550.54.15");
        assert!(info.gpus[0].memory_total.is_some());
    }

    #[test]
    fn parse_nvidia_csv_with_pci_bus_id() {
        let s = "0, NVIDIA GeForce RTX 4090, 00000000:01:00.0, 24564, 100, 24464, 5, 42, 550.54.15";
        let info = parse_gpu_info(s);
        assert_eq!(info.gpus.len(), 1);
        assert_eq!(info.gpus[0].bus, "01:00.0");
    }

    #[test]
    fn parse_lspci_single_section() {
        let s = r"01:00.0 VGA compatible controller: NVIDIA Corporation GA107M [GeForce RTX 3050 Ti Mobile] (rev a1)
	Subsystem: NVIDIA Corporation Device 25a2
	Kernel driver in use: nvidia";
        let info = parse_gpu_info(s);
        assert_eq!(info.gpus.len(), 1);
        assert_eq!(info.gpus[0].bus, "01:00.0");
        assert!(info.gpus[0].name.contains("GeForce RTX 3050"));
        assert_eq!(info.gpus[0].driver, "nvidia");
        assert_eq!(info.gpus[0].revision, "a1");
    }
}
