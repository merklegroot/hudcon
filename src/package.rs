//! Package manager and repository info aligned with [hudsse](https://github.com/merklegroot/hudsse)
//! `PackagePageClient`, `packageUtil`, and `detect_package_*.sh` / `.ps1`.

use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One configured repository line (matches hudsse `PackageRepository`).
#[derive(Debug, Clone, Serialize)]
pub struct PackageRepository {
    pub package_manager: String,
    pub repository: String,
}

/// Snapshot for the Package Management page (manager string, formats, repos).
#[derive(Debug, Clone, Serialize)]
pub struct PackageInfo {
    /// Comma-separated managers, or `"Unknown"`.
    pub package_manager: String,
    pub package_formats: Vec<String>,
    pub repositories: Vec<PackageRepository>,
}

pub fn gather_package_info() -> PackageInfo {
    let package_manager = detect_package_managers();
    let package_formats = parse_package_formats(&package_manager);
    let repositories = gather_repositories(&package_manager);
    PackageInfo {
        package_manager,
        package_formats,
        repositories,
    }
}

fn detect_package_managers() -> String {
    #[cfg(windows)]
    {
        return detect_package_managers_windows();
    }
    #[cfg(unix)]
    {
        return detect_package_managers_unix();
    }
    #[cfg(not(any(windows, unix)))]
    {
        "Unknown".to_string()
    }
}

#[cfg(unix)]
fn sh_command_available(cmd: &str) -> bool {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("command -v {cmd} >/dev/null 2>&1"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn detect_package_managers_unix() -> String {
    let mut managers: Vec<&'static str> = Vec::new();
    if sh_command_available("apt") {
        managers.push("APT");
    }
    if sh_command_available("dnf") {
        managers.push("DNF");
    }
    if sh_command_available("yum") {
        managers.push("YUM");
    }
    if sh_command_available("zypper") {
        managers.push("Zypper");
    }
    if sh_command_available("pacman") {
        managers.push("Pacman");
    }
    if sh_command_available("emerge") {
        managers.push("Portage");
    }
    if sh_command_available("nix-env") {
        managers.push("Nix");
    }
    if sh_command_available("brew") {
        managers.push("Homebrew");
    }
    if sh_command_available("apk") {
        managers.push("APK");
    }
    if sh_command_available("xbps-install") {
        managers.push("XBPS");
    }
    if sh_command_available("pkg") {
        managers.push("Pkg");
    }
    if sh_command_available("ports") || Path::new("/usr/ports").is_dir() {
        managers.push("Ports");
    }
    if managers.is_empty() {
        "Unknown".to_string()
    } else {
        managers.join(", ")
    }
}

#[cfg(windows)]
fn win_where(name: &str) -> bool {
    Command::new("where")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn powershell_command_available(cmd: &str) -> bool {
    let script = format!(
        "if (Get-Command {} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}",
        cmd
    );
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn detect_package_managers_windows() -> String {
    let mut managers: Vec<&'static str> = Vec::new();
    if win_where("dism") {
        managers.push("DISM");
    }
    if win_where("winget") {
        managers.push("Winget");
    }
    if powershell_command_available("Get-PackageProvider") {
        managers.push("OneGet");
    }
    if managers.is_empty() {
        "Unknown".to_string()
    } else {
        managers.join(", ")
    }
}

/// Same mapping as hudsse `packageUtil.mapPackageManagerToFormat` (per-token, uppercase).
fn map_package_manager_to_format(package_manager: &str) -> &'static str {
    match package_manager.trim().to_uppercase().as_str() {
        "APT" => "DEB",
        "DNF" | "YUM" => "RPM",
        "PACMAN" => "TAR.XZ",
        "PORTAGE" => "EBUILD",
        "NIX" => "NIX",
        "HOMEBREW" => "BOTTLE",
        "APK" => "APK",
        "XBPS" => "XBPS",
        "PKG" => "PKG",
        "PORTS" => "PORTS",
        "DISM" => "MSI",
        "WINGET" => "APPX",
        "ONEGET" => "NUGET",
        "ZYPPER" => "RPM",
        _ => "Unknown",
    }
}

fn parse_package_formats(package_manager: &str) -> Vec<String> {
    let t = package_manager.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("unknown") {
        return vec!["Unknown".to_string()];
    }
    let format_list: Vec<String> = t
        .split(',')
        .map(|m| m.trim())
        .filter(|m| !m.is_empty())
        .map(|m| map_package_manager_to_format(m).to_string())
        .collect();
    if format_list.is_empty() {
        vec!["Unknown".to_string()]
    } else {
        format_list
    }
}

fn gather_repositories(package_manager: &str) -> Vec<PackageRepository> {
    #[cfg(windows)]
    {
        return gather_repositories_windows();
    }
    #[cfg(unix)]
    {
        gather_repositories_unix(package_manager)
    }
    #[cfg(not(any(windows, unix)))]
    {
        Vec::new()
    }
}

#[cfg(unix)]
fn gather_repositories_unix(package_manager: &str) -> Vec<PackageRepository> {
    let mut repos = Vec::new();
    let pm = package_manager.to_lowercase();

    if pm.contains("apt") && sh_command_available("apt") {
        collect_apt_repos(&mut repos);
    }
    if (pm.contains("dnf") || pm.contains("yum"))
        && (sh_command_available("dnf") || sh_command_available("yum"))
    {
        collect_yum_dnf_repos(&mut repos);
    }
    if pm.contains("pacman") && sh_command_available("pacman") {
        collect_pacman_repos(&mut repos);
    }
    if pm.contains("zypper") && sh_command_available("zypper") {
        collect_zypper_repos(&mut repos);
    }
    if pm.contains("apk") && sh_command_available("apk") {
        collect_apk_repos(&mut repos);
    }
    repos
}

#[cfg(unix)]
fn collect_apt_repos(out: &mut Vec<PackageRepository>) {
    let path = Path::new("/etc/apt/sources.list");
    if path.is_file() {
        if let Ok(f) = fs::File::open(path) {
            for line in BufReader::new(f).lines().flatten() {
                let t = line.trim();
                if !t.starts_with('#') && t.contains("deb") {
                    out.push(PackageRepository {
                        package_manager: "APT".to_string(),
                        repository: line.trim().to_string(),
                    });
                }
            }
        }
    }
    let list_d = Path::new("/etc/apt/sources.list.d");
    if list_d.is_dir() {
        let mut files: Vec<PathBuf> = match fs::read_dir(list_d) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "list"))
                .collect(),
            Err(_) => Vec::new(),
        };
        files.sort();
        for p in files {
            if let Ok(f) = fs::File::open(&p) {
                for line in BufReader::new(f).lines().flatten() {
                    let t = line.trim();
                    if !t.starts_with('#') && t.contains("deb") {
                        out.push(PackageRepository {
                            package_manager: "APT".to_string(),
                            repository: line.trim().to_string(),
                        });
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
fn collect_yum_dnf_repos(out: &mut Vec<PackageRepository>) {
    let dir = Path::new("/etc/yum.repos.d");
    if !dir.is_dir() {
        return;
    }
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "repo"))
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    for p in files {
        if let Ok(s) = fs::read_to_string(&p) {
            let mut repo_name = String::new();
            for line in s.lines() {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
                    repo_name = line[1..line.len() - 1].to_string();
                } else if let Some(rest) = line.strip_prefix("baseurl=") {
                    let baseurl = rest.trim();
                    if !repo_name.is_empty() && !baseurl.is_empty() {
                        out.push(PackageRepository {
                            package_manager: "DNF/YUM".to_string(),
                            repository: format!("[{repo_name}] {baseurl}"),
                        });
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
fn collect_pacman_repos(out: &mut Vec<PackageRepository>) {
    let conf = Path::new("/etc/pacman.conf");
    if conf.is_file() {
        if let Ok(s) = fs::read_to_string(conf) {
            let mut current_section = String::new();
            for line in s.lines() {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
                    current_section = line[1..line.len() - 1].to_string();
                    if current_section != "options" {
                        out.push(PackageRepository {
                            package_manager: "Pacman".to_string(),
                            repository: format!("[{}]", current_section),
                        });
                    }
                } else if line.starts_with("Server=") && current_section != "options" {
                    out.push(PackageRepository {
                        package_manager: "Pacman".to_string(),
                        repository: line.to_string(),
                    });
                }
            }
        }
    }
    let pacman_d = Path::new("/etc/pacman.d");
    if pacman_d.is_dir() {
        let mut files: Vec<PathBuf> = match fs::read_dir(pacman_d) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "conf"))
                .collect(),
            Err(_) => Vec::new(),
        };
        files.sort();
        for p in files {
            if let Ok(s) = fs::read_to_string(&p) {
                for line in s.lines() {
                    let line = line.trim();
                    if line.starts_with("Server=") {
                        out.push(PackageRepository {
                            package_manager: "Pacman".to_string(),
                            repository: line.to_string(),
                        });
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
fn collect_zypper_repos(out: &mut Vec<PackageRepository>) {
    let Ok(output) = Command::new("zypper")
        .args(["repos"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            out.push(PackageRepository {
                package_manager: "Zypper".to_string(),
                repository: line.to_string(),
            });
        }
    }
}

#[cfg(unix)]
fn collect_apk_repos(out: &mut Vec<PackageRepository>) {
    let path = Path::new("/etc/apk/repositories");
    if !path.is_file() {
        return;
    }
    if let Ok(f) = fs::File::open(path) {
        for line in BufReader::new(f).lines().flatten() {
            let t = line.trim();
            if !t.is_empty() && !t.starts_with('#') {
                out.push(PackageRepository {
                    package_manager: "APK".to_string(),
                    repository: t.to_string(),
                });
            }
        }
    }
}

#[cfg(windows)]
fn gather_repositories_windows() -> Vec<PackageRepository> {
    let mut repos = Vec::new();
    if win_where("winget") {
        if let Ok(output) = Command::new("winget")
            .args(["source", "list"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let t = line.trim();
                    if t.is_empty() {
                        continue;
                    }
                    let parts: Vec<&str> = t.split_whitespace().collect();
                    if parts.len() >= 3 {
                        repos.push(PackageRepository {
                            package_manager: "Winget".to_string(),
                            repository: t.to_string(),
                        });
                    }
                }
            }
        }
    }
    if powershell_command_available("Get-PackageSource") {
        let script = r#"
            $ErrorActionPreference = 'SilentlyContinue'
            Get-PackageSource | ForEach-Object {
                Write-Output ("OneGet: " + $_.Name + " - " + $_.Location)
            }
        "#;
        if let Ok(output) = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let t = line.trim();
                    if t.is_empty() {
                        continue;
                    }
                    push_repo_first_colon(&mut repos, t);
                }
            }
        }
    }
    repos
}

/// Same split as hudsse `parsePackageRepositories`: `Manager: rest`.
#[cfg(windows)]
fn push_repo_first_colon(out: &mut Vec<PackageRepository>, line: &str) {
    let Some(idx) = line.find(':') else {
        return;
    };
    let pm = line[..idx].trim();
    let repo = line[idx + 1..].trim();
    if pm.is_empty() || repo.is_empty() {
        return;
    }
    out.push(PackageRepository {
        package_manager: pm.to_string(),
        repository: repo.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_match_hudsse_util() {
        assert_eq!(map_package_manager_to_format("APT"), "DEB");
        assert_eq!(map_package_manager_to_format("dnf"), "RPM");
        assert_eq!(map_package_manager_to_format("Winget"), "APPX");
        let f = parse_package_formats("APT, DNF");
        assert_eq!(f, vec!["DEB", "RPM"]);
        let u = parse_package_formats("Unknown");
        assert_eq!(u, vec!["Unknown"]);
    }
}
