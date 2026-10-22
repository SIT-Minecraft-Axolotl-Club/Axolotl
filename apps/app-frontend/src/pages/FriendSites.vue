<script setup lang="ts">
import { CheckIcon, DownloadIcon, GlobeIcon, RefreshCwIcon, ServerIcon } from '@modrinth/assets'
import { ButtonStyled, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import { SitmcConfig } from '@/config'
import type { FriendInstance, FriendsDocument, FriendSite } from '@/helpers/friend-sites'
import {
	friends_fetch,
	friendSitesWithInstances,
	installedInstanceNames,
	installFriendInstance,
	isInstallable,
} from '@/helpers/friend-sites'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'friend-sites.title', defaultMessage: 'Instances from friend sites' },
	summary: {
		id: 'friend-sites.summary',
		defaultMessage: '{sites} sites · {instances} instances',
	},
	refresh: { id: 'friend-sites.refresh', defaultMessage: 'Refresh' },
	loading: {
		id: 'friend-sites.loading',
		defaultMessage: 'Looking for instances from friend sites...',
	},
	empty: {
		id: 'friend-sites.empty',
		defaultMessage: 'No friend site publishes an instance right now.',
	},
	failed: {
		id: 'friend-sites.failed',
		defaultMessage: 'The friend-site list could not be loaded.',
	},
	retry: { id: 'friend-sites.retry', defaultMessage: 'Retry' },
	install: { id: 'friend-sites.install', defaultMessage: 'Install' },
	installing: { id: 'friend-sites.installing', defaultMessage: 'Installing...' },
	installed: { id: 'friend-sites.installed', defaultMessage: 'Installed' },
	unavailable: { id: 'friend-sites.unavailable', defaultMessage: 'Not available for download' },
	stale: {
		id: 'friend-sites.stale',
		defaultMessage: 'Not refreshed just now; this list may be out of date.',
	},
	visit: { id: 'friend-sites.visit', defaultMessage: 'Visit the site' },
})

/** Tauri rejects a command with a string or a plain object, not always an Error. */
function describeError(error: unknown): string {
	if (error instanceof Error) return error.message
	if (typeof error === 'string') return error
	if (error && typeof error === 'object') {
		const message = (error as { message?: unknown }).message
		if (typeof message === 'string' && message.length > 0) return message
	}
	return String(error)
}

const urls = [SitmcConfig.friendsUrl, SitmcConfig.friendsInteropUrl].filter(Boolean)
const isConfigured = urls.length > 0

const loading = ref(false)
const error = ref<string | null>(null)
const catalog = ref<FriendsDocument | null>(null)
const installed = ref<Set<string>>(new Set())
const installingKey = ref<string | null>(null)

const sites = computed<FriendSite[]>(() => friendSitesWithInstances(catalog.value))

const instanceCount = computed(() =>
	sites.value.reduce((total, site) => total + site.instances.length, 0),
)

async function load(): Promise<void> {
	if (!isConfigured || loading.value) return

	loading.value = true
	error.value = null

	try {
		catalog.value = await friends_fetch(urls)
		installed.value = await installedInstanceNames()
	} catch (loadError) {
		error.value = describeError(loadError)
	} finally {
		loading.value = false
	}
}

function instanceKey(site: FriendSite, instance: FriendInstance): string {
	return `${site.site_id}:${instance.id}`
}

function isInstalled(instance: FriendInstance): boolean {
	return installed.value.has(instance.name.trim())
}

/** First character of a name, used as the tile for an entry without an icon. */
function monogram(name: string): string {
	return name.trim().charAt(0).toUpperCase() || '?'
}

function loaderLabel(instance: FriendInstance): string {
	const minecraft = instance.minecraft
	if (!minecraft) return ''
	const loader = minecraft.loader?.trim()
	if (!loader) return ''
	const version = minecraft.loader_version ? ` ${minecraft.loader_version}` : ''
	return `${loader}${version}`
}

function serverLabel(instance: FriendInstance): string {
	const server = instance.server
	if (!server?.address) return ''
	return server.port && server.port !== 25565 ? `${server.address}:${server.port}` : server.address
}

async function install(site: FriendSite, instance: FriendInstance): Promise<void> {
	const key = instanceKey(site, instance)
	if (installingKey.value) return

	installingKey.value = key
	try {
		await installFriendInstance(instance)
		installed.value = await installedInstanceNames()
	} catch (installError) {
		handleError(installError)
	} finally {
		installingKey.value = null
	}
}

onMounted(() => {
	void load()
})
</script>

<template>
	<div class="flex h-full min-h-0 flex-col gap-4 overflow-y-auto p-6">
		<header class="flex flex-wrap items-center justify-between gap-3">
			<h1 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h1>
			<div class="flex items-center gap-3">
				<span v-if="instanceCount > 0" class="text-xs text-secondary">
					{{
						formatMessage(messages.summary, {
							sites: sites.length,
							instances: instanceCount,
						})
					}}
				</span>
				<ButtonStyled>
					<button :disabled="loading" @click="load">
						<RefreshCwIcon :class="{ 'animate-spin': loading }" />
						{{ formatMessage(messages.refresh) }}
					</button>
				</ButtonStyled>
			</div>
		</header>

		<section
			v-if="error"
			class="flex flex-wrap items-center gap-3 rounded-xl border border-divider bg-surface-2 p-5"
		>
			<p class="m-0 flex-1 text-sm text-red">{{ formatMessage(messages.failed) }} {{ error }}</p>
			<ButtonStyled size="small">
				<button :disabled="loading" @click="load">
					{{ formatMessage(messages.retry) }}
				</button>
			</ButtonStyled>
		</section>

		<section
			v-else-if="loading && sites.length === 0"
			class="flex items-center gap-3 rounded-xl border border-divider bg-surface-2 p-5"
		>
			<DownloadIcon class="size-5 shrink-0 text-brand" />
			<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.loading) }}</p>
		</section>

		<section
			v-else-if="sites.length === 0"
			class="rounded-xl border border-divider bg-surface-2 p-5"
		>
			<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.empty) }}</p>
		</section>

		<template v-else>
			<section
				v-for="site in sites"
				:key="site.site_id"
				class="rounded-xl border border-divider bg-surface-2 p-5"
			>
				<header class="mb-4 flex flex-wrap items-center justify-between gap-3">
					<div class="flex min-w-0 flex-col">
						<h2 class="m-0 truncate text-lg font-semibold text-contrast">
							{{ site.name }}
						</h2>
						<span v-if="site.description" class="truncate text-xs text-secondary">
							{{ site.description }}
						</span>
					</div>
					<div class="flex items-center gap-3">
						<a
							v-if="site.domain"
							v-tooltip="formatMessage(messages.visit)"
							:href="`https://${site.domain}`"
							target="_blank"
							rel="noreferrer"
							class="flex items-center gap-1 text-xs text-secondary transition-colors hover:text-contrast"
						>
							<GlobeIcon class="size-3.5" />
							{{ site.domain }}
						</a>
						<span
							v-if="site.stale"
							v-tooltip="formatMessage(messages.stale)"
							class="rounded-full bg-surface-3 px-2 py-0.5 text-[11px] text-secondary"
						>
							!
						</span>
					</div>
				</header>

				<div class="flex flex-col gap-3">
					<article
						v-for="instance in site.instances"
						:key="instance.id"
						class="flex flex-wrap items-center gap-4 rounded-lg border border-divider bg-surface-3 p-4"
					>
						<img
							v-if="instance.icon_url"
							:src="instance.icon_url"
							alt=""
							class="size-12 shrink-0 rounded-lg object-cover"
						/>
						<div
							v-else
							class="flex size-12 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-xs font-semibold text-secondary"
						>
							{{ monogram(instance.name) }}
						</div>

						<div class="flex min-w-0 flex-1 flex-col gap-1">
							<span class="truncate font-medium text-contrast">{{ instance.name }}</span>
							<span v-if="instance.description" class="truncate text-xs text-secondary">
								{{ instance.description }}
							</span>
							<span class="flex flex-wrap gap-3 text-xs text-secondary">
								<span v-if="instance.minecraft?.game_version">
									{{ instance.minecraft.game_version }}
								</span>
								<span v-if="loaderLabel(instance)" class="capitalize">
									{{ loaderLabel(instance) }}
								</span>
								<span v-if="serverLabel(instance)" class="flex min-w-0 items-center gap-1">
									<ServerIcon class="size-3 shrink-0" />
									<span class="truncate">{{ serverLabel(instance) }}</span>
								</span>
							</span>
						</div>

						<div class="flex shrink-0 items-center gap-2">
							<span
								v-if="isInstalled(instance)"
								class="flex items-center gap-1 text-xs font-semibold text-secondary"
							>
								<CheckIcon class="size-3.5 text-brand" />
								{{ formatMessage(messages.installed) }}
							</span>
							<ButtonStyled v-else-if="isInstallable(instance)" color="brand">
								<button :disabled="installingKey !== null" @click="install(site, instance)">
									{{
										installingKey === instanceKey(site, instance)
											? formatMessage(messages.installing)
											: formatMessage(messages.install)
									}}
								</button>
							</ButtonStyled>
							<span v-else class="text-xs text-secondary">
								{{ formatMessage(messages.unavailable) }}
							</span>
						</div>
					</article>
				</div>
			</section>
		</template>
	</div>
</template>
