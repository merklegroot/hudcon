//! Basic .NET CLI detection and optional install via Microsoft’s official scripts / winget (similar to hudsse).

use serde::Serialize;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const INSTALL_SCRIPT_URL_SH: &str = "https://dot.net/v1/dotnet-install.sh";

/// Outcome of [`install_dotnet_sdk`] (may include long installer log text, truncated for IPC).
#[derive(Debug, Clone, Serialize)]
pub struct DotNetInstallResult {
    pub success: bool,
    pub message: String,
    /// How to put `dotnet` on PATH after a user-local install (Unix `~/.dotnet`, etc.).
    pub path_hint: Option<String>,
    /// True when user PATH / shell profile was updated (or this process env was aligned).
    pub path_configured: bool,
}

/// Result of [`add_dotnet_user_install_to_path`].
#[derive(Debug, Clone, Serialize)]
pub struct DotNetPathConfigureResult {
    pub success: bool,
    pub message: String,
    pub path_configured: bool,
}

/// Minimal snapshot for a .NET page (install status, path, `dotnet --version`).
#[derive(Debug, Clone, Serialize)]
pub struct DotNetBasicInfo {
    /// True when `dotnet --version` succeeds or an executable path was resolved.
    pub installed: bool,
    /// `command -v dotnet` / `where`, or `~/.dotnet/dotnet` when that binary exists.
    pub executable_path: Option<String>,
    /// Trimmed stdout of `dotnet --version` when it succeeds.
    pub sdk_version: Option<String>,
    /// When not usable: short reason (e.g. not on PATH, or non-zero exit).
    pub last_error: Option<String>,
    /// SDK was resolved from the user install dir (`~/.dotnet`) but `dotnet` is not on PATH.
    pub path_note: Option<String>,
}

pub fn gather_dotnet_basic_info() -> DotNetBasicInfo {
    let which_path = dotnet_on_path_executable();
    let user_cli = user_local_dotnet_cli();

    let mut sdk_version: Option<String> = None;
    let mut last_error: Option<String> = None;
    let mut version_via_user_install = false;

    match run_dotnet_version(Command::new("dotnet")) {
        Ok(v) => sdk_version = Some(v),
        Err(e_path) => {
            if let Some(ref p) = user_cli {
                match run_dotnet_version(Command::new(p)) {
                    Ok(v) => {
                        sdk_version = Some(v);
                        version_via_user_install = true;
                    }
                    Err(e_user) => {
                        last_error = Some(format!(
                            "{} Also tried {}: {}",
                            e_path,
                            p.display(),
                            e_user
                        ));
                    }
                }
            } else {
                last_error = Some(e_path);
            }
        }
    }

    let executable_path = which_path
        .clone()
        .or_else(|| user_cli.as_ref().map(|p| p.display().to_string()));

    let path_note = if version_via_user_install && which_path.is_none() {
        Some(path_note_add_dotnet_to_shell_path())
    } else {
        None
    };

    let installed =
        sdk_version.is_some() || which_path.is_some() || user_cli.is_some();

    let sdk_missing = sdk_version.is_none();

    DotNetBasicInfo {
        installed,
        executable_path,
        sdk_version,
        last_error: last_error.filter(|_| sdk_missing),
        path_note,
    }
}

/// `dotnet` as resolved by the shell / `where` (PATH only).
fn dotnet_on_path_executable() -> Option<String> {
    #[cfg(windows)]
    {
        let output = Command::new("where.exe")
            .arg("dotnet")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(str::to_string)
    }
    #[cfg(unix)]
    {
        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg("command -v dotnet")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!line.is_empty()).then_some(line)
    }
    #[cfg(not(any(windows, unix)))]
    {
        None
    }
}

/// Microsoft script default: `~/.dotnet/dotnet` (Unix) or `%USERPROFILE%\.dotnet\dotnet.exe`.
fn user_local_dotnet_cli() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        let dir = dotnet_home_install_dir().ok()?;
        let p = dir.join("dotnet");
        p.is_file().then_some(p)
    }
    #[cfg(windows)]
    {
        let u = std::env::var_os("USERPROFILE")?;
        let p = PathBuf::from(u).join(".dotnet").join("dotnet.exe");
        p.is_file().then_some(p)
    }
    #[cfg(not(any(windows, unix)))]
    {
        None
    }
}

fn run_dotnet_version(mut cmd: Command) -> Result<String, String> {
    let output = cmd
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Err(e) if e.kind() == ErrorKind::NotFound => Err("dotnet not found".to_string()),
        Err(e) => Err(e.to_string()),
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if v.is_empty() {
                Err("empty output from dotnet --version".to_string())
            } else {
                Ok(v)
            }
        }
        Ok(o) => {
            let mut msg = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if msg.is_empty() {
                msg = format!("dotnet --version exited with {}", o.status);
            }
            if msg.len() > 200 {
                msg.truncate(200);
                msg.push('…');
            }
            Err(msg)
        }
    }
}

fn path_note_add_dotnet_to_shell_path() -> String {
    if cfg!(target_os = "macos") {
        "dotnet is installed under ~/.dotnet but is not on PATH for this app or your terminal. Add to ~/.zprofile or ~/.bash_profile: export DOTNET_ROOT=\"$HOME/.dotnet\" && export PATH=\"$PATH:$HOME/.dotnet\", then open a new terminal or restart HUDcon.".into()
    } else if cfg!(target_os = "windows") {
        "dotnet is under %USERPROFILE%\\.dotnet but not on PATH. Add that folder to your user PATH or restart the terminal.".into()
    } else {
        "dotnet is under ~/.dotnet but not on PATH. Add to your shell profile: export DOTNET_ROOT=\"$HOME/.dotnet\" && export PATH=\"$PATH:$HOME/.dotnet\".".into()
    }
}

fn channel_string(major: u32) -> Result<String, String> {
    match major {
        6..=10 => Ok(format!("{major}.0")),
        _ => Err(format!("Unsupported .NET major version {major} (use 6–10).")),
    }
}

fn truncate_message(s: &str, max_chars: usize) -> String {
    let count: Vec<char> = s.chars().collect();
    if count.len() <= max_chars {
        return s.to_string();
    }
    let tail: String = count[count.len().saturating_sub(max_chars)..].iter().collect();
    format!("…(truncated, last {max_chars} characters)\n{tail}")
}

fn combine_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut m = String::new();
    let a = String::from_utf8_lossy(stdout);
    let b = String::from_utf8_lossy(stderr);
    if !a.trim().is_empty() {
        m.push_str(&a);
        if !a.ends_with('\n') {
            m.push('\n');
        }
    }
    if !b.trim().is_empty() {
        m.push_str(&b);
    }
    m.trim().to_string()
}

/// `DOTNET_ROOT` + append install dir to `PATH` for **this process** so later `dotnet` spawns work without restart.
fn apply_dotnet_env_current_process(dotnet_root: &Path) {
    let root = dotnet_root.to_string_lossy().to_string();
    std::env::set_var("DOTNET_ROOT", &root);
    let sep = if cfg!(windows) { ';' } else { ':' };
    let key = if cfg!(windows) { "Path" } else { "PATH" };
    let current = std::env::var(key).unwrap_or_default();
    let root_cmp = root.to_lowercase();
    let on_path = if cfg!(windows) {
        current
            .split(';')
            .map(|s| s.trim().to_lowercase())
            .any(|p| p == root_cmp)
    } else {
        current
            .split(':')
            .map(str::trim)
            .any(|p| p == root.as_str())
    };
    if on_path {
        return;
    }
    let new_path = if current.is_empty() {
        root.clone()
    } else {
        format!("{current}{sep}{root}")
    };
    std::env::set_var(key, &new_path);
}

#[cfg(unix)]
const HUDCON_DOTNET_PROFILE_MARKER: &str = "# >>> hudcon dotnet >>>";

#[cfg(unix)]
fn unix_dotnet_profile_snippet() -> &'static str {
    "# >>> hudcon dotnet >>>\n\
     export DOTNET_ROOT=\"$HOME/.dotnet\"\n\
     export PATH=\"$PATH:$HOME/.dotnet\"\n\
     # <<< hudcon dotnet <<<\n"
}

#[cfg(unix)]
fn unix_shell_profile_targets(home: &Path) -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        let mut v = vec![home.join(".zprofile")];
        let bp = home.join(".bash_profile");
        if bp.is_file() {
            v.push(bp);
        }
        v
    } else {
        let mut v = vec![home.join(".profile")];
        let z = home.join(".zprofile");
        let shell = std::env::var("SHELL").unwrap_or_default();
        if z.is_file() || shell.contains("zsh") {
            v.push(z);
        }
        v
    }
}

#[cfg(unix)]
fn append_hudcon_dotnet_block(path: &Path) -> std::io::Result<bool> {
    use std::io::Write;
    let mut existing = String::new();
    if path.is_file() {
        std::fs::read_to_string(path).map(|s| existing = s)?;
        if existing.contains(HUDCON_DOTNET_PROFILE_MARKER) {
            return Ok(false);
        }
    }
    let snippet = unix_dotnet_profile_snippet();
    let needs_newline = !existing.is_empty() && !existing.ends_with('\n');
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    if needs_newline {
        writeln!(out)?;
    }
    out.write_all(snippet.as_bytes())?;
    Ok(true)
}

/// Writes an idempotent `export` block so **new** login shells pick up `dotnet` (hudsse-style login-shell PATH).
#[cfg(unix)]
fn persist_unix_dotnet_profile_snippets(home: &Path) -> Result<Vec<String>, String> {
    let mut touched = Vec::new();
    for p in unix_shell_profile_targets(home) {
        match append_hudcon_dotnet_block(&p) {
            Ok(true) => touched.push(p.display().to_string()),
            Ok(false) => {}
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
    }
    Ok(touched)
}

#[cfg(windows)]
fn persist_windows_user_dotnet_env(dotnet_root: &Path) -> Result<(), String> {
    let root = dotnet_root.to_string_lossy().replace('\'', "''");
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
$dotnet = '{root}'
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($null -eq $userPath) {{ $userPath = '' }}
$parts = $userPath -split ';' | ForEach-Object {{ $_.Trim() }} | Where-Object {{ $_ -ne '' }}
$already = $false
foreach ($p in $parts) {{
  if ($p -ieq $dotnet) {{ $already = $true; break }}
}}
if (-not $already) {{
  $newPath = if ($parts.Count -eq 0) {{ $dotnet }} else {{ ($parts -join ';') + ';' + $dotnet }}
  [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
}}
[Environment]::SetEnvironmentVariable('DOTNET_ROOT', $dotnet, 'User')
"#
    );
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(combine_output(&out.stdout, &out.stderr))
    }
}

#[cfg(windows)]
fn configure_windows_dotnet_path_after_install() -> (bool, Option<String>) {
    let user_root = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let user_dotnet = user_root
        .as_ref()
        .map(|h| h.join(".dotnet"))
        .filter(|p| p.join("dotnet.exe").is_file());
    let program_files = Path::new(r"C:\Program Files\dotnet");
    let pf_ok = program_files.join("dotnet.exe").is_file();

    if let Some(root) = user_dotnet {
        apply_dotnet_env_current_process(&root);
        match persist_windows_user_dotnet_env(&root) {
            Ok(()) => (
                true,
                Some(
                    "Added %USERPROFILE%\\.dotnet to your user PATH and set DOTNET_ROOT. This app’s process PATH is updated too."
                        .into(),
                ),
            ),
            Err(e) => (
                true,
                Some(format!(
                    "Could not update user PATH in the registry ({e}). This session’s PATH was still updated."
                )),
            ),
        }
    } else if pf_ok {
        apply_dotnet_env_current_process(program_files);
        (
            true,
            Some(
                "Using .NET under Program Files; DOTNET_ROOT and PATH updated for this app. Open a new terminal if other apps still miss `dotnet`."
                    .into(),
            ),
        )
    } else {
        (
            false,
            Some(
                "Install reported success but dotnet.exe was not found under %USERPROFILE%\\.dotnet or C:\\Program Files\\dotnet. Try a new terminal or sign out and back in."
                    .into(),
            ),
        )
    }
}

/// Updates PATH / shell profile when `dotnet` exists under the user install directory (`~/.dotnet`, etc.) but is not on PATH.
#[cfg(any(windows, unix))]
pub fn add_dotnet_user_install_to_path() -> DotNetPathConfigureResult {
    if dotnet_on_path_executable().is_some() {
        return DotNetPathConfigureResult {
            success: true,
            message: "dotnet is already on your PATH.".into(),
            path_configured: false,
        };
    }

    let Some(exe) = user_local_dotnet_cli() else {
        return DotNetPathConfigureResult {
            success: false,
            message: "No dotnet executable found under the default user install location (~/.dotnet or %USERPROFILE%\\.dotnet)."
                .into(),
            path_configured: false,
        };
    };

    let Some(dotnet_root) = exe.parent().map(|p| p.to_path_buf()) else {
        return DotNetPathConfigureResult {
            success: false,
            message: "Could not resolve .NET install directory.".into(),
            path_configured: false,
        };
    };

    apply_dotnet_env_current_process(&dotnet_root);

    #[cfg(unix)]
    {
        let message = match dotnet_root.parent() {
            Some(home) => match persist_unix_dotnet_profile_snippets(home) {
                Ok(touched) if touched.is_empty() => {
                    "A `.NET` PATH block was already in your shell profile. This app’s process PATH is updated."
                        .into()
                }
                Ok(touched) => format!(
                    "Added `.NET` to {}. New login shells will have `dotnet` on PATH; this app is updated too.",
                    touched.join(", ")
                ),
                Err(e) => format!(
                    "Could not write your shell profile ({e}). {}",
                    unix_path_hint_after_install()
                ),
            },
            None => "Updated PATH for this app only (could not resolve home for shell profile).".into(),
        };
        return DotNetPathConfigureResult {
            success: true,
            message,
            path_configured: true,
        };
    }

    #[cfg(windows)]
    {
        let message = match persist_windows_user_dotnet_env(&dotnet_root) {
            Ok(()) => "Added your user .dotnet folder to PATH and set DOTNET_ROOT. This app’s process was updated too."
                .into(),
            Err(e) => format!(
                "Could not update user PATH in the registry ({e}). This session’s PATH was still updated."
            ),
        };
        return DotNetPathConfigureResult {
            success: true,
            message,
            path_configured: true,
        };
    }
}

#[cfg(not(any(windows, unix)))]
pub fn add_dotnet_user_install_to_path() -> DotNetPathConfigureResult {
    DotNetPathConfigureResult {
        success: false,
        message: "Not supported on this platform.".into(),
        path_configured: false,
    }
}

/// Downloads and runs Microsoft’s `dotnet-install` script (Unix) or uses `winget` / PowerShell (Windows).
/// Installs the SDK under `~/.dotnet` (Unix) or `%USERPROFILE%\.dotnet` (script path on Windows).
/// Supported majors: **6–10** (`--channel` / winget id `Microsoft.DotNet.SDK.{major}`).
pub fn install_dotnet_sdk(major_version: u32) -> DotNetInstallResult {
    match channel_string(major_version) {
        Ok(ch) => {
            #[cfg(windows)]
            {
                install_windows_sdk(major_version, &ch)
            }
            #[cfg(unix)]
            {
                install_unix_sdk(&ch)
            }
            #[cfg(not(any(windows, unix)))]
            {
                DotNetInstallResult {
                    success: false,
                    message: "This platform is not supported for automated .NET install.".into(),
                    path_hint: None,
                    path_configured: false,
                }
            }
        }
        Err(e) => DotNetInstallResult {
            success: false,
            message: e,
            path_hint: None,
            path_configured: false,
        },
    }
}

#[cfg(unix)]
fn dotnet_home_install_dir() -> Result<PathBuf, String> {
    if let Ok(h) = std::env::var("HOME") {
        if !h.trim().is_empty() {
            return Ok(PathBuf::from(h).join(".dotnet"));
        }
    }
    // GUI apps on macOS sometimes omit HOME; USER + /Users is the usual layout.
    #[cfg(target_os = "macos")]
    {
        if let Ok(u) = std::env::var("USER") {
            if !u.trim().is_empty() {
                return Ok(PathBuf::from("/Users").join(u).join(".dotnet"));
            }
        }
    }
    Err("Could not resolve home directory (HOME unset). Cannot install to ~/.dotnet.".into())
}

/// PATH for subprocesses: macOS `.app` / Tauri often inherit a minimal PATH, but `dotnet-install.sh`
/// shells out to `curl`, `tar`, etc. — they must stay on PATH.
#[cfg(unix)]
fn enriched_unix_path() -> String {
    let extra = if cfg!(target_os = "macos") {
        "/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/usr/local/bin"
    } else {
        "/usr/local/bin:/usr/bin:/bin:/sbin"
    };
    match std::env::var("PATH") {
        Ok(p) if !p.trim().is_empty() => format!("{extra}:{p}"),
        _ => extra.to_string(),
    }
}

#[cfg(unix)]
fn unix_curl_bin() -> &'static str {
    if cfg!(target_os = "macos") {
        "/usr/bin/curl"
    } else {
        "curl"
    }
}

#[cfg(unix)]
fn unix_bash_bin() -> &'static str {
    if Path::new("/bin/bash").is_file() {
        "/bin/bash"
    } else {
        "bash"
    }
}

#[cfg(unix)]
fn unix_chmod_bin() -> &'static str {
    if Path::new("/bin/chmod").is_file() {
        "/bin/chmod"
    } else {
        "chmod"
    }
}

/// Runtime machine type (not `cfg!(target_arch)`), so Rosetta / universal builds still get the right SDK.
#[cfg(unix)]
fn unix_uname_machine() -> Option<String> {
    for bin in ["/usr/bin/uname", "/bin/uname"] {
        if !Path::new(bin).is_file() {
            continue;
        }
        let out = Command::new(bin)
            .arg("-m")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            continue;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
        if !s.is_empty() {
            return Some(s);
        }
    }
    let out = Command::new("uname")
        .arg("-m")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("PATH", enriched_unix_path())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
    (!s.is_empty()).then_some(s)
}

#[cfg(unix)]
fn host_dotnet_arch_args() -> Vec<&'static str> {
    let Some(m) = unix_uname_machine() else {
        return Vec::new();
    };
    match m.as_str() {
        "arm64" | "aarch64" => vec!["--architecture", "arm64"],
        "x86_64" | "amd64" => vec!["--architecture", "x64"],
        _ => Vec::new(),
    }
}

#[cfg(unix)]
fn download_to_path(url: &str, dest: &Path) -> Result<(), String> {
    let curl = unix_curl_bin();
    let r = Command::new(curl)
        .args(["-fSL", url, "-o"])
        .arg(dest)
        .env("PATH", enriched_unix_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match r {
        Ok(o) if o.status.success() => return Ok(()),
        Ok(o) => {
            let err = combine_output(&o.stdout, &o.stderr);
            if !err.is_empty() {
                return Err(format!("curl failed (exit {}): {}", o.status, err.trim()));
            }
            return Err(format!("curl failed with exit {}", o.status));
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(format!("curl: {e}")),
    }

    let w = Command::new("wget")
        .args(["-q", "-O"])
        .arg(dest)
        .arg(url)
        .env("PATH", enriched_unix_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match w {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => {
            let err = combine_output(&o.stdout, &o.stderr);
            let detail = err.trim();
            Err(format!(
                "wget failed (exit {}): {}",
                o.status,
                if detail.is_empty() {
                    "(no output)"
                } else {
                    detail
                }
            ))
        }
        Err(e) => Err(format!(
            "Could not download the install script (curl at {curl} and wget both failed: {e})."
        )),
    }
}

#[cfg(unix)]
fn unix_path_hint_after_install() -> String {
    if cfg!(target_os = "macos") {
        "Add to ~/.zprofile or ~/.bash_profile: export DOTNET_ROOT=\"$HOME/.dotnet\" && export PATH=\"$PATH:$HOME/.dotnet\". Open a new terminal (or restart this app) so PATH updates.".into()
    } else {
        "Add to your shell profile: export DOTNET_ROOT=\"$HOME/.dotnet\" && export PATH=\"$PATH:$HOME/.dotnet\"".into()
    }
}

#[cfg(unix)]
fn install_unix_sdk(channel: &str) -> DotNetInstallResult {
    let install_dir = match dotnet_home_install_dir() {
        Ok(p) => p,
        Err(e) => {
            return DotNetInstallResult {
                success: false,
                message: e,
                path_hint: None,
                path_configured: false,
            }
        }
    };

    let tmp = std::env::temp_dir().join(format!("hudcon-dotnet-install-{}.sh", std::process::id()));
    if let Err(e) = download_to_path(INSTALL_SCRIPT_URL_SH, &tmp) {
        return DotNetInstallResult {
            success: false,
            message: e,
            path_hint: None,
            path_configured: false,
        };
    }

    let _ = Command::new(unix_chmod_bin())
        .arg("+x")
        .arg(&tmp)
        .env("PATH", enriched_unix_path())
        .status();

    let arch = host_dotnet_arch_args();
    let mut cmd = Command::new(unix_bash_bin());
    cmd.arg(&tmp)
        .arg("--install-dir")
        .arg(&install_dir)
        .arg("--channel")
        .arg(channel);
    if arch.len() == 2 {
        cmd.arg(arch[0]).arg(arch[1]);
    }
    cmd.arg("--verbose")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PATH", enriched_unix_path());
    #[cfg(target_os = "macos")]
    {
        if std::env::var_os("HOME")
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            if let Ok(u) = std::env::var("USER") {
                if !u.trim().is_empty() {
                    cmd.env("HOME", format!("/Users/{u}"));
                }
            }
        }
    }

    let out = cmd.output();

    let _ = std::fs::remove_file(&tmp);

    match out {
        Err(e) => DotNetInstallResult {
            success: false,
            message: format!("Failed to run install script: {e}"),
            path_hint: None,
            path_configured: false,
        },
        Ok(o) => {
            let success = o.status.success();
            let raw = combine_output(&o.stdout, &o.stderr);
            let message = truncate_message(&raw, 12_000);
            let (path_configured, path_hint) = if success {
                apply_dotnet_env_current_process(&install_dir);
                let hint = match install_dir.parent() {
                    Some(home) => match persist_unix_dotnet_profile_snippets(home) {
                        Ok(touched) if touched.is_empty() => Some(
                            "`.NET` PATH block was already in your shell profile. This app’s process PATH is updated."
                                .into(),
                        ),
                        Ok(touched) => Some(format!(
                            "Added `.NET` to {} (new terminals / login shells will have `dotnet` on PATH). This app’s process PATH is updated.",
                            touched.join(", ")
                        )),
                        Err(e) => Some(format!(
                            "Could not write your shell profile ({e}). {}",
                            unix_path_hint_after_install()
                        )),
                    },
                    None => None,
                };
                (true, hint)
            } else {
                (false, None)
            };
            DotNetInstallResult {
                success,
                message: if message.is_empty() {
                    if success {
                        "Install script finished.".into()
                    } else {
                        format!("Install script exited with {}", o.status)
                    }
                } else {
                    message
                },
                path_hint,
                path_configured,
            }
        }
    }
}

#[cfg(windows)]
fn install_windows_sdk(major: u32, _channel: &str) -> DotNetInstallResult {
    let winget_id = format!("Microsoft.DotNet.SDK.{major}");
    let w = Command::new("winget")
        .args([
            "install",
            "--id",
            &winget_id,
            "-e",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match w {
        Ok(o) if o.status.success() => {
            let (path_configured, path_hint) = configure_windows_dotnet_path_after_install();
            DotNetInstallResult {
                success: true,
                message: truncate_message(&combine_output(&o.stdout, &o.stderr), 8000),
                path_hint,
                path_configured,
            }
        }
        Ok(o) => {
            let combined = combine_output(&o.stdout, &o.stderr);
            let ps1 = install_windows_via_powershell_script(major);
            if ps1.success {
                return ps1;
            }
            DotNetInstallResult {
                success: false,
                message: format!(
                    "winget install {} failed (exit {}). {}",
                    winget_id,
                    o.status,
                    truncate_message(&combined, 4000)
                ),
                path_hint: ps1.path_hint,
                path_configured: ps1.path_configured,
            }
        }
        Err(_) => install_windows_via_powershell_script(major),
    }
}

#[cfg(windows)]
fn install_windows_via_powershell_script(major: u32) -> DotNetInstallResult {
    const INSTALL_SCRIPT_URL_PS1: &str = "https://dot.net/v1/dotnet-install.ps1";
    let channel = format!("{major}.0");
    let script = format!(
        r#"$ProgressPreference='SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$p = Join-Path $env:TEMP 'hudcon-dotnet-install.ps1'
Invoke-WebRequest -Uri '{}' -OutFile $p -UseBasicParsing
& $p -Channel '{}' -InstallDir (Join-Path $env:USERPROFILE '.dotnet')"#,
        INSTALL_SCRIPT_URL_PS1, channel
    );

    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match out {
        Err(e) => DotNetInstallResult {
            success: false,
            message: format!("Could not run winget or PowerShell install: {e}"),
            path_hint: None,
            path_configured: false,
        },
        Ok(o) => {
            let success = o.status.success();
            let msg = truncate_message(&combine_output(&o.stdout, &o.stderr), 12_000);
            let (path_configured, path_hint) = if success {
                configure_windows_dotnet_path_after_install()
            } else {
                (false, None)
            };
            DotNetInstallResult {
                success,
                message: if msg.is_empty() {
                    if success {
                        "PowerShell install script finished.".into()
                    } else {
                        format!("PowerShell install exited with {}", o.status)
                    }
                } else {
                    msg
                },
                path_hint,
                path_configured,
            }
        }
    }
}
