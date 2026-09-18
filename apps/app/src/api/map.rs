//! Hosting for the society's map pages.
//!
//! The society's map servers answer with `X-Frame-Options: SAMEORIGIN`, so the
//! pages can never render inside an `<iframe>` on the launcher page and the
//! society decided not to change the servers. A child webview is a top-level
//! browsing context and is therefore not subject to that header, so the
//! frontend only reserves a rectangle and asks this module to paint a webview
//! over it.

use tauri::{
    LogicalPosition, LogicalSize, Webview, WebviewBuilder, WebviewUrl, Window,
};

/// Label of the single webview that hosts the society maps. It is stable so the
/// frontend can open, resize, navigate and close it without holding a handle.
const SOCIETY_MAP_WEBVIEW_LABEL: &str = "society-map";

/// Smallest extent a webview may be given. A container can measure as collapsed
/// while a page transition runs, and Windows rejects a zero-sized child.
const MIN_EXTENT: f64 = 1.0;

/// Coordinates arrive as CSS pixels measured from the main window's content
/// area, which is the logical coordinate space a child webview lives in. Tauri
/// applies the window's scale factor when handing them to the platform, so no
/// manual DPI arithmetic belongs here.
fn bounds(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    let finite = |value: f64, fallback: f64| {
        if value.is_finite() { value } else { fallback }
    };
    (
        LogicalPosition::new(finite(x, 0.0), finite(y, 0.0)),
        LogicalSize::new(
            finite(width, MIN_EXTENT).max(MIN_EXTENT),
            finite(height, MIN_EXTENT).max(MIN_EXTENT),
        ),
    )
}

/// Parses the page to embed. Only remote HTTP(S) pages are accepted: a webview
/// pointed at launcher-local content would share the launcher's own origin.
fn parse_remote_url(url: &str) -> Result<tauri::Url, String> {
    let parsed = url
        .parse::<tauri::Url>()
        .map_err(|error| format!("Invalid society map URL {url}: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!(
            "Refusing to host society map URL {url}: only http and https pages are supported"
        ));
    }
    Ok(parsed)
}

/// Looks the map webview up among this window's own webviews. Scoping the
/// search to the window keeps a surviving webview from another window (the
/// launcher rebuilds its main window in lightweight mode) from being reused or
/// from blocking the creation of a fresh one.
fn society_map_webview(window: &Window) -> Option<Webview> {
    window
        .webviews()
        .into_iter()
        .find(|webview| webview.label() == SOCIETY_MAP_WEBVIEW_LABEL)
}

/// Creates the society map webview if needed, then shows `url` at the requested
/// bounds. Reusing the webview keeps map switches cheap and preserves the
/// page's own session, so only the first open pays for webview creation.
#[tauri::command]
pub fn map_webview_open(
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    window: Window,
) -> Result<(), String> {
    let url = parse_remote_url(&url)?;
    let (position, size) = bounds(x, y, width, height);

    let webview = match society_map_webview(&window) {
        Some(webview) => webview,
        None => window
            .add_child(
                WebviewBuilder::new(
                    SOCIETY_MAP_WEBVIEW_LABEL,
                    WebviewUrl::External(url.clone()),
                ),
                position,
                size,
            )
            .map_err(|error| {
                format!("Failed to create the society map webview: {error}")
            })?,
    };

    webview.set_position(position).map_err(|error| {
        format!("Failed to move the society map webview: {error}")
    })?;
    webview.set_size(size).map_err(|error| {
        format!("Failed to resize the society map webview: {error}")
    })?;

    let target = url.to_string();
    webview.navigate(url).map_err(|error| {
        format!("Failed to load the society map page {target}: {error}")
    })?;
    webview.show().map_err(|error| {
        format!("Failed to show the society map webview: {error}")
    })
}

/// Moves and resizes the society map webview. Called for every layout change the
/// page can observe, so a missing webview is a normal no-op rather than an
/// error the caller would have to special-case.
#[tauri::command]
pub fn map_webview_set_bounds(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    window: Window,
) -> Result<(), String> {
    let Some(webview) = society_map_webview(&window) else {
        return Ok(());
    };
    let (position, size) = bounds(x, y, width, height);

    webview.set_position(position).map_err(|error| {
        format!("Failed to move the society map webview: {error}")
    })?;
    webview.set_size(size).map_err(|error| {
        format!("Failed to resize the society map webview: {error}")
    })
}

/// Navigates the society map webview to another map. The frontend always opens a
/// map before switching, so a missing webview is a no-op.
#[tauri::command]
pub fn map_webview_navigate(url: String, window: Window) -> Result<(), String> {
    let Some(webview) = society_map_webview(&window) else {
        return Ok(());
    };
    let url = parse_remote_url(&url)?;

    let target = url.to_string();
    webview.navigate(url).map_err(|error| {
        format!("Failed to load the society map page {target}: {error}")
    })
}

/// Destroys the society map webview so a page that is left behind cannot keep
/// loading or streaming in the background. Destroying rather than hiding is
/// what releases the webview's own resources, and an absent webview is already
/// the desired end state.
#[tauri::command]
pub fn map_webview_close(window: Window) -> Result<(), String> {
    let Some(webview) = society_map_webview(&window) else {
        return Ok(());
    };

    webview.close().map_err(|error| {
        format!("Failed to close the society map webview: {error}")
    })
}
