//! Re-downloads modpack-managed instance files whose bytes no longer match what
//! the modpack installed.
//!
//! The launcher records an expected hash for every file a modpack owns, but
//! never re-checked the copy on disk. A file that was truncated by a failed
//! write, a disk error, or antivirus interference therefore stayed broken
//! forever: for a resource pack that shows up as the client dropping every
//! selected pack at startup, and for a mod as a launch failure. Repair here
//! restores the original bytes through the normal download pipeline, so a
//! hash-verified download replaces the damaged file.

use crate::state::instances::adapters::sqlite::content_rows;
use crate::state::{ContentProvider, Instance, State};
use crate::util::fetch::{
    self, DownloadRequest, Integrity, ResourceClass,
};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstanceFileRepairOutcome {
    /// The file on disk already matches the recorded hash.
    Intact,
    /// A replacement was downloaded and verified.
    Repaired,
    /// No modpack record owns this path, so the launcher cannot restore it.
    Unmanaged,
    /// The record exists but the provider exposes no public download for it.
    Unavailable,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceFileRepair {
    pub relative_path: String,
    pub outcome: InstanceFileRepairOutcome,
}

impl InstanceFileRepair {
    pub fn repaired(&self) -> bool {
        self.outcome == InstanceFileRepairOutcome::Repaired
    }

    fn failed(relative_path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            relative_path: relative_path.into(),
            outcome: InstanceFileRepairOutcome::Failed(message.into()),
        }
    }
}

/// Verifies each requested instance-relative path against the hash its modpack
/// recorded, and re-downloads any file whose bytes no longer match.
///
/// Paths the modpack does not own are reported as [`InstanceFileRepairOutcome::Unmanaged`]
/// so a caller can decide what to do with them; a file that is simply absent is
/// reported as repaired when it can be fetched, which also heals a player who
/// deleted pack content.
pub async fn repair_instance_files(
    instance: &Instance,
    relative_paths: &[String],
) -> crate::Result<Vec<InstanceFileRepair>> {
    let state = State::get().await?;
    let targets = content_rows::get_pack_member_repair_targets(
        &instance.id,
        &["resourcepacks"],
        &state.pool,
    )
    .await?;

    let instance_dir = state.directories.resolve_game_dir(
        &instance.path,
        instance.game_dir_override.as_deref(),
    );

    let mut results = Vec::with_capacity(relative_paths.len());
    for relative_path in relative_paths {
        let Some(target) = targets.iter().find(|target| {
            target.relative_path.eq_ignore_ascii_case(relative_path)
                || target
                    .relative_path
                    .replace('\\', "/")
                    .eq_ignore_ascii_case(&relative_path.replace('\\', "/"))
        }) else {
            results.push(InstanceFileRepair {
                relative_path: relative_path.clone(),
                outcome: InstanceFileRepairOutcome::Unmanaged,
            });
            continue;
        };

        let Some(full_path) = safe_instance_path(&instance_dir, &target.relative_path)
        else {
            results.push(InstanceFileRepair::failed(
                &target.relative_path,
                "recorded path escapes the instance directory",
            ));
            continue;
        };

        let integrity = Integrity::sha1(target.expected_sha1.clone());
        if file_matches(&full_path, &integrity).await {
            results.push(InstanceFileRepair {
                relative_path: target.relative_path.clone(),
                outcome: InstanceFileRepairOutcome::Intact,
            });
            continue;
        }

        match restore_file(&full_path, target, &state).await {
            Ok(()) => {
                // Keep the recorded hash in step with the restored bytes so
                // later content views and upgrades do not read the repair as a
                // player edit.
                if let Err(error) =
                    refresh_recorded_file_hash(&instance.id, &full_path, target, &state)
                        .await
                {
                    tracing::warn!(
                        path = %target.relative_path,
                        %error,
                        "Restored an instance file but could not refresh its recorded hash"
                    );
                }
                results.push(InstanceFileRepair {
                    relative_path: target.relative_path.clone(),
                    outcome: InstanceFileRepairOutcome::Repaired,
                });
            }
            Err(RestoreError::Unmanaged) => {
                results.push(InstanceFileRepair {
                    relative_path: target.relative_path.clone(),
                    outcome: InstanceFileRepairOutcome::Unmanaged,
                });
            }
            Err(RestoreError::Unavailable(reason)) => {
                results.push(InstanceFileRepair {
                    relative_path: target.relative_path.clone(),
                    outcome: InstanceFileRepairOutcome::Unavailable,
                });
                tracing::warn!(
                    path = %target.relative_path,
                    reason,
                    "No download source is available to repair an instance file"
                );
            }
            Err(RestoreError::Failed(message)) => {
                results.push(InstanceFileRepair::failed(
                    &target.relative_path,
                    message,
                ));
            }
        }
    }

    Ok(results)
}

enum RestoreError {
    Unmanaged,
    Unavailable(String),
    Failed(String),
}

async fn restore_file(
    destination: &Path,
    target: &content_rows::PackMemberRepairTarget,
    state: &State,
) -> Result<(), RestoreError> {
    if target.provider != Some(ContentProvider::Modrinth) {
        return Err(RestoreError::Unmanaged);
    }
    let (Some(version_id), Some(_project_id)) = (
        target.provider_release_id.as_deref(),
        target.provider_project_id.as_deref(),
    ) else {
        return Err(RestoreError::Unavailable(
            "the recorded provider reference is incomplete".to_string(),
        ));
    };
    let version_id = version_id.to_string();

    let version = crate::api::cache::get_version(&version_id, None)
        .await
        .map_err(|error| RestoreError::Failed(error.to_string()))?
        .ok_or_else(|| {
            RestoreError::Unavailable(format!(
                "Modrinth version {version_id} is gone"
            ))
        })?;

    let file_name = destination
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let file = version
        .files
        .iter()
        .find(|file| {
            file.hashes
                .get("sha1")
                .is_some_and(|sha1| sha1.eq_ignore_ascii_case(&target.expected_sha1))
        })
        .or_else(|| version.files.iter().find(|file| file.filename == file_name))
        .ok_or_else(|| {
            RestoreError::Unavailable(format!(
                "Modrinth version {version_id} no longer offers {file_name}"
            ))
        })?;

    let mut integrity = Integrity {
        size: Some(u64::from(file.size)),
        ..Integrity::default()
    };
    integrity.sha1 = file.hashes.get("sha1").cloned();
    integrity.sha512 = file.hashes.get("sha512").cloned();
    if integrity.sha1.is_none() {
        // The recorded hash is the proof the repaired file is the original
        // content, so a provider entry without one cannot be trusted here.
        integrity.sha1 = Some(target.expected_sha1.clone());
    }

    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| RestoreError::Failed(error.to_string()))?;
    }

    fetch::download_to_path(
        DownloadRequest::new(&file.url, ResourceClass::Modrinth)
            .with_integrity(integrity),
        destination,
        &state.download_semaphore,
        &state.pool,
        None,
    )
    .await
    .map_err(|error| RestoreError::Failed(error.to_string()))?;

    Ok(())
}

async fn file_matches(path: &Path, integrity: &Integrity) -> bool {
    if !path.is_file() {
        return false;
    }
    fetch::verify_file(path, integrity).await.is_ok()
}

/// Rewrites the stored hash and size of a repaired file so content views and
/// upgrades see the restored bytes as the modpack's own content.
async fn refresh_recorded_file_hash(
    instance_id: &str,
    full_path: &Path,
    target: &content_rows::PackMemberRepairTarget,
    state: &State,
) -> crate::Result<()> {
    let (size, sha1) = fetch::sha1_file_async(full_path).await?;
    let size = i64::try_from(size).map_err(|_| {
        crate::ErrorKind::InputError("Restored file is too large".to_string())
    })?;
    sqlx::query(
        "UPDATE instance_files
		 SET sha1 = ?, size = ?, modified_at = ?
		 WHERE instance_id = ?
			AND lower(replace(relative_path, '\\', '/')) = lower(replace(?, '\\', '/'))",
    )
    .bind(&sha1)
    .bind(size)
    .bind(chrono::Utc::now().timestamp())
    .bind(instance_id)
    .bind(&target.relative_path)
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Resolves an instance-relative content path, refusing anything that would
/// escape the instance directory.
fn safe_instance_path(instance_dir: &Path, relative_path: &str) -> Option<PathBuf> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(instance_dir.join(relative))
}

/// Collects the relative paths of the requested files, which the resource pack
/// audit reports as file names only.
pub fn resource_pack_relative_paths(pack_names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    pack_names
        .into_iter()
        .filter(|name| !name.trim().is_empty())
        .map(|name| format!("resourcepacks/{name}"))
        .filter(|path| seen.insert(path.to_ascii_lowercase()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn repair_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite pool");
        for statement in [
            "CREATE TABLE instances (id TEXT PRIMARY KEY, applied_content_set_id TEXT)",
            "CREATE TABLE instance_content_entries (id TEXT PRIMARY KEY, file_id TEXT)",
            "CREATE TABLE instance_content_provider_refs (
				content_entry_id TEXT, provider TEXT, provider_project_id TEXT,
				provider_release_id TEXT, is_origin INTEGER)",
            "CREATE TABLE instance_files (
				id TEXT PRIMARY KEY, instance_id TEXT, relative_path TEXT,
				missing INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE instance_pack_members (
				id TEXT PRIMARY KEY, content_set_id TEXT, content_entry_id TEXT,
				expected_sha1 TEXT, provider TEXT, provider_project_id TEXT,
				provider_release_id TEXT, materialization_state TEXT NOT NULL DEFAULT 'present')",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        pool
    }

    async fn insert_member(
        pool: &sqlx::SqlitePool,
        member: &str,
        file: &str,
        relative_path: &str,
        provider: &str,
        expected_sha1: &str,
        materialization_state: &str,
    ) {
        sqlx::query(
            "INSERT INTO instance_pack_members (id, content_set_id, content_entry_id, expected_sha1, provider, provider_project_id, provider_release_id, materialization_state) VALUES (?, 'set', ?, ?, ?, 'project', 'release', ?)",
        )
        .bind(member)
        .bind(format!("entry-{member}"))
        .bind(expected_sha1)
        .bind(provider)
        .bind(materialization_state)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instance_content_entries (id, file_id) VALUES (?, ?)",
        )
        .bind(format!("entry-{member}"))
        .bind(file)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instance_files (id, instance_id, relative_path, missing) VALUES (?, 'instance', ?, 0)",
        )
        .bind(file)
        .bind(relative_path)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn repair_targets_only_cover_present_pack_members_in_the_requested_directories() {
        let pool = repair_pool().await;
        sqlx::query(
            "INSERT INTO instances (id, applied_content_set_id) VALUES ('instance', 'set')",
        )
        .execute(&pool)
        .await
        .unwrap();
        insert_member(
            &pool,
            "pack",
            "file-pack",
            "resourcepacks/Club Pack.zip",
            "modrinth",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "present",
        )
        .await;
        insert_member(
            &pool,
            "mod",
            "file-mod",
            "mods/example.jar",
            "modrinth",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "present",
        )
        .await;
        insert_member(
            &pool,
            "removed",
            "file-removed",
            "resourcepacks/Removed.zip",
            "modrinth",
            "cccccccccccccccccccccccccccccccccccccccc",
            "removed",
        )
        .await;

        let targets = content_rows::get_pack_member_repair_targets(
            "instance",
            &["resourcepacks"],
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].relative_path, "resourcepacks/Club Pack.zip");
        assert_eq!(
            targets[0].expected_sha1,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].provider, Some(ContentProvider::Modrinth));
    }

    #[tokio::test]
    async fn file_matches_only_accepts_the_recorded_hash() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pack.zip");
        std::fs::write(&path, b"pack bytes").unwrap();
        let (_, sha1) = fetch::sha1_file_async(&path).await.unwrap();

        assert!(file_matches(&path, &Integrity::sha1(sha1.clone())).await);
        assert!(
            !file_matches(
                &path,
                &Integrity::sha1("0000000000000000000000000000000000000000")
            )
            .await
        );
        assert!(
            !file_matches(&directory.path().join("absent.zip"), &Integrity::sha1(sha1))
                .await
        );
    }

    #[test]
    fn resource_pack_paths_are_deduplicated_and_scoped() {
        let paths = resource_pack_relative_paths([
            "Club Pack.zip".to_string(),
            "club pack.zip".to_string(),
            String::new(),
        ]);
        assert_eq!(paths, vec!["resourcepacks/Club Pack.zip".to_string()]);
    }

    #[test]
    fn safe_instance_paths_cannot_escape_the_instance_directory() {
        let root = Path::new("C:\\instances\\one");
        assert_eq!(
            safe_instance_path(root, "resourcepacks/pack.zip"),
            Some(root.join("resourcepacks").join("pack.zip"))
        );
        assert_eq!(safe_instance_path(root, "../outside.zip"), None);
        assert_eq!(safe_instance_path(root, "resourcepacks/../../x.zip"), None);
        assert_eq!(safe_instance_path(root, "C:\\absolute.zip"), None);
    }
}
