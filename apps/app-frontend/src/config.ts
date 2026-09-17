const trimTrailingSlash = (url: string) => url.replace(/\/$/, '')

export const AxolotlBrandConfig = Object.freeze({
	productName: 'Axolotl Launcher',
	shortProductName: 'Axolotl',
	website: 'https://www.sitmc.club/',
	// The launcher is a fork of the upstream Axolotl project, so the repository
	// stays pointed at upstream: it is where the source, LICENSE, and third-party
	// attributions linked from the About page actually live.
	repositoryUrl: 'https://github.com/Mystic-Stars/Axolotl',
	supportUrl: 'https://www.sitmc.club/',
	qqGroupNumber: '',
	qqChannelUrl: '',
	sponsorUrl: '',
	surveyUrl: '',
	bundleIdentifier: 'red.ghs.axolotl',
	deepLinkScheme: 'axolotl',
	userAgent: (version: string, os: string) => `garbage-human-studio/axolotl/${version} (${os})`,
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
 * The club services this build talks to.
 *
 * Accounts live on the club's Blessing Skin site and nowhere else, and the game
 * instances come from the server-side catalog described in
 * `docs/launcher-protocol.md`. Every address the club has to publish itself is a
 * build-time variable that defaults to empty, which disables the feature instead
 * of falling back to another community's server.
 */
const manifestUrl = (import.meta.env.VITE_SITMC_MANIFEST_URL ?? '').trim()
const announcementsUrl = (
	import.meta.env.VITE_SITMC_ANNOUNCEMENTS_URL ??
	import.meta.env.VITE_AXO_ANNOUNCEMENTS_URL ??
	''
).trim()
const updateUrl = (import.meta.env.VITE_SITMC_UPDATE_URL ?? '').trim()

export const SitmcConfig = Object.freeze({
	serverLabel: 'SIT-Minecraft',
	site: 'https://skin.sitmc.club',
	registerUrl: 'https://skin.sitmc.club/auth/register',
	loginUrl: 'https://skin.sitmc.club/auth/login',
	apiRoot: 'https://skin.sitmc.club/api/yggdrasil',
	officialSite: 'https://www.sitmc.club/',
	privacyUrl: 'https://www.sitmc.club/privacy',
	manifestUrl: trimTrailingSlash(manifestUrl),
	announcementsUrl: trimTrailingSlash(announcementsUrl),
	updateUrl: trimTrailingSlash(updateUrl),
})
