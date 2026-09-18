import { invoke } from '@tauri-apps/api/core'

/**
 * Rectangle a club map webview occupies, in CSS pixels relative to the main
 * window's content area. The Rust side applies the window scale factor.
 */
export interface ClubMapBounds {
	x: number
	y: number
	width: number
	height: number
}

/**
 * Creates the club map webview if it does not exist yet, then loads `url` at
 * the given bounds. The club servers send `X-Frame-Options: SAMEORIGIN`, so the
 * pages are hosted in a child webview instead of an iframe.
 */
export async function openClubMapWebview(url: string, bounds: ClubMapBounds) {
	await invoke<void>('map_webview_open', {
		url,
		x: bounds.x,
		y: bounds.y,
		width: bounds.width,
		height: bounds.height,
	})
}

/** Moves and resizes the club map webview. A no-op while no map is open. */
export async function setClubMapWebviewBounds(bounds: ClubMapBounds) {
	await invoke<void>('map_webview_set_bounds', {
		x: bounds.x,
		y: bounds.y,
		width: bounds.width,
		height: bounds.height,
	})
}

/** Navigates the club map webview to another map. */
export async function navigateClubMapWebview(url: string) {
	await invoke<void>('map_webview_navigate', { url })
}

/** Destroys the club map webview. Safe to call when no map is open. */
export async function closeClubMapWebview() {
	await invoke<void>('map_webview_close')
}
