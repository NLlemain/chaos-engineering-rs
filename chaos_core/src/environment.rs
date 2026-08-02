use crate::{ChaosError, Result};
use std::path::Path;

pub fn command_available(command: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };

    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .map(str::to_ascii_lowercase)
            .collect()
    } else {
        vec![String::new()]
    };

    std::env::split_paths(&path).any(|directory| {
        extensions.iter().any(|extension| {
            let candidate = if extension.is_empty() || Path::new(command).extension().is_some() {
                directory.join(command)
            } else {
                directory.join(format!("{}{}", command, extension))
            };
            candidate.is_file()
        })
    })
}

pub fn require_command(command: &str) -> Result<()> {
    if command_available(command) {
        Ok(())
    } else {
        Err(ChaosError::SystemError(format!(
            "Required command '{}' was not found on PATH",
            command
        )))
    }
}

pub async fn require_elevated_privileges() -> Result<()> {
    if is_elevated().await {
        Ok(())
    } else {
        Err(ChaosError::PermissionDenied(
            "This injector requires administrator/root privileges".to_string(),
        ))
    }
}

#[cfg(unix)]
async fn is_elevated() -> bool {
    // SAFETY: geteuid has no preconditions and does not mutate process state.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(windows)]
async fn is_elevated() -> bool {
    let script = "([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)";
    tokio::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .await
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

#[cfg(not(any(unix, windows)))]
async fn is_elevated() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_command_from_the_current_rust_toolchain() {
        assert!(command_available(if cfg!(windows) {
            "rustc.exe"
        } else {
            "rustc"
        }));
        assert!(!command_available("definitely-not-a-chaos-command"));
    }
}
