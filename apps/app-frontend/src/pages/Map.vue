<script setup lang="ts">
import { ExternalIcon } from '@modrinth/assets'
import { ButtonStyled, commonMessages, defineMessages, NavTabs, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import {
	closeClubMapWebview,
	type ClubMapBounds,
	openClubMapWebview,
	setClubMapWebviewBounds,
} from '@/helpers/club-map'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: { id: 'app.map.title', defaultMessage: 'Map' },
	pixelsit: { id: 'app.map.tabs.pixelsit', defaultMessage: 'PixelSIT' },
	survival: { id: 'app.map.tabs.survival', defaultMessage: 'Survival Satellite' },
	hostFailed: {
		id: 'app.map.host-failed',
		defaultMessage: 'This map could not be displayed inside the launcher.',
	},
})

const MAP_STORAGE_KEY = 'axolotl-map-selected-map'

/** Layout changes are coalesced for this long before the webview is moved. */
const BOUNDS_SETTLE_MS = 100

const maps = [
	{
		id: 'pixelsit',
		label: messages.pixelsit,
		url: 'https://www.sitmc.club/pixelsit',
	},
	{
		id: 'survival',
		label: messages.survival,
		url: 'https://survival.sitmc.club/',
	},
] as const

type MapId = (typeof maps)[number]['id']

const DEFAULT_MAP_ID: MapId = 'pixelsit'

function readStoredMapId(): MapId {
	const stored = globalThis.localStorage?.getItem(MAP_STORAGE_KEY)
	return maps.some((map) => map.id === stored) ? (stored as MapId) : DEFAULT_MAP_ID
}

const selectedMapId = ref<MapId>(readStoredMapId())

const selectedMapIndex = computed(() => {
	const index = maps.findIndex((map) => map.id === selectedMapId.value)
	return index === -1 ? 0 : index
})

const selectedMap = computed(() => maps[selectedMapIndex.value])

const tabs = computed(() => maps.map((map) => ({ label: formatMessage(map.label), href: map.id })))

watch(selectedMapId, (mapId) => globalThis.localStorage?.setItem(MAP_STORAGE_KEY, mapId))

function selectMap(index: number) {
	const map = maps[index]
	if (map) selectedMapId.value = map.id
}

async function openSelectedMapInBrowser() {
	await openUrl(selectedMap.value.url)
}

/**
 * Native child webview painted above the page. The club servers send
 * `X-Frame-Options: SAMEORIGIN`, so the page only reserves this rectangle and
 * hands its measured bounds to the Rust side; the webview itself is not part of
 * the document and cannot be styled or covered from here.
 */
const mapHost = ref<HTMLDivElement | null>(null)

/** Readable failure detail, shown instead of the map when hosting it fails. */
const hostError = ref<string | null>(null)

/** Guards against an IPC result that settles after the page has been left. */
let disposed = false

let hostSizeObserver: ResizeObserver | undefined
let boundsTimer: ReturnType<typeof setTimeout> | undefined

function describeError(error: unknown): string {
	if (error instanceof Error) return error.message
	if (typeof error === 'string') return error
	return String(error)
}

/**
 * Bounds of the reserved rectangle in CSS pixels relative to the window's
 * content area, which is the coordinate space the Rust side expects.
 */
function measureMapHost(): ClubMapBounds | null {
	const host = mapHost.value
	if (!host) return null

	const rect = host.getBoundingClientRect()
	if (rect.width < 1 || rect.height < 1) return null

	return { x: rect.left, y: rect.top, width: rect.width, height: rect.height }
}

async function mountMapWebview() {
	const bounds = measureMapHost()
	if (!bounds) return

	try {
		await openClubMapWebview(selectedMap.value.url, bounds)
		// A page that was already left has sent its own close after this open, so
		// it must not touch the map webview any more.
		if (!disposed) hostError.value = null
	} catch (error) {
		if (disposed) return
		hostError.value = describeError(error)
		// Destroy the webview so the fallback stays readable: a surviving native
		// view would paint over it.
		await closeClubMapWebview().catch(() => {})
	}
}

async function syncMapWebviewBounds() {
	if (hostError.value) return

	const bounds = measureMapHost()
	if (!bounds) return

	try {
		await setClubMapWebviewBounds(bounds)
	} catch (error) {
		// The map stays usable at its previous bounds, and while the webview is
		// painted over the page a message here could not be seen anyway.
		console.error('Failed to reposition the club map webview', error)
	}
}

/** Coalesces the resize, scroll and layout bursts a single change produces. */
function scheduleBoundsSync() {
	if (boundsTimer !== undefined) return

	boundsTimer = setTimeout(() => {
		boundsTimer = undefined
		void syncMapWebviewBounds()
	}, BOUNDS_SETTLE_MS)
}

function syncBoundsAfterBecomingVisible() {
	if (document.visibilityState === 'hidden') return
	void syncMapWebviewBounds()
}

watch(selectedMap, () => {
	void mountMapWebview()
})

onMounted(() => {
	void mountMapWebview()

	const host = mapHost.value
	if (host && typeof ResizeObserver !== 'undefined') {
		hostSizeObserver = new ResizeObserver(scheduleBoundsSync)
		hostSizeObserver.observe(host)
	}

	window.addEventListener('resize', scheduleBoundsSync)
	// Scroll events do not bubble, but a capturing listener still sees the
	// page's own scroll container move the rectangle.
	document.addEventListener('scroll', scheduleBoundsSync, { capture: true, passive: true })
	document.addEventListener('visibilitychange', syncBoundsAfterBecomingVisible)
})

onBeforeUnmount(() => {
	disposed = true

	hostSizeObserver?.disconnect()
	hostSizeObserver = undefined

	if (boundsTimer !== undefined) {
		clearTimeout(boundsTimer)
		boundsTimer = undefined
	}

	window.removeEventListener('resize', scheduleBoundsSync)
	document.removeEventListener('scroll', scheduleBoundsSync, { capture: true })
	document.removeEventListener('visibilitychange', syncBoundsAfterBecomingVisible)

	void closeClubMapWebview().catch(() => {})
})
</script>

<template>
	<main class="box-border flex h-full min-h-0 w-full flex-col gap-3 p-6">
		<h1 class="sr-only">{{ formatMessage(messages.title) }}</h1>
		<div class="flex flex-wrap items-center gap-3">
			<div data-onboarding-id="map-tabs">
				<NavTabs
					:active-index="selectedMapIndex"
					:links="tabs"
					mode="local"
					@tab-click="selectMap"
				/>
			</div>
			<ButtonStyled type="outlined">
				<button type="button" @click="openSelectedMapInBrowser">
					<ExternalIcon aria-hidden="true" />
					{{ formatMessage(commonMessages.openInBrowserButton) }}
				</button>
			</ButtonStyled>
		</div>
		<div
			ref="mapHost"
			class="relative min-h-96 flex-1 rounded-[var(--radius-lg)] border border-solid border-divider bg-surface-4"
		>
			<div
				v-if="hostError"
				class="absolute inset-0 flex flex-col items-center justify-center gap-3 p-6 text-center"
			>
				<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.hostFailed) }}</p>
				<p class="m-0 text-xs text-red">{{ hostError }}</p>
				<ButtonStyled type="outlined">
					<button type="button" @click="openSelectedMapInBrowser">
						<ExternalIcon aria-hidden="true" />
						{{ formatMessage(commonMessages.openInBrowserButton) }}
					</button>
				</ButtonStyled>
			</div>
		</div>
	</main>
</template>
