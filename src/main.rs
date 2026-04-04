use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

/// Raw mode treats `\n` as line-feed only; the cursor column is unchanged. Use CRLF so lines start at column 0.
fn write_crlf_line(s: impl AsRef<str>) -> io::Result<()> {
    let mut out = io::stdout();
    write!(out, "{}\r\n", s.as_ref())?;
    out.flush()
}

fn write_crlf() -> io::Result<()> {
    let mut out = io::stdout();
    write!(out, "\r\n")?;
    out.flush()
}

/// One space between a fixed-width label column and the value so lines align.
fn kv_line(label: &str, value: impl std::fmt::Display) -> String {
    const LABEL_WIDTH: usize = 18;
    format!("{label:<w$} {value}", w = LABEL_WIDTH)
}

fn show_processor_info() -> io::Result<()> {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    );
    sys.refresh_cpu_all();

    write_crlf_line("Processor")?;
    write_crlf_line("---------")?;

    if let Some(cpu) = sys.cpus().first() {
        write_crlf_line(&kv_line("Brand:", cpu.brand().trim()))?;
        write_crlf_line(&kv_line("Frequency:", format!("{} MHz", cpu.frequency())))?;
    } else {
        write_crlf_line(&kv_line("Brand:", "(unavailable)"))?;
    }

    if let Some(n) = sys.physical_core_count() {
        write_crlf_line(&kv_line("Physical cores:", n))?;
    }
    write_crlf_line(&kv_line("Logical cores:", sys.cpus().len()))?;
    write_crlf()?;
    Ok(())
}

fn run_menu() -> io::Result<()> {
    enable_raw_mode()?;
    let _guard = RawModeGuard;

    loop {
        write_crlf_line("(P)rocessor Info")?;
        write_crlf_line("e(X)it")?;

        let code = loop {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => break key.code,
                _ => {}
            }
        };

        match code {
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'p') => {
                write_crlf()?;
                show_processor_info()?;
            }
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'x') => {
                write_crlf()?;
                break;
            }
            _ => {
                write_crlf()?;
                write_crlf_line("Unknown choice. Try P or X.")?;
                write_crlf()?;
            }
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run_menu() {
        eprintln!("Error: {e}");
    }
}
