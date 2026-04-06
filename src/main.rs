use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::queue;
use crossterm::style::{Print, PrintStyledContent, ResetColor, Stylize};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use hudcon::cpu;
use hudcon::gpu;
use hudcon::lscpu;
use hudcon::machine;
use hudcon::disk;
use hudcon::memory;
use hudcon::package;
use hudcon::path;

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
    "(P)ackage management",
    "(H) Path",
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
    let snap = cpu::gather_cpu_info();

    write_section_title("CPU")?;
    write_section_rule()?;

    if let Some(ref info) = snap.lscpu {
        print_lscpu_block(info)?;
        write_kv("Current MHz:", fmt_mhz(snap.current_mhz))?;
        write_kv("Max (advertised):", fmt_mhz_opt(snap.max_advertised_mhz))?;
        write_cpu_features(&info.features)?;
        write_crlf()?;
        return Ok(());
    }

    if let Some(ref v) = snap.vendor {
        write_kv("Vendor:", v)?;
    }
    if let Some(ref m) = snap.cpu_model {
        if !m.is_empty() {
            write_kv("CPU Model:", m)?;
        } else {
            write_kv("CPU Model:", "(unavailable)")?;
        }
    } else {
        write_kv("CPU Model:", "(unavailable)")?;
    }
    if snap.vendor.is_some() || snap.cpu_model.as_ref().map_or(false, |s| !s.is_empty()) {
        write_kv("Current MHz:", fmt_mhz(snap.current_mhz))?;
        write_kv("Max (advertised):", fmt_mhz_opt(snap.max_advertised_mhz))?;
    }

    if let Some(n) = snap.physical_cores {
        write_kv("Physical cores:", n)?;
    }
    write_kv("Logical cores:", snap.logical_cores)?;
    write_crlf()?;
    Ok(())
}

fn show_machine_info() -> io::Result<()> {
    let m = machine::gather_machine_info();

    write_section_title("Machine Information")?;
    write_section_rule()?;
    write_kv("OS:", &m.os)?;
    write_kv("Virtualization:", &m.virtualization)?;

    write_section_title("System Details")?;
    write_section_rule()?;
    write_kv("Machine Name:", &m.host_name)?;
    write_kv("Local IP Address:", &m.local_ip)?;
    write_kv("Machine Model:", &m.machine_model)?;
    write_kv("CPU Model:", &m.cpu_model)?;
    write_kv("Distro Flavor:", &m.distro_flavor)?;
    write_kv("Kernel Version:", &m.kernel_version)?;
    write_kv("Motherboard:", &m.motherboard)?;
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

fn show_package_info() -> io::Result<()> {
    let info = package::gather_package_info();

    write_section_title("Package Management")?;
    write_crlf()?;

    write_section_title("Package Information")?;
    write_section_rule()?;
    write_kv("Package Manager:", &info.package_manager)?;
    let formats = if info.package_formats.len() == 1 {
        info.package_formats[0].clone()
    } else {
        info.package_formats.join(", ")
    };
    write_kv("Package Formats:", formats)?;
    write_crlf()?;

    write_section_title("Package Repositories")?;
    write_section_rule()?;
    if info.repositories.is_empty() {
        write_kv("Repositories:", "No repositories found")?;
    } else {
        for repo in &info.repositories {
            write_kv_str(&format!("{}:", repo.package_manager), &repo.repository)?;
        }
    }

    Ok(())
}

fn show_path_info() -> io::Result<()> {
    let info = path::gather_path_info();

    write_section_title("Path Information")?;
    write_crlf()?;

    write_section_title("PATH variable")?;
    write_section_rule()?;
    if info.path.is_empty() {
        write_kv("PATH:", "(empty or unset)")?;
    } else {
        write_kv_str("PATH:", &info.path)?;
    }
    write_crlf()?;

    write_section_title(&format!("Path folders ({})", info.folders.len()))?;
    write_section_rule()?;
    if info.folders.is_empty() {
        write_kv("Folders:", "No path folders found")?;
    } else {
        for (i, folder) in info.folders.iter().enumerate() {
            write_kv_str(&format!("{:>3}.", i + 1), folder)?;
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
                KeyCode::Char(c) if c.eq_ignore_ascii_case(&menu_hotkey_for(MENU_LINES[5])) => {
                    write_crlf()?;
                    show_package_info()?;
                    continue 'menu;
                }
                KeyCode::Char(c) if c.eq_ignore_ascii_case(&menu_hotkey_for(MENU_LINES[6])) => {
                    write_crlf()?;
                    show_path_info()?;
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
