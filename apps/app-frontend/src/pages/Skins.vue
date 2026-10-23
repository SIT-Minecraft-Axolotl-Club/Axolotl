<script setup lang="ts">
import {
	CheckIcon,
	EditIcon,
	ExternalIcon,
	EyeIcon,
	LogInIcon,
	RotateCounterClockwiseIcon,
	SpinnerIcon,
	XIcon,
} from '@modrinth/assets'
import {
	ButtonStyled,
	commonMessages,
	ConfirmModal,
	defineMessages,
	injectNotificationManager,
	useVIntl,
} from '@modrinth/ui'
import SkinPreviewRenderer from '@modrinth/ui/src/components/skin/SkinPreviewRenderer.vue'
import { arrayBufferToBase64 } from '@modrinth/utils'
import { useQuery } from '@tanstack/vue-query'
import { invoke } from '@tauri-apps/api/core'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computedAsync } from '@vueuse/core'
import type { Ref } from 'vue'
import { computed, inject, onMounted, onUnmounted, ref, useTemplateRef, watch } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'

import type AccountsCard from '@/components/ui/AccountsCard.vue'
import EditSkinModal from '@/components/ui/skin/EditSkinModal.vue'
import VirtualSkinSectionList from '@/components/ui/skin/VirtualSkinSectionList.vue'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { SitmcConfig } from '@/config'
import { check_reachable, get_default_user, users } from '@/helpers/auth'
import type { RenderResult } from '@/helpers/rendering/batch-skin-renderer.ts'
import { skinBlobUrlMap } from '@/helpers/rendering/batch-skin-renderer.ts'
import type { Cape, Skin, SkinTextureUrl } from '@/helpers/skins.ts'
import {
	equip_skin,
	filterSavedSkins,
	flush_pending_skin_change,
	flush_pending_skin_change_for_profile,
	get_available_capes,
	get_available_skins,
	get_normalized_skin_texture,
	normalize_skin_texture,
	remove_custom_skin,
	save_custom_skin,
	set_custom_skin_order,
} from '@/helpers/skins.ts'
import { useTheming } from '@/store/state'

async function generateSkinPreviews(skins: Skin[], capes: Cape[]) {
	const { generateSkinPreviews: generate } =
		await import('@/helpers/rendering/skin-preview-renderer')
	await generate(skins, capes)
}

type VirtualSkinSectionListExpose = {
	getAddSkinButtonElement: () => HTMLElement | null | undefined
}

const PENDING_SKIN_REFRESH_DELAY_MS = 11_000
const messages = defineMessages({
	skinSelectorTitle: {
		id: 'app.skins.title',
		defaultMessage: 'Skins',
	},
	rateLimitTitle: {
		id: 'app.skins.rate-limit.title',
		defaultMessage: 'Slow down!',
	},
	rateLimitText: {
		id: 'app.skins.rate-limit.text',
		defaultMessage:
			"You're changing your skin too frequently. Mojang's servers have temporarily blocked further requests. Please wait a moment before trying again.",
	},
	droppedFileErrorTitle: {
		id: 'app.skins.dropped-file-error.title',
		defaultMessage: 'Error processing file',
	},
	droppedFileErrorText: {
		id: 'app.skins.dropped-file-error.text',
		defaultMessage: 'Failed to read the dropped file.',
	},
	reorderSkinErrorTitle: {
		id: 'app.skins.reorder-error.title',
		defaultMessage: 'Failed to reorder skins',
	},
	reorderSkinErrorText: {
		id: 'app.skins.reorder-error.text',
		defaultMessage: 'Your skin order could not be saved.',
	},
	deleteSkinTitle: {
		id: 'app.skins.delete-modal.title',
		defaultMessage: 'Are you sure you want to delete this skin?',
	},
	deleteSkinDescription: {
		id: 'app.skins.delete-modal.description',
		defaultMessage: 'This will permanently delete the selected skin. This action cannot be undone.',
	},
	previewingBadge: {
		id: 'app.skins.previewing-badge',
		defaultMessage: 'Previewing',
	},
	applyButton: {
		id: 'app.skins.apply-button',
		defaultMessage: 'Apply',
	},
	editSkinButton: {
		id: 'app.skins.preview.edit-button',
		defaultMessage: 'Edit skin',
	},
	axolotlAlt: {
		id: 'app.skins.sign-in.axolotl-alt',
		defaultMessage: 'SITMC Launcher',
	},
	signInTitle: {
		id: 'app.skins.sign-in.title',
		defaultMessage: 'Please sign in',
	},
	signInDescription: {
		id: 'app.skins.sign-in.description',
		defaultMessage:
			'Please sign into your Minecraft account to use the skin management features of SITMC Launcher.',
	},
	signInButton: {
		id: 'app.skins.sign-in.button',
		defaultMessage: 'Sign In',
	},
	offlineCompatibility: {
		id: 'app.skins.offline-account.compatibility',
		defaultMessage:
			'Offline skins are applied locally with a resource pack. Compatibility is best on Minecraft 1.6–1.19.2; newer versions are supported on a best-effort basis. The skin is not uploaded to Mojang, may also replace other default-skinned players on your client, and some skin mods or server plugins may override it.',
	},
	offlineCompatibilityTitle: {
		id: 'app.skins.offline-account.compatibility-title',
		defaultMessage: 'Offline skin compatibility',
	},
	thirdPartyManagementTitle: {
		id: 'app.skins.third-party-account.title',
		defaultMessage: 'Third-party skin management',
	},
	thirdPartyManagementDescription: {
		id: 'app.skins.third-party-account.description',
		defaultMessage:
			'Skins for this account are managed by its Yggdrasil provider. Open the provider website to change them.',
	},
	noSkinsTitle: {
		id: 'app.skins.no-skins.title',
		defaultMessage: 'No skins on the skin site yet',
	},
	noSkinsDescription: {
		id: 'app.skins.no-skins.description',
		defaultMessage:
			'Add a PNG below and it is uploaded to the skin site for this character.',
	},
	siteUnreachableTitle: {
		id: 'app.skins.site-error.unreachable.title',
		defaultMessage: 'Cannot reach the skin site',
	},
	siteUnreachableText: {
		id: 'app.skins.site-error.unreachable.text',
		defaultMessage:
			'The skin site did not answer. Check your connection and try again — your skin was not changed.',
	},
	siteTimeoutTitle: {
		id: 'app.skins.site-error.timeout.title',
		defaultMessage: 'The skin site is too slow to answer',
	},
	siteTimeoutText: {
		id: 'app.skins.site-error.timeout.text',
		defaultMessage: 'The request timed out. Try again in a moment — your skin was not changed.',
	},
	siteTooManyChangesTitle: {
		id: 'app.skins.site-error.too-many-changes.title',
		defaultMessage: 'Too many changes at once',
	},
	siteTooManyChangesText: {
		id: 'app.skins.site-error.too-many-changes.text',
		defaultMessage:
			'You are changing your skin too often. Wait a moment before trying again.',
	},
	siteAuthTitle: {
		id: 'app.skins.site-error.auth.title',
		defaultMessage: 'Sign in again to manage your skins',
	},
	siteAuthText: {
		id: 'app.skins.site-error.auth.text',
		defaultMessage:
			'The skin site no longer accepts your session. Sign in again from the account menu.',
	},
	siteRejectedTitle: {
		id: 'app.skins.site-error.rejected.title',
		defaultMessage: 'The skin site rejected this skin',
	},
	siteRejectedText: {
		id: 'app.skins.site-error.rejected.text',
		defaultMessage:
			'The site refused the texture. It has to be a PNG that follows the size rules it shows on its upload page.',
	},
	siteMissingTitle: {
		id: 'app.skins.site-error.missing.title',
		defaultMessage: 'That skin is no longer on the site',
	},
	siteMissingText: {
		id: 'app.skins.site-error.missing.text',
		defaultMessage:
			'The site does not have this texture any more. Reload the page and pick another one.',
	},
	siteServerTitle: {
		id: 'app.skins.site-error.server.title',
		defaultMessage: 'The skin site failed to apply that',
	},
	siteServerText: {
		id: 'app.skins.site-error.server.text',
		defaultMessage: 'The site reported an error. Try again later.',
	},
	siteGenericTitle: {
		id: 'app.skins.site-error.generic.title',
		defaultMessage: 'The skin site refused that',
	},
	siteGenericText: {
		id: 'app.skins.site-error.generic.text',
		defaultMessage: 'The skin site answered: {detail}',
	},
	skinAppliedTitle: {
		id: 'app.skins.applied.title',
		defaultMessage: 'Skin applied',
	},
	skinAppliedText: {
		id: 'app.skins.applied.text',
		defaultMessage: '{name} is now what your character wears.',
	},
	cooldown: {
		id: 'app.skins.cooldown',
		defaultMessage: 'Wait {seconds}s before changing again',
	},
	openSkinLibrary: {
		id: 'app.skins.open-library',
		defaultMessage: 'Open the skin library',
	},
})

const editSkinModal = useTemplateRef('editSkinModal')
const addSkinFileInput = useTemplateRef<HTMLInputElement>('addSkinFileInput')
const skinSectionList = useTemplateRef<VirtualSkinSectionListExpose>('skinSectionList')
const skinPreviewArea = useTemplateRef<HTMLElement>('skinPreviewArea')

const { formatMessage } = useVIntl()
const router = useRouter()
const notifications = injectNotificationManager()
const { addNotification, handleError } = notifications

const themeStore = useTheming()
const skins = ref<Skin[]>([])
const capes = ref<Cape[]>([])
const { browserOffline, offline, setNetworkReachable } = useNetworkStatus()

const accountsCard = inject('accountsCard') as Ref<typeof AccountsCard>
const accountChangeRevision = computed(() => accountsCard.value?.accountChangeRevision)
const currentUser = ref(undefined)
const currentUserId = ref<string | undefined>(undefined)
const currentAccountType = ref<'microsoft' | 'offline' | 'yggdrasil' | undefined>(undefined)

/** The society's own skin site answers its own launcher endpoints; nobody else does. */
const isSocietySkinSite = computed(
	() =>
		currentAccountType.value === 'yggdrasil' &&
		currentUser.value?.yggdrasil?.api_root === SitmcConfig.apiRoot,
)

const username = computed(() => currentUser.value?.profile?.name ?? undefined)
const selectedSkin = ref<Skin | null>(null)
const isApplyingSkin = ref(false)

const originalSelectedSkin = ref<Skin | null>(null)

const savedSkins = computed(() => {
	try {
		return filterSavedSkins(skins.value)
	} catch (error) {
		handleError(error as Error)
		return []
	}
})
const authServerQuery = useQuery({
	queryKey: ['authServerReachability'],
	enabled: computed(() => !browserOffline.value),
	queryFn: async () => {
		try {
			await check_reachable()
			setNetworkReachable(true)
			return true
		} catch (error) {
			setNetworkReachable(false)
			throw error
		}
	},
	refetchInterval: 5 * 60 * 1000,
	retry: false,
	refetchOnWindowFocus: false,
})
const currentCape = computed(() => {
	if (selectedSkin.value?.cape_id) {
		const overrideCape = capes.value.find((c) => c.id === selectedSkin.value?.cape_id)
		if (overrideCape) {
			return overrideCape
		}
	}
	return undefined
})

/**
 * WebGL cannot use a texture the browser loaded as a cross-origin image, and the
 * site that hosts the capes sends no CORS header. The cape is therefore handed to
 * the preview as a same-origin data URL, exactly like the skin texture.
 */
const capeTexture = computedAsync(async () => {
	const cape = currentCape.value
	if (!cape?.texture) {
		return undefined
	}

	try {
		const normalized = await normalize_skin_texture(cape.texture, true)
		const base64 = arrayBufferToBase64(normalized)
		return `data:image/png;base64,${base64}`
	} catch (error) {
		console.warn('Failed to prepare the cape texture for the preview:', error)
		return undefined
	}
})

const skinTexture = computedAsync(async () => {
	const skin = selectedSkin.value
	if (skin?.texture) {
		try {
			return await get_normalized_skin_texture(skin)
		} catch (error) {
			if (skin.texture.startsWith('data:image/')) {
				return skin.texture
			}

			handleError(error as Error)
			return ''
		}
	} else {
		return ''
	}
})
const skinVariant = computed(() => selectedSkin.value?.variant)
const skinNametag = computed(() => (themeStore.hideNametagSkinsPage ? undefined : username.value))
const isSkinManagementReadOnly = computed(
	() =>
		!isSocietySkinSite.value &&
		(currentAccountType.value === 'yggdrasil' ||
			(currentAccountType.value !== 'offline' &&
				(offline.value ||
					(authServerQuery.isError.value && !authServerQuery.isLoading.value)))),
)
const hasPendingSkinChange = computed(
	() => !skinsMatch(selectedSkin.value, originalSelectedSkin.value),
)

let userCheckInterval: number | null = null
let pendingSkinRefreshTimeout: number | null = null
let isUnmounted = false
let unlistenNativeDrop: (() => void) | null = null
let skinNavigationRevision = 0
const skinNavigationRevisions = new WeakMap<object, number>()

const isDraggingSkinFile = ref(false)
const isAddSkinButtonDragActive = ref(false)

const deleteSkinModal = ref()
const skinToDelete = ref<Skin | null>(null)

/**
 * The skin site rate-limits writes per account (`throttle:30,1`), and a player who
 * clicks several skins in a row spends that budget in seconds and then gets
 * nothing but 429s for a minute. Every successful change therefore locks the
 * write actions for a short while and says how long is left.
 */
const WRITE_COOLDOWN_SECONDS = 5
const cooldownSeconds = ref(0)
let cooldownTimer: number | null = null

const isSkinManagementCoolingDown = computed(() => cooldownSeconds.value > 0)
const cooldownHint = computed(() =>
	formatMessage(messages.cooldown, { seconds: cooldownSeconds.value }),
)

function startSkinWriteCooldown() {
	cooldownSeconds.value = WRITE_COOLDOWN_SECONDS
	if (cooldownTimer !== null) window.clearInterval(cooldownTimer)
	cooldownTimer = window.setInterval(() => {
		cooldownSeconds.value = Math.max(0, cooldownSeconds.value - 1)
		if (cooldownSeconds.value === 0 && cooldownTimer !== null) {
			window.clearInterval(cooldownTimer)
			cooldownTimer = null
		}
	}, 1000)
}

const emptyLibraryTitle = computed(() => formatMessage(messages.noSkinsTitle))

const emptyLibraryDescription = computed(() => formatMessage(messages.noSkinsDescription))

/** A society account's closet is its whole library, so an empty one is worth explaining. */
const showEmptyLibraryHint = computed(
	() => isSocietySkinSite.value && savedSkins.value.length === 0,
)

function confirmDeleteSkin(skin: Skin) {
	if (isSkinManagementReadOnly.value) return

	skinToDelete.value = skin
	deleteSkinModal.value?.show()
}

async function deleteSkin() {
	if (isSkinManagementReadOnly.value) return

	const deletedSkin = skinToDelete.value
	if (!deletedSkin) return

	try {
		await remove_custom_skin(deletedSkin)
		removeLocalSkin(deletedSkin)
	} catch (error) {
		handleError(error as Error)
	} finally {
		skinToDelete.value = null
	}
}

async function loadCapes() {
	try {
		capes.value = (await get_available_capes()) ?? []
		console.info(`[skins] loaded ${capes.value.length} capes`)
	} catch (error) {
		console.warn('[skins] failed to load capes', error)
		if (currentUser.value && error instanceof Error) {
			handleError(error)
		}
	}
}

async function loadSkins() {
	const startedAt = performance.now()
	console.info('[skins] requesting the available skins')
	try {
		const loadedSkins = (await get_available_skins()) ?? []
		console.info(
			`[skins] backend returned ${loadedSkins.length} skins in ${Math.round(performance.now() - startedAt)}ms`,
			loadedSkins,
		)
		const loadedEquippedSkin = loadedSkins.find((s) => s.is_equipped)
		const locallyKnownEquippedSkin =
			originalSelectedSkin.value &&
			(loadedSkins.find((skin) => skinsMatch(skin, originalSelectedSkin.value)) ??
				(originalSelectedSkin.value.texture.startsWith('data:image/')
					? originalSelectedSkin.value
					: undefined))
		const shouldPreserveKnownEquippedSkin =
			isSkinManagementReadOnly.value &&
			locallyKnownEquippedSkin &&
			!skinsMatch(loadedEquippedSkin, locallyKnownEquippedSkin)

		skins.value =
			shouldPreserveKnownEquippedSkin && locallyKnownEquippedSkin
				? mergeEquippedSkin(loadedSkins, locallyKnownEquippedSkin)
				: loadedSkins
		generateSkinPreviews(skins.value, capes.value)
		selectedSkin.value = skins.value.find((s) => s.is_equipped) ?? null
		originalSelectedSkin.value = selectedSkin.value
		console.info(
			`[skins] showing ${skins.value.length} skins, equipped: ${selectedSkin.value?.texture_key ?? 'none'}`,
		)
	} catch (error) {
		console.warn('[skins] failed to load the available skins', error)
		if (currentUser.value && error instanceof Error) {
			handleError(error)
		}
	}
}

function mergeEquippedSkin(list: Skin[], equippedSkin: Skin) {
	let foundEquippedSkin = false
	const mergedSkins = list.map((skin) => {
		const isEquipped = skinsMatch(skin, equippedSkin)
		foundEquippedSkin ||= isEquipped

		return {
			...skin,
			is_equipped: isEquipped,
		}
	})

	if (!foundEquippedSkin) {
		mergedSkins.unshift({
			...equippedSkin,
			is_equipped: true,
		})
	}

	return mergedSkins
}

function skinsMatch(a?: Skin | null, b?: Skin | null) {
	return (
		a?.source === b?.source &&
		a?.texture_key === b?.texture_key &&
		a?.variant === b?.variant &&
		(a?.cape_id ?? null) === (b?.cape_id ?? null)
	)
}

function skinsMatchIgnoringSource(a?: Skin | null, b?: Skin | null) {
	return (
		a?.texture_key === b?.texture_key &&
		a?.variant === b?.variant &&
		(a?.cape_id ?? null) === (b?.cape_id ?? null)
	)
}

function isSkinSelected(skin: Skin) {
	return skinsMatch(selectedSkin.value, skin)
}

function isSkinActive(skin: Skin) {
	return hasPendingSkinChange.value && skinsMatch(originalSelectedSkin.value, skin)
}

function getErrorMessage(error: unknown) {
	if (error instanceof Error) return error.message
	if (typeof error === 'string') return error
	if (error && typeof error === 'object') {
		const record = error as Record<string, unknown>
		for (const key of ['message', 'error', 'detail']) {
			if (typeof record[key] === 'string') return record[key]
		}
		try {
			return JSON.stringify(error)
		} catch {
			// fall through to the generic message below
		}
	}
	return String(error)
}

type SiteErrorMessage = { title: string; text: string }

/**
 * The skin site answers with HTTP wording that means nothing to a player, so the
 * usual failures are recognised here and said in the player's own language. The
 * raw text is kept only as the last resort.
 */
function describeSiteError(error: unknown): SiteErrorMessage {
	const raw = getErrorMessage(error)

	if (/\b429\b|Too Many Requests/i.test(raw)) {
		return {
			title: formatMessage(messages.siteTooManyChangesTitle),
			text: formatMessage(messages.siteTooManyChangesText),
		}
	}
	if (/\b401\b|\b403\b|invalid_token|missing_token|no longer valid/i.test(raw)) {
		return {
			title: formatMessage(messages.siteAuthTitle),
			text: formatMessage(messages.siteAuthText),
		}
	}
	if (/\b404\b|\b405\b|not_in_closet|unknown_texture|unknown_player/i.test(raw)) {
		return {
			title: formatMessage(messages.siteMissingTitle),
			text: formatMessage(messages.siteMissingText),
		}
	}
	if (/\b(400|409|413|415|422)\b|validate_failed|file_too_large|invalid_(size|image|type|name)|wrong_texture_type|missing_(player|file|name|texture)/i.test(raw)) {
		return {
			title: formatMessage(messages.siteRejectedTitle),
			text: formatMessage(messages.siteRejectedText),
		}
	}
	if (/\b5\d\d\b/.test(raw)) {
		return {
			title: formatMessage(messages.siteServerTitle),
			text: formatMessage(messages.siteServerText),
		}
	}
	if (/timed? ?out|timeout/i.test(raw)) {
		return {
			title: formatMessage(messages.siteTimeoutTitle),
			text: formatMessage(messages.siteTimeoutText),
		}
	}
	if (/request|connect|network|dns|tcp|unreachable|error sending/i.test(raw)) {
		return {
			title: formatMessage(messages.siteUnreachableTitle),
			text: formatMessage(messages.siteUnreachableText),
		}
	}

	return {
		title: formatMessage(messages.siteGenericTitle),
		text: formatMessage(messages.siteGenericText, { detail: raw }),
	}
}
function isMinecraftSkinRateLimitError(error: unknown) {
	const message = getErrorMessage(error)
	return message.includes('429 Too Many Requests') || message.includes('client error (429')
}

function changeSkin(newSkin: Skin) {
	if (isSkinManagementReadOnly.value) return

	selectedSkin.value = newSkin
}

function resetSelectedSkin() {
	selectedSkin.value =
		skins.value.find((skin) => skinsMatch(skin, originalSelectedSkin.value)) ??
		originalSelectedSkin.value
}

function removeLocalSkin(deletedSkin: Skin) {
	const nextSkins = skins.value.filter((skin) => !skinsMatch(skin, deletedSkin))
	skins.value = nextSkins

	if (selectedSkin.value && skinsMatch(selectedSkin.value, deletedSkin)) {
		selectedSkin.value =
			nextSkins.find((skin) => skinsMatch(skin, originalSelectedSkin.value)) ??
			nextSkins.find((skin) => skin.is_equipped) ??
			null
	}

	if (originalSelectedSkin.value && skinsMatch(originalSelectedSkin.value, deletedSkin)) {
		originalSelectedSkin.value = nextSkins.find((skin) => skin.is_equipped) ?? null
	}

	generateSkinPreviews(skins.value, capes.value)
}

function setLocallyEquippedSkin(skinToApply: Skin) {
	skins.value = skins.value.map((skin) => ({
		...skin,
		is_equipped: skinsMatch(skin, skinToApply),
	}))
	originalSelectedSkin.value =
		skins.value.find((skin) => skinsMatch(skin, skinToApply)) ?? skinToApply
	selectedSkin.value = originalSelectedSkin.value
	void accountsCard.value?.setEquippedSkin(originalSelectedSkin.value)
}

function insertLocalSkin(savedSkin: Skin) {
	const firstNonCustomSkinIndex = skins.value.findIndex((skin) => skin.source !== 'custom')

	if (firstNonCustomSkinIndex === -1) {
		skins.value = [...skins.value, savedSkin]
		return
	}

	const nextSkins = [...skins.value]
	nextSkins.splice(firstNonCustomSkinIndex, 0, savedSkin)
	skins.value = nextSkins
}

function updateLocalSkin(savedSkin: Skin, applied: boolean, previousSkin?: Skin) {
	let foundSkin = false
	const replacesSelectedSkin =
		selectedSkin.value?.texture_key === savedSkin.texture_key ||
		(previousSkin ? skinsMatch(selectedSkin.value, previousSkin) : false)
	const replacesOriginalSkin =
		originalSelectedSkin.value?.texture_key === savedSkin.texture_key ||
		(previousSkin ? skinsMatch(originalSelectedSkin.value, previousSkin) : false)

	skins.value = skins.value.map((skin) => {
		const isUpdatedSkin = skin.texture_key === savedSkin.texture_key
		const isPreviousSkin = previousSkin && skinsMatch(skin, previousSkin)

		if (isUpdatedSkin || isPreviousSkin) {
			foundSkin = true
			return {
				...savedSkin,
				is_equipped: applied || savedSkin.is_equipped,
			}
		}

		return {
			...skin,
			is_equipped: applied ? false : skin.is_equipped,
		}
	})

	if (!foundSkin) {
		insertLocalSkin({
			...savedSkin,
			is_equipped: applied || savedSkin.is_equipped,
		})
	}

	if (applied) {
		const locallyEquippedSkin =
			skins.value.find((skin) => skin.texture_key === savedSkin.texture_key) ?? savedSkin

		originalSelectedSkin.value = locallyEquippedSkin
		selectedSkin.value = locallyEquippedSkin
		void accountsCard.value?.setEquippedSkin(locallyEquippedSkin)
	} else {
		const locallySavedSkin =
			skins.value.find((skin) => skin.texture_key === savedSkin.texture_key) ?? savedSkin

		if (replacesSelectedSkin) {
			selectedSkin.value = locallySavedSkin
		}

		if (replacesOriginalSkin) {
			originalSelectedSkin.value = locallySavedSkin
		}
	}

	generateSkinPreviews(skins.value, capes.value)
}

async function reorderSavedSkins(orderedSkins: Skin[]) {
	// Site textures are ordered by the site's own closet, so there is no launcher
	// side order to persist for them.
	if (isSocietySkinSite.value) return

	const previousSkins = skins.value
	const previousSelectedSkin = selectedSkin.value
	const previousOriginalSelectedSkin = originalSelectedSkin.value
	const orderedTextureKeys = orderedSkins.map((skin) => skin.texture_key)
	const orderedTextureKeySet = new Set(orderedTextureKeys)
	const remainingSavedSkins = previousSkins.filter(
		(skin) => skin.source !== 'default' && !orderedTextureKeySet.has(skin.texture_key),
	)
	const defaultSkins = previousSkins.filter((skin) => skin.source === 'default')
	const nextSavedSkins = [...orderedSkins, ...remainingSavedSkins]

	skins.value = [...nextSavedSkins, ...defaultSkins]
	generateSkinPreviews(skins.value, capes.value)

	try {
		const persistedSavedSkins = await preserveExternalSkins(nextSavedSkins)

		if (persistedSavedSkins.some((skin, index) => skin !== nextSavedSkins[index])) {
			skins.value = [...persistedSavedSkins, ...defaultSkins]
			generateSkinPreviews(skins.value, capes.value)
		}

		await set_custom_skin_order(
			persistedSavedSkins
				.filter((skin) => skin.source === 'custom')
				.map((skin) => skin.texture_key),
		)
	} catch (error) {
		skins.value = previousSkins
		selectedSkin.value = previousSelectedSkin
		originalSelectedSkin.value = previousOriginalSelectedSkin
		generateSkinPreviews(skins.value, capes.value)
		addNotification({
			type: 'error',
			title: formatMessage(messages.reorderSkinErrorTitle),
			text: error instanceof Error ? error.message : formatMessage(messages.reorderSkinErrorText),
		})
		await loadSkins()
	}
}

async function preserveExternalSkins(skinsToPersist: Skin[]) {
	const preservedSkins: Skin[] = []

	for (const skin of skinsToPersist) {
		if (skin.source !== 'custom_external') {
			preservedSkins.push(skin)
			continue
		}

		const textureBlob = await normalize_skin_texture(skin.texture)
		const capeId = skin.cape_id ? capes.value.find((cape) => cape.id === skin.cape_id) : undefined
		const savedSkin = await save_custom_skin(skin, textureBlob, skin.variant, capeId, false)
		const preservedSkin: Skin = {
			...savedSkin,
			source: 'custom',
			is_equipped: skin.is_equipped,
		}

		if (skinsMatchIgnoringSource(selectedSkin.value, skin)) {
			selectedSkin.value = preservedSkin
		}

		if (skinsMatchIgnoringSource(originalSelectedSkin.value, skin)) {
			originalSelectedSkin.value = preservedSkin
			void accountsCard.value?.setEquippedSkin(preservedSkin)
		}

		preservedSkins.push(preservedSkin)
	}

	return preservedSkins
}

function schedulePendingSkinRefresh() {
	if (pendingSkinRefreshTimeout !== null) {
		window.clearTimeout(pendingSkinRefreshTimeout)
	}

	const pendingProfileId = currentUser.value?.profile?.id

	pendingSkinRefreshTimeout = window.setTimeout(async () => {
		pendingSkinRefreshTimeout = null

		if (isUnmounted) {
			return
		}

		try {
			if (pendingProfileId) {
				await flush_pending_skin_change_for_profile(pendingProfileId)
			} else {
				await flush_pending_skin_change()
			}
		} catch (error) {
			handleError(error as Error)
			schedulePendingSkinRefresh()
			return
		}

		if (accountsCard.value) {
			await accountsCard.value.refreshValues()
		}

		await loadCapes()
		await loadSkins()
	}, PENDING_SKIN_REFRESH_DELAY_MS)
}

async function applySelectedSkin() {
	const skinToApply = selectedSkin.value
	if (
		!skinToApply ||
		!hasPendingSkinChange.value ||
		isApplyingSkin.value ||
		isSkinManagementCoolingDown.value ||
		isSkinManagementReadOnly.value
	)
		return

	isApplyingSkin.value = true
	try {
		await equip_skin(skinToApply)
		setLocallyEquippedSkin(skinToApply)
		startSkinWriteCooldown()
		notifications.addNotification({
			type: 'success',
			title: formatMessage(messages.skinAppliedTitle),
			text: formatMessage(messages.skinAppliedText, {
				name: skinToApply.name ?? '',
			}),
		})
		schedulePendingSkinRefresh()
	} catch (error) {
		if (isSocietySkinSite.value) {
			// The site's own wording says nothing useful to a player, and its
			// "slow down" answer is not Mojang's rate limit.
			const described = describeSiteError(error)
			notifications.addNotification({
				type: 'error',
				title: described.title,
				text: described.text,
			})
		} else if (isMinecraftSkinRateLimitError(error)) {
			notifications.addNotification({
				type: 'error',
				title: formatMessage(messages.rateLimitTitle),
				text: formatMessage(messages.rateLimitText),
			})
		} else {
			handleError(error as Error)
		}
	} finally {
		isApplyingSkin.value = false
	}
}

async function onSkinSaved(options: { applied: boolean; skin?: Skin; previousSkin?: Skin }) {
	if (options.skin) {
		updateLocalSkin(options.skin, options.applied, options.previousSkin)
	}

	if (!options.skin) {
		await loadCapes()
		await loadSkins()
	}

	if (options.applied) {
		schedulePendingSkinRefresh()
	}
}

async function loadCurrentUser() {
	try {
		const defaultId = await get_default_user(offline.value)
		currentUserId.value = defaultId

		const allAccounts = await users(offline.value)
		const selectedAccount = allAccounts.find((acc) => acc.account_id === defaultId)
		currentAccountType.value = selectedAccount?.account_type
		currentUser.value = selectedAccount
	} catch (e) {
		handleError(e as Error)
		currentUser.value = undefined
		currentUserId.value = undefined
		currentAccountType.value = undefined
	}
}

async function refreshSelectedAccount() {
	await loadCurrentUser()
	await loadCapes()
	await loadSkins()
}

watch(accountChangeRevision, (revision, previousRevision) => {
	if (revision === undefined || previousRevision === undefined) return
	void refreshSelectedAccount()
})

function getBakedSkinTextures(skin: Skin): RenderResult | undefined {
	const key = `${skin.texture_key}+${skin.variant}+${skin.cape_id ?? 'no-cape'}`
	return skinBlobUrlMap.get(key)
}

async function login() {
	if (offline.value) return
	accountsCard.value?.login()
}

function openAddSkinFileBrowser() {
	if (isSkinManagementReadOnly.value) return

	addSkinFileInput.value?.click()
}

/** The site's own skin library, for browsing skins other players published. */
async function openSkinLibrary() {
	await openUrl(SitmcConfig.skinLibraryUrl).catch((error) => handleError(error as Error))
}

async function onAddSkinFileInputChange(e: Event) {
	if (isSkinManagementReadOnly.value) return

	const files = (e.target as HTMLInputElement).files
	const file = files?.[0]

	if (!file) {
		return
	}

	await processSkinFileBuffer(await file.arrayBuffer())

	if (addSkinFileInput.value) {
		addSkinFileInput.value.value = ''
	}
}

function isSkinImagePath(path: string) {
	return path.toLowerCase().endsWith('.png')
}

function isSkinFileDrag(event: DragEvent) {
	const items = Array.from(event.dataTransfer?.items ?? [])
	const files = Array.from(event.dataTransfer?.files ?? [])

	return (
		items.some((item) => item.kind === 'file' && item.type === 'image/png') ||
		files.some((file) => file.type === 'image/png' || isSkinImagePath(file.name))
	)
}

function onAddSkinDragOver(event: DragEvent) {
	if (isSkinManagementReadOnly.value) return

	if (!isSkinFileDrag(event)) {
		return
	}

	event.preventDefault()
	isAddSkinButtonDragActive.value = true
}

function onAddSkinDragLeave() {
	if (isSkinManagementReadOnly.value) return

	isAddSkinButtonDragActive.value = false
}

async function onAddSkinDrop(event: DragEvent) {
	if (isSkinManagementReadOnly.value) return

	isAddSkinButtonDragActive.value = false

	const file = Array.from(event.dataTransfer?.files ?? []).find(
		(file) => file.type === 'image/png' || isSkinImagePath(file.name),
	)

	if (!file) {
		return
	}

	await processSkinFileBuffer(await file.arrayBuffer())
}

async function processSkinFileBuffer(buffer: Uint8Array | ArrayBuffer) {
	if (isSkinManagementReadOnly.value) return

	const fakeEvent = new MouseEvent('click')
	const originalSkinTexUrl = `data:image/png;base64,` + arrayBufferToBase64(buffer)
	try {
		const skinTextureNormalized = await normalize_skin_texture(originalSkinTexUrl, true)
		const skinTexUrl: SkinTextureUrl = {
			original: originalSkinTexUrl,
			normalized: `data:image/png;base64,` + arrayBufferToBase64(skinTextureNormalized),
		}
		editSkinModal.value?.showNew(fakeEvent, skinTexUrl)
	} catch (error) {
		handleError(error as Error)
	}
}

/** Native drop handler: Tauri blocks HTML5 `drop` for OS-level file drags,
 *  so we need onDragDropEvent to receive the file. HTML5 dragenter/dragover
 *  still fire and provide visual feedback — this only handles the drop. */
async function setupNativeDropHandler() {
	try {
		unlistenNativeDrop = await getCurrentWebview().onDragDropEvent(
			(event: { payload: DragDropEvent }) => {
				const payload = event.payload
				if (payload.type !== 'drop') return
				if (isSkinManagementReadOnly.value) return

				const pngPath = (payload.paths as string[]).find((p: string) =>
					p.toLowerCase().endsWith('.png'),
				)
				if (!pngPath) return

				readDroppedSkinFile(pngPath)
			},
		)
	} catch (error) {
		console.warn('Failed to set up native drop handler on skin page', error)
	}
}

async function readDroppedSkinFile(path: string) {
	try {
		const data = await invoke<ArrayBuffer>('plugin:files|file_read_dragged_file', { path })
		await processSkinFileBuffer(new Uint8Array(data))
	} catch (error) {
		handleError(error as Error)
	}
}

watch(
	() => selectedSkin.value?.cape_id,
	() => {},
)

watch(isSkinManagementReadOnly, (readOnly) => {
	if (readOnly) {
		isDraggingSkinFile.value = false
		isAddSkinButtonDragActive.value = false
	}
})

const removeNavigationFailureHandler = router.afterEach((to, _from, failure) => {
	if (
		failure &&
		skinNavigationRevisions.get(to) === skinNavigationRevision &&
		skinPreviewArea.value
	) {
		skinPreviewArea.value.style.visibility = ''
	}
})

onBeforeRouteLeave(async (to) => {
	const previewArea = skinPreviewArea.value
	if (!previewArea) return true

	skinNavigationRevision += 1
	skinNavigationRevisions.set(to, skinNavigationRevision)
	previewArea.style.visibility = 'hidden'

	await new Promise<void>((resolve) => {
		window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()))
	})
	return true
})

onMounted(() => {
	userCheckInterval = window.setInterval(checkUserChanges, 250)
	void setupNativeDropHandler()
})

onUnmounted(() => {
	isUnmounted = true
	removeNavigationFailureHandler()
	if (userCheckInterval !== null) {
		window.clearInterval(userCheckInterval)
	}

	if (unlistenNativeDrop) {
		unlistenNativeDrop()
		unlistenNativeDrop = null
	}

	if (pendingSkinRefreshTimeout !== null) {
		window.clearTimeout(pendingSkinRefreshTimeout)
		pendingSkinRefreshTimeout = null
	}

	if (cooldownTimer !== null) {
		window.clearInterval(cooldownTimer)
		cooldownTimer = null
	}
})

async function checkUserChanges() {
	try {
		const defaultId = await get_default_user(offline.value)
		if (defaultId !== currentUserId.value) {
			await refreshSelectedAccount()
		}
	} catch (error) {
		if (currentUser.value && error instanceof Error) {
			handleError(error)
		}
	}
}

await Promise.all([loadCapes(), loadCurrentUser()])
await loadSkins()
</script>

<template>
	<EditSkinModal
		ref="editSkinModal"
		:capes="capes"
		@saved="onSkinSaved"
		@deleted="() => loadSkins()"
	/>
	<input
		ref="addSkinFileInput"
		type="file"
		accept="image/png"
		class="hidden"
		@change="onAddSkinFileInputChange"
	/>
	<ConfirmModal
		ref="deleteSkinModal"
		:title="formatMessage(messages.deleteSkinTitle)"
		:description="formatMessage(messages.deleteSkinDescription)"
		:proceed-label="formatMessage(commonMessages.deleteLabel)"
		@proceed="deleteSkin"
	/>

	<div
		v-if="currentUser"
		data-onboarding-id="skins-page"
		class="skin-layout box-border min-h-full p-4"
	>
		<div class="sticky top-6 self-start p-2 pt-0">
			<h1 class="m-0 text-2xl font-bold flex items-center gap-2">
				{{ formatMessage(messages.skinSelectorTitle) }}
			</h1>
			<div
				ref="skinPreviewArea"
				class="ml-5 mt-4 flex h-[calc(80vh-1rem)] items-center justify-center max-[700px]:h-[calc(50vh-1rem)]"
			>
				<SkinPreviewRenderer
					:cape-src="capeTexture"
					:texture-src="skinTexture || ''"
					:variant="skinVariant"
					:nametag="skinNametag"
					:initial-rotation="Math.PI / 8"
				>
					<template v-if="hasPendingSkinChange" #nametag-badge>
						<div
							class="flex w-max shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-full bg-bg-blue px-3 py-1 text-sm font-semibold leading-5 text-brand-blue"
						>
							<EyeIcon class="size-5 shrink-0" />
							{{ formatMessage(messages.previewingBadge) }}
						</div>
					</template>
					<template #subtitle>
						<div
							v-if="hasPendingSkinChange"
							class="flex max-w-[calc(100vw-2rem)] flex-wrap items-center justify-center gap-2 px-2"
						>
							<button
								class="flex h-10 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-[14px] border-0 bg-surface-4 px-4 py-2.5 text-base font-semibold leading-5 text-contrast shadow-md transition-[filter,transform] duration-200 enabled:hover:brightness-[--hover-brightness] enabled:focus-visible:brightness-[--hover-brightness] enabled:active:scale-95 disabled:cursor-not-allowed disabled:opacity-50 [&>svg]:size-5 [&>svg]:shrink-0"
								:disabled="isApplyingSkin || isSkinManagementReadOnly"
								@click="resetSelectedSkin"
							>
								<RotateCounterClockwiseIcon />
								{{ formatMessage(commonMessages.resetButton) }}
							</button>
							<button
								class="flex h-10 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-[14px] border-0 bg-brand px-4 py-2.5 text-base font-semibold leading-5 text-[rgba(0,0,0,0.9)] shadow-md transition-[filter,transform] duration-200 enabled:hover:brightness-[--hover-brightness] enabled:focus-visible:brightness-[--hover-brightness] enabled:active:scale-95 disabled:cursor-not-allowed disabled:opacity-50 [&>svg]:size-5 [&>svg]:shrink-0"
								:disabled="
									isApplyingSkin ||
									isSkinManagementReadOnly ||
									isSkinManagementCoolingDown
								"
								@click="applySelectedSkin"
							>
								<SpinnerIcon v-if="isApplyingSkin" class="animate-spin" />
								<CheckIcon v-else />
								{{ formatMessage(messages.applyButton) }}
							</button>
						</div>
						<button
							v-else
							class="flex h-10 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-[14px] border-0 bg-surface-4 px-4 py-2.5 text-base font-semibold leading-5 shadow-md transition-[filter,transform] duration-200 enabled:hover:brightness-[--hover-brightness] enabled:focus-visible:brightness-[--hover-brightness] enabled:active:scale-95 disabled:cursor-not-allowed disabled:opacity-50 [&>svg]:size-5 [&>svg]:shrink-0"
							:disabled="!selectedSkin || isSkinManagementReadOnly"
							@click="(e: MouseEvent) => selectedSkin && editSkinModal?.show(e, selectedSkin)"
						>
							<EditIcon />
							{{ formatMessage(messages.editSkinButton) }}
						</button>
					</template>
				</SkinPreviewRenderer>
			</div>
		</div>

		<div class="pt-2">
			<div class="mb-4 flex flex-wrap items-center justify-end gap-3">
				<ButtonStyled>
					<button @click="openSkinLibrary">
						<ExternalIcon />
						{{ formatMessage(messages.openSkinLibrary) }}
					</button>
				</ButtonStyled>
			</div>
			<!--
				One list only: the skin site's closet. The launcher does not offer the
				game's bundled skins.
			-->
			<div class="flex flex-col gap-4">
				<div
					v-if="showEmptyLibraryHint"
					class="rounded-lg bg-bg-raised p-6 text-center shadow-lg"
				>
					<h2 class="m-0 text-lg font-semibold text-contrast">
						{{ emptyLibraryTitle }}
					</h2>
					<p class="mb-0 mt-2 text-sm text-secondary">
						{{ emptyLibraryDescription }}
					</p>
				</div>
				<VirtualSkinSectionList
					ref="skinSectionList"
					active-tab="saved"
					:saved-skins="savedSkins"
					:default-skin-sections="[]"
					:get-baked-skin-textures="getBakedSkinTextures"
					:is-skin-selected="isSkinSelected"
					:is-skin-active="isSkinActive"
					:is-add-skin-button-drag-active="isAddSkinButtonDragActive"
					:read-only="isSkinManagementReadOnly"
					@select="changeSkin"
					@edit="(skin, event) => editSkinModal?.show(event, skin)"
					@delete="confirmDeleteSkin"
					@reorder-saved-skins="reorderSavedSkins"
					@add-skin="openAddSkinFileBrowser"
					@add-skin-dragenter="onAddSkinDragOver"
					@add-skin-dragover="onAddSkinDragOver"
					@add-skin-dragleave="onAddSkinDragLeave"
					@add-skin-drop="onAddSkinDrop"
				/>
			</div>
		</div>
	</div>

	<div
		v-else
		data-onboarding-id="skins-page"
		class="box-border flex min-h-full items-center justify-center pt-[25%]"
	>
		<div
			class="relative mx-auto flex w-full max-w-xl flex-col gap-5 rounded-lg bg-bg-raised p-7 shadow-lg"
		>
			<img
				src="@/assets/axolotl.png"
				:alt="formatMessage(messages.axolotlAlt)"
				class="absolute -top-32 right-8 h-36 w-36 object-contain md:right-20"
			/>
			<div
				class="absolute top-0 left-0 w-full h-[1px] opacity-40 bg-gradient-to-r from-transparent via-green-500 to-transparent"
				style="
					background: linear-gradient(
						to right,
						transparent 2rem,
						var(--color-green) calc(100% - 13rem),
						var(--color-green) calc(100% - 5rem),
						transparent calc(100% - 2rem)
					);
				"
			></div>

			<div class="flex flex-col gap-5">
				<h1 class="text-3xl font-extrabold m-0">
					{{ formatMessage(messages.signInTitle) }}
				</h1>
				<p class="text-lg m-0">
					{{ formatMessage(messages.signInDescription) }}
				</p>
				<ButtonStyled
					v-if="!offline"
					v-show="accountsCard"
					color="brand"
					:disabled="accountsCard.loginDisabled"
				>
					<button :disabled="accountsCard.loginDisabled" @click="login">
						<LogInIcon v-if="!accountsCard.loginDisabled" />
						<SpinnerIcon v-else class="animate-spin" />
						{{ formatMessage(messages.signInButton) }}
					</button>
				</ButtonStyled>
			</div>
		</div>
	</div>
</template>

<style lang="scss" scoped>
.skin-layout {
	display: grid;
	grid-template-columns: minmax(0, 1fr) minmax(0, 2.5fr);
	gap: 2.5rem;

	@media (max-width: 700px) {
		grid-template-columns: 1fr;
	}
}
</style>
