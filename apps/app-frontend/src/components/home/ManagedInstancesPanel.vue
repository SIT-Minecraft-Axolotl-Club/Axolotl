<script setup lang="ts">
import { DownloadIcon, RefreshCwIcon } from '@modrinth/assets'
import { ButtonStyled, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import { useManagedInstances } from '@/composables/useManagedInstances'
import { SitmcConfig } from '@/config'
import { run } from '@/helpers/instance'
import { managed_ensure_runnable } from '@/helpers/managed'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: { id: 'managed-instances.title', defaultMessage: 'Club game instances' },
	subtitle: {
		id: 'managed-instances.subtitle',
		defaultMessage: 'Instances are published by the club server and are updated to the newest version before every launch.',
	},
	checking: { id: 'managed-instances.checking', defaultMessage: 'Checking the server for versions...' },
	updating: { id: 'managed-instances.updating', defaultMessage: 'Updating {name}' },
	progress: { id: 'managed-instances.progress', defaultMessage: '{done} of {total} done' },
	ready: { id: 'managed-instances.ready', defaultMessage: 'Up to date' },
	retry: { id: 'managed-instances.retry', defaultMessage: 'Retry' },
	refresh: { id: 'managed-instances.refresh', defaultMessage: 'Check for updates' },
	play: { id: 'managed-instances.play', defaultMessage: 'Play' },
	starting: { id: 'managed-instances.starting', defaultMessage: 'Starting...' },
	noInstances: {
		id: 'managed-instances.empty',
		defaultMessage: 'The server currently publishes no game instances.',
	},
	retiredTitle: { id: 'managed-instances.retired-title', defaultMessage: 'Retired instances' },
	retiredHint: {
		id: 'managed-instances.retired-hint',
		defaultMessage: 'These instances were removed from the server manifest. They can no longer be launched, but their data stays on disk.',
	},
	unconfigured: {
		id: 'managed-instances.unconfigured',
		defaultMessage: 'No club instance manifest address is configured, so game instances cannot be fetched.',
	},
})

const {
	state,
	error,
	activeRecords,
	retiredRecords,
	specsById,
	isBusy,
	isConfigured,
	updatingName,
	updateProgress,
	manifestUrl,
	syncNow,
	ensureSynced,
	refresh,
} = useManagedInstances()

const launchingId = ref<string | null>(null)
const launchError = ref<string | null>(null)

const statusText = computed(() => {
	if (state.value === 'syncing') return formatMessage(messages.checking)
	if (state.value === 'updating') {
		return formatMessage(messages.updating, { name: updatingName.value ?? '' })
	}
	if (state.value === 'ready') return formatMessage(messages.ready)
	return ''
})

function specFor(serverInstanceId: string) {
	return specsById.value.get(serverInstanceId) ?? null
}

function versionLabel(serverInstanceId: string) {
	const spec = specFor(serverInstanceId)
	if (!spec) return ''
	const loader = spec.minecraft.loader
	if (loader && loader !== 'vanilla') {
		const loaderVersion = spec.minecraft.loader_version
		return `${spec.minecraft.game_version} · ${loader}${loaderVersion ? ` ${loaderVersion}` : ''}`
	}
	return spec.minecraft.game_version
}

function serverLabel(serverInstanceId: string) {
	const spec = specFor(serverInstanceId)
	const address = spec?.server?.address
	if (!address) return ''
	const port = spec?.server?.port
	return port && port !== 25565 ? `${address}:${port}` : address
}

function quickPlayAddress(serverInstanceId: string) {
	const spec = specFor(serverInstanceId)
	const address = spec?.server?.address
	if (!address) return null
	const port = spec?.server?.port
	return port && port !== 25565 ? `${address}:${port}` : address
}

async function launch(instanceId: string, serverInstanceId: string) {
	if (launchingId.value) return
	launchingId.value = instanceId
	launchError.value = null
	try {
		// The server owns the revision, so re-read the manifest before launching
		// instead of trusting a record that may have been synced minutes ago. A
		// failed refresh (offline) leaves the previous records in place, so an
		// already installed instance can still start.
		await syncNow()
		// The launcher refuses stale instances in its own launch path as well;
		// checking here just turns that into an immediate, readable error.
		await managed_ensure_runnable(instanceId)
		await run(instanceId, quickPlayAddress(serverInstanceId))
		await refresh()
	} catch (launchFailure) {
		launchError.value =
			launchFailure instanceof Error ? launchFailure.message : String(launchFailure)
	} finally {
		launchingId.value = null
	}
}

onMounted(() => {
	void ensureSynced()
})
</script>

<template>
	<section
		v-if="isConfigured"
		data-onboarding-id="managed-instances"
		class="mb-6 rounded-xl border border-divider bg-surface-2 p-5"
	>
		<header class="mb-4 flex flex-wrap items-start justify-between gap-3">
			<div class="flex flex-col gap-1">
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.title) }}
				</h2>
				<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.subtitle) }}</p>
			</div>
			<div class="flex items-center gap-3">
				<span v-if="statusText" class="text-xs text-secondary">{{ statusText }}</span>
				<ButtonStyled>
					<button :disabled="isBusy" @click="syncNow">
						<RefreshCwIcon :class="{ 'animate-spin': isBusy }" />
						{{ formatMessage(messages.refresh) }}
					</button>
				</ButtonStyled>
			</div>
		</header>

		<div
			v-if="state === 'updating'"
			class="mb-4 flex items-center gap-3 rounded-lg bg-surface-3 px-4 py-3"
		>
			<DownloadIcon class="size-5 shrink-0 text-brand" />
			<span class="text-sm text-secondary">
				{{ formatMessage(messages.progress, updateProgress) }}
			</span>
		</div>

		<div v-if="error" class="mb-4 flex flex-wrap items-center gap-3">
			<p class="m-0 flex-1 text-sm text-red">{{ error }}</p>
			<ButtonStyled>
				<button :disabled="isBusy" @click="syncNow">
					{{ formatMessage(messages.retry) }}
				</button>
			</ButtonStyled>
		</div>
		<p v-if="launchError" class="m-0 mb-4 text-sm text-red">{{ launchError }}</p>

		<p v-if="activeRecords.length === 0" class="m-0 text-sm text-secondary">
			{{ formatMessage(messages.noInstances) }}
		</p>

		<div v-else class="flex flex-col gap-3">
			<article
				v-for="record in activeRecords"
				:key="record.server_instance_id"
				class="flex items-center gap-4 rounded-lg border border-divider bg-surface-3 p-4"
			>
				<img
					v-if="record.icon_url"
					:src="record.icon_url"
					alt=""
					class="size-12 shrink-0 rounded-lg object-cover"
				/>
				<div
					v-else
					class="flex size-12 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-xs font-semibold text-secondary"
				>
					{{ SitmcConfig.serverLabel }}
				</div>

				<div class="flex min-w-0 flex-1 flex-col gap-1">
					<span class="truncate font-medium text-contrast">{{ record.name }}</span>
					<span v-if="record.description" class="truncate text-xs text-secondary">
						{{ record.description }}
					</span>
					<span class="flex flex-wrap gap-3 text-xs text-secondary">
						<span v-if="versionLabel(record.server_instance_id)">
							{{ versionLabel(record.server_instance_id) }}
						</span>
						<span v-if="serverLabel(record.server_instance_id)">
							{{ serverLabel(record.server_instance_id) }}
						</span>
					</span>
				</div>

				<ButtonStyled color="brand" class="shrink-0">
					<button
						:disabled="isBusy || launchingId !== null"
						@click="launch(record.instance_id, record.server_instance_id)"
					>
						{{
							launchingId === record.instance_id
								? formatMessage(messages.starting)
								: formatMessage(messages.play)
						}}
					</button>
				</ButtonStyled>
			</article>
		</div>

		<details v-if="retiredRecords.length > 0" class="mt-4">
			<summary class="cursor-pointer text-xs text-secondary">
				{{ formatMessage(messages.retiredTitle) }} ({{ retiredRecords.length }})
			</summary>
			<p class="m-0 mt-2 text-xs text-secondary">{{ formatMessage(messages.retiredHint) }}</p>
			<ul class="m-0 mt-2 flex list-none flex-col gap-1 p-0">
				<li v-for="record in retiredRecords" :key="record.server_instance_id" class="text-xs">
					{{ record.name }}
				</li>
			</ul>
		</details>
	</section>

	<section
		v-else-if="manifestUrl === ''"
		class="mb-6 rounded-xl border border-divider bg-surface-2 p-5"
	>
		<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.unconfigured) }}</p>
	</section>
</template>
