use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::queue;
use crossterm::style::{Print, PrintStyledContent, ResetColor, Stylize};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

use hudcon::gpu;
use hudcon::lscpu;
use hudcon::machine;
use hudcon::disk;
use hudcon::memory;

struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn write_section_title(text: &str) -> io::Result<()> {
    let mut out = io::stdout();
    queue!(
        out,
        PrintStyledContent(text.bold().cyan()),
        Print("\r\n"),
        ResetColor
    )?;
    out.flush()
}

fn write_section_rule() -> io::Result<()> {
    let mut out = io::stdout();
    queue!(
        out,
        PrintStyledContent("---------".grey()),
        Print("\r\n"),
        ResetColor
    )?;
    out.flush()
}

/// Menu rows. The dash rule above the menu uses the width of the longest line.
/// Format: `(X)label` where `X` is the hotkey (styled in the menu).
const MENU_LINES: &[&str] = &[
    "(C)PU info",
    "(M)achine info",
    "(G)raphics cards",
    "(R)AM",
    "(D)isk",
];

fn menu_width() -> usize {
    MENU_LINES
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0)
}

fn menu_hotkey_for(line: &str) -> char {
    parse_parenthesized_hotkey(line)
        .expect("MENU_LINES entries must look like `(X)label`")
        .0
}

/// Parses `(C)rest` into the hotkey character and the text after `)`.
fn parse_parenthesized_hotkey(s: &str) -> Option<(char, &str)> {
    let s = s.strip_prefix('(')?;
    let mut it = s.chars();
    let key = it.next()?;
    let tail = it.as_str();
    let rest = tail.strip_prefix(')')?;
    Some((key, rest))
}

/// Dash line matching the width of the widest [`MENU_LINES`] entry.
fn write_banner_rule() -> io::Result<()> {
    let mut out = io::stdout();
    let dashes = "-".repeat(menu_width());
    queue!(
        out,
        PrintStyledContent(dashes.grey()),
        Print("\r\n"),
        ResetColor
    )?;
    out.flush()
}

fn write_menu_line(line: &str) -> io::Result<()> {
    let (key, rest) = parse_parenthesized_hotkey(line).expect("menu line must look like `(X)label`");
    let key_str = key.to_string();
    let mut out = io::stdout();
    queue!(
        out,
        PrintStyledContent("(".grey()),
        PrintStyledContent(key_str.as_str().yellow().bold()),
        PrintStyledContent(")".grey()),
        PrintStyledContent(rest.white()),
        Print("\r\n"),
        ResetColor
    )?;
    out.flush()
}

fn write_menu_exit_line() -> io::Result<()> {
    let mut out = io::stdout();
    queue!(
        out,
        PrintStyledContent("e(".grey()),
        PrintStyledContent("X".yellow().bold()),
        PrintStyledContent(")it".grey()),
        Print("\r\n"),
        ResetColor
    )?;
    out.flush()
}

fn write_crlf() -> io::Result<()> {
    let mut out = io::stdout();
    write!(out, "\r\n")?;
    out.flush()
}

fn short_gpu_name(s: &str) -> String {
    if s.len() > 48 {
        format!("{}…", &s[..46])
    } else {
        s.to_string()
    }
}

/// One-line recap: which GPU indices are driving OpenGL vs firmware primary display.
fn write_gpu_session_summary(info: &gpu::GpuInfo) -> io::Result<()> {
    let mut parts = Vec::new();
    for g in &info.gpus {
        let mut tags = Vec::new();
        if g.opengl_active {
            tags.push("OpenGL");
        }
        if g.primary_display {
            tags.push("boot VGA");
        }
        if !tags.is_empty() {
            parts.push(format!(
                "GPU {} ({}): {}",
                g.index,
                short_gpu_name(&g.name),
                tags.join(" + ")
            ));
        }
    }
    if parts.is_empty() {
        return Ok(());
    }
    write_kv("In use (summary):", parts.join(" | "))?;
    Ok(())
}

const KV_LABEL_WIDTH: usize = 18;

/// Label + value row (label dim, value green or muted for `n/a`).
fn write_kv(label: &str, value: impl std::fmt::Display) -> io::Result<()> {
    let value_str = format!("{value}");
    write_kv_str(label, &value_str)
}

fn write_kv_str(label: &str, value: &str) -> io::Result<()> {
    let padded = format!("{label:<KV_LABEL_WIDTH$}");
    let mut out = io::stdout();
    queue!(out, PrintStyledContent(padded.as_str().grey()), Print(" "))?;
    if value == "n/a" {
        queue!(out, PrintStyledContent(value.grey()))?;
    } else {
        queue!(out, PrintStyledContent(value.green()))?;
    }
    queue!(out, Print("\r\n"), ResetColor)?;
    out.flush()
}

fn fmt_mhz(mhz: u64) -> String {
    if mhz == 0 {
        "n/a".to_string()
    } else {
        format!("{mhz} MHz")
    }
}

fn fmt_mhz_opt(mhz: Option<u64>) -> String {
    match mhz {
        Some(m) if m > 0 => format!("{m} MHz"),
        _ => "n/a".to_string(),
    }
}

fn fmt_opt_u32(o: Option<u32>) -> String {
    o.map(|n| n.to_string()).unwrap_or_else(|| "n/a".to_string())
}

fn fmt_opt_str(o: &Option<String>) -> String {
    o.as_deref().unwrap_or("n/a").to_string()
}

/// Hardware max frequency in MHz (e.g. cpufreq `cpuinfo_max_freq`), if known.
fn advertised_max_cpu_mhz() -> Option<u64> {
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

#[cfg(target_os = "linux")]
fn try_lscpu_output() -> Option<String> {
    use std::process::Command;
    let out = Command::new("lscpu").output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

fn write_cpu_features(f: &lscpu::CpuFeatures) -> io::Result<()> {
    let mut sse = Vec::new();
    if f.sse {
        sse.push("SSE");
    }
    if f.sse2 {
        sse.push("SSE2");
    }
    if f.sse3 {
        sse.push("SSE3");
    }
    if f.ssse3 {
        sse.push("SSSE3");
    }
    if f.sse4_1 {
        sse.push("SSE4.1");
    }
    if f.sse4_2 {
        sse.push("SSE4.2");
    }
    let mut avx = Vec::new();
    if f.avx {
        avx.push("AVX");
    }
    if f.avx2 {
        avx.push("AVX2");
    }
    if f.avx512 {
        avx.push("AVX512");
    }
    if f.fma {
        avx.push("FMA");
    }
    if f.aes {
        avx.push("AES");
    }
    if f.sha {
        avx.push("SHA");
    }
    if f.neon {
        avx.push("NEON");
    }

    if sse.is_empty() && avx.is_empty() {
        return Ok(());
    }

    write_section_title("CPU features")?;
    write_section_rule()?;
    if !sse.is_empty() {
        write_kv("SSE family:", sse.join(", "))?;
    }
    if !avx.is_empty() {
        write_kv("AVX / other:", avx.join(", "))?;
    }
    Ok(())
}

fn print_lscpu_block(info: &lscpu::LscpuInfo) -> io::Result<()> {
    write_kv("Vendor:", fmt_opt_str(&info.vendor))?;
    write_kv("CPU Model:", fmt_opt_str(&info.model))?;
    write_kv("CPU Cores:", fmt_opt_u32(info.cpu_cores))?;
    write_kv("Architecture:", fmt_opt_str(&info.architecture))?;
    write_kv(
        "CPU Frequency:",
        info.cpu_mhz.map(|m| format!("{m} MHz")).unwrap_or_else(|| "n/a".to_string()),
    )?;
    write_kv("Threads per Core:", fmt_opt_u32(info.threads_per_core))?;
    write_kv("Cores per Socket:", fmt_opt_u32(info.cores_per_socket))?;
    write_kv("Sockets:", fmt_opt_u32(info.sockets))?;
    write_kv("Virtualization:", fmt_opt_str(&info.virtualization))?;
    write_kv(
        "L1d Cache:",
        info.l1d_kb.map(lscpu::format_cache_kb).unwrap_or_else(|| "n/a".to_string()),
    )?;
    write_kv(
        "L1i Cache:",
        info.l1i_kb.map(lscpu::format_cache_kb).unwrap_or_else(|| "n/a".to_string()),
    )?;
    write_kv(
        "L2 Cache:",
        info.l2_kb.map(lscpu::format_cache_kb).unwrap_or_else(|| "n/a".to_string()),
    )?;
    write_kv(
        "L3 Cache:",
        info.l3_kb.map(lscpu::format_cache_kb).unwrap_or_else(|| "n/a".to_string()),
    )?;
    Ok(())
}

fn show_cpu_info() -> io::Result<()> {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    );
    sys.refresh_cpu_all();

    write_section_title("CPU")?;
    write_section_rule()?;

    #[cfg(target_os = "linux")]
    {
        if let Some(raw) = try_lscpu_output() {
            if let Some(ref info) = lscpu::parse_lscpu(&raw) {
                print_lscpu_block(info)?;
                if let Some(cpu) = sys.cpus().first() {
                    write_kv("Current MHz:", fmt_mhz(cpu.frequency()))?;
                    write_kv("Max (advertised):", fmt_mhz_opt(advertised_max_cpu_mhz()))?;
                }
                write_cpu_features(&info.features)?;
                write_crlf()?;
                return Ok(());
            }
        }
    }

    if let Some(cpu) = sys.cpus().first() {
        write_kv("Vendor:", cpu.vendor_id())?;
        write_kv("CPU Model:", cpu.brand().trim())?;
        write_kv("Current MHz:", fmt_mhz(cpu.frequency()))?;
        write_kv("Max (advertised):", fmt_mhz_opt(advertised_max_cpu_mhz()))?;
    } else {
        write_kv("CPU Model:", "(unavailable)")?;
    }

    if let Some(n) = sys.physical_core_count() {
        write_kv("Physical cores:", n)?;
    }
    write_kv("Logical cores:", sys.cpus().len())?;
    write_crlf()?;
    Ok(())
}

fn show_machine_info() -> io::Result<()> {
    let sys = machine::system_for_cpu_model();
    #[cfg(target_os = "linux")]
    let lscpu_raw = try_lscpu_output();
    #[cfg(not(target_os = "linux"))]
    let lscpu_raw: Option<String> = None;
    let lscpu_ref = lscpu_raw.as_deref();

    write_section_title("Machine Information")?;
    write_section_rule()?;
    write_kv("OS:", machine::friendly_os_type())?;
    write_kv("Virtualization:", machine::virtualization_env_label())?;

    write_section_title("System Details")?;
    write_section_rule()?;
    write_kv("Machine Name:", machine::host_name_string())?;
    write_kv("Local IP Address:", machine::local_ip_addresses())?;
    write_kv("Machine Model:", machine::machine_model())?;
    write_kv("CPU Model:", machine::cpu_model_string(lscpu_ref, &sys))?;
    write_kv("Distro Flavor:", machine::distro_flavor())?;
    write_kv("Kernel Version:", machine::kernel_version_string())?;
    write_kv("Motherboard:", machine::motherboard_name())?;
    write_crlf()?;
    Ok(())
}

fn show_gpu_info() -> io::Result<()> {
    let info = gpu::gather_gpu_info();

    write_section_title("Graphics Cards")?;
    write_section_rule()?;

    if let Some(ref r) = info.opengl_renderer {
        if !r.is_empty() {
            write_kv("OpenGL Renderer:", r)?;
        }
    }

    write_gpu_session_summary(&info)?;

    if info.gpus.is_empty() {
        write_kv("GPUs:", "No GPU information available")?;
        write_crlf()?;
        return Ok(());
    }

    for card in &info.gpus {
        write_section_title(&format!("GPU {}: {}", card.index, card.name))?;
        write_section_rule()?;
        write_kv("Active for:", card.active_for_display())?;
        write_kv("Driver:", &card.driver)?;
        if card.bus != "Unknown" && card.bus != "n/a" {
            write_kv("Bus:", &card.bus)?;
        }
        if card.revision != "Unknown" && card.revision != "n/a" {
            write_kv("Revision:", &card.revision)?;
        }
        if let Some(ref m) = card.memory_total {
            write_kv("Memory Total:", m)?;
        }
        if let Some(ref m) = card.memory_used {
            write_kv("Memory Used:", m)?;
        }
        if let Some(ref m) = card.memory_free {
            write_kv("Memory Free:", m)?;
        }
        if let Some(u) = card.utilization {
            if u > 0 {
                write_kv("Utilization:", format!("{u}%"))?;
            }
        }
        if let Some(t) = card.temperature {
            if t > 0 {
                write_kv("Temperature:", format!("{t}°C"))?;
            }
        }
        write_crlf()?;
    }

    Ok(())
}

fn show_memory_info() -> io::Result<()> {
    let info = memory::gather_memory_info();

    write_section_title("Memory Information")?;
    write_crlf()?;

    write_section_title("Memory Usage")?;
    write_section_rule()?;
    write_kv("RAM Usage:", format!("{}% used", info.used_percent))?;
    write_kv("Total RAM:", &info.total_ram)?;
    write_kv("Free RAM:", &info.free_ram)?;
    write_kv("Used RAM:", &info.used_ram)?;
    write_crlf()?;

    write_section_title("Top RAM Consuming Processes")?;
    write_section_rule()?;
    if info.top_processes.is_empty() {
        write_kv("Processes:", "No process information available")?;
    } else {
        for (i, p) in info.top_processes.iter().enumerate() {
            write_section_title(&format!("{}. {}", i + 1, p.name))?;
            write_section_rule()?;
            write_kv("PID:", &p.pid)?;
            write_kv("Memory:", &p.memory_absolute)?;
            write_kv("% of RAM:", &p.memory_usage)?;
            write_crlf()?;
        }
    }

    Ok(())
}

fn show_disk_info() -> io::Result<()> {
    let info = disk::gather_disk_info();

    write_section_title("Disk Information")?;
    write_crlf()?;

    write_section_title("Physical Disks")?;
    write_section_rule()?;
    if info.physical_disks.is_empty() {
        write_kv("Drives:", "No physical disk information available")?;
        write_crlf()?;
    } else {
        for pd in &info.physical_disks {
            write_section_title(&pd.device)?;
            write_section_rule()?;
            if pd.disk_type != "Unknown" {
                write_kv("Type:", &pd.disk_type)?;
            }
            write_kv("Size:", &pd.size)?;
            write_kv("Model:", &pd.model)?;
            write_crlf()?;
        }
    }

    write_section_title("Disk Usage (Partitions)")?;
    write_section_rule()?;
    if info.disks.is_empty() {
        write_kv("Mounts:", "No disk usage information available")?;
        write_crlf()?;
    } else {
        for d in &info.disks {
            write_section_title(&d.mount)?;
            write_section_rule()?;
            write_kv("Filesystem:", &d.filesystem)?;
            write_kv("Usage:", format!("{}% used", d.used_percent))?;
            write_kv("Total:", &d.total)?;
            write_kv("Used:", &d.used)?;
            write_kv("Available:", &d.available)?;
            write_crlf()?;
        }
    }

    Ok(())
}

fn run_menu() -> io::Result<()> {
    enable_raw_mode()?;
    let _guard = RawModeGuard;

    write_section_title("HUDcon")?;
    write_crlf()?;

    'menu: loop {
        write_banner_rule()?;
        for line in MENU_LINES {
            write_menu_line(line)?;
        }
        write_menu_exit_line()?;

        loop {
            let code = loop {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => break key.code,
                    _ => {}
                }
            };

            match code {
                KeyCode::Char(c) if c.eq_ignore_ascii_case(&menu_hotkey_for(MENU_LINES[0])) => {
                    write_crlf()?;
                    show_cpu_info()?;
                    continue 'menu;
                }
                KeyCode::Char(c) if c.eq_ignore_ascii_case(&menu_hotkey_for(MENU_LINES[1])) => {
                    write_crlf()?;
                    show_machine_info()?;
                    continue 'menu;
                }
                KeyCode::Char(c) if c.eq_ignore_ascii_case(&menu_hotkey_for(MENU_LINES[2])) => {
                    write_crlf()?;
                    show_gpu_info()?;
                    continue 'menu;
                }
                KeyCode::Char(c) if c.eq_ignore_ascii_case(&menu_hotkey_for(MENU_LINES[3])) => {
                    write_crlf()?;
                    show_memory_info()?;
                    continue 'menu;
                }
                KeyCode::Char(c) if c.eq_ignore_ascii_case(&menu_hotkey_for(MENU_LINES[4])) => {
                    write_crlf()?;
                    show_disk_info()?;
                    continue 'menu;
                }
                KeyCode::Char(c) if c.eq_ignore_ascii_case(&'x') => {
                    write_crlf()?;
                    break 'menu;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run_menu() {
        use std::io::stderr;
        let mut err = stderr();
        let _ = queue!(
            err,
            PrintStyledContent("Error: ".red().bold()),
            PrintStyledContent(format!("{e}").red()),
            Print("\r\n"),
            ResetColor
        );
        let _ = err.flush();
    }
}
