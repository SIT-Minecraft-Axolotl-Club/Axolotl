import { invoke } from '@tauri-apps/api/core'

/**
 * Rectangle a society map webview occupies, in CSS pixels relative to the main
 * window's content area. The Rust side applies the window scale factor.
 */
export interface SocietyMapBounds {
	x: number
	y: number
	width: number
	height: number
}

/**
 * Creates the society map webview if it does not exist yet, then loads `url` at
 * the given bounds. The society servers send `X-Frame-Options: SAMEORIGIN`, so the
 * pages are hosted in a child webview instead of an iframe.
 */
export async function openSocietyMapWebview(url: string, bounds: SocietyMapBounds) {
	await invoke<void>('map_webview_open', {
		url,
		x: bounds.x,
		y: bounds.y,
		width: bounds.width,
		height: bounds.height,
	})
}

/** Moves and resizes the society map webview. A no-op while no map is open. */
export async function setSocietyMapWebviewBounds(bounds: SocietyMapBounds) {
	await invoke<void>('map_webview_set_bounds', {
		x: bounds.x,
		y: bounds.y,
		width: bounds.width,
		height: bounds.height,
	})
}

/** Navigates the society map webview to another map. */
export async function navigateSocietyMapWebview(url: string) {
	await invoke<void>('map_webview_navigate', { url })
}

/** Destroys the society map webview. Safe to call when no map is open. */
export async function closeSocietyMapWebview() {
	await invoke<void>('map_webview_close')
}
