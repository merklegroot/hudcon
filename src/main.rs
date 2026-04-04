use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::queue;
use crossterm::style::{Print, PrintStyledContent, ResetColor, Stylize};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

use hudcon::lscpu;

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

/// First menu row. The dash rule above the menu uses the same display width as this string.
/// Format: `(X)label` where `X` is the hotkey (styled in the menu).
const MENU_TOP_LINE: &str = "(C)PU info";

fn menu_top_line_width() -> usize {
    MENU_TOP_LINE.chars().count()
}

fn menu_top_hotkey() -> char {
    parse_parenthesized_hotkey(MENU_TOP_LINE)
        .expect("MENU_TOP_LINE must look like `(X)label`")
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

/// Dash line matching the width of [`MENU_TOP_LINE`].
fn write_banner_rule() -> io::Result<()> {
    let mut out = io::stdout();
    let dashes = "-".repeat(menu_top_line_width());
    queue!(
        out,
        PrintStyledContent(dashes.grey()),
        Print("\r\n"),
        ResetColor
    )?;
    out.flush()
}

fn write_warn_line(s: impl AsRef<str>) -> io::Result<()> {
    let mut out = io::stdout();
    queue!(
        out,
        PrintStyledContent(s.as_ref().yellow()),
        Print("\r\n"),
        ResetColor
    )?;
    out.flush()
}

fn write_menu_top_line() -> io::Result<()> {
    let (key, rest) = parse_parenthesized_hotkey(MENU_TOP_LINE).expect("MENU_TOP_LINE must look like `(X)label`");
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

fn run_menu() -> io::Result<()> {
    enable_raw_mode()?;
    let _guard = RawModeGuard;

    write_section_title("HUDcon")?;
    write_crlf()?;

    loop {
        write_banner_rule()?;
        write_menu_top_line()?;
        write_menu_exit_line()?;

        let code = loop {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => break key.code,
                _ => {}
            }
        };

        match code {
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&menu_top_hotkey()) => {
                write_crlf()?;
                show_cpu_info()?;
            }
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'x') => {
                write_crlf()?;
                break;
            }
            _ => {
                write_crlf()?;
                write_warn_line(format!(
                    "Unknown choice. Try {} or X.",
                    menu_top_hotkey().to_ascii_uppercase()
                ))?;
                write_crlf()?;
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
