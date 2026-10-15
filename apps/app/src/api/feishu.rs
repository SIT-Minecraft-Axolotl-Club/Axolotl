//! Feishu document component support for the Documentation page.
//!
//! The page embeds the society's wiki through Feishu's own web component, which
//! authenticates with a signature over an application secret. Producing that
//! signature is what these commands are for: the secret stays in the backend
//! and the page only receives the short-lived result.

use crate::api::Result;

/// Application the society registered for the launcher on the Feishu open
/// platform. A build can override both values without a source change.
///
/// The secret is a client-side credential and cannot be kept truly secret in a
/// distributed application; it is here rather than in the frontend bundle so it
/// is at least not published with the web assets.
const FEISHU_APP_ID: &str = match option_env!("AXOLOTL_FEISHU_APP_ID") {
    Some(value) => value,
    None => "cli_aa5ae99c6af81c14",
};
const FEISHU_APP_SECRET: &str = match option_env!("AXOLOTL_FEISHU_APP_SECRET") {
    Some(value) => value,
    None => "fzkAORgZl6b6RHSN4tOTpeePbyyWBAeo",
};

/// Address signed as the page the component is authenticated for.
///
/// Feishu only requires that the component is authenticated with the same string
/// that was signed, and the launcher's own page is served from a custom scheme
/// Feishu would reject, so a stable public address of the society is signed
/// instead. It is never fetched.
const FEISHU_SIGNED_URL: &str = match option_env!("AXOLOTL_FEISHU_SIGNED_URL") {
    Some(value) => value,
    None => theseus::feishu::DEFAULT_SIGNED_URL,
};

fn app() -> theseus::feishu::FeishuApp {
    theseus::feishu::FeishuApp {
        app_id: FEISHU_APP_ID.to_string(),
        app_secret: FEISHU_APP_SECRET.to_string(),
        signed_url: FEISHU_SIGNED_URL.to_string(),
    }
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("feishu")
        .invoke_handler(tauri::generate_handler![
            feishu_component_auth,
            feishu_resolve_document,
            feishu_docs_tree,
        ])
        .build()
}

/// Signs one authentication attempt for the document component.
#[tauri::command]
pub async fn feishu_component_auth()
-> Result<theseus::feishu::FeishuComponentAuth> {
    Ok(theseus::feishu::component_auth(&app()).await?)
}

/// Resolves `docs_url` to the document the component has to render.
///
/// A document address is passed straight through; a wiki address is looked up
/// and the document behind it is used, because Feishu's document component does
/// not support wiki nodes.
#[tauri::command]
pub async fn feishu_resolve_document(
    docs_url: String,
) -> Result<theseus::feishu::FeishuDocumentRef> {
    Ok(theseus::feishu::resolve_document(&app(), &docs_url).await?)
}

/// Lists the pages below the documentation this build points at.
///
/// The document component cannot move between pages, so the Documentation page
/// draws its own navigation from this.
#[tauri::command]
pub async fn feishu_docs_tree(
    docs_url: Option<String>,
) -> Result<theseus::docs_tree::WikiTree> {
    let url = docs_url
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| theseus::docs_tree::DEFAULT_DOCS_URL.to_string());
    Ok(theseus::docs_tree::docs_tree(&app(), &url).await?)
}
