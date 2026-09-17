import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'

import type { CreatePackLocation } from './install'
import {
	install_create_modpack_instance,
	install_pack_to_existing_instance,
	installJobInstanceId,
	wait_for_install_job,
} from './install'

/**
 * Mirror of the server manifest documented in `docs/launcher-protocol.md`.
 *
 * The club backend decides which instances exist, at which `revision` and from
 * which pack file; the launcher only reconciles and installs.
 */
export interface ManagedLauncherPolicy {
	min_version: string | null
	latest_version: string | null
	force_update: boolean
}

export interface ManagedMinecraftSpec {
	game_version: string
	loader: string | null
	loader_version: string | null
}

export interface ManagedPackSpec {
	kind: string
	url: string
	sha1: string | null
	size: number | null
}

export interface ManagedServerSpec {
	address: string
	port: number | null
}

export interface ManagedInstanceSpec {
	id: string
	name: string
	description: string | null
	icon_url: string | null
	sort: number
	revision: number
	minecraft: ManagedMinecraftSpec
	pack: ManagedPackSpec
	server: ManagedServerSpec | null
	required: boolean
}

export interface ManagedManifest {
	schema: number
	generated_at: string | null
	launcher: ManagedLauncherPolicy | null
	instances: ManagedInstanceSpec[]
}

export type ManagedInstanceActionKind = 'create' | 'update' | 'current' | 'retired'

export interface ManagedInstanceAction {
	spec: ManagedInstanceSpec
	instance_id: string | null
	applied_revision: number | null
	action: ManagedInstanceActionKind
}

export interface ManagedSyncReport {
	schema: number
	launcher: ManagedLauncherPolicy | null
	instances: ManagedInstanceAction[]
}

export interface ManagedInstanceRecord {
	server_instance_id: string
	instance_id: string
	revision: number
	target_revision: number
	name: string
	description: string
	icon_url: string
	sort: number
	required: boolean
	retired: boolean
	server_address: string
	server_port: number
	updated: number
}

/** Only pack format the launcher can install today. */
export const MANAGED_PACK_KIND = 'mrpack'

/**
 * Local instance ids the club server owns, from the last successful sync.
 *
 * The launcher refuses to delete a managed instance in its own instance layer as
 * well, so this is only what keeps the action out of the menus; a stale set can
 * therefore never delete anything, it can only show an action that fails.
 */
const managedInstanceIds = ref<Set<string>>(new Set())

/** Re-reads the managed instance ids. Safe to call before any sync. */
export async function refreshManagedInstanceIds(): Promise<Set<string>> {
	try {
		const records = await managed_list()
		managedInstanceIds.value = new Set(
			records.map((record) => record.instance_id).filter((id) => id.length > 0),
		)
	} catch {
		// Offline, or the launcher never synced: keep whatever was known last.
	}

	return managedInstanceIds.value
}

/** Whether a local instance is owned by the club server, per the last sync. */
export function isManagedInstance(instanceId: string | null | undefined): boolean {
	if (!instanceId) return false
	return managedInstanceIds.value.has(instanceId)
}

export async function managed_fetch_manifest(manifestUrl: string) {
	return await invoke<ManagedManifest>('plugin:instance|managed_fetch_manifest', { manifestUrl })
}

export async function managed_sync(manifestUrl: string) {
	return await invoke<ManagedSyncReport>('plugin:instance|managed_sync', { manifestUrl })
}

export async function managed_list() {
	return await invoke<ManagedInstanceRecord[]>('plugin:instance|managed_list')
}

export async function managed_prepare_pack(serverInstanceId: string, revision: number) {
	return await invoke<string>('plugin:instance|managed_prepare_pack', {
		serverInstanceId,
		revision,
	})
}

export async function managed_mark_installed(
	serverInstanceId: string,
	instanceId: string,
	revision: number,
) {
	return await invoke<void>('plugin:instance|managed_mark_installed', {
		serverInstanceId,
		instanceId,
		revision,
	})
}

export async function managed_register_instance(
	serverInstanceId: string,
	instanceId: string,
	revision: number,
) {
	return await invoke<void>('plugin:instance|managed_register_instance', {
		serverInstanceId,
		instanceId,
		revision,
	})
}

/**
 * Refuses to launch an instance the server has taken out of service or moved to
 * a newer revision. Purely local, so it also holds when the network is down.
 */
export async function managed_ensure_runnable(instanceId: string) {
	return await invoke<void>('plugin:instance|managed_ensure_runnable', { instanceId })
}

/** Actions that require downloading or installing something. */
export function managedPendingActions(report: ManagedSyncReport): ManagedInstanceAction[] {
	return report.instances.filter((action) => action.action === 'create' || action.action === 'update')
}

/**
 * Installs or updates one managed instance.
 *
 * The pack is downloaded and integrity-checked by the Rust side first; the actual
 * install then runs through the normal install-job pipeline, so progress,
 * retries, resume and rollback all behave like a manual install. Returns the
 * local instance id the action now refers to.
 */
export async function applyManagedInstanceAction(
	action: ManagedInstanceAction,
): Promise<string> {
	const spec = action.spec
	if (spec.pack.kind !== MANAGED_PACK_KIND) {
		throw new Error(
			`Managed instance "${spec.id}" uses unsupported pack kind "${spec.pack.kind}"`,
		)
	}

	const path = await managed_prepare_pack(spec.id, spec.revision)
	const location: CreatePackLocation = { type: 'fromFile', path }

	if (action.action === 'update' && action.instance_id) {
		const job = await install_pack_to_existing_instance(action.instance_id, location, {
			name: spec.name,
		})
		const settled = await wait_for_install_job(job.job_id)
		const instanceId = installJobInstanceId(settled) ?? action.instance_id
		await managed_mark_installed(spec.id, instanceId, spec.revision)
		return instanceId
	}

	const job = await install_create_modpack_instance(location, { name: spec.name })
	const settled = await wait_for_install_job(job.job_id)
	const instanceId = installJobInstanceId(settled)
	if (!instanceId) {
		throw new Error(`Managed instance "${spec.id}" finished installing without an instance id`)
	}
	await managed_register_instance(spec.id, instanceId, spec.revision)
	return instanceId
}
