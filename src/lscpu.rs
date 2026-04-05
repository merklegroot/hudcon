//! Parse `lscpu` output to match [hudsse](https://github.com/merklegroot/hudsse) `parseCpuInfo` (Linux).

use std::collections::HashSet;

use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct CpuFeatures {
    pub sse: bool,
    pub sse2: bool,
    pub sse3: bool,
    pub ssse3: bool,
    pub sse4_1: bool,
    pub sse4_2: bool,
    pub avx: bool,
    pub avx2: bool,
    pub avx512: bool,
    pub fma: bool,
    pub aes: bool,
    pub sha: bool,
    pub neon: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct LscpuInfo {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub cpu_cores: Option<u32>,
    pub architecture: Option<String>,
    pub cpu_mhz: Option<u32>,
    pub threads_per_core: Option<u32>,
    pub cores_per_socket: Option<u32>,
    pub sockets: Option<u32>,
    pub virtualization: Option<String>,
    pub l1d_kb: Option<u64>,
    pub l1i_kb: Option<u64>,
    pub l2_kb: Option<u64>,
    pub l3_kb: Option<u64>,
    pub features: CpuFeatures,
}

fn parse_vendor(vendor_id: &str) -> String {
    let u = vendor_id.to_uppercase();
    if u.contains("AUTHENTICAMD") || u.contains("AMD") {
        return "AMD".to_string();
    }
    if u.contains("GENUINEINTEL") || u.contains("INTEL") {
        return "Intel".to_string();
    }
    if u.contains("ARM") {
        return "ARM".to_string();
    }
    vendor_id.to_string()
}

fn parse_cache_size_kb(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    let size_part = trimmed.split('(').next()?.trim();
    let mut parts = size_part.split_whitespace();
    let size: f64 = parts.next()?.parse().ok()?;
    let unit = parts.next()?.to_uppercase();
    let kb = match unit.as_str() {
        "K" | "KIB" | "KB" => size,
        "M" | "MIB" | "MB" => size * 1024.0,
        "G" | "GIB" | "GB" => size * 1024.0 * 1024.0,
        _ => return None,
    };
    Some(kb.round() as u64)
}

fn parse_cpu_features(flags: &str) -> CpuFeatures {
    let set: HashSet<String> = flags.split_whitespace().map(|w| w.to_uppercase()).collect();
    let has = |s: &str| set.contains(s);
    CpuFeatures {
        sse: has("SSE"),
        sse2: has("SSE2"),
        sse3: has("SSE3"),
        ssse3: has("SSSE3"),
        sse4_1: has("SSE4_1") || has("SSE4.1"),
        sse4_2: has("SSE4_2") || has("SSE4.2"),
        avx: has("AVX"),
        avx2: has("AVX2"),
        avx512: has("AVX512F") || has("AVX512"),
        fma: has("FMA"),
        aes: has("AES"),
        sha: has("SHA_NI") || has("SHA-NI"),
        neon: has("NEON"),
    }
}

/// Parse `lscpu` stdout (same keys as hudsse `parseCpuInfo`).
pub fn parse_lscpu(output: &str) -> Option<LscpuInfo> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lines: Vec<&str> = trimmed.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return None;
    }
    if !lines.iter().any(|l| l.contains(':')) {
        return None;
    }

    let mut info = LscpuInfo::default();
    let mut flags_line: Option<String> = None;

    for line in lines {
        let Some(colon) = line.find(':') else {
            continue;
        };
        let key = line[..colon].trim();
        let value = line[colon + 1..].trim();
        let upper_key = key.to_uppercase();

        match upper_key.as_str() {
            "VENDOR ID" | "VENDOR" => {
                info.vendor = Some(parse_vendor(value));
            }
            "MODEL NAME" => {
                info.model = Some(value.to_string());
            }
            "CPU(S)" => {
                if let Ok(n) = value.parse::<u32>() {
                    if n > 0 {
                        info.cpu_cores = Some(n);
                    }
                }
            }
            "ARCHITECTURE" => {
                info.architecture = Some(value.to_string());
            }
            "CPU MHZ" | "CPU MAX MHZ" => {
                if let Ok(m) = value.parse::<f64>() {
                    if m > 0.0 {
                        info.cpu_mhz = Some(m.round() as u32);
                    }
                }
            }
            "THREAD(S) PER CORE" => {
                if let Ok(n) = value.parse::<u32>() {
                    if n > 0 {
                        info.threads_per_core = Some(n);
                    }
                }
            }
            "CORE(S) PER SOCKET" => {
                if let Ok(n) = value.parse::<u32>() {
                    if n > 0 {
                        info.cores_per_socket = Some(n);
                    }
                }
            }
            "SOCKET(S)" => {
                if let Ok(n) = value.parse::<u32>() {
                    if n > 0 {
                        info.sockets = Some(n);
                    }
                }
            }
            "VIRTUALIZATION" => {
                info.virtualization = Some(value.to_string());
            }
            "FLAGS" => {
                flags_line = Some(value.to_string());
            }
            _ => {}
        }

        if upper_key.contains("L1D") && upper_key.contains("CACHE") {
            if let Some(kb) = parse_cache_size_kb(value) {
                info.l1d_kb = Some(kb);
            }
        }
        if upper_key.contains("L1I") && upper_key.contains("CACHE") {
            if let Some(kb) = parse_cache_size_kb(value) {
                info.l1i_kb = Some(kb);
            }
        }
        if upper_key.contains("L2") && upper_key.contains("CACHE") && !upper_key.contains("L1") {
            if let Some(kb) = parse_cache_size_kb(value) {
                info.l2_kb = Some(kb);
            }
        }
        if upper_key.contains("L3") && upper_key.contains("CACHE") {
            if let Some(kb) = parse_cache_size_kb(value) {
                info.l3_kb = Some(kb);
            }
        }
    }

    if let Some(f) = flags_line {
        info.features = parse_cpu_features(&f);
    }

    if info.model.is_none() {
        return None;
    }
    Some(info)
}

pub fn format_cache_kb(kb: u64) -> String {
    if kb < 1024 {
        format!("{kb} KB")
    } else if kb < 1024 * 1024 {
        format!("{:.1} MB", kb as f64 / 1024.0)
    } else {
        format!("{:.1} GB", kb as f64 / (1024.0 * 1024.0))
    }
}
