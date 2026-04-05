//! Integration test: real host CPU architecture vs a small allowlist of common arches.

use hudcon::lscpu::parse_lscpu;
use sysinfo::System;

/// True if `s` names a widely used CPU ISA (normalized, lowercase).
fn is_common_architecture(s: &str) -> bool {
    let s = s.trim().to_lowercase();
    matches!(
        s.as_str(),
        "x86_64"
            | "aarch64"
            | "arm64"
            | "arm"
            | "riscv64"
            | "mips"
            | "mips64"
            | "powerpc"
            | "powerpc64"
            | "ppc64"
            | "ppc64le"
            | "s390x"
            | "loongarch64"
            | "i686"
            | "i386"
            | "x86"
    ) || s.starts_with("armv")
        || s.starts_with("riscv")
}

#[test]
fn actual_cpu_reports_common_architecture() {
    let sys_arch = System::cpu_arch();
    assert!(
        is_common_architecture(&sys_arch),
        "sysinfo::System::cpu_arch() should be a known ISA, got {sys_arch:?}"
    );

    assert!(
        is_common_architecture(std::env::consts::ARCH),
        "std::env::consts::ARCH should be a known ISA, got {:?}",
        std::env::consts::ARCH
    );

    #[cfg(target_os = "linux")]
    {
        let Ok(output) = std::process::Command::new("lscpu").output() else {
            return;
        };
        if !output.status.success() {
            return;
        }
        let raw = String::from_utf8_lossy(&output.stdout);
        let Some(info) = parse_lscpu(&raw) else {
            panic!("failed to parse real lscpu output");
        };
        let Some(ref arch) = info.architecture else {
            panic!("lscpu output missing Architecture field");
        };
        assert!(
            is_common_architecture(arch),
            "lscpu Architecture should be a known ISA, got {arch:?}"
        );
    }
}
