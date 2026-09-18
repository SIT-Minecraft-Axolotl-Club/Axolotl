import type { MessageDescriptor } from '@modrinth/ui'

import {
	settingsCategoryDefinitions,
	type SettingsCategoryId,
} from './settings-category-definitions.ts'

export interface SettingsSearchEntry {
	id: string
	categoryId: SettingsCategoryId
	targetId?: string
	label: MessageDescriptor
	description?: MessageDescriptor
	keywords?: MessageDescriptor[]
}

const message = (id: string, defaultMessage: string): MessageDescriptor => ({ id, defaultMessage })

export const settingsSearchEntries: SettingsSearchEntry[] = [
	{
		id: 'appearance-color-theme',
		categoryId: 'interface',
		targetId: 'settings-target-appearance-color-theme',
		label: message('app.appearance-settings.color-theme.title', 'Color theme'),
		description: message(
			'app.appearance-settings.color-theme.description',
			'Select your preferred color theme for Axolotl Launcher.',
		),
	},
	{
		id: 'appearance-accent-color',
		categoryId: 'interface',
		targetId: 'settings-target-appearance-accent-color',
		label: message('app.appearance-settings.accent-color.title', 'Accent color'),
		description: message(
			'app.appearance-settings.accent-color.description',
			'Choose the color used for buttons, selections, and highlights.',
		),
	},
	{
		id: 'appearance-launcher-background',
		categoryId: 'interface',
		targetId: 'settings-target-appearance-launcher-background',
		label: message('app.appearance-settings.custom-background.title', 'Launcher background'),
		description: message(
			'app.appearance-settings.custom-background.description',
			'Choose a custom image and fine-tune how it blends with the launcher interface.',
		),
	},
	{
		id: 'appearance-transparent-background',
		categoryId: 'interface',
		targetId: 'settings-target-appearance-transparent-background',
		label: message(
			'app.appearance-settings.transparent-background.title',
			'Transparent background',
		),
		description: message(
			'app.appearance-settings.transparent-background.description',
			'Let your desktop show through the launcher window.',
		),
	},
	{
		id: 'appearance-advanced-rendering',
		categoryId: 'interface',
		targetId: 'settings-target-appearance-advanced-rendering',
		label: message('app.appearance-settings.advanced-rendering.title', 'Advanced rendering'),
		description: message(
			'app.appearance-settings.advanced-rendering.description',
			'Enable advanced visual effects that may affect performance.',
		),
	},
	{
		id: 'appearance-page-transitions',
		categoryId: 'interface',
		targetId: 'settings-target-appearance-page-transitions',
		label: message('app.appearance-settings.page-transitions.title', 'Page transition animations'),
	},
	{
		id: 'appearance-home-layout',
		categoryId: 'home-navigation',
		targetId: 'settings-target-appearance-home-layout',
		label: message('app.appearance-settings.home-layout.title', 'Home layout'),
	},
	{
		id: 'appearance-default-landing-page',
		categoryId: 'home-navigation',
		targetId: 'settings-target-appearance-default-landing-page',
		label: message('app.appearance-settings.default-landing-page.title', 'Default landing page'),
	},
	{
		id: 'appearance-sidebar-instance-limit',
		categoryId: 'home-navigation',
		targetId: 'settings-target-appearance-sidebar-instance-limit',
		label: message(
			'app.appearance-settings.sidebar-instance-count.title',
			'Sidebar instance limit',
		),
	},
	{
		id: 'appearance-auto-hide-downloads',
		categoryId: 'home-navigation',
		targetId: 'settings-target-appearance-auto-hide-downloads',
		label: message(
			'app.appearance-settings.auto-hide-downloads-button.title',
			'Auto-hide downloads button',
		),
	},
	{
		id: 'logs-level',
		categoryId: 'logs',
		targetId: 'settings-target-logs-level',
		label: message('app.settings.logs.level.title', 'Log level'),
		description: message(
			'app.settings.logs.level.description',
			'Choose the lowest level written to the launcher log files.',
		),
	},
	{
		id: 'logs-export',
		categoryId: 'logs',
		targetId: 'settings-target-logs-export',
		label: message('app.settings.logs.export.title', 'Export logs'),
		description: message(
			'app.settings.logs.export.description',
			'Package launcher logs with an optional environment summary, instance log, and crash analysis.',
		),
	},
	{
		id: 'appearance-show-scroll-top',
		categoryId: 'home-navigation',
		targetId: 'settings-target-appearance-show-scroll-top',
		label: message('app.appearance-settings.show-scroll-top.title', 'Show "back to top" button'),
		description: message(
			'app.appearance-settings.show-scroll-top.description',
			'Show a floating back-to-top button on scrollable pages.',
		),
	},
	{
		id: 'shortcuts-quick-scroll',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcuts-enable',
		label: message('app.shortcut-settings.enable', 'Enable quick scrolling'),
		description: message(
			'app.shortcut-settings.scroll-description',
			'Move through long pages with the keyboard.',
		),
	},
	{
		id: 'shortcuts-scroll-home',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcutScrollTop',
		label: message('app.shortcut-settings.home-action', 'Scroll to the top'),
		description: message(
			'app.shortcut-settings.home-action-description',
			'Moves the page you are reading to the top.',
		),
	},
	{
		id: 'shortcuts-scroll-end',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcutScrollBottom',
		label: message('app.shortcut-settings.end-action', 'Scroll to the bottom'),
		description: message(
			'app.shortcut-settings.end-action-description',
			'Moves the page you are reading to the bottom.',
		),
	},
	{
		id: 'shortcuts-scroll-page-up',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcutScrollUp',
		label: message('app.shortcut-settings.page-up-action', 'Scroll up one screen'),
		description: message(
			'app.shortcut-settings.page-up-action-description',
			'Moves the page you are reading up by one screen.',
		),
	},
	{
		id: 'shortcuts-scroll-page-down',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcutScrollDown',
		label: message('app.shortcut-settings.page-down-action', 'Scroll down one screen'),
		description: message(
			'app.shortcut-settings.page-down-action-description',
			'Moves the page you are reading down by one screen.',
		),
	},
	{
		id: 'shortcuts-nav-home',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcuts-nav',
		label: message('app.shortcut-settings.nav-home', 'Home'),
		description: message('app.shortcut-settings.nav-home-description', 'Jump to the Home page.'),
	},
	{
		id: 'shortcuts-nav-map',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcuts-nav',
		label: message('app.shortcut-settings.nav-map', 'Map'),
		description: message('app.shortcut-settings.nav-map-description', 'Open the map.'),
	},
	{
		id: 'shortcuts-nav-downloads',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcuts-nav',
		label: message('app.shortcut-settings.nav-downloads', 'Downloads'),
		description: message('app.shortcut-settings.nav-downloads-description', 'Jump to downloads.'),
	},
	{
		id: 'shortcuts-nav-settings',
		categoryId: 'shortcut-settings',
		targetId: 'settings-target-shortcuts-nav',
		label: message('app.shortcut-settings.nav-settings', 'Settings'),
		description: message('app.shortcut-settings.nav-settings-description', 'Jump to the settings.'),
	},
	{
		id: 'appearance-native-decorations',
		categoryId: 'interface',
		targetId: 'settings-target-appearance-native-decorations',
		label: message('app.appearance-settings.native-decorations.title', 'Native decorations'),
	},
	{
		id: 'appearance-close-behavior',
		categoryId: 'interface',
		targetId: 'settings-target-appearance-close-behavior',
		label: message(
			'app.appearance-settings.close-behavior.title',
			'Choose how to close Axolotl Launcher',
		),
		keywords: [
			message('app.appearance-settings.close-behavior.close', 'Close directly'),
			message('app.appearance-settings.close-behavior.lightweight', 'Hide to tray'),
			message('app.appearance-settings.lightweight-mode.title', 'Lightweight mode'),
		],
	},
	{
		id: 'appearance-show-play-time',
		categoryId: 'home-navigation',
		targetId: 'settings-target-appearance-show-play-time',
		label: message('app.appearance-settings.show-play-time.title', 'Show play time'),
	},
	{
		id: 'appearance-unknown-pack-warning',
		categoryId: 'content-downloads',
		targetId: 'settings-target-appearance-unknown-pack-warning',
		label: message(
			'app.appearance-settings.unknown-pack-warning.title',
			'Warn me before installing unknown modpacks',
		),
	},
	{
		id: 'content-auto-install-dependencies',
		categoryId: 'content-downloads',
		targetId: 'settings-target-content-auto-install-dependencies',
		label: message(
			'app.appearance-settings.auto-install-dependencies.title',
			'Automatically install dependencies',
		),
	},
	{
		id: 'content-skip-nonessential-warnings',
		categoryId: 'content-downloads',
		targetId: 'settings-target-content-skip-nonessential-warnings',
		label: message(
			'app.appearance-settings.skip-non-essential-warnings.title',
			'Skip non-essential warnings',
		),
	},
	{
		id: 'language-launcher-language',
		categoryId: 'language-translation',
		targetId: 'settings-target-language',
		label: message('app.settings.tabs.language', 'Language'),
	},
	{
		id: 'privacy-telemetry',
		categoryId: 'privacy-data',
		targetId: 'settings-target-privacy-telemetry',
		label: message('app.settings.privacy.telemetry', 'Telemetry'),
	},
	{
		id: 'privacy-discord-rpc',
		categoryId: 'privacy-data',
		targetId: 'settings-target-privacy-discord-rpc',
		label: message('app.settings.privacy.discord-rpc', 'Discord rich presence'),
	},
	{
		id: 'java-installations',
		categoryId: 'java-performance',
		label: message('app.settings.tabs.java-installations', 'Java installations'),
	},
	{
		id: 'java-memory',
		categoryId: 'java-performance',
		targetId: 'settings-target-java-memory',
		label: message('app.settings.defaults.memory', 'Memory allocated'),
	},
	{
		id: 'java-arguments',
		categoryId: 'java-performance',
		targetId: 'settings-target-java-arguments',
		label: message('app.settings.defaults.java-arguments', 'Java arguments'),
	},
	{
		id: 'resources-download-mirrors',
		categoryId: 'content-downloads',
		targetId: 'settings-target-resources-download-mirrors',
		label: message('app.settings.resources.download-mirrors', 'Download mirrors'),
		keywords: [message('app.settings.tabs.resource-management', 'Resource management')],
	},
	{
		id: 'resources-download-engine',
		categoryId: 'content-downloads',
		targetId: 'settings-target-resources-download-engine',
		label: message('app.settings.resources.download-engine', 'Download engine'),
	},
	{
		id: 'resources-download-concurrency',
		categoryId: 'content-downloads',
		targetId: 'settings-target-resources-maximum-downloads',
		label: message('app.settings.resources.maximum-downloads', 'Maximum concurrent downloads'),
		keywords: [message('app.settings.resources.maximum-writes', 'Maximum concurrent writes')],
	},
	{
		id: 'resources-missing-content-import',
		categoryId: 'content-downloads',
		targetId: 'settings-target-resources-missing-content-import',
		label: message(
			'app.settings.resources.missing-content-auto-import',
			'Automatically import missing modpack files',
		),
	},
	{
		id: 'resources-database-backups',
		categoryId: 'storage-backups',
		targetId: 'settings-target-resources-database-backups',
		label: message('app.settings.resources.database-backups', 'App database backups'),
		keywords: [message('app.settings.tabs.resource-management', 'Resource management')],
	},
	{
		id: 'storage-app-directory',
		categoryId: 'storage-backups',
		targetId: 'settings-target-storage-app-directory',
		label: message('app.settings.resources.axolotl-data-directory', 'Axolotl data directory'),
	},
	{
		id: 'storage-minecraft-directories',
		categoryId: 'storage-backups',
		targetId: 'settings-target-storage-minecraft-directories',
		label: message('app.settings.resources.minecraft-directories', 'Minecraft directories'),
	},
	{
		id: 'storage-cache',
		categoryId: 'storage-backups',
		targetId: 'settings-target-storage-cache',
		label: message('app.settings.resources.app-cache', 'App cache'),
	},
	{
		id: 'storage-overview',
		categoryId: 'storage-backups',
		targetId: 'settings-target-storage-overview',
		label: message('app.settings.storage.total', 'Storage usage'),
	},
	{
		id: 'updates-source',
		categoryId: 'updates',
		targetId: 'settings-target-updates-channel',
		label: message('app.settings.updates.title', 'Update source'),
	},
	{
		id: 'updates-history',
		categoryId: 'updates',
		label: message('app.settings.updates.announcements.history', 'Release history'),
	},
	{
		id: 'about-product',
		categoryId: 'about',
		targetId: 'settings-target-about-product',
		label: message('app.settings.tabs.about', 'About'),
	},
	{
		id: 'feature-flags',
		categoryId: 'feature-flags',
		label: message('settings.feature-flags.title', 'Feature flags'),
	},
]

export function validateSettingsSearchEntries(entries = settingsSearchEntries): string[] {
	const seenIds = new Set<string>()
	const errors: string[] = []

	for (const entry of entries) {
		if (seenIds.has(entry.id)) errors.push(`Duplicate settings search entry: ${entry.id}`)
		seenIds.add(entry.id)
		if (!entry.categoryId) errors.push(`Missing category for settings search entry: ${entry.id}`)
	}

	return errors
}

export function getSettingsSearchTargetId(entry: SettingsSearchEntry): string {
	return entry.targetId ?? `settings-category-${entry.categoryId}`
}

export function validateSettingsSearchMappings(entries = settingsSearchEntries): string[] {
	const categoryIds = new Set(settingsCategoryDefinitions.map((category) => category.id))
	const errors: string[] = []

	for (const entry of entries) {
		if (!categoryIds.has(entry.categoryId)) {
			errors.push(`Missing settings category for search entry: ${entry.id}`)
		}
		if (!getSettingsSearchTargetId(entry)) {
			errors.push(`Missing settings search target for entry: ${entry.id}`)
		}
	}

	return errors
}
