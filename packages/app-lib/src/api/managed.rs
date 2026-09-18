//! Server-managed game instances.
//!
//! The club backend publishes a manifest that declares which instances a player
//! may launch, at which `revision`, and from which pack file. This module keeps
//! the local record of those instances, downloads their packs, and gates
//! launching on the recorded revision. The manifest service is optional at
//! runtime: a failed sync keeps the previous snapshot and the previous records.
//!
//! See `docs/launcher-protocol.md` for the contract these types mirror.

use crate::State;
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::state::InstanceInstallStage;
use crate::util::fetch::{INSECURE_REQWEST_CLIENT, sha1_file_async};
use chrono::Utc;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Highest manifest schema this build understands. A manifest that declares a
/// higher schema is rejected instead of being partially interpreted.
const SUPPORTED_MANIFEST_SCHEMA: u32 = 1;

/// Only pack format the existing pack install pipeline can consume.
const MRPACK_KIND: &str = "mrpack";

/// Folder inside the launcher cache directory that holds downloaded packs.
const MANAGED_CACHE_FOLDER: &str = "managed";

/// Longest managed instance id accepted, matching the manifest contract.
const MAX_INSTANCE_ID_LEN: usize = 64;

/// Port used when the manifest provides a `server` object without a port.
const DEFAULT_SERVER_PORT: u16 = 25565;

/// One manifest document as served by the club backend.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedManifest {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub launcher: Option<ManagedLauncherPolicy>,
    #[serde(default)]
    pub instances: Vec<ManagedInstanceSpec>,
}

/// Launcher update policy carried by a manifest.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedLauncherPolicy {
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub latest_version: Option<String>,
    #[serde(default)]
    pub force_update: bool,
}

/// One managed instance declared by a manifest.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedInstanceSpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub sort: i32,
    pub revision: i64,
    pub minecraft: ManagedMinecraftSpec,
    pub pack: ManagedPackSpec,
    #[serde(default)]
    pub server: Option<ManagedServerSpec>,
    #[serde(default = "default_required")]
    pub required: bool,
}

/// Game and loader versions a managed instance must be installed with.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedMinecraftSpec {
    pub game_version: String,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub loader_version: Option<String>,
}

/// Where the content of a managed instance comes from.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedPackSpec {
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// Server a managed instance connects to on launch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedServerSpec {
    pub address: String,
    #[serde(default)]
    pub port: Option<u16>,
}

fn default_required() -> bool {
    true
}

/// Outcome of reconciling a manifest with the local records.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedSyncReport {
    pub schema: u32,
    #[serde(default)]
    pub launcher: Option<ManagedLauncherPolicy>,
    pub instances: Vec<ManagedInstanceAction>,
}

/// What the launcher should do with one managed instance.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedInstanceAction {
    pub spec: ManagedInstanceSpec,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub applied_revision: Option<i64>,
    pub action: ManagedInstanceActionKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedInstanceActionKind {
    Create,
    Update,
    Current,
    Retired,
}

/// Local record of one managed instance.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManagedInstanceRecord {
    pub server_instance_id: String,
    pub instance_id: String,
    pub revision: i64,
    pub target_revision: i64,
    pub name: String,
    pub description: String,
    pub icon_url: String,
    pub sort: i32,
    pub required: bool,
    pub retired: bool,
    pub server_address: String,
    pub server_port: u16,
    pub updated: i64,
}

#[derive(sqlx::FromRow)]
struct ManagedInstanceRow {
    server_instance_id: String,
    instance_id: String,
    revision: i64,
    target_revision: i64,
    name: String,
    description: String,
    icon_url: String,
    sort: i64,
    required: i64,
    retired: i64,
    server_address: String,
    server_port: i64,
    updated: i64,
}

#[derive(sqlx::FromRow)]
struct ManagedGateRow {
    server_instance_id: String,
    revision: i64,
    target_revision: i64,
    retired: i64,
}

impl From<ManagedInstanceRow> for ManagedInstanceRecord {
    fn from(row: ManagedInstanceRow) -> Self {
        Self {
            server_instance_id: row.server_instance_id,
            instance_id: row.instance_id,
            revision: row.revision,
            target_revision: row.target_revision,
            name: row.name,
            description: row.description,
            icon_url: row.icon_url,
            sort: clamp_i32(row.sort),
            required: row.required != 0,
            retired: row.retired != 0,
            server_address: row.server_address,
            server_port: clamp_u16(row.server_port),
            updated: row.updated,
        }
    }
}

/// Fetches and parses the manifest.
///
/// A manifest that declares a schema newer than the one this build implements
/// is rejected, so a stale launcher cannot silently install the wrong content.
#[tracing::instrument]
pub async fn fetch_manifest(
    manifest_url: &str,
) -> crate::Result<ManagedManifest> {
    let body = fetch_manifest_body(manifest_url).await?;
    parse_manifest(manifest_url, &body)
}

/// Fetches the manifest, reconciles it with the local managed instance
/// records, and reports what should happen to every managed instance.
///
/// This does not download packs or create instances: the caller uses the
/// reported `instance_id` with the existing instance and pack install commands.
#[tracing::instrument]
pub async fn sync_manifest(
    manifest_url: &str,
) -> crate::Result<ManagedSyncReport> {
    let state = State::get().await?;
    let body = fetch_manifest_body(manifest_url).await?;
    let parsed = parse_manifest(manifest_url, &body)?;

    let ManagedManifest {
        schema,
        launcher,
        instances,
        ..
    } = parsed;

    // Reject a malformed manifest before replacing the cached snapshot, so a
    // server mistake keeps the last manifest that was known to be usable.
    for spec in &instances {
        validate_managed_spec(spec)?;
    }

    let previous = cached_specs(&state).await;
    store_manifest_body(&state, manifest_url, &body).await?;

    let updated = Utc::now().timestamp();
    let mut actions = Vec::with_capacity(instances.len());

    for spec in &instances {
        if spec.pack.kind != MRPACK_KIND {
            tracing::warn!(
                server_instance_id = %spec.id,
                pack_kind = %spec.pack.kind,
                "Managed instance uses a pack format this launcher cannot install"
            );
        }

        let existing = load_row(&state, &spec.id).await?;
        let applied = existing.as_ref().map(|row| row.revision);
        let instance_id = existing
            .as_ref()
            .map(|row| row.instance_id.clone())
            .filter(|value| !value.is_empty());
        let target = match applied {
            Some(applied) if applied > spec.revision => {
                tracing::warn!(
                    server_instance_id = %spec.id,
                    applied,
                    requested = spec.revision,
                    "Refusing a managed instance revision downgrade"
                );
                applied
            }
            _ => spec.revision,
        };
        store_spec(&state, spec, target, updated).await?;

        // A record alone is not proof that the instance is usable: the local
        // instance may have been deleted, or an earlier install may never have
        // finished. Both have to end in an install, and only the local instance
        // can tell the difference, so it is consulted here rather than trusted
        // from the record.
        let linked_stage = match instance_id.as_deref() {
            Some(local_id) => crate::state::get_instance(local_id, &state.pool)
                .await?
                .map(|metadata| metadata.instance.install_stage),
            None => None,
        };

        let action = match (instance_id.as_deref(), applied) {
            (None, _) => ManagedInstanceActionKind::Create,
            (Some(_), Some(_)) if linked_stage.is_none() => {
                ManagedInstanceActionKind::Create
            }
            (Some(_), Some(applied))
                if applied == target
                    && linked_stage
                        == Some(InstanceInstallStage::Installed) =>
            {
                ManagedInstanceActionKind::Current
            }
            (Some(_), _) => ManagedInstanceActionKind::Update,
        };
        actions.push(ManagedInstanceAction {
            spec: spec.clone(),
            instance_id,
            applied_revision: applied,
            action,
        });
    }

    let declared: HashSet<&str> =
        instances.iter().map(|spec| spec.id.as_str()).collect();
    for row in list_rows(&state).await? {
        if declared.contains(row.server_instance_id.as_str()) {
            continue;
        }

        // The server stopped publishing this instance, so the launcher follows:
        // the local instance, its files and the record all go. Keeping the entry
        // would only leave something the player cannot launch and cannot remove.
        if !row.instance_id.is_empty()
            && crate::state::get_instance(&row.instance_id, &state.pool)
                .await?
                .is_some()
        {
            crate::state::remove_instance(&row.instance_id, &state).await?;
            emit_instance(&row.instance_id, InstancePayloadType::Removed)
                .await?;
        }

        delete_row(&state, &row.server_instance_id).await?;
    }

    actions.sort_by(|left, right| {
        left.spec
            .sort
            .cmp(&right.spec.sort)
            .then_with(|| left.spec.name.cmp(&right.spec.name))
    });

    Ok(ManagedSyncReport {
        schema,
        launcher,
        instances: actions,
    })
}

/// Local records of every known managed instance, ordered for display.
#[tracing::instrument]
pub async fn list_managed_instances()
-> crate::Result<Vec<ManagedInstanceRecord>> {
    let state = State::get().await?;
    Ok(list_rows(&state)
        .await?
        .into_iter()
        .map(ManagedInstanceRecord::from)
        .collect())
}

/// The club instance that owns a local instance, and whether the server requires
/// it.
///
/// The flag decides two things on the client: a required instance is installed
/// without asking and may not be deleted, while an optional one is only
/// downloaded once the player starts it and can be deleted again.
pub struct ManagedOwner {
    pub server_instance_id: String,
    pub required: bool,
}

/// Looks up the club instance owning a local instance, if any.
#[tracing::instrument]
pub async fn managed_owner(
    instance_id: &str,
) -> crate::Result<Option<ManagedOwner>> {
    let state = State::get().await?;

    let row = sqlx::query_as::<_, (String, i64)>(
        "SELECT server_instance_id, required FROM managed_instances
         WHERE instance_id = ? LIMIT 1",
    )
    .bind(instance_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(row.map(|(server_instance_id, required)| ManagedOwner {
        server_instance_id,
        required: required != 0,
    }))
}

/// Downloads and verifies the pack of a managed instance and returns the local
/// file path to hand to the existing pack install commands.
///
/// Only `mrpack` is supported today, and the manifest must declare a SHA-1:
/// an unverified pack is never installed.
#[tracing::instrument]
pub async fn prepare_pack(
    server_instance_id: &str,
    revision: i64,
) -> crate::Result<String> {
    validate_server_instance_id(server_instance_id)?;
    let state = State::get().await?;
    let spec = cached_spec(&state, server_instance_id, revision).await?;

    if spec.pack.kind != MRPACK_KIND {
        return Err(crate::ErrorKind::InputError(format!(
            "Managed instance {server_instance_id} uses pack format \"{}\", which this launcher cannot install yet",
            spec.pack.kind
        ))
        .as_error());
    }
    let expected_sha1 = spec
        .pack
        .sha1
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Managed instance {server_instance_id} does not declare a pack SHA-1, so the pack cannot be verified"
            ))
            .as_error()
        })?;

    let directory = state.directories.caches_dir().join(MANAGED_CACHE_FOLDER);
    tokio::fs::create_dir_all(&directory).await?;
    let target =
        directory.join(format!("{server_instance_id}-{revision}.mrpack"));

    if tokio::fs::try_exists(&target).await? {
        let (size, sha1) = sha1_file_async(&target).await?;
        if sha1 == expected_sha1
            && spec.pack.size.is_none_or(|expected| expected == size)
        {
            return Ok(target.to_string_lossy().to_string());
        }
    }

    download_pack(&spec.pack.url, &target, &expected_sha1, spec.pack.size)
        .await?;

    Ok(target.to_string_lossy().to_string())
}

/// Records that the local instance now holds the given applied revision.
///
/// The required `target_revision` is owned by [`sync_manifest`], so a manifest
/// that moved on while the install was running still forces another update.
#[tracing::instrument]
pub async fn mark_instance_installed(
    server_instance_id: &str,
    instance_id: &str,
    revision: i64,
) -> crate::Result<()> {
    let state = State::get().await?;
    let result = sqlx::query(
        "UPDATE managed_instances
         SET instance_id = ?, revision = ?, updated = ?
         WHERE server_instance_id = ?",
    )
    .bind(instance_id)
    .bind(revision)
    .bind(Utc::now().timestamp())
    .bind(server_instance_id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(crate::ErrorKind::InputError(format!(
            "Unknown managed instance {server_instance_id}; sync the instance manifest first"
        ))
        .as_error());
    }

    Ok(())
}

/// Records which local instance represents a managed instance.
///
/// The caller invokes this after the pack install job for `revision` has
/// settled successfully, so the applied revision is stored together with the
/// instance id. `target_revision` is left to [`sync_manifest`]: a manifest that
/// moved on while the install was running still forces another update.
#[tracing::instrument]
pub async fn register_instance(
    server_instance_id: &str,
    instance_id: &str,
    revision: i64,
) -> crate::Result<()> {
    let state = State::get().await?;
    sqlx::query(
        "INSERT INTO managed_instances (
            server_instance_id, instance_id, revision, target_revision, name,
            updated
         )
         VALUES (?, ?, ?, ?, '', ?)
         ON CONFLICT(server_instance_id) DO UPDATE SET
           instance_id = excluded.instance_id,
           revision = excluded.revision,
           updated = excluded.updated",
    )
    .bind(server_instance_id)
    .bind(instance_id)
    .bind(revision)
    .bind(revision)
    .bind(Utc::now().timestamp())
    .execute(&state.pool)
    .await?;

    Ok(())
}

/// Launch gate: rejects a managed instance that is retired, not installed, or
/// not installed at the revision the manifest currently requires.
///
/// Only local state is read, so the gate never blocks a launch on the network.
/// Instances that are not managed pass through unchanged.
#[tracing::instrument]
pub async fn ensure_instance_runnable(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let rows = sqlx::query_as::<_, ManagedGateRow>(
        "SELECT server_instance_id, revision, target_revision, retired
         FROM managed_instances
         WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_all(&state.pool)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    for row in &rows {
        if row.retired != 0 {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Managed instance {} is no longer offered by the server and cannot be launched",
                row.server_instance_id
            ))
            .as_error());
        }
        if row.revision != row.target_revision {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Managed instance {} must be updated from revision {} to revision {} before it can be launched",
                row.server_instance_id, row.revision, row.target_revision
            ))
            .as_error());
        }
    }

    let stage = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .map(|metadata| metadata.instance.install_stage);
    if stage != Some(InstanceInstallStage::Installed) {
        return Err(crate::ErrorKind::InstanceNotReady {
            instance_id: instance_id.to_string(),
            stage: stage
                .map(|stage| stage.as_str().to_string())
                .unwrap_or_else(|| "not_installed".to_string()),
        }
        .as_error());
    }

    Ok(())
}

/// Rejects an identifier that could escape the managed cache directory or that
/// no server should ever emit.
fn validate_server_instance_id(value: &str) -> crate::Result<()> {
    let safe = !value.is_empty()
        && value.len() <= MAX_INSTANCE_ID_LEN
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
        });
    if !safe {
        return Err(crate::ErrorKind::InputError(format!(
            "Managed instance id \"{value}\" is not a usable identifier"
        ))
        .as_error());
    }
    Ok(())
}

fn validate_managed_spec(spec: &ManagedInstanceSpec) -> crate::Result<()> {
    validate_server_instance_id(&spec.id)?;
    if spec.revision < 0 {
        return Err(crate::ErrorKind::InputError(format!(
            "Managed instance {} declares a negative revision",
            spec.id
        ))
        .as_error());
    }
    if spec.minecraft.game_version.trim().is_empty() {
        return Err(crate::ErrorKind::InputError(format!(
            "Managed instance {} does not declare a Minecraft game version",
            spec.id
        ))
        .as_error());
    }
    if spec.pack.url.trim().is_empty() {
        return Err(crate::ErrorKind::InputError(format!(
            "Managed instance {} does not declare a pack URL",
            spec.id
        ))
        .as_error());
    }
    Ok(())
}

fn validate_manifest_schema(schema: u32) -> crate::Result<()> {
    if schema > SUPPORTED_MANIFEST_SCHEMA {
        return Err(crate::ErrorKind::InputError(format!(
            "The instance manifest requires schema {schema}, but this launcher only supports schema {SUPPORTED_MANIFEST_SCHEMA}; update the launcher"
        ))
        .as_error());
    }
    if schema == 0 {
        return Err(crate::ErrorKind::InputError(
            "The instance manifest does not declare a schema version"
                .to_string(),
        )
        .as_error());
    }
    Ok(())
}

async fn fetch_manifest_body(manifest_url: &str) -> crate::Result<String> {
    let response = INSECURE_REQWEST_CLIENT
        .get(manifest_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        return Err(crate::ErrorKind::HttpError {
            status: status.as_u16(),
            method: "GET".to_string(),
            url: manifest_url.to_string(),
        }
        .as_error());
    }
    Ok(response.text().await?)
}

fn parse_manifest(
    manifest_url: &str,
    body: &str,
) -> crate::Result<ManagedManifest> {
    let manifest: ManagedManifest =
        serde_json::from_str(body).map_err(|error| {
            crate::ErrorKind::InputError(format!(
                "Manifest from {manifest_url} is not valid JSON: {error}"
            ))
            .as_error()
        })?;
    validate_manifest_schema(manifest.schema)?;
    Ok(manifest)
}

async fn load_row(
    state: &State,
    server_instance_id: &str,
) -> crate::Result<Option<ManagedInstanceRow>> {
    Ok(sqlx::query_as::<_, ManagedInstanceRow>(
        "SELECT server_instance_id, instance_id, revision, target_revision,
                name, description, icon_url, sort, required, retired,
                server_address, server_port, updated
         FROM managed_instances
         WHERE server_instance_id = ?",
    )
    .bind(server_instance_id)
    .fetch_optional(&state.pool)
    .await?)
}

async fn list_rows(state: &State) -> crate::Result<Vec<ManagedInstanceRow>> {
    Ok(sqlx::query_as::<_, ManagedInstanceRow>(
        "SELECT server_instance_id, instance_id, revision, target_revision,
                name, description, icon_url, sort, required, retired,
                server_address, server_port, updated
         FROM managed_instances
         ORDER BY sort ASC, name ASC",
    )
    .fetch_all(&state.pool)
    .await?)
}

async fn store_spec(
    state: &State,
    spec: &ManagedInstanceSpec,
    target_revision: i64,
    updated: i64,
) -> crate::Result<()> {
    let description = spec.description.as_deref().unwrap_or("");
    let icon_url = spec.icon_url.as_deref().unwrap_or("");
    let server_address = spec
        .server
        .as_ref()
        .map(|server| server.address.as_str())
        .unwrap_or("");
    let server_port = spec
        .server
        .as_ref()
        .map(|server| server.port.unwrap_or(DEFAULT_SERVER_PORT))
        .unwrap_or(0);

    sqlx::query(
        "INSERT OR IGNORE INTO managed_instances (
            server_instance_id, instance_id, revision, target_revision, name,
            description, icon_url, sort, required, retired, server_address,
            server_port, updated
         )
         VALUES (?, '', 0, ?, ?, ?, ?, ?, ?, FALSE, ?, ?, ?)",
    )
    .bind(&spec.id)
    .bind(target_revision)
    .bind(&spec.name)
    .bind(description)
    .bind(icon_url)
    .bind(i64::from(spec.sort))
    .bind(i64::from(spec.required))
    .bind(server_address)
    .bind(i64::from(server_port))
    .bind(updated)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "UPDATE managed_instances
         SET target_revision = ?, name = ?, description = ?, icon_url = ?,
             sort = ?, required = ?, retired = FALSE, server_address = ?,
             server_port = ?, updated = ?
         WHERE server_instance_id = ?",
    )
    .bind(target_revision)
    .bind(&spec.name)
    .bind(description)
    .bind(icon_url)
    .bind(i64::from(spec.sort))
    .bind(i64::from(spec.required))
    .bind(server_address)
    .bind(i64::from(server_port))
    .bind(updated)
    .bind(&spec.id)
    .execute(&state.pool)
    .await?;

    Ok(())
}

/// Forgets a club instance the server no longer publishes.
async fn delete_row(
    state: &State,
    server_instance_id: &str,
) -> crate::Result<()> {
    sqlx::query("DELETE FROM managed_instances WHERE server_instance_id = ?")
        .bind(server_instance_id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

async fn store_manifest_body(
    state: &State,
    manifest_url: &str,
    body: &str,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO managed_manifest_cache (manifest_url, body, updated)
         VALUES (?, ?, ?)
         ON CONFLICT(manifest_url) DO UPDATE SET
           body = excluded.body,
           updated = excluded.updated",
    )
    .bind(manifest_url)
    .bind(body)
    .bind(Utc::now().timestamp())
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Latest successfully fetched manifest body, or `None` when nothing usable has
/// been stored yet. The snapshot is untrusted local data, so a body that no
/// longer parses is ignored instead of failing the sync.
async fn load_cached_manifest(
    state: &State,
) -> crate::Result<Option<ManagedManifest>> {
    let body: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT body FROM managed_manifest_cache
         ORDER BY updated DESC
         LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;

    Ok(body.and_then(|body| serde_json::from_str(&body).ok()))
}

async fn cached_specs(state: &State) -> HashMap<String, ManagedInstanceSpec> {
    load_cached_manifest(state)
        .await
        .ok()
        .flatten()
        .map(|manifest| {
            manifest
                .instances
                .into_iter()
                .map(|spec| (spec.id.clone(), spec))
                .collect()
        })
        .unwrap_or_default()
}

async fn cached_spec(
    state: &State,
    server_instance_id: &str,
    revision: i64,
) -> crate::Result<ManagedInstanceSpec> {
    let manifest = load_cached_manifest(state).await?.ok_or_else(|| {
        crate::ErrorKind::InputError(
            "No instance manifest has been synced yet; sync it before preparing a managed pack"
                .to_string(),
        )
        .as_error()
    })?;
    validate_manifest_schema(manifest.schema)?;

    let spec = manifest
        .instances
        .into_iter()
        .find(|spec| spec.id == server_instance_id)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "The cached instance manifest no longer declares managed instance {server_instance_id}"
            ))
            .as_error()
        })?;

    if spec.revision != revision {
        return Err(crate::ErrorKind::InputError(format!(
            "The cached instance manifest declares revision {} for {server_instance_id}, but revision {revision} was requested; sync the manifest again",
            spec.revision
        ))
        .as_error());
    }

    Ok(spec)
}

async fn download_pack(
    url: &str,
    target: &Path,
    expected_sha1: &str,
    expected_size: Option<u64>,
) -> crate::Result<()> {
    let response = INSECURE_REQWEST_CLIENT.get(url).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(crate::ErrorKind::HttpError {
            status: status.as_u16(),
            method: "GET".to_string(),
            url: url.to_string(),
        }
        .as_error());
    }

    let part = part_path(target);
    if let Err(error) = stream_to_file(response, &part).await {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(error);
    }

    let (size, sha1) = match sha1_file_async(&part).await {
        Ok(hashed) => hashed,
        Err(error) => {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(error);
        }
    };
    if sha1 != expected_sha1 {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(crate::ErrorKind::HashError(
            expected_sha1.to_string(),
            sha1,
        )
        .as_error());
    }
    if let Some(expected) = expected_size
        && expected != size
    {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(crate::ErrorKind::InputError(format!(
            "Managed pack size mismatch: expected {expected} bytes, got {size}"
        ))
        .as_error());
    }

    tokio::fs::rename(&part, target).await?;
    Ok(())
}

/// Streams a response into the `.part` staging file next to its destination, so
/// a failed or truncated transfer never replaces a verified pack.
async fn stream_to_file(
    response: reqwest::Response,
    part: &Path,
) -> crate::Result<()> {
    let mut file = tokio::fs::File::create(part).await?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}

fn part_path(target: &Path) -> PathBuf {
    let mut path = target.as_os_str().to_os_string();
    path.push(".part");
    PathBuf::from(path)
}

fn clamp_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn clamp_u16(value: i64) -> u16 {
    value.clamp(0, i64::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_supported_manifest_schema() {
        assert!(validate_manifest_schema(SUPPORTED_MANIFEST_SCHEMA).is_ok());
    }

    #[test]
    fn rejects_a_missing_or_newer_manifest_schema() {
        assert!(
            validate_manifest_schema(SUPPORTED_MANIFEST_SCHEMA + 1).is_err()
        );
        assert!(validate_manifest_schema(0).is_err());
    }

    #[test]
    fn rejects_identifiers_that_could_escape_the_cache_directory() {
        assert!(validate_server_instance_id("survival-12_a").is_ok());
        assert!(validate_server_instance_id("").is_err());
        assert!(validate_server_instance_id("../escape").is_err());
        assert!(
            validate_server_instance_id(&"a".repeat(MAX_INSTANCE_ID_LEN + 1))
                .is_err()
        );
    }
}
