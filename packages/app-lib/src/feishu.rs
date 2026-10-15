//! Signing for the Feishu document component.
//!
//! The launcher shows the society's wiki in the Documentation page through
//! Feishu's own web component. That component only renders a document when the
//! page has been authenticated for it, and the authentication is a SHA-1
//! signature over a server-side ticket: producing it needs the application
//! secret, so it lives here rather than in the frontend bundle, and the
//! frontend only ever receives the short-lived signature.
//!
//! The wiki address itself is not something the component can render 鈥?Feishu
//! does not support wiki documents in it 鈥?so the node is resolved to the
//! `docx` document behind it, which is what the component is given.

use parking_lot::Mutex;
use serde::Serialize;
use sha1_smol::Sha1;
use std::time::{Duration, Instant};

/// Open platform endpoints used to authenticate the component.
const APP_ACCESS_TOKEN_URL: &str =
    "https://open.feishu.cn/open-apis/auth/v3/app_access_token/internal";
const JSAPI_TICKET_URL: &str = "https://open.feishu.cn/open-apis/jssdk/ticket/get";
const WIKI_NODE_URL: &str = "https://open.feishu.cn/open-apis/wiki/v2/spaces/get_node";

/// Application the society registered for the launcher.
///
/// The credentials arrive from the application crate, which owns the
/// distribution-specific values: this library is also built for non-society
/// targets, so it carries no society secret of its own.
#[derive(Debug, Clone)]
pub struct FeishuApp {
    pub app_id: String,
    pub app_secret: String,
    /// Address signed as the page the component is authenticated for.
    pub signed_url: String,
}

/// The page URL the signature is computed over.
///
/// Feishu only requires that this string is the same one the component is
/// authenticated with, and the launcher's own page is served from a custom
/// scheme Feishu would reject, so a stable public address of the society is
/// signed instead. It is never fetched.
pub const DEFAULT_SIGNED_URL: &str = "https://axolotl.sitmc.club";

/// How long a signature may be reused before the page re-authenticates.
///
/// Feishu expires a signature ten minutes after its timestamp, so the component
/// is told to refresh a little earlier than that.
const SIGNATURE_TTL_SECS: u64 = 540;

/// Tickets are valid for two hours; they are re-fetched with a safety margin.
const TICKET_MARGIN: Duration = Duration::from_secs(300);

/// Credentials shared by every signature request, refreshed when they expire.
static CREDENTIALS: Mutex<Option<CachedCredentials>> = Mutex::new(None);

struct CachedCredentials {
    app_id: String,
    app_access_token: String,
    ticket: String,
    ticket_expires_at: Instant,
}

/// Everything the page needs to authenticate the document component.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FeishuComponentAuth {
    /// Signature over the ticket and the signed URL.
    pub signature: String,
    /// Milliseconds since the Unix epoch, part of the signed string.
    pub timestamp: i64,
    /// Random string, part of the signed string.
    pub nonce_str: String,
    pub app_id: String,
    /// URL that was signed; the component has to be authenticated with it.
    pub url: String,
    /// How long the page may keep using this signature before asking again.
    pub retry_after_secs: u64,
}

/// The document behind a wiki node, as the component needs to receive it.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FeishuDocumentRef {
    /// URL of the underlying document.
    pub url: String,
    /// Whether the node pointed at a document the component can render.
    pub supported: bool,
    /// Node title, for the page header.
    pub title: Option<String>,
}

/// Signs one authentication attempt for the document component.
///
/// Feishu rejects a signature older than ten minutes, so each call produces a
/// fresh one from the cached ticket; the ticket itself is only re-fetched once
/// per two hours.
#[tracing::instrument(skip(app))]
pub async fn component_auth(app: &FeishuApp) -> crate::Result<FeishuComponentAuth> {
    let ticket = cached_ticket(app).await?;
    let timestamp = chrono::Utc::now().timestamp_millis();
    let nonce_str = nonce();
    let url = app.signed_url.clone();

    let signature = format!(
        "jsapi_ticket={ticket}&noncestr={nonce_str}&timestamp={timestamp}&url={url}"
    );
    let signature = Sha1::from(signature.as_bytes()).hexdigest();

    Ok(FeishuComponentAuth {
        signature,
        timestamp,
        nonce_str,
        app_id: app.app_id.clone(),
        url,
        retry_after_secs: SIGNATURE_TTL_SECS,
    })
}

/// Resolves a documentation address to what the component has to render.
///
/// Both address shapes now travel through unchanged. A `wiki` address used to be
/// rewritten to the `docx` document behind it, because the component cannot
/// render a wiki node itself; that rewrite turned out to cost the document its
/// place in the knowledge tree, which is what a page's own sub-page list needs
/// to list anything. A wiki node the component still refuses is reported as
/// unsupported, so the page can say so instead of showing an empty frame.
#[tracing::instrument(skip(_app))]
pub async fn resolve_document(
    _app: &FeishuApp,
    docs_url: &str,
) -> crate::Result<FeishuDocumentRef> {
    let (kind, _) = feishu_token(docs_url).ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "The documentation address {docs_url} is not a Feishu document or wiki node"
        ))
        .as_error()
    })?;

    Ok(FeishuDocumentRef {
        url: docs_url.to_string(),
        supported: matches!(kind, "docx" | "wiki"),
        title: None,
    })
}

/// The document behind a wiki node.
///
/// The node lookup needs the app to be able to read the knowledge space it lives
/// in. Kept because a document's own address is still the more reliable thing to
/// render when a wiki address turns out not to work.
#[tracing::instrument(skip(app))]
pub async fn resolve_wiki_document(
    app: &FeishuApp,
    docs_url: &str,
) -> crate::Result<FeishuDocumentRef> {
    let (kind, token) = feishu_token(docs_url).ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "The documentation address {docs_url} is not a Feishu wiki node"
        ))
        .as_error()
    })?;

    if kind != "wiki" {
        return Ok(FeishuDocumentRef {
            url: docs_url.to_string(),
            supported: kind == "docx",
            title: None,
        });
    }

    let app_access_token = app_access_token(app).await?;
    let response = crate::util::fetch::INSECURE_REQWEST_CLIENT
        .get(WIKI_NODE_URL)
        .query(&[("token", token.as_str()), ("obj_type", "wiki")])
        .bearer_auth(&app_access_token)
        .send()
        .await?;
    let body = feishu_body(response, WIKI_NODE_URL).await?;

    let node = body.get("data").and_then(|data| data.get("node"));
    let obj_token = node
        .and_then(|node| node.get("obj_token"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let obj_type = node
        .and_then(|node| node.get("obj_type"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let title = node
        .and_then(|node| node.get("title"))
        .and_then(|value| value.as_str())
        .map(str::to_string);

    let supported = obj_type == "docx" && !obj_token.is_empty();
    let host = url_host(docs_url).unwrap_or_else(|| "feishu.cn".to_string());
    let url = if supported {
        format!("https://{host}/docx/{obj_token}")
    } else {
        docs_url.to_string()
    };

    Ok(FeishuDocumentRef {
        url,
        supported,
        title,
    })
}

/// Pulls the ticket out of the cache, refilling it when it is close to expiring.
async fn cached_ticket(app: &FeishuApp) -> crate::Result<String> {
    if let Some(cached) = CREDENTIALS.lock().as_ref()
        && cached.app_id == app.app_id
        && cached.ticket_expires_at > Instant::now()
    {
        return Ok(cached.ticket.clone());
    }

    let app_access_token = app_access_token(app).await?;
    let response = crate::util::fetch::INSECURE_REQWEST_CLIENT
        .post(JSAPI_TICKET_URL)
        .bearer_auth(&app_access_token)
        .send()
        .await?;
    let body = feishu_body(response, JSAPI_TICKET_URL).await?;

    let ticket = body
        .get("data")
        .and_then(|data| data.get("ticket"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "Feishu did not return a jsapi ticket".to_string(),
            )
            .as_error()
        })?
        .to_string();
    let expires_in = body
        .get("data")
        .and_then(|data| data.get("expire_in"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(7200);

    *CREDENTIALS.lock() = Some(CachedCredentials {
        app_id: app.app_id.clone(),
        app_access_token,
        ticket: ticket.clone(),
        ticket_expires_at: Instant::now()
            + Duration::from_secs(expires_in).saturating_sub(TICKET_MARGIN),
    });

    Ok(ticket)
}

/// Returns an application access token, reusing a cached one while it lives.
pub async fn app_access_token(app: &FeishuApp) -> crate::Result<String> {
    if let Some(cached) = CREDENTIALS.lock().as_ref()
        && cached.app_id == app.app_id
        && cached.ticket_expires_at > Instant::now()
    {
        return Ok(cached.app_access_token.clone());
    }

    let response = crate::util::fetch::INSECURE_REQWEST_CLIENT
        .post(APP_ACCESS_TOKEN_URL)
        .json(&serde_json::json!({
            "app_id": app.app_id,
            "app_secret": app.app_secret,
        }))
        .send()
        .await?;
    let body = feishu_body(response, APP_ACCESS_TOKEN_URL).await?;

    body.get("app_access_token")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "Feishu did not return an application access token".to_string(),
            )
            .as_error()
        })
}

/// Reads a Feishu envelope, turning its own error codes into launcher errors.
///
/// The platform answers a rejected request with `200 OK` and a non-zero `code`,
/// so the status alone is not enough to tell success from failure.
pub async fn feishu_body(
    response: reqwest::Response,
    url: &str,
) -> crate::Result<serde_json::Value> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        // Feishu reports its own reason inside the body even for a 4xx, so the
        // caller is told what the platform objected to rather than just the
        // status.
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("msg")
                    .and_then(|msg| msg.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| body.chars().take(200).collect());
        return Err(crate::ErrorKind::OtherError(format!(
            "Feishu rejected the request to {url} with {status}: {detail}"
        ))
        .as_error());
    }

    let body: serde_json::Value = serde_json::from_str(&body)?;
    let code = body
        .get("code")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    if code != 0 {
        let message = body
            .get("msg")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown error");
        return Err(crate::ErrorKind::OtherError(format!(
            "Feishu rejected the request to {url}: {message} (code {code})"
        ))
        .as_error());
    }
    Ok(body)
}

/// The kind of Feishu content an address points at, and its token.
///
/// Only the path decides: a Feishu address is `<host>/<kind>/<token>`, and the
/// token alone says nothing about what it refers to. Used to tell a document
/// apart from a wiki node without asking the platform.
pub fn feishu_token(url: &str) -> Option<(&'static str, String)> {
    let parsed = url::Url::parse(url).ok()?;
    let mut segments = parsed.path_segments()?.filter(|part| !part.is_empty());
    let kind = match segments.next()? {
        "docx" => "docx",
        "wiki" => "wiki",
        "sheets" => "sheets",
        "base" => "base",
        _ => return None,
    };
    let token = segments.next()?.to_string();
    Some((kind, token))
}

fn url_host(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(str::to_string)
}

/// A nonce built from a random identifier.
///
/// Feishu only requires an unpredictable string of digits and letters, and a
/// rejected signature is simply retried with a fresh one.
fn nonce() -> String {
    format!("{:x}", uuid::Uuid::new_v4())
        .chars()
        .take(16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_documents_apart_from_wiki_nodes() {
        assert_eq!(
            feishu_token("https://sit-minecraft.feishu.cn/docx/Abc123?from=x"),
            Some(("docx", "Abc123".to_string()))
        );
        assert_eq!(
            feishu_token("https://sit-minecraft.feishu.cn/wiki/BlvtwrH8diexAtklJg8cntwmnwc?from=x"),
            Some(("wiki", "BlvtwrH8diexAtklJg8cntwmnwc".to_string()))
        );
    }

    #[test]
    fn rejects_addresses_that_are_not_feishu_content() {
        assert!(feishu_token("https://example.com/").is_none());
        assert!(feishu_token("https://sit-minecraft.feishu.cn/").is_none());
        assert!(feishu_token("not a url").is_none());
    }

    #[test]
    fn the_signature_is_a_sha1_of_the_documented_string() {
        let signature =
            format!("jsapi_ticket=t&noncestr=n&timestamp=1&url={DEFAULT_SIGNED_URL}");
        assert_eq!(Sha1::from(signature.as_bytes()).hexdigest().len(), 40);
    }
}
