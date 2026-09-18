import { getVersion } from '@tauri-apps/api/app'
import { ref } from 'vue'

import { SitmcConfig } from '@/config'
import {
	announcementItems,
	fetchAnnouncementPayload,
	isAnnouncementActive,
	parseAnnouncements,
	type RemoteAnnouncement,
} from '@/helpers/remote-announcements'
import { getUpdateChannel } from '@/helpers/settings'

const CACHE_TTL_MS = 30 * 60 * 1000
const MEMO_TTL_MS = 60 * 1000
const REQUEST_TIMEOUT_MS = 10 * 1000

let endpoint: string | null = null
let memo: { at: number; items: RemoteAnnouncement[] } | null = null
let inFlight: Promise<RemoteAnnouncement[] | null> | null = null

function cacheKey(href: string) {
	return `axolotl-home-announcement:${href}`
}

async function resolveEndpoint(): Promise<string | null> {
	if (endpoint) return endpoint
	if (!SitmcConfig.announcementsUrl) return null

	const url = new URL(SitmcConfig.announcementsUrl)
	try {
		const [version, channel] = await Promise.all([getVersion(), getUpdateChannel()])
		url.searchParams.set('version', version)
		url.searchParams.set('channel', channel === 'release' ? 'stable' : 'beta')
	} catch {
		// Outside the desktop shell the app version is unavailable; the endpoint still answers.
	}
	endpoint = url.href
	return endpoint
}

function readCache(href: string): RemoteAnnouncement[] | null {
	try {
		const cached = JSON.parse(localStorage.getItem(cacheKey(href)) ?? 'null')
		if (
			cached &&
			typeof cached.savedAt === 'number' &&
			Date.now() - cached.savedAt < CACHE_TTL_MS
		) {
			return parseAnnouncements(cached.items)
		}
	} catch {
		// Ignore unreadable caches.
	}
	return null
}

function writeCache(href: string, items: RemoteAnnouncement[]) {
	try {
		localStorage.setItem(cacheKey(href), JSON.stringify({ savedAt: Date.now(), items }))
	} catch {
		// Ignore cache write failures.
	}
}

async function load(): Promise<RemoteAnnouncement[] | null> {
	if (memo && Date.now() - memo.at < MEMO_TTL_MS) return memo.items
	if (inFlight) return inFlight

	const href = await resolveEndpoint()
	if (!href) return null
	const cached = readCache(href)

	const request = (async () => {
		const abort = new AbortController()
		const timeout = setTimeout(() => abort.abort(), REQUEST_TIMEOUT_MS)
		try {
			const payload = await fetchAnnouncementPayload(href, abort.signal)
			if (payload === null) return cached
			const parsed = parseAnnouncements(announcementItems(payload))
			if (!parsed) return cached
			memo = { at: Date.now(), items: parsed }
			writeCache(href, parsed)
			return parsed
		} catch {
			return cached
		} finally {
			clearTimeout(timeout)
			inFlight = null
		}
	})()

	inFlight = request
	return request
}

export function useLatestAnnouncement() {
	const announcement = ref<RemoteAnnouncement | null>(null)
	const loading = ref(true)

	function apply(items: RemoteAnnouncement[] | null) {
		const active = (items ?? [])
			.filter((entry) => isAnnouncementActive(entry))
			.sort((first, second) => Date.parse(second.published_at) - Date.parse(first.published_at))
		announcement.value = active[0] ?? null
	}

	async function refresh() {
		loading.value = true
		try {
			apply(await load())
		} finally {
			loading.value = false
		}
	}

	return { announcement, loading, refresh }
}
