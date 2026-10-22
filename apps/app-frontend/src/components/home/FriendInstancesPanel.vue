<script setup lang="ts">
import { DownloadIcon, RefreshCwIcon } from '@modrinth/assets'
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
	refresh: { id: 'friend-sites.refresh', defaultMessage: 'Refresh' },
	loading: {
		id: 'friend-sites.loading',
		defaultMessage: 'Looking for instances from friend sites...',
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
		defaultMessage: 'This site could not be refreshed just now; the list may be out of date.',
	},
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

const isVisible = computed(() => isConfigured && (sites.value.length > 0 || error.value !== null))

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

function versionLabel(instance: FriendInstance): string {
	const minecraft = instance.minecraft
	if (!minecraft) return ''
	const loader = minecraft.loader?.trim()
	return loader
		? `${minecraft.game_version} · ${loader}${
				minecraft.loader_version ? ` ${minecraft.loader_version}` : ''
			}`
		: minecraft.game_version
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
	<section v-if="isVisible" class="mb-6 rounded-xl border border-divider bg-surface-2 p-5">
		<header class="mb-4 flex flex-wrap items-center justify-between gap-3">
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
			<ButtonStyled>
				<button :disabled="loading" @click="load">
					<RefreshCwIcon :class="{ 'animate-spin': loading }" />
					{{ formatMessage(messages.refresh) }}
				</button>
			</ButtonStyled>
		</header>

		<div v-if="error" class="mb-4 flex flex-wrap items-center gap-3">
			<p class="m-0 flex-1 text-sm text-red">{{ formatMessage(messages.failed) }} {{ error }}</p>
			<ButtonStyled>
				<button :disabled="loading" @click="load">
					{{ formatMessage(messages.retry) }}
				</button>
			</ButtonStyled>
		</div>

		<p
			v-else-if="loading && sites.length === 0"
			class="m-0 flex items-center gap-3 text-sm text-secondary"
		>
			<DownloadIcon class="size-5 shrink-0 text-brand" />
			{{ formatMessage(messages.loading) }}
		</p>

		<div v-else class="flex flex-col gap-5">
			<section v-for="site in sites" :key="site.site_id" class="flex flex-col gap-2">
				<header class="flex flex-wrap items-center gap-2">
					<h3 class="m-0 text-base font-semibold text-contrast">{{ site.name }}</h3>
					<a
						v-if="site.domain"
						:href="`https://${site.domain}`"
						target="_blank"
						rel="noreferrer"
						class="text-xs text-secondary hover:text-contrast"
					>
						{{ site.domain }}
					</a>
					<span v-if="site.stale" class="text-xs text-secondary">
						{{ formatMessage(messages.stale) }}
					</span>
				</header>
				<p v-if="site.description" class="m-0 text-xs text-secondary">
					{{ site.description }}
				</p>

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
							{{ site.name }}
						</div>

						<div class="flex min-w-0 flex-1 flex-col gap-1">
							<span class="truncate font-medium text-contrast">{{ instance.name }}</span>
							<span v-if="instance.description" class="truncate text-xs text-secondary">
								{{ instance.description }}
							</span>
							<span class="flex flex-wrap gap-3 text-xs text-secondary">
								<span v-if="versionLabel(instance)">{{ versionLabel(instance) }}</span>
								<span v-if="serverLabel(instance)">{{ serverLabel(instance) }}</span>
							</span>
						</div>

						<div class="flex shrink-0 items-center gap-2">
							<span v-if="isInstalled(instance)" class="text-xs font-semibold text-secondary">
								{{ formatMessage(messages.installed) }}
							</span>
							<ButtonStyled v-else-if="isInstallable(instance)" color="brand">
								<button :disabled="installingKey !== null" @click="install(site, instance)">
									<DownloadIcon />
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
		</div>
	</section>
</template>
