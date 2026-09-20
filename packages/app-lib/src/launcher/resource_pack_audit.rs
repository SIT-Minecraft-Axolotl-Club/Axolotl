//! Pre-launch audit of an instance's resource packs.
//!
//! A resource pack that is truncated or whose textures are not valid PNG is
//! what makes the client answer `Caught error loading resourcepacks, removing
//! all selected resourcepacks` and reload everything while the window sits
//! black. Vanilla only reports that an error happened, never which pack caused
//! it, so the launcher checks the packs itself and can name the culprit before
//! the game starts.

use std::io::Read;
use std::path::{Path, PathBuf};

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const MAX_TEXTURE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenResourcePack {
    pub name: String,
    pub path: PathBuf,
    pub reason: String,
    pub texture: Option<String>,
}

#[derive(Debug, Default)]
pub struct ResourcePackAudit {
    pub packs_dir: PathBuf,
    pub checked: usize,
    pub broken: Vec<BrokenResourcePack>,
}

impl ResourcePackAudit {
    pub fn is_clean(&self) -> bool {
        self.broken.is_empty()
    }
}

/// Audits every `.zip` pack in `instance_dir/resourcepacks`.
///
/// Only `.zip` files are considered: Minecraft also accepts a pack as an
/// unpacked directory, and those are left to the game.
pub async fn audit_instance_resource_packs(
    instance_dir: &Path,
) -> crate::Result<ResourcePackAudit> {
    let packs_dir = instance_dir.join("resourcepacks");
    let mut audit = ResourcePackAudit {
        packs_dir: packs_dir.clone(),
        ..ResourcePackAudit::default()
    };
    if !packs_dir.is_dir() {
        return Ok(audit);
    }

    let mut read_dir = tokio::fs::read_dir(&packs_dir).await?;
    let mut candidates = Vec::new();
    while let Some(entry) = read_dir.next_entry().await? {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("zip")) {
            candidates.push(path);
        }
    }
    candidates.sort();

    for path in candidates {
        audit.checked += 1;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let path_for_task = path.clone();
        let inspection = tokio::task::spawn_blocking(move || {
            inspect_pack(&path_for_task)
        })
        .await
        .map_err(|error| {
            crate::ErrorKind::LauncherError(format!(
                "Resource pack audit task failed: {error}"
            ))
            .as_error()
        })?;

        if let Err(reason) = inspection {
            audit.broken.push(BrokenResourcePack {
                name,
                path,
                reason: reason.message,
                texture: reason.texture,
            });
        }
    }

    Ok(audit)
}

struct InspectionFailure {
    message: String,
    texture: Option<String>,
}

/// Removes the packs the audit flagged as broken.
///
/// This runs only when the pack is unreadable or its textures cannot possibly
/// load, so the pack could never have worked in game. It keeps the player from
/// losing every selected pack (including working ones) on the next launch,
/// which is what vanilla does once a reload fails.
pub async fn quarantine_broken_packs(
    audit: &ResourcePackAudit,
) -> crate::Result<Vec<String>> {
    let mut quarantined = Vec::new();
    for pack in &audit.broken {
        let target = audit
            .packs_dir
            .join(format!("{}.broken", pack.name));
        // A previous run may have left its own quarantined copy behind.
        match tokio::fs::remove_file(&target).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(
                    pack = %pack.name,
                    %error,
                    "Failed to clear the previous quarantined resource pack"
                );
                continue;
            }
        }
        match tokio::fs::rename(&pack.path, &target).await {
            Ok(()) => quarantined.push(pack.name.clone()),
            Err(error) => {
                tracing::warn!(
                    pack = %pack.name,
                    %error,
                    "Failed to quarantine a broken resource pack"
                );
            }
        }
    }
    Ok(quarantined)
}

fn inspect_pack(path: &Path) -> Result<(), InspectionFailure> {
    let file = std::fs::File::open(path).map_err(|error| InspectionFailure {
        message: format!("could not be opened: {error}"),
        texture: None,
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        InspectionFailure {
            message: format!("is not a readable zip archive: {error}"),
            texture: None,
        }
    })?;

    let mut has_metadata = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            InspectionFailure {
                message: format!("could not be read: {error}"),
                texture: None,
            }
        })?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().to_string();
        if entry_name == "pack.mcmeta" {
            has_metadata = true;
            continue;
        }
        if !entry_name.to_ascii_lowercase().ends_with(".png") {
            continue;
        }
        if entry.size() > MAX_TEXTURE_BYTES {
            continue;
        }

        let mut header = [0u8; 8];
        let mut read = 0usize;
        while read < header.len() {
            let bytes = entry.read(&mut header[read..]).map_err(|error| {
                InspectionFailure {
                    message: format!(
                        "could not be decompressed (truncated archive?): {error}"
                    ),
                    texture: Some(entry_name.clone()),
                }
            })?;
            if bytes == 0 {
                break;
            }
            read += bytes;
        }
        if read < header.len() || header != PNG_SIGNATURE {
            return Err(InspectionFailure {
                message: "contains a texture that is not a valid PNG".to_string(),
                texture: Some(entry_name),
            });
        }
        // Draining each texture catches archives whose central directory is
        // intact but whose data is truncated, which is the common shape of an
        // interrupted download.
        std::io::copy(&mut entry, &mut std::io::sink()).map_err(|error| {
            InspectionFailure {
                message: format!(
                    "contains a truncated texture (incomplete download?): {error}"
                ),
                texture: Some(entry_name),
            }
        })?;
    }

    if !has_metadata {
        return Err(InspectionFailure {
            message: "has no pack.mcmeta, so the game cannot load it".to_string(),
            texture: None,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_pack(dir: &Path, name: &str, texture: &[u8], metadata: bool) {
        let file = std::fs::File::create(dir.join(name)).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        if metadata {
            zip.start_file("pack.mcmeta", options).unwrap();
            zip.write_all(br#"{"pack":{"pack_format":64,"description":"t"}}"#)
                .unwrap();
        }
        zip.start_file("assets/minecraft/textures/entity/end_sky.png", options)
            .unwrap();
        zip.write_all(texture).unwrap();
        zip.finish().unwrap();
    }

    #[tokio::test]
    async fn reports_packs_with_invalid_textures() {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path();
        let packs = instance.join("resourcepacks");
        std::fs::create_dir_all(&packs).unwrap();
        write_pack(&packs, "good.zip", &PNG_SIGNATURE, true);
        write_pack(&packs, "broken.zip", b"not a png at all", true);

        let audit = audit_instance_resource_packs(instance).await.unwrap();
        assert_eq!(audit.checked, 2);
        assert_eq!(audit.broken.len(), 1);
        assert_eq!(audit.broken[0].name, "broken.zip");
        assert_eq!(
            audit.broken[0].texture.as_deref(),
            Some("assets/minecraft/textures/entity/end_sky.png")
        );
    }

    #[tokio::test]
    async fn reports_packs_without_metadata_and_quarantines_them() {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path();
        let packs = instance.join("resourcepacks");
        std::fs::create_dir_all(&packs).unwrap();
        write_pack(&packs, "no-mcmeta.zip", &PNG_SIGNATURE, false);

        let audit = audit_instance_resource_packs(instance).await.unwrap();
        assert_eq!(audit.broken.len(), 1);

        let quarantined = quarantine_broken_packs(&audit).await.unwrap();
        assert_eq!(quarantined, vec!["no-mcmeta.zip".to_string()]);
        assert!(!packs.join("no-mcmeta.zip").exists());
        assert!(packs.join("no-mcmeta.zip.broken").exists());

        let rescan = audit_instance_resource_packs(instance).await.unwrap();
        assert!(rescan.is_clean());
    }

    #[tokio::test]
    async fn missing_resourcepacks_directory_is_not_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let audit = audit_instance_resource_packs(directory.path())
            .await
            .unwrap();
        assert_eq!(audit.checked, 0);
        assert!(audit.is_clean());
    }
}
