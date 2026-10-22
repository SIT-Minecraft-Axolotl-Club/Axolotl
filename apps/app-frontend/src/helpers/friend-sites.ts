import { invoke } from '@tauri-apps/api/core'

import type { CreatePackLocation } from './install'
import {
	install_create_modpack_instance,
	installJobInstanceId,
	wait_for_install_job,
} from './install'
import { list, remove } from './instance'

/**
 * Mirror of the friend-site catalog the society backend aggregates.
 *
 * The backend polls its peer sites and groups their published instances by
 * source site, so the launcher only reads one origin and never aggregates.
 * Nothing in this document is required, owned, or updated by the local server:
 * a friend instance is an ordinary local instance the player installs.
 */
export interface FriendMinecraftSpec {
	game_version: string
	loader: string | null
	loader_version: string | null
}

export interface FriendPackSpec {
	kind: string
	url: string
	sha1: string | null
	size: number | null
}

export interface FriendServerSpec {
	address: string
	port: number | null
}

export interface FriendInstance {
	id: string
	name: string
	description: string | null
	icon_url: string | null
	sort: number
	revision: number | null
	minecraft: FriendMinecraftSpec | null
	pack: FriendPackSpec | null
	server: FriendServerSpec | null
	required: boolean
}

export interface FriendSite {
	site_id: string
	name: string
	description: string | null
	domain: string | null
	manifest_url: string | null
	/** The local server's last poll of this site failed; its list may be old. */
	stale: boolean
	instances: FriendInstance[]
}

export interface FriendsDocument {
	schema: number
	generated_at: string | null
	sites: FriendSite[]
}

/** One instance together with the site that recommends it. */
export interface FriendRecommendation {
	site: FriendSite
	instance: FriendInstance
}

/** Only pack format the launcher can install today. */
const MRPACK_KIND = 'mrpack'

/**
 * Reads the friend catalog, trying each address in order.
 *
 * The backend exposes the same document at a launcher-facing and a versioned
 * path; passing both means a deployment that only serves one still works.
 */
export async function friends_fetch(urls: string[]): Promise<FriendsDocument> {
	return await invoke<FriendsDocument>('plugin:instance|managed_fetch_friends', { urls })
}

/** Groups the catalog by site, keeping the order the backend sent. */
export function friendSitesWithInstances(document: FriendsDocument | null): FriendSite[] {
	if (!document) return []

	return document.sites
		.filter((site) => site.instances.length > 0)
		.map((site) => ({
			...site,
			instances: [...site.instances].sort(
				(left, right) => left.sort - right.sort || left.name.localeCompare(right.name),
			),
		}))
}

/** Whether the launcher can install this entry. */
export function isInstallable(instance: FriendInstance): boolean {
	return (
		instance.pack !== null &&
		instance.pack.kind === MRPACK_KIND &&
		instance.pack.url.trim().length > 0
	)
}

/**
 * The instances already on disk, by name.
 *
 * A friend instance is installed as an ordinary local instance, so its name is
 * what ties the catalog entry to the copy on disk. The name is only used to find
 * that copy; the instance id it maps to is what launches or removes it.
 */
export async function installedInstancesByName(): Promise<Map<string, string>> {
	const instances = await list().catch(() => [])
	const byName = new Map<string, string>()
	for (const instance of instances) {
		const name = instance.name.trim()
		if (name) byName.set(name, instance.id)
	}
	return byName
}

/** Address the game connects to on launch, for quick play. */
export function quickPlayAddress(instance: FriendInstance): string | null {
	const server = instance.server
	if (!server?.address) return null
	return server.port && server.port !== 25565 ? `${server.address}:${server.port}` : server.address
}

/** Removes an installed friend instance from this computer. */
export async function uninstallFriendInstance(instanceId: string): Promise<void> {
	await remove(instanceId)
}

/**
 * Downloads and installs a friend instance as a local instance.
 *
 * The pack travels into the install job as a URL plus its declared SHA-1 and
 * size, exactly like a server-managed instance, so the launcher's downloader
 * owns progress, resumption and retries.
 */
export async function installFriendInstance(instance: FriendInstance): Promise<string> {
	const pack = instance.pack
	if (!pack || pack.kind !== MRPACK_KIND || !pack.url.trim()) {
		throw new Error(`Friend instance "${instance.id}" has no installable pack`)
	}

	const location: CreatePackLocation = {
		type: 'fromFile',
		url: pack.url,
		sha1: pack.sha1,
		size: pack.size,
		display: {
			name: instance.name,
			gameVersion: instance.minecraft?.game_version ?? null,
			loader: instance.minecraft?.loader ?? null,
			loaderVersion: instance.minecraft?.loader_version ?? null,
			iconUrl: instance.icon_url,
		},
	}

	const job = await install_create_modpack_instance(location, { name: instance.name })
	const settled = await wait_for_install_job(job.job_id)
	const instanceId = installJobInstanceId(settled)
	if (!instanceId) {
		throw new Error(`Friend instance "${instance.id}" finished installing without an instance id`)
	}

	return instanceId
}
