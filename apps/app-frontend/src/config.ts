const trimTrailingSlash = (url: string) => url.replace(/\/$/, '')

export const AxolotlBrandConfig = Object.freeze({
	productName: 'SITMC Launcher',
	shortProductName: 'SITMC',
	societyName: '上海应用技术大学Minecraft社',
	website: 'https://www.sitmc.club/',
	repositoryUrl: 'https://github.com/SIT-Minecraft-Axolotl-Club/Axolotl',
	// The AGPL LICENSE text only exists upstream; the society fork carries COPYING.md
	// and the third-party attributions that the About page links to as well.
	upstreamRepositoryUrl: 'https://github.com/Mystic-Stars/Axolotl',
	supportUrl: 'https://qm.qq.com/q/YTb3tqvyYE',
	qqGroupNumber: '623697015',
	qqChannelUrl: '',
	sponsorUrl: '',
	surveyUrl: '',
	// The bundle identifier is deliberately unchanged by the rename: Windows keys
	// the installed app, its data directory and the updater on it, so changing it
	// would orphan every existing installation instead of updating it.
	bundleIdentifier: 'red.ghs.axolotl',
	// Same reason: an existing install already registered this deep-link scheme.
	deepLinkScheme: 'axolotl',
	userAgent: (version: string, os: string) => `sitmc-launcher/${version} (${os})`,
	capabilities: Object.freeze({
		publicModrinthApi: true,
		privateModrinthServices: false,
		ghsTelemetry: false,
	}),
})

const siteUrl = trimTrailingSlash(import.meta.env.MODRINTH_URL || 'https://modrinth.com')
const officialLabrinthBaseUrl = trimTrailingSlash(
	import.meta.env.MODRINTH_API_BASE_URL || 'https://api.modrinth.com',
)
type DownloadSourceMode = 'auto' | 'official_only' | 'mirror_preferred' | 'official_preferred'

// The Modrinth API always uses the official source; Modrinth download mirror
// routing is handled by the Rust download layer.
export function setModrinthSourceMode(_sourceMode: DownloadSourceMode) {}

export function setModrinthMirrorEnabled(_enabled: boolean) {}

export function getOfficialLabrinthBaseUrl() {
	return officialLabrinthBaseUrl
}

export function getLabrinthBaseUrl() {
	return officialLabrinthBaseUrl
}

export const config = {
	siteUrl,
	labrinthBaseUrl: getLabrinthBaseUrl,
}

/**
 * The society services this build talks to.
 *
 * Accounts live on the society's Blessing Skin site and nowhere else, and the game
 * instances come from the server-side catalog described in
 * `docs/launcher-protocol.md`. The service addresses are baked in here so a plain
 * `pnpm app:build` produces a working society build; each `VITE_SITMC_*` variable
 * still overrides its default, and setting one to an empty string disables that
 * feature instead of reaching for another community's server.
 */
const DEFAULT_MANIFEST_URL = 'https://skin.sitmc.club/api/launcher/manifest'
const DEFAULT_FRIENDS_URL = 'https://skin.sitmc.club/api/launcher/friends'
const DEFAULT_FRIENDS_INTEROP_URL = 'https://skin.sitmc.club/api/launcher/interop/v1/friends'
const DEFAULT_ANNOUNCEMENTS_URL = 'https://skin.sitmc.club/api/launcher/announcements'
const DEFAULT_UPDATE_URL = 'https://skin.sitmc.club/api/launcher/update'
const DEFAULT_DOCS_URL = 'https://sit-minecraft.feishu.cn/wiki/BlvtwrH8diexAtklJg8cntwmnwc'

const manifestUrl = (import.meta.env.VITE_SITMC_MANIFEST_URL ?? DEFAULT_MANIFEST_URL).trim()
const friendsUrl = (import.meta.env.VITE_SITMC_FRIENDS_URL ?? DEFAULT_FRIENDS_URL).trim()
const friendsInteropUrl = (
	import.meta.env.VITE_SITMC_FRIENDS_INTEROP_URL ?? DEFAULT_FRIENDS_INTEROP_URL
).trim()
const announcementsUrl = (
	import.meta.env.VITE_SITMC_ANNOUNCEMENTS_URL ??
	import.meta.env.VITE_AXO_ANNOUNCEMENTS_URL ??
	DEFAULT_ANNOUNCEMENTS_URL
).trim()
const updateUrl = (import.meta.env.VITE_SITMC_UPDATE_URL ?? DEFAULT_UPDATE_URL).trim()
const docsUrl = (import.meta.env.VITE_SITMC_DOCS_URL ?? DEFAULT_DOCS_URL).trim()

export const SitmcConfig = Object.freeze({
	serverLabel: 'SIT-Minecraft',
	site: 'https://skin.sitmc.club',
	skinLibraryUrl: 'https://skin.sitmc.club/skinlib',
	registerUrl: 'https://skin.sitmc.club/auth/register',
	loginUrl: 'https://skin.sitmc.club/auth/login',
	apiRoot: 'https://skin.sitmc.club/api/yggdrasil',
	officialSite: 'https://www.sitmc.club/',
	privacyUrl: 'https://www.sitmc.club/privacy',
	manifestUrl: trimTrailingSlash(manifestUrl),
	/** Friend-site catalog, grouped by source site. */
	friendsUrl: trimTrailingSlash(friendsUrl),
	/** Versioned mirror of the same friend catalog. */
	friendsInteropUrl: trimTrailingSlash(friendsInteropUrl),
	announcementsUrl: trimTrailingSlash(announcementsUrl),
	updateUrl: trimTrailingSlash(updateUrl),
	docsUrl: trimTrailingSlash(docsUrl),
})
