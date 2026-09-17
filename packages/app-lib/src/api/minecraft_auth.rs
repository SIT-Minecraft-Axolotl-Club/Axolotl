//! Authentication flow interface

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::time::Duration;
use uuid::Uuid;

use crate::State;
use crate::sitmc;
pub use crate::state::YggdrasilLoginResult;
use crate::state::{
    Credentials, MinecraftAccountType, MinecraftLoginFlow, MinecraftProfile,
    YggdrasilAccount,
};
pub use crate::state::{MinecraftDeviceLoginFlow, MinecraftDeviceLoginPoll};
pub use crate::state::{SitmcDeviceLoginFlow, SitmcDeviceLoginPoll};
use crate::util::fetch::INSECURE_REQWEST_CLIENT;

/// Checks whether the launcher's own services are reachable.
///
/// This decides whether the launcher considers itself online, so it probes the
/// club's account site instead of Mojang: the account site is the service this
/// launcher actually depends on, and it stays reachable in networks where
/// Mojang does not.
#[tracing::instrument]
pub async fn check_reachable() -> crate::Result<()> {
    let resp = INSECURE_REQWEST_CLIENT
        .get(sitmc::YGGDRASIL_API_ROOT)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    resp.error_for_status()?;
    Ok(())
}

pub async fn set_mojang_auth_use_mirror(
    use_mirror: bool,
    automatic: bool,
) -> crate::Result<()> {
    let state = State::get().await?;
    state.set_mojang_auth_use_mirror(use_mirror);
    if use_mirror && automatic {
        tracing::info!(
            "Mojang services are unreachable; routing Mojang service requests through the Fallen proxy"
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub struct MojangServiceStatus {
    pub service: &'static str,
    pub url: &'static str,
    pub reachable: bool,
}

const MOJANG_SERVICES: [(&str, &str); 5] = [
    ("auth", "https://authserver.mojang.com/"),
    ("account", "https://api.mojang.com/"),
    (
        "session",
        "https://sessionserver.mojang.com/session/minecraft/hasJoined",
    ),
    ("services", "https://api.minecraftservices.com/"),
    (
        "profiles",
        "https://api.mojang.com/users/profiles/minecraft/",
    ),
];

#[tracing::instrument]
pub async fn check_mojang_services() -> Vec<MojangServiceStatus> {
    futures::future::join_all(MOJANG_SERVICES.map(
        |(service, url)| async move {
            let reachable = INSECURE_REQWEST_CLIENT
                .get(url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
                .is_ok();
            MojangServiceStatus {
                service,
                url,
                reachable,
            }
        },
    ))
    .await
}

#[tracing::instrument]
pub async fn begin_login() -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;

    crate::state::login_begin(&state.pool).await
}

#[tracing::instrument]
pub async fn begin_browser_login() -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;
    crate::state::browser_login_begin(&state.pool).await
}

#[tracing::instrument]
pub async fn begin_device_login() -> crate::Result<MinecraftDeviceLoginFlow> {
    crate::state::device_login_begin().await
}

#[tracing::instrument]
pub async fn poll_device_login(
    device_code: &str,
) -> crate::Result<MinecraftDeviceLoginPoll> {
    let state = State::get().await?;
    crate::state::device_login_poll(device_code, &state.pool).await
}

#[tracing::instrument]
pub async fn finish_login(
    code: &str,
    state: &str,
    flow: MinecraftLoginFlow,
) -> crate::Result<Credentials> {
    let app_state = State::get().await?;

    crate::state::login_finish(code, state, flow, &app_state.pool).await
}

/// Starts a sign-in with the club's account site.
///
/// The player approves a short code in a browser and picks a character there, so
/// the launcher never sees the account password.
#[tracing::instrument]
pub async fn begin_sitmc_device_login() -> crate::Result<SitmcDeviceLoginFlow> {
    crate::state::begin_device_login().await
}

#[tracing::instrument]
pub async fn poll_sitmc_device_login(
    flow_id: Uuid,
) -> crate::Result<SitmcDeviceLoginPoll> {
    let state = State::get().await?;
    crate::state::poll_device_login(flow_id, &state.pool).await
}

/// Signs in with the account name and password of the club's account site.
///
/// This is the fallback for when the OpenID Connect exchange is unavailable.
/// Only the club's own site is accepted, because an account anywhere else cannot
/// play on the club's servers.
#[tracing::instrument(skip(password))]
pub async fn begin_yggdrasil_login(
    login: &str,
    password: &str,
) -> crate::Result<YggdrasilLoginResult> {
    let state = State::get().await?;
    crate::state::begin_yggdrasil_login(
        sitmc::YGGDRASIL_API_ROOT,
        login,
        password,
        &state.pool,
    )
    .await
}

#[tracing::instrument]
pub async fn finish_yggdrasil_login(
    flow_id: uuid::Uuid,
    profile_id: uuid::Uuid,
) -> crate::Result<Credentials> {
    let state = State::get().await?;
    crate::state::finish_yggdrasil_login(flow_id, profile_id, &state.pool).await
}

pub fn normalize_yggdrasil_api_root(api_root: &str) -> crate::Result<String> {
    crate::state::normalize_api_root(api_root)
}

#[tracing::instrument]
pub async fn get_default_user(
    offline_mode: bool,
) -> crate::Result<Option<String>> {
    let state = State::get().await?;
    let user = if offline_mode {
        Credentials::get_offline_credential(&state.pool).await?
    } else {
        Credentials::get_active(&state.pool).await?
    };
    Ok(user
        .filter(Credentials::is_supported_provider)
        .map(|user| user.account_id()))
}

#[tracing::instrument]
pub async fn set_default_user(account_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let users = Credentials::get_all_without_refresh(&state.pool).await?;
    let mut user = users
        .into_iter()
        .find(|user| user.account_id() == account_id)
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to get nonexistent user with ID {account_id}"
            ))
            .as_error()
        })?;

    user.active = true;
    user.upsert(&state.pool).await?;

    Ok(())
}

/// Remove a user account from the database
#[tracing::instrument]
pub async fn remove_user(account_id: &str) -> crate::Result<()> {
    let state = State::get().await?;

    let mut users = Credentials::get_all_without_refresh(&state.pool).await?;

    if let Some(index) = users
        .iter()
        .position(|user| user.account_id() == account_id)
    {
        let user = users.remove(index);
        Credentials::remove(account_id, &state.pool).await?;

        if user.active
            && let Some(mut user) = users.into_iter().next()
        {
            user.active = true;
            user.upsert(&state.pool).await?;
        }
    }

    Ok(())
}

#[derive(Serialize)]
pub struct MinecraftUser {
    pub account_id: String,
    pub profile: MinecraftProfile,
    pub account_type: MinecraftAccountType,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: DateTime<Utc>,
    pub active: bool,
    pub yggdrasil: Option<YggdrasilAccount>,
}

impl MinecraftUser {
    async fn from_credentials(credentials: Credentials) -> Self {
        let profile = (*credentials.maybe_online_profile().await).clone();
        Self {
            account_id: credentials.account_id(),
            profile,
            account_type: credentials.account_type,
            access_token: credentials.access_token,
            refresh_token: credentials.refresh_token,
            expires: credentials.expires,
            active: credentials.active,
            yggdrasil: credentials.yggdrasil,
        }
    }
}

/// Get a copy of the list of all user credentials with profile data ready for
/// serialization.
#[tracing::instrument]
pub async fn users(offline_mode: bool) -> crate::Result<Vec<MinecraftUser>> {
    let state = State::get().await?;
    let users = if offline_mode {
        Credentials::get_all_without_refresh(&state.pool).await?
    } else {
        Credentials::get_all(&state.pool).await?
    };
    let credentials = users
        .into_iter()
        .filter(Credentials::is_supported_provider)
        .filter(|credentials| !offline_mode || credentials.is_offline());
    let mut hydrated_users = Vec::new();
    for credentials in credentials {
        hydrated_users.push(MinecraftUser::from_credentials(credentials).await);
    }
    Ok(hydrated_users)
}
