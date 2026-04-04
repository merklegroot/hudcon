//! Integration tests for `lscpu` parsing and formatting (no TUI / menu).

use hudcon::lscpu::{format_cache_kb, parse_lscpu};

#[test]
fn format_cache_kb_scales_units() {
    assert_eq!(format_cache_kb(512), "512 KB");
    assert_eq!(format_cache_kb(1024), "1.0 MB");
    assert_eq!(format_cache_kb(1024 * 1024), "1.0 GB");
}

#[test]
fn parse_lscpu_reads_model_and_cores() {
    let raw = r"Model name:                      Example CPU
CPU(s):                           16
Architecture:                     x86_64
";
    let info = parse_lscpu(raw).expect("valid minimal lscpu sample");
    assert_eq!(info.model.as_deref(), Some("Example CPU"));
    assert_eq!(info.cpu_cores, Some(16));
    assert_eq!(info.architecture.as_deref(), Some("x86_64"));
}
