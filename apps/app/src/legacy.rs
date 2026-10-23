//! One-time move of an installation from the identifier this launcher was
//! forked from.
//!
//! The identifier is what Windows keys an installed application on, and the
//! launcher derives its data directory from it. Keeping the upstream identifier
//! (`red.ghs.axolotl`) meant this build and the launcher it was forked from
//! shared one data directory — one SQLite database, one settings file, one set
//! of accounts — so installing both was enough to corrupt one of them.
//!
//! The identifier is now `club.sitmc.launcher`, and the data an existing player
//! already has is moved across on the first start, before the database is
//! opened. Both directories live under the same application data root, so the
//! move is a rename and completes instantly; the copy path only exists for the
//! case where a rename is refused. Only after the move succeeded is the old
//! launcher uninstalled: its uninstaller is free to remove "its" data directory
//! at that point because the directory is already gone.

use std::fs;
use std::path::{Path, PathBuf};

/// Identifier this launcher used before the rebrand.
const LEGACY_IDENTIFIER: &str = "red.ghs.axolotl";

/// Identifier this launcher uses now.
const CURRENT_IDENTIFIER: &str = "club.sitmc.launcher";

/// Display name the legacy installer registered itself under.
#[cfg(target_os = "windows")]
const LEGACY_UNINSTALL_KEY: &str = "Axolotl Launcher";

/// Marker written once the move is done, so a leftover file that was skipped
/// because the new directory already had it cannot make every start retry.
const MIGRATION_MARKER: &str = ".legacy-migration-complete";

/// Whether an installation from the legacy identifier is waiting to be moved.
pub fn migration_pending() -> bool {
    if crate::portable::is_portable_mode() {
        // Portable installs keep their data beside the executable, not under the
        // application data directory, so there is nothing to move.
        return false;
    }

    let Some(data_root) = dirs::data_dir() else {
        return false;
    };
    if data_root.join(CURRENT_IDENTIFIER).join(MIGRATION_MARKER).is_file() {
        return false;
    }

    legacy_dir().is_some_and(|dir| dir.is_dir())
}

/// Moves the legacy data directory to the current one and, once that succeeded,
/// removes the legacy launcher.
///
/// Runs before the state database is opened, so the move is not fighting this
/// process for open handles.
pub fn migrate() -> Result<(), String> {
    let Some(legacy_dir) = legacy_dir() else {
        return Ok(());
    };
    let Some(data_root) = dirs::data_dir() else {
        return Ok(());
    };

    if !legacy_dir.is_dir() {
        return Ok(());
    }

    let current_dir = data_root.join(CURRENT_IDENTIFIER);

    // The whole directory is normally renamed, which is instant because both live
    // under the same application data root. The target may already exist because
    // the log file is created before this runs, so a failed rename is not an
    // error: the merge below handles it.
    if !current_dir.exists() {
        if let Some(parent) = current_dir.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::rename(&legacy_dir, &current_dir).is_ok() {
            report_success(&legacy_dir, &current_dir);
            complete(&current_dir);
            finish();
            return Ok(());
        }
    }

    match merge_directory(&legacy_dir, &current_dir) {
        Ok(()) => {
            report_success(&legacy_dir, &current_dir);
            complete(&current_dir);
            finish();
            Ok(())
        }
        Err(error) => {
            // The data is left untouched and the old launcher stays installed; a
            // failed move must never be followed by an uninstall.
            eprintln!(
                "Could not move launcher data from {} to {}: {error}. The old launcher and its data were left in place.",
                legacy_dir.display(),
                current_dir.display()
            );
            Err(format!(
                "旧版启动器数据未能迁移（{error}）。请关闭旧版启动器后重试；旧数据与旧启动器均未删除。"
            ))
        }
    }
}

fn report_success(legacy_dir: &Path, current_dir: &Path) {
    eprintln!(
        "Moved launcher data from {} to {}",
        legacy_dir.display(),
        current_dir.display()
    );
}

/// Marks the move as finished and clears whatever the merge had to skip.
///
/// A file the new directory already had is left in the old one by the merge, and
/// a leftover directory would otherwise make `migration_pending` true forever.
/// The marker is written first so even a failed cleanup cannot cause a retry.
fn complete(current_dir: &Path) {
    if let Err(error) = fs::write(current_dir.join(MIGRATION_MARKER), b"moved from red.ghs.axolotl\n") {
        eprintln!("Could not record the completed migration: {error}");
        return;
    }

    if let Some(legacy_dir) = legacy_dir()
        && legacy_dir.is_dir()
        && let Err(error) = fs::remove_dir_all(&legacy_dir)
    {
        eprintln!(
            "Left {} in place after the move: {error}",
            legacy_dir.display()
        );
    }
}

fn finish() {
    if std::env::var_os("SITMC_SKIP_LEGACY_UNINSTALL").is_none() {
        uninstall_legacy_launcher();
    }
}

fn legacy_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|root| root.join(LEGACY_IDENTIFIER))
}

/// Moves `source` into `target`, merging with whatever the target already holds.
///
/// Each entry is renamed when it can be, which is instant on the same volume;
/// only an entry that cannot be renamed is copied. A target entry that already
/// exists is merged into, never overwritten.
fn merge_directory(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = target.join(entry.file_name());

        if to.exists() {
            if entry.file_type()?.is_dir() && to.is_dir() {
                merge_directory(&from, &to)?;
            } else {
                // Keep what the target already has.
                eprintln!("Keeping {} (already exists in the new directory)", to.display());
            }
            continue;
        }

        if fs::rename(&from, &to).is_err() {
            if entry.file_type()?.is_dir() {
                copy_directory(&from, &to)?;
            } else {
                fs::copy(&from, &to)?;
            }
            remove_directory(&from)?;
        }
    }

    // Only now is the source empty enough to remove.
    remove_directory(source)?;
    Ok(())
}

fn copy_directory(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn remove_directory(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    // A file that cannot be removed is left alone: the next start retries rather
    // than deleting a directory that still holds something.
    let mut last_error = None;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let result = if entry.file_type()?.is_dir() {
            remove_directory(&child)
        } else {
            fs::remove_file(&child)
        };
        if let Err(error) = result {
            last_error = Some(error);
        }
    }
    if last_error.is_none() {
        fs::remove_dir(path)?;
    }
    match last_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Removes the launcher this build was forked from, using the uninstaller that
/// installer registered.
///
/// Called only after the data directory has been moved out from under it, so its
/// uninstaller cannot take the player's instances with it. A per-machine install
/// needs administrator rights; the silent uninstall then simply fails and the
/// old launcher is left installed, which is the safe outcome.
#[cfg(target_os = "windows")]
fn uninstall_legacy_launcher() {
    let Some(uninstaller) = legacy_uninstall_command() else {
        return;
    };

    match std::process::Command::new(&uninstaller).arg("/S").status() {
        Ok(status) if status.success() => {
            eprintln!("Uninstalled the legacy launcher with {uninstaller}");
        }
        Ok(status) => {
            eprintln!("Legacy launcher uninstaller exited with {status}; leaving it installed");
        }
        Err(error) => {
            eprintln!("Could not run the legacy launcher uninstaller: {error}");
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn uninstall_legacy_launcher() {}

/// The `UninstallString` the legacy installer registered, per-user first.
#[cfg(target_os = "windows")]
fn legacy_uninstall_command() -> Option<String> {
    let roots = [
        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
    ];

    for root in roots {
        let key = format!("{root}\\{LEGACY_UNINSTALL_KEY}");
        let output = std::process::Command::new("reg")
            .args(["query", &key, "/v", "UninstallString"])
            .output()
            .ok()?;

        if !output.status.success() {
            continue;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(command) = parse_uninstall_string(&text) {
            return Some(command);
        }
    }

    None
}

/// Pulls the executable path out of a `reg query` line.
#[cfg(target_os = "windows")]
fn parse_uninstall_string(text: &str) -> Option<String> {
    let line = text.lines().find(|line| line.contains("UninstallString"))?;
    let value = line.split("REG_SZ").nth(1)?.trim();

    // Registered values are quoted when the path contains spaces, e.g.
    // `"C:\Program Files\Axolotl Launcher\uninstall.exe"`.
    let command = if let Some(rest) = value.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        value.split_whitespace().next()?.to_string()
    };

    PathBuf::from(&command).is_file().then_some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_into_an_existing_target_without_losing_it() {
        let root = std::env::temp_dir().join(format!("sitmc-migrate-{}", uuid_like()));
        let legacy = root.join("old");
        let current = root.join("new");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(current.join("logs")).unwrap();
        fs::write(legacy.join("app.db"), b"legacy").unwrap();
        fs::write(legacy.join("settings.json"), b"legacy").unwrap();
        fs::write(current.join("logs").join("session.log"), b"current").unwrap();

        merge_directory(&legacy, &current).unwrap();

        assert_eq!(fs::read(current.join("app.db")).unwrap(), b"legacy");
        assert_eq!(fs::read(current.join("settings.json")).unwrap(), b"legacy");
        assert_eq!(
            fs::read(current.join("logs").join("session.log")).unwrap(),
            b"current"
        );
        assert!(!legacy.exists(), "the legacy directory should be gone");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn keeps_a_target_file_that_the_legacy_copy_also_has() {
        let root = std::env::temp_dir().join(format!("sitmc-keep-{}", uuid_like()));
        let legacy = root.join("old");
        let current = root.join("new");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&current).unwrap();
        fs::write(legacy.join("app.db"), b"legacy").unwrap();
        fs::write(current.join("app.db"), b"current").unwrap();

        merge_directory(&legacy, &current).unwrap();

        assert_eq!(fs::read(current.join("app.db")).unwrap(), b"current");

        fs::remove_dir_all(&root).ok();
    }

    fn uuid_like() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
