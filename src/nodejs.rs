//! Node.js / npm detection (versions on PATH), same style as [`crate::dotnet`].

use serde::Serialize;
use std::io::ErrorKind;
use std::process::{Command, Stdio};

/// Snapshot for the Node.js tab (`node --version`, `npm --version`).
#[derive(Debug, Clone, Serialize)]
pub struct NodeJsBasicInfo {
    /// Trimmed stdout of `node --version`.
    pub node_version: Option<String>,
    /// Trimmed stdout of `npm --version`.
    pub npm_version: Option<String>,
}

pub fn gather_nodejs_basic_info() -> NodeJsBasicInfo {
    NodeJsBasicInfo {
        node_version: run_tool_version("node").ok(),
        npm_version: run_tool_version("npm").ok(),
    }
}

fn run_tool_version(cmd: &str) -> Result<String, String> {
    let output = Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Err(e) if e.kind() == ErrorKind::NotFound => Err(format!("{cmd} not found")),
        Err(e) => Err(e.to_string()),
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if v.is_empty() {
                Err(format!("empty output from {cmd} --version"))
            } else {
                Ok(v)
            }
        }
        Ok(o) => {
            let mut msg = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if msg.is_empty() {
                msg = format!("{cmd} --version exited with {}", o.status);
            }
            if msg.len() > 200 {
                msg.truncate(200);
                msg.push('…');
            }
            Err(msg)
        }
    }
}
