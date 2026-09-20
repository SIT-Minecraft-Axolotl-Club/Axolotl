import { arrayBufferToBase64 } from '@modrinth/utils'
import { invoke } from '@tauri-apps/api/core'

export interface Cape {
	id: string
	name: string
	texture: string
	is_equipped: boolean
}

export type SkinModel = 'CLASSIC' | 'SLIM' | 'UNKNOWN'
export type SkinSource = 'default' | 'custom_external' | 'custom'

export interface Skin {
	texture_key: string
	name?: string
	section?: string
	variant: SkinModel
	cape_id?: string
	texture: string
	source: SkinSource
	is_equipped: boolean
}

export interface SkinTextureUrl {
	original: string
	normalized: string
}

export const DEFAULT_MODEL_SORTING = ['Steve', 'Alex'] as string[]

export const DEFAULT_MODELS: Record<string, SkinModel> = {
	Steve: 'CLASSIC',
	Alex: 'SLIM',
	Zuri: 'CLASSIC',
	Sunny: 'CLASSIC',
	Noor: 'SLIM',
	Makena: 'SLIM',
	Kai: 'CLASSIC',
	Efe: 'SLIM',
	Ari: 'CLASSIC',
}

export function filterSavedSkins(list: Skin[]) {
	const customSkins = list.filter((s) => s.source !== 'default')
	fixUnknownSkins(customSkins)
	return customSkins
}

/**
 * How long a texture may take to decode before it is treated as unreadable. A
 * skin card renders behind a loading skeleton until its texture resolves, so a
 * request that never settles would leave the card loading forever.
 */
const MODEL_TYPE_TIMEOUT_MS = 8000

export async function determineModelType(texture: string): Promise<'SLIM' | 'CLASSIC'> {
	return new Promise((resolve, reject) => {
		const canvas = document.createElement('canvas')
		const context = canvas.getContext('2d')

		if (!context) {
			return reject(new Error('Failed to create canvas rendering context.'))
		}

		const image = new Image()
		let settled = false

		function settle(result: 'SLIM' | 'CLASSIC' | Error) {
			if (settled) return
			settled = true
			window.clearTimeout(timeout)
			image.onload = null
			image.onerror = null

			if (result instanceof Error) {
				reject(result)
			} else {
				resolve(result)
			}
		}

		const timeout = window.setTimeout(
			() => settle(new Error('Timed out reading the skin texture.')),
			MODEL_TYPE_TIMEOUT_MS,
		)

		// The texture server does not send CORS headers, so a cross-origin byte
		// read is not available; the canvas then throws and the caller falls back
		// to the classic model. Asking for CORS instead would make an image that
		// never fires either event, which is what left cards loading forever.
		image.src = texture

		image.onload = () => {
			try {
				canvas.width = image.width
				canvas.height = image.height

				context.drawImage(image, 0, 0)

				const armX = 54
				const armY = 20
				const armWidth = 2
				const armHeight = 12
				const imageData = context.getImageData(armX, armY, armWidth, armHeight).data
				for (let alphaIndex = 3; alphaIndex < imageData.length; alphaIndex += 4) {
					if (imageData[alphaIndex] !== 0) {
						settle('CLASSIC')
						return
					}
				}

				settle('SLIM')
			} catch (error) {
				settle(error instanceof Error ? error : new Error(String(error)))
			}
		}

		image.onerror = () => settle(new Error('Failed to load the image.'))
	})
}

export async function fixUnknownSkins(list: Skin[]) {
	const unknownSkins = list.filter((s) => s.variant === 'UNKNOWN')
	for (const unknownSkin of unknownSkins) {
		try {
			unknownSkin.variant = await determineModelType(unknownSkin.texture)
		} catch (error) {
			console.warn('Failed to determine the model of a skin, assuming classic:', error)
			unknownSkin.variant = 'CLASSIC'
		}
	}
}

export function filterDefaultSkins(list: Skin[]) {
	return list
		.filter(
			(s) =>
				s.source === 'default' &&
				(!s.name || !(s.name in DEFAULT_MODELS) || s.variant === DEFAULT_MODELS[s.name]),
		)
		.sort((a, b) => {
			const aIndex = a.name ? DEFAULT_MODEL_SORTING.indexOf(a.name) : -1
			const bIndex = b.name ? DEFAULT_MODEL_SORTING.indexOf(b.name) : -1
			return (aIndex === -1 ? Infinity : aIndex) - (bIndex === -1 ? Infinity : bIndex)
		})
}

export async function get_available_capes(): Promise<Cape[]> {
	return invoke('plugin:minecraft-skins|get_available_capes', {})
}

export async function get_available_skins(): Promise<Skin[]> {
	return invoke('plugin:minecraft-skins|get_available_skins', {})
}

export async function add_and_equip_custom_skin(
	textureBlob: Uint8Array,
	variant: SkinModel,
	cape?: Cape,
): Promise<Skin> {
	return await invoke('plugin:minecraft-skins|add_and_equip_custom_skin', {
		textureBlob,
		variant,
		cape,
	})
}

export async function equip_skin(skin: Skin): Promise<void> {
	await invoke('plugin:minecraft-skins|equip_skin', {
		skin,
	})
}

export async function remove_custom_skin(skin: Skin): Promise<void> {
	await invoke('plugin:minecraft-skins|remove_custom_skin', {
		skin,
	})
}

export async function set_custom_skin_order(textureKeys: string[]): Promise<void> {
	await invoke('plugin:minecraft-skins|set_custom_skin_order', {
		textureKeys,
	})
}

export async function save_custom_skin(
	skin: Skin,
	textureBlob: Uint8Array,
	variant: SkinModel,
	cape: Cape | undefined,
	replaceTexture: boolean,
): Promise<Skin> {
	return await invoke('plugin:minecraft-skins|save_custom_skin', {
		skin,
		textureBlob,
		variant,
		cape,
		replaceTexture,
	})
}

export async function get_normalized_skin_texture(skin: Skin): Promise<string> {
	const data = await normalize_skin_texture(skin.texture, true)
	const base64 = arrayBufferToBase64(data)
	return `data:image/png;base64,${base64}`
}

export async function normalize_skin_texture(
	texture: Uint8Array | string,
	preserveInnerAlpha = false,
): Promise<Uint8Array> {
	return await invoke('plugin:minecraft-skins|normalize_skin_texture', {
		texture,
		preserveInnerAlpha,
	})
}

export async function unequip_skin(): Promise<void> {
	await invoke('plugin:minecraft-skins|unequip_skin')
}

export async function flush_pending_skin_change(): Promise<void> {
	await invoke('plugin:minecraft-skins|flush_pending_skin_change')
}

export async function flush_pending_skin_change_for_profile(profileId: string): Promise<void> {
	await invoke('plugin:minecraft-skins|flush_pending_skin_change_for_profile', {
		profileId,
	})
}

export async function get_dragged_skin_data(path: string): Promise<Uint8Array> {
	const data = await invoke('plugin:minecraft-skins|get_dragged_skin_data', { path })
	return new Uint8Array(data)
}
