<script setup lang="ts">
import {
	CheckIcon,
	ChevronDownIcon,
	DownloadIcon,
	GlobeIcon,
	RefreshCwIcon,
	ServerIcon,
	UsersIcon,
} from '@modrinth/assets'
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

/** Rows shown before the player asks for the whole list. */
const PREVIEW_LIMIT = 4

const messages = defineMessages({
	title: { id: 'friend-sites.title', defaultMessage: 'Instances from friend sites' },
	summary: {
		id: 'friend-sites.summary',
		defaultMessage: '{sites} sites · {instances} instances',
	},
	refresh: { id: 'friend-sites.refresh', defaultMessage: 'Refresh' },
	collapse: { id: 'friend-sites.collapse', defaultMessage: 'Collapse' },
	expand: { id: 'friend-sites.expand', defaultMessage: 'Expand' },
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
		defaultMessage: 'Not refreshed just now; this list may be out of date.',
	},
	showAll: { id: 'friend-sites.show-all', defaultMessage: 'Show all {count}' },
	showLess: { id: 'friend-sites.show-less', defaultMessage: 'Show fewer' },
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
const collapsed = ref(false)
const showAllRows = ref(false)

const sites = computed<FriendSite[]>(() => friendSitesWithInstances(catalog.value))

const instanceCount = computed(() =>
	sites.value.reduce((total, site) => total + site.instances.length, 0),
)

const isVisible = computed(() => isConfigured && (sites.value.length > 0 || error.value !== null))

/**
 * The rows to render, grouped by site but capped while collapsed.
 *
 * A homepage section has to stay short however many sites the backend
 * aggregates, so only the first few recommendations are shown until the player
 * asks for the rest.
 */
const visibleGroups = computed(() => {
	const rows = sites.value.flatMap((site) => site.instances.map((instance) => ({ site, instance })))
	const limited = showAllRows.value ? rows : rows.slice(0, PREVIEW_LIMIT)

	const groups = new Map<string, { site: FriendSite; instances: FriendInstance[] }>()
	for (const { site, instance } of limited) {
		const group = groups.get(site.site_id) ?? { site, instances: [] }
		group.instances.push(instance)
		groups.set(site.site_id, group)
	}

	return [...groups.values()]
})

const hiddenCount = computed(() => Math.max(0, instanceCount.value - PREVIEW_LIMIT))

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
	<section v-if="isVisible" class="mb-6 rounded-xl border border-surface-4 bg-surface-2">
		<header class="flex items-center gap-3 px-5 py-4">
			<button
				class="flex min-w-0 flex-1 items-center gap-3 rounded-lg text-left"
				@click="collapsed = !collapsed"
			>
				<span
					class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-surface-3 text-brand"
				>
					<UsersIcon class="size-5" />
				</span>
				<span class="flex min-w-0 flex-col">
					<span class="truncate text-base font-semibold text-contrast">
						{{ formatMessage(messages.title) }}
					</span>
					<span v-if="instanceCount > 0" class="truncate text-xs text-secondary">
						{{
							formatMessage(messages.summary, {
								sites: sites.length,
								instances: instanceCount,
							})
						}}
					</span>
				</span>
			</button>

			<div class="flex shrink-0 items-center gap-1">
				<ButtonStyled circular type="transparent" size="small">
					<button v-tooltip="formatMessage(messages.refresh)" :disabled="loading" @click="load">
						<RefreshCwIcon :class="{ 'animate-spin': loading }" />
					</button>
				</ButtonStyled>
				<ButtonStyled circular type="transparent" size="small">
					<button
						v-tooltip="formatMessage(collapsed ? messages.expand : messages.collapse)"
						@click="collapsed = !collapsed"
					>
						<ChevronDownIcon :class="{ 'rotate-180': collapsed }" />
					</button>
				</ButtonStyled>
			</div>
		</header>

		<div v-if="!collapsed" class="border-t border-surface-4 px-5 py-4">
			<div v-if="error" class="flex flex-wrap items-center gap-3">
				<p class="m-0 flex-1 text-sm text-red">{{ formatMessage(messages.failed) }} {{ error }}</p>
				<ButtonStyled size="small">
					<button :disabled="loading" @click="load">
						{{ formatMessage(messages.retry) }}
					</button>
				</ButtonStyled>
			</div>

			<p
				v-else-if="loading && sites.length === 0"
				class="m-0 flex items-center gap-2 text-sm text-secondary"
			>
				<DownloadIcon class="size-4 text-brand" />
				{{ formatMessage(messages.loading) }}
			</p>

			<div v-else class="flex flex-col gap-3">
				<section v-for="group in visibleGroups" :key="group.site.site_id">
					<header class="mb-1 flex flex-wrap items-center gap-2">
						<h3 class="m-0 text-xs font-semibold tracking-wide text-secondary uppercase">
							{{ group.site.name }}
						</h3>
						<a
							v-if="group.site.domain"
							v-tooltip="formatMessage(messages.visit)"
							:href="`https://${group.site.domain}`"
							target="_blank"
							rel="noreferrer"
							class="flex items-center gap-1 text-[11px] text-secondary transition-colors hover:text-contrast"
						>
							<GlobeIcon class="size-3" />
							{{ group.site.domain }}
						</a>
						<span
							v-if="group.site.stale"
							v-tooltip="formatMessage(messages.stale)"
							class="rounded-full bg-surface-3 px-1.5 py-0.5 text-[10px] text-secondary"
						>
							!
						</span>
					</header>

					<div class="flex flex-col gap-1">
						<div
							v-for="instance in group.instances"
							:key="instance.id"
							class="flex items-center gap-3 rounded-lg px-2 py-1.5 transition-colors hover:bg-surface-3"
						>
							<div
								class="flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-md bg-surface-3"
							>
								<img
									v-if="instance.icon_url"
									:src="instance.icon_url"
									alt=""
									class="size-full object-cover"
								/>
								<span v-else class="text-xs font-semibold text-secondary">
									{{ monogram(instance.name) }}
								</span>
							</div>

							<div class="flex min-w-0 flex-1 flex-col">
								<span
									:title="instance.description ?? undefined"
									class="truncate text-sm font-medium text-contrast"
								>
									{{ instance.name }}
								</span>
								<span class="flex min-w-0 items-center gap-2 text-[11px] text-secondary">
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

							<span
								v-if="isInstalled(instance)"
								class="flex shrink-0 items-center gap-1 text-xs text-secondary"
							>
								<CheckIcon class="size-3.5 text-brand" />
								{{ formatMessage(messages.installed) }}
							</span>
							<ButtonStyled v-else-if="isInstallable(instance)" color="brand" size="small">
								<button :disabled="installingKey !== null" @click="install(group.site, instance)">
									{{
										installingKey === instanceKey(group.site, instance)
											? formatMessage(messages.installing)
											: formatMessage(messages.install)
									}}
								</button>
							</ButtonStyled>
							<span v-else class="shrink-0 text-[11px] text-secondary">
								{{ formatMessage(messages.unavailable) }}
							</span>
						</div>
					</div>
				</section>

				<ButtonStyled v-if="hiddenCount > 0" type="transparent" size="small">
					<button @click="showAllRows = !showAllRows">
						{{
							formatMessage(showAllRows ? messages.showLess : messages.showAll, {
								count: instanceCount,
							})
						}}
					</button>
				</ButtonStyled>
			</div>
		</div>
	</section>
</template>
