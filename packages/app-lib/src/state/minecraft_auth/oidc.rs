//! Sign-in through the society account site using Yggdrasil Connect.
//!
//! The site runs Blessing Skin with the Yggdrasil Connect extension, which
//! replaces the password-based part of the Yggdrasil API with OpenID Connect.
//! The flow a launcher is meant to use is:
//!
//! 1. the Yggdrasil API metadata announces the OpenID provider through
//!    `meta.feature.openid_configuration_url`;
//! 2. a device authorization grant (RFC 8628) requests
//!    `openid offline_access Yggdrasil.PlayerProfiles.Select Yggdrasil.Server.Join`;
//! 3. the player picks a character on the provider's authorization page, so the
//!    issued access token is bound to that character;
//! 4. the access token *is* the Minecraft access token: the session server
//!    validates it directly and authlib-injector passes it to the game, so there
//!    is no second exchange step;
//! 5. the character and the account are read from the user information endpoint.
//!
//! The classic Yggdrasil username and password login stays available as a
//! fallback for deployments without the extension; see `super::yggdrasil`.

use super::{
    Credentials, MinecraftAccountType, MinecraftProfile, YggdrasilAccount,
};
use super::{YggdrasilProfile, yggdrasil};
use crate::util::fetch::INSECURE_REQWEST_CLIENT;
use crate::{ErrorKind, sitmc};
use base64::Engine;
use base64::prelude::{BASE64_URL_SAFE, BASE64_URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use reqwest::StatusCode;
use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use sqlx::Sqlite;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration as StdDuration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

/// How long a started sign-in may sit around before the player has to begin
/// again. The provider hands out device codes with their own, shorter, lifetime;
/// this only bounds the launcher's own bookkeeping.
const DEVICE_LOGIN_LIFETIME: StdDuration = StdDuration::from_secs(900);

/// Scopes Yggdrasil Connect requires an OpenID provider to support. A provider
/// missing any of them does not implement the protocol, and signing in through
/// it would fail later in a way that is much harder to explain.
const REQUIRED_PROVIDER_SCOPES: [&str; 3] = [
    "openid",
    "Yggdrasil.PlayerProfiles.Select",
    "Yggdrasil.Server.Join",
];

static PENDING_DEVICE_LOGINS: LazyLock<
    Mutex<HashMap<Uuid, PendingDeviceLogin>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

static PROVIDER_METADATA: LazyLock<Mutex<HashMap<String, ProviderMetadata>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// A device authorization grant the player still has to approve.
#[derive(Serialize, Debug, Clone)]
pub struct SitmcDeviceLoginFlow {
    /// Handle the launcher polls with.
    pub flow_id: Uuid,
    /// Short code the player confirms on the site.
    pub user_code: String,
    /// Page where the code has to be entered.
    pub verification_uri: String,
    /// Same page with the code already filled in.
    pub verification_uri_complete: String,
    /// Remaining lifetime of the code, in seconds.
    pub expires_in: u64,
    /// How often the launcher polls, in seconds.
    pub interval: u64,
}

/// State of a sign-in that is waiting for the player.
#[derive(Serialize, Debug)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SitmcDeviceLoginPoll {
    /// The player has not approved the launcher yet.
    Pending { slow_down: bool },
    /// The account is signed in.
    Complete { credentials: Credentials },
}

struct PendingDeviceLogin {
    created: Instant,
    device_code: String,
    client_id: String,
    interval: StdDuration,
    last_poll: Option<Instant>,
    tokens: Option<OidcTokens>,
}

#[derive(Clone, Debug)]
struct OidcTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    /// Kept only as a fallback source for the character; the user information
    /// endpoint is authoritative. See [`id_token_selected_profile`].
    id_token: Option<String>,
}

#[derive(Clone, Debug)]
struct ProviderMetadata {
    device_authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
    client_id: String,
}

#[derive(Deserialize)]
struct RawProviderMetadata {
    #[serde(default)]
    device_authorization_endpoint: Option<String>,
    token_endpoint: String,
    #[serde(default)]
    userinfo_endpoint: Option<String>,
    #[serde(default)]
    scopes_supported: Vec<String>,
    #[serde(default)]
    shared_client_id: Option<String>,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    /// Only used as a fallback source for the character; see
    /// [`id_token_selected_profile`].
    #[serde(default)]
    id_token: Option<String>,
}

/// The user information endpoint returns a superset of the ID token claims,
/// including the character the access token is bound to.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserInfoResponse {
    #[serde(default)]
    selected_profile: Option<YggdrasilProfile>,
}

#[derive(Deserialize)]
struct IdTokenClaims {
    #[serde(default, rename = "selectedProfile")]
    selected_profile: Option<YggdrasilProfile>,
}

#[derive(Deserialize)]
struct OAuthErrorResponse {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

enum DeviceTokenPoll {
    Pending { slow_down: bool },
    Complete(OidcTokens),
}

/// Starts a sign-in and returns the code the player confirms on the site.
#[tracing::instrument]
pub async fn begin_device_login() -> crate::Result<SitmcDeviceLoginFlow> {
    let provider = provider_metadata().await?;
    let response = request_device_code(&provider).await?;
    let flow_id = Uuid::new_v4();
    let interval = response.interval.unwrap_or(5).max(1);

    let mut pending = PENDING_DEVICE_LOGINS.lock().await;
    pending.retain(|_, login| login.created.elapsed() < DEVICE_LOGIN_LIFETIME);
    pending.insert(
        flow_id,
        PendingDeviceLogin {
            created: Instant::now(),
            device_code: response.device_code,
            client_id: provider.client_id.clone(),
            interval: StdDuration::from_secs(interval),
            last_poll: None,
            tokens: None,
        },
    );

    Ok(SitmcDeviceLoginFlow {
        flow_id,
        user_code: response.user_code,
        verification_uri: response.verification_uri,
        verification_uri_complete: response
            .verification_uri_complete
            .unwrap_or_default(),
        expires_in: response.expires_in,
        interval,
    })
}

/// Reports whether the player has approved the sign-in.
///
/// Safe to call repeatedly: once the provider has issued tokens, later calls
/// reuse them instead of asking again.
#[tracing::instrument(skip(exec))]
pub async fn poll_device_login(
    flow_id: Uuid,
    exec: impl sqlx::Executor<'_, Database = Sqlite> + Copy,
) -> crate::Result<SitmcDeviceLoginPoll> {
    let (device_code, client_id, tokens) = {
        let mut pending = PENDING_DEVICE_LOGINS.lock().await;
        pending
            .retain(|_, login| login.created.elapsed() < DEVICE_LOGIN_LIFETIME);
        let login =
            pending.get_mut(&flow_id).ok_or_else(device_login_expired)?;

        if login.tokens.is_none() {
            if let Some(last_poll) = login.last_poll
                && last_poll.elapsed() < login.interval
            {
                return Ok(SitmcDeviceLoginPoll::Pending { slow_down: false });
            }
            login.last_poll = Some(Instant::now());
        }

        (
            login.device_code.clone(),
            login.client_id.clone(),
            login.tokens.clone(),
        )
    };

    let tokens = match tokens {
        Some(tokens) => tokens,
        None => {
            let provider = provider_metadata().await?;
            match request_device_token(
                &provider.token_endpoint,
                &client_id,
                &device_code,
            )
            .await?
            {
                DeviceTokenPoll::Pending { slow_down } => {
                    return Ok(SitmcDeviceLoginPoll::Pending { slow_down });
                }
                DeviceTokenPoll::Complete(tokens) => {
                    let mut pending = PENDING_DEVICE_LOGINS.lock().await;
                    if let Some(login) = pending.get_mut(&flow_id) {
                        login.tokens = Some(tokens.clone());
                    }
                    tokens
                }
            }
        }
    };

    let provider = provider_metadata().await?;
    let credentials = complete_login(&provider, &tokens, exec).await?;
    let _ = PENDING_DEVICE_LOGINS.lock().await.remove(&flow_id);

    Ok(SitmcDeviceLoginPoll::Complete { credentials })
}

/// Renews a session that was opened through the account site.
///
/// Called when the access token can no longer be used, which is what an OpenID
/// Connect refresh token is for. The provider may not return a new character, so
/// the stored one is kept unless the user information endpoint answers.
pub(super) async fn refresh_credentials_with_oidc(
    credentials: &mut Credentials,
    exec: impl sqlx::Executor<'_, Database = Sqlite> + Copy,
) -> crate::Result<()> {
    let provider = provider_metadata().await?;
    let tokens =
        request_refreshed_tokens(&provider, &credentials.refresh_token).await?;

    let lifetime = token_lifetime(&tokens);
    credentials.access_token = tokens.access_token.clone();
    if let Some(refresh_token) = tokens.refresh_token.clone() {
        credentials.refresh_token = refresh_token;
    }
    credentials.expires = Utc::now() + lifetime;

    match fetch_selected_profile(
        &provider.userinfo_endpoint,
        &tokens.access_token,
    )
    .await
    {
        Ok(Some(profile)) => {
            credentials.offline_profile.id = profile.id;
            credentials.offline_profile.name = profile.name;
        }
        Ok(None) => {}
        Err(error) => tracing::warn!(
            "Keeping the stored character after a refresh; the user information endpoint did not answer: {error}"
        ),
    }

    credentials.upsert(exec).await?;

    Ok(())
}

async fn complete_login(
    provider: &ProviderMetadata,
    tokens: &OidcTokens,
    exec: impl sqlx::Executor<'_, Database = Sqlite> + Copy,
) -> crate::Result<Credentials> {
    let profile =
        fetch_selected_profile(&provider.userinfo_endpoint, &tokens.access_token)
            .await?
            .or_else(|| {
                tokens
                    .id_token
                    .as_deref()
                    .and_then(id_token_selected_profile)
            })
            .ok_or_else(|| {
                ErrorKind::OtherError(
                    "The account site did not report the character this sign-in is bound to. Start the sign-in again and pick a character on the authorization page."
                        .to_string(),
                )
                .as_error()
            })?;

    let server_name =
        yggdrasil::fetch_yggdrasil_metadata(sitmc::YGGDRASIL_API_ROOT)
            .await
            .map_or_else(
                |_| sitmc::SITE_LABEL.to_string(),
                |metadata| metadata.server_name,
            );

    let credentials = Credentials {
        account_id: Some(format!(
            "yggdrasil:{}:{}",
            sitmc::YGGDRASIL_API_ROOT,
            profile.id.as_hyphenated()
        )),
        offline_profile: MinecraftProfile {
            id: profile.id,
            name: profile.name,
            ..MinecraftProfile::default()
        },
        account_type: MinecraftAccountType::Yggdrasil,
        access_token: tokens.access_token.clone(),
        refresh_token: tokens.refresh_token.clone().unwrap_or_default(),
        expires: Utc::now() + token_lifetime(tokens),
        active: true,
        yggdrasil: Some(YggdrasilAccount {
            api_root: sitmc::YGGDRASIL_API_ROOT.to_string(),
            server_name,
            login: String::new(),
            client_token: String::new(),
        }),
    };
    credentials.upsert(exec).await?;

    Ok(credentials)
}

/// Lifetime of the freshly issued access token, with a conservative fallback for
/// providers that omit `expires_in`.
fn token_lifetime(tokens: &OidcTokens) -> Duration {
    Duration::seconds(tokens.expires_in.unwrap_or(3600).max(60) as i64)
}

/// Reads the OpenID provider description the account site announces.
///
/// Cached per API root: it changes only when the site is reconfigured, and the
/// device flow needs its endpoints more than once.
async fn provider_metadata() -> crate::Result<ProviderMetadata> {
    if let Some(metadata) = PROVIDER_METADATA
        .lock()
        .await
        .get(sitmc::YGGDRASIL_API_ROOT)
        .cloned()
    {
        return Ok(metadata);
    }

    let api_metadata =
        yggdrasil::fetch_yggdrasil_metadata(sitmc::YGGDRASIL_API_ROOT).await?;
    let discovery_url = yggdrasil::openid_configuration_url(&api_metadata.raw)
        .ok_or_else(|| {
            ErrorKind::OtherError(
                "The account site does not announce an OpenID provider, so \
                 Yggdrasil Connect sign-in is unavailable. Sign in with the \
                 account name and password instead, or ask the site \
                 administrator to enable the Yggdrasil Connect feature."
                    .to_string(),
            )
            .as_error()
        })?;

    let raw: RawProviderMetadata = get_json(&discovery_url).await?;

    let missing: Vec<&str> = REQUIRED_PROVIDER_SCOPES
        .iter()
        .copied()
        .filter(|scope| !raw.scopes_supported.iter().any(|it| it == scope))
        .collect();
    if !missing.is_empty() {
        return Err(ErrorKind::OtherError(format!(
            "The account site does not support Yggdrasil Connect (its OpenID provider is missing the {} scope(s)). Sign in with the account name and password instead.",
            missing.join(", ")
        ))
        .as_error());
    }

    let device_authorization_endpoint =
        raw.device_authorization_endpoint.ok_or_else(|| {
            ErrorKind::OtherError(
                "The account site does not offer the device authorization \
                 grant, which is how this launcher signs in without asking for \
                 your password. Sign in with the account name and password \
                 instead."
                    .to_string(),
            )
            .as_error()
        })?;
    let userinfo_endpoint = raw.userinfo_endpoint.ok_or_else(|| {
        ErrorKind::OtherError(
            "The account site does not offer the user information endpoint, so \
             the launcher cannot tell which character to start the game with."
                .to_string(),
        )
        .as_error()
    })?;

    let metadata = ProviderMetadata {
        device_authorization_endpoint,
        token_endpoint: raw.token_endpoint,
        userinfo_endpoint,
        client_id: raw
            .shared_client_id
            .unwrap_or_else(|| sitmc::OIDC_CLIENT_ID.to_string()),
    };

    PROVIDER_METADATA
        .lock()
        .await
        .insert(sitmc::YGGDRASIL_API_ROOT.to_string(), metadata.clone());

    Ok(metadata)
}

/// The character the access token is bound to.
///
/// The user information endpoint is the spec's own way of validating an access
/// token, so a successful answer here also proves the token still works.
async fn fetch_selected_profile(
    userinfo_endpoint: &str,
    access_token: &str,
) -> crate::Result<Option<YggdrasilProfile>> {
    let response = INSECURE_REQWEST_CLIENT
        .get(userinfo_endpoint)
        .header(ACCEPT, "application/json")
        .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN")
        .bearer_auth(access_token)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(response_error(response).await);
    }

    let userinfo: UserInfoResponse = response.json().await?;
    Ok(userinfo.selected_profile)
}

/// Reads `selectedProfile` out of an ID token without verifying it.
///
/// Only a fallback for providers that omit the claim from their user information
/// response: the value decides which character the launcher starts, and the
/// session server still validates the access token it is bound to. The primary
/// source is always the user information endpoint, which is answered over TLS by
/// the provider itself.
fn id_token_selected_profile(id_token: &str) -> Option<YggdrasilProfile> {
    let payload = id_token.split('.').nth(1)?;
    let decoded = BASE64_URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| BASE64_URL_SAFE.decode(payload))
        .ok()?;
    let claims: IdTokenClaims = serde_json::from_slice(&decoded).ok()?;
    claims.selected_profile
}

async fn request_device_code(
    provider: &ProviderMetadata,
) -> crate::Result<DeviceCodeResponse> {
    let response = INSECURE_REQWEST_CLIENT
        .post(&provider.device_authorization_endpoint)
        .header(ACCEPT, "application/json")
        .form(&[
            ("client_id", provider.client_id.as_str()),
            ("scope", sitmc::OIDC_SCOPE),
        ])
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(oauth_error(status, &body));
    }

    serde_json::from_str(&body).map_err(|error| {
        ErrorKind::OtherError(format!(
            "The account site returned an unexpected sign-in response: {error}"
        ))
        .as_error()
    })
}

async fn request_device_token(
    token_endpoint: &str,
    client_id: &str,
    device_code: &str,
) -> crate::Result<DeviceTokenPoll> {
    let response = INSECURE_REQWEST_CLIENT
        .post(token_endpoint)
        .header(ACCEPT, "application/json")
        .form(&[
            ("grant_type", sitmc::OIDC_DEVICE_CODE_GRANT_TYPE),
            ("client_id", client_id),
            ("device_code", device_code),
        ])
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if let Ok(error) = serde_json::from_str::<OAuthErrorResponse>(&body) {
        return match error.error.as_str() {
            "authorization_pending" => {
                Ok(DeviceTokenPoll::Pending { slow_down: false })
            }
            "slow_down" => Ok(DeviceTokenPoll::Pending { slow_down: true }),
            "expired_token" => Err(ErrorKind::InputError(
                "The SIT-Minecraft sign-in request expired; start it again."
                    .to_string(),
            )
            .as_error()),
            _ => Err(oauth_error(status, &body)),
        };
    }

    if !status.is_success() {
        return Err(oauth_error(status, &body));
    }

    Ok(DeviceTokenPoll::Complete(parse_tokens(&body)?))
}

async fn request_refreshed_tokens(
    provider: &ProviderMetadata,
    refresh_token: &str,
) -> crate::Result<OidcTokens> {
    let response = INSECURE_REQWEST_CLIENT
        .post(&provider.token_endpoint)
        .header(ACCEPT, "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", provider.client_id.as_str()),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(oauth_error(status, &body));
    }

    parse_tokens(&body)
}

fn parse_tokens(body: &str) -> crate::Result<OidcTokens> {
    let token: TokenResponse = serde_json::from_str(body).map_err(|error| {
        ErrorKind::OtherError(format!(
            "The account site returned an unexpected token response: {error}"
        ))
        .as_error()
    })?;

    Ok(OidcTokens {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_in: token.expires_in,
        id_token: token.id_token,
    })
}

async fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> crate::Result<T> {
    let response = INSECURE_REQWEST_CLIENT
        .get(url)
        .header(ACCEPT, "application/json")
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(response_error(response).await);
    }
    Ok(response.json().await?)
}

/// Turns an OAuth failure into an error that says what to do about it.
fn oauth_error(status: StatusCode, body: &str) -> crate::Error {
    let parsed = serde_json::from_str::<OAuthErrorResponse>(body).ok();
    let (code, detail) = match parsed {
        Some(error) => {
            let detail = match error.error_description {
                Some(description) if !description.trim().is_empty() => {
                    format!("{}: {description}", error.error)
                }
                _ => error.error.clone(),
            };
            (error.error, detail)
        }
        None => (
            String::new(),
            if body.trim().is_empty() {
                format!("HTTP {status}")
            } else {
                format!("HTTP {status}: {body}")
            },
        ),
    };

    let hint = match code.as_str() {
        "invalid_scope" => {
            " The account site has to allow the launcher to request \
             `openid`, `Yggdrasil.PlayerProfiles.Select` and \
             `Yggdrasil.Server.Join`."
        }
        "invalid_client" => {
            " The launcher's client ID has to be allowed to use the \
             device authorization grant on the account site."
        }
        "access_denied" => {
            " The sign-in was refused on the authorization page."
        }
        _ => "",
    };

    ErrorKind::OtherError(format!(
        "The SIT-Minecraft account site rejected the sign-in ({detail}).{hint}"
    ))
    .as_error()
}

async fn response_error(response: reqwest::Response) -> crate::Error {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<OAuthErrorResponse>(&body)
        .ok()
        .and_then(|error| {
            error
                .error_description
                .or(Some(error.error))
                .filter(|message| !message.trim().is_empty())
        })
        .unwrap_or_else(|| {
            if body.trim().is_empty() {
                format!("The account site returned HTTP {status}")
            } else {
                format!("The account site returned HTTP {status}: {body}")
            }
        });
    ErrorKind::OtherError(message).as_error()
}

fn device_login_expired() -> crate::Error {
    ErrorKind::InputError(
        "The SIT-Minecraft sign-in request expired; start it again."
            .to_string(),
    )
    .as_error()
}
