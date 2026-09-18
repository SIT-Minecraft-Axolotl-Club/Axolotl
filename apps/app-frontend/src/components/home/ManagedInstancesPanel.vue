<script setup lang="ts">
import { DownloadIcon, RefreshCwIcon, TrashIcon } from '@modrinth/assets'
import { ButtonStyled, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import { useManagedInstances } from '@/composables/useManagedInstances'
import { SitmcConfig } from '@/config'
import { remove as removeInstance, run } from '@/helpers/instance'
import type { ManagedInstanceRecord } from '@/helpers/managed'
import { managed_ensure_runnable } from '@/helpers/managed'

const { formatMessage } = useVIntl()

/** Tauri rejects a command with a string or a plain object, not always an Error. */
function describeError(error: unknown): string {
	if (error instanceof Error) return error.message
	if (typeof error === 'string') return error
	if (error && typeof error === 'object') {
		const message = (error as { message?: unknown }).message
		if (typeof message === 'string' && message.length > 0) return message
		try {
			return JSON.stringify(error)
		} catch {
			// Fall through to the plain conversion below.
		}
	}
	return String(error)
}

const messages = defineMessages({
	title: { id: 'managed-instances.title', defaultMessage: 'Society game instances' },
	subtitle: {
		id: 'managed-instances.subtitle',
		defaultMessage: 'Instances are published by the society server. Required ones are kept up to date automatically; optional ones are downloaded when you start them.',
	},
	checking: { id: 'managed-instances.checking', defaultMessage: 'Checking the server for versions...' },
	updating: { id: 'managed-instances.updating', defaultMessage: 'Updating {name}' },
	progress: { id: 'managed-instances.progress', defaultMessage: '{done} of {total} done' },
	ready: { id: 'managed-instances.ready', defaultMessage: 'Up to date' },
	retry: { id: 'managed-instances.retry', defaultMessage: 'Retry' },
	refresh: { id: 'managed-instances.refresh', defaultMessage: 'Check for updates' },
	play: { id: 'managed-instances.play', defaultMessage: 'Play' },
	playDownloads: {
		id: 'managed-instances.play-downloads',
		defaultMessage: 'Play (downloads first)',
	},
	starting: { id: 'managed-instances.starting', defaultMessage: 'Starting...' },
	downloading: { id: 'managed-instances.downloading', defaultMessage: 'Downloading...' },
	notDownloaded: { id: 'managed-instances.not-downloaded', defaultMessage: 'Not downloaded yet' },
	remove: { id: 'managed-instances.remove', defaultMessage: 'Remove from this computer' },
	removeHint: {
		id: 'managed-instances.remove-hint',
		defaultMessage: 'Removing it frees the disk space; the instance stays listed so you can download it again.',
	},
	badgeRequired: { id: 'managed-instances.badge-required', defaultMessage: 'Required' },
	badgeOptional: { id: 'managed-instances.badge-optional', defaultMessage: 'On demand' },
	noInstances: {
		id: 'managed-instances.empty',
		defaultMessage: 'The server currently publishes no game instances.',
	},
	unconfigured: {
		id: 'managed-instances.unconfigured',
		defaultMessage: 'No society instance manifest address is configured, so game instances cannot be fetched.',
	},
})

const {
	state,
	error,
	records,
	activeRecords,
	optionalActionsByServerId,
	specsById,
	isBusy,
	isConfigured,
	updatingName,
	updateProgress,
	manifestUrl,
	syncNow,
	ensureSynced,
	refresh,
	installOptional,
} = useManagedInstances()

const launchingId = ref<string | null>(null)
const removingId = ref<string | null>(null)
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

/** An instance waiting for the player to download it, if this is one. */
function pendingAction(serverInstanceId: string) {
	return optionalActionsByServerId.value.get(serverInstanceId) ?? null
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
			describeError(launchFailure)
	} finally {
		launchingId.value = null
	}
}

/**
 * Starts an instance, downloading it first when it is not on this computer.
 *
 * Optional instances are deliberately not fetched in the background, so the
 * player's press is what installs them; the launch follows once the install
 * settled and the record points at the new local instance.
 */
async function play(record: ManagedInstanceRecord) {
	const action = pendingAction(record.server_instance_id)

	if (!action) {
		await launch(record.instance_id, record.server_instance_id)
		return
	}

	await installOptional(action)
	await refresh()

	const updated = records.value.find(
		(candidate) => candidate.server_instance_id === record.server_instance_id,
	)
	if (!updated?.instance_id) return

	await launch(updated.instance_id, record.server_instance_id)
}

/** Frees an optional instance. The record stays, so it can be downloaded again. */
async function removeOptional(record: ManagedInstanceRecord) {
	if (!record.instance_id || removingId.value) return
	removingId.value = record.instance_id
	launchError.value = null
	try {
		await removeInstance(record.instance_id)
		await refresh()
		// Re-reads the manifest so the instance is offered for download again.
		await syncNow()
	} catch (removalFailure) {
		launchError.value =
			describeError(removalFailure)
	} finally {
		removingId.value = null
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
				class="flex flex-wrap items-center gap-4 rounded-lg border border-divider bg-surface-3 p-4"
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
					<span class="flex items-center gap-2">
						<span class="truncate font-medium text-contrast">{{ record.name }}</span>
						<span
							class="shrink-0 rounded-full px-2 py-0.5 text-[11px]"
							:class="
								record.required
									? 'bg-surface-2 text-secondary'
									: 'bg-surface-1 text-secondary'
							"
						>
							{{
								record.required
									? formatMessage(messages.badgeRequired)
									: formatMessage(messages.badgeOptional)
							}}
						</span>
					</span>
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
						<span v-if="pendingAction(record.server_instance_id)" class="text-brand">
							{{ formatMessage(messages.notDownloaded) }}
						</span>
					</span>
				</div>

				<div class="flex shrink-0 items-center gap-2">
					<ButtonStyled v-if="!record.required && record.instance_id && !pendingAction(record.server_instance_id)">
						<button
							v-tooltip="formatMessage(messages.removeHint)"
							:disabled="isBusy || removingId !== null"
							@click="removeOptional(record)"
						>
							<TrashIcon />
							{{ formatMessage(messages.remove) }}
						</button>
					</ButtonStyled>

					<ButtonStyled color="brand" class="shrink-0">
						<button
							:disabled="isBusy || launchingId !== null || removingId !== null"
							@click="play(record)"
						>
							{{
								launchingId === record.instance_id
									? formatMessage(messages.starting)
									: formatMessage(messages.play)
							}}
						</button>
					</ButtonStyled>
				</div>
			</article>
		</div>
	</section>

	<section
		v-else-if="manifestUrl === ''"
		class="mb-6 rounded-xl border border-divider bg-surface-2 p-5"
	>
		<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.unconfigured) }}</p>
	</section>
</template>
