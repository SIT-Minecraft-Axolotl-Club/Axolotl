import { getVersion } from '@tauri-apps/api/app'
import { computed, ref } from 'vue'

import { SitmcConfig } from '@/config'
import type {
	ManagedInstanceAction,
	ManagedInstanceRecord,
	ManagedInstanceSpec,
	ManagedLauncherPolicy,
	ManagedSyncReport,
} from '@/helpers/managed'
import {
	applyManagedInstanceAction,
	managed_list,
	managed_sync,
	managedOptionalActions,
	managedRequiredActions,
	refreshManagedInstanceIds,
} from '@/helpers/managed'
import { compareSemanticVersions } from '@/helpers/version-compatibility'

export type ManagedSyncState = 'unconfigured' | 'idle' | 'syncing' | 'updating' | 'ready' | 'error'

const manifestUrl = SitmcConfig.manifestUrl

const state = ref<ManagedSyncState>(manifestUrl ? 'idle' : 'unconfigured')
const error = ref<string | null>(null)
const report = ref<ManagedSyncReport | null>(null)
const launcherPolicy = ref<ManagedLauncherPolicy | null>(null)
const records = ref<ManagedInstanceRecord[]>([])
const totalUpdates = ref(0)
const completedUpdates = ref(0)
const updatingName = ref<string | null>(null)
/** Minimum launcher version the server demands, while this build is below it. */
const launcherUpdateRequired = ref<string | null>(null)

/**
 * Whether the player has signed in yet.
 *
 * The launcher cannot start the game without a society account, so the sync does
 * not download anything on its own before the sign-in gate has been passed. The
 * manifest is still read — the blocking launcher-update notice must not wait for
 * a sign-in — but the required instances, and the game files their install pulls
 * in, wait for the first sync after the player signs in.
 */
const downloadsAllowed = ref(false)

let inFlight: Promise<void> | null = null
let syncedThisSession = false

const isBusy = computed(() => state.value === 'syncing' || state.value === 'updating')

const isConfigured = computed(() => state.value !== 'unconfigured')

const updateProgress = computed(() => ({
	done: completedUpdates.value,
	total: totalUpdates.value,
}))

/** Instances that may still be launched, in the order the server asked for. */
const activeRecords = computed(() =>
	records.value
		.filter((record) => !record.retired)
		.sort((left, right) => left.sort - right.sort || left.name.localeCompare(right.name)),
)

/** Instances the server took out of service. Kept on disk, but not launchable. */
const retiredRecords = computed(() => records.value.filter((record) => record.retired))

/**
 * Instances the player can still download.
 *
 * The server publishes them, but they are not required, so nothing is fetched
 * until the player asks for them. They are keyed by the server's instance id so
 * the panel can look one up while rendering a record.
 */
const optionalActions = computed(() => (report.value ? managedOptionalActions(report.value) : []))

const optionalActionsByServerId = computed(
	() => new Map(optionalActions.value.map((action) => [action.spec.id, action])),
)

/** Manifest details by server instance id, for display alongside a record. */
const specsById = computed(() => {
	const specs = new Map<string, ManagedInstanceSpec>()
	for (const action of report.value?.instances ?? []) {
		specs.set(action.spec.id, action.spec)
	}
	return specs
})

async function refreshRecords(): Promise<void> {
	records.value = await managed_list()
	// Keeps the "this instance belongs to the server" set in step with the
	// records, so the instance menus never offer to delete a managed instance.
	await refreshManagedInstanceIds()
}

/**
 * Decides whether this build is too old to be used at all.
 *
 * The comparison fails open: a version string that cannot be parsed is reported
 * and ignored rather than locking the player out of the launcher.
 */
async function applyLauncherPolicy(policy: ManagedLauncherPolicy | null): Promise<void> {
	launcherPolicy.value = policy

	const minimum = policy?.min_version?.trim()
	if (!minimum) {
		launcherUpdateRequired.value = null
		return
	}

	const current = await getVersion().catch(() => null)
	if (!current) return

	const comparison = compareSemanticVersions(current, minimum)
	if (comparison === null) {
		console.warn(
			`Cannot compare launcher version ${current} with the minimum ${minimum}; ignoring it`,
		)
		return
	}

	launcherUpdateRequired.value = comparison < 0 ? minimum : null
}

/**
 * How often the launcher re-reads the manifest while it stays open.
 *
 * The launch gate can only compare against what has been synced, so without a
 * periodic refresh a player who leaves the launcher running could start a
 * revision the server has already replaced.
 */
const SYNC_INTERVAL_MS = 5 * 60 * 1000

let syncTimer: ReturnType<typeof setInterval> | null = null
let focusSyncAttached = false

function attachFocusSync() {
	if (focusSyncAttached || typeof window === 'undefined') return
	focusSyncAttached = true
	// Coming back to the window is the moment a player is most likely to press
	// launch, and the cheapest moment to make sure the revisions are current.
	window.addEventListener('focus', () => void syncNow())
	document.addEventListener('visibilitychange', () => {
		if (document.visibilityState === 'visible') void syncNow()
	})
}

function startPeriodicSync() {
	if (!manifestUrl) return
	attachFocusSync()
	if (syncTimer === null) {
		syncTimer = setInterval(() => void syncNow(), SYNC_INTERVAL_MS)
	}
}

function stopPeriodicSync() {
	if (syncTimer === null) return
	clearInterval(syncTimer)
	syncTimer = null
}

async function runSync(): Promise<void> {
	state.value = 'syncing'
	error.value = null

	const result = await managed_sync(manifestUrl)
	report.value = result
	await applyLauncherPolicy(result.launcher ?? null)

	const pendingActions = downloadsAllowed.value ? managedRequiredActions(result) : []
	totalUpdates.value = pendingActions.length
	completedUpdates.value = 0

	if (pendingActions.length > 0) {
		state.value = 'updating'
		for (const action of pendingActions) {
			updatingName.value = action.spec.name
			await applyManagedInstanceAction(action)
			completedUpdates.value += 1
		}
	}

	await refreshRecords()
	updatingName.value = null
	state.value = 'ready'
	syncedThisSession = true
}

/**
 * Brings every managed instance up to the revision the server asked for.
 *
 * Concurrent calls share one run, and failures leave the previous local records
 * and packs in place so the launcher stays usable.
 */
async function syncNow(): Promise<void> {
	if (!manifestUrl || state.value === 'unconfigured') return
	startPeriodicSync()
	if (inFlight) {
		await inFlight
		return
	}

	const run = runSync()
	inFlight = run
	try {
		await run
	} catch (syncError) {
		error.value = syncError instanceof Error ? syncError.message : String(syncError)
		state.value = 'error'
		await refreshRecords().catch(() => {})
	} finally {
		updatingName.value = null
		inFlight = null
	}
}

/**
 * Reports the sign-in state from the shell.
 *
 * Signing in starts the downloads the earlier syncs held back; signing out only
 * keeps the next ones from starting, so a pack already downloading finishes.
 */
function setDownloadsAllowed(allowed: boolean): void {
	if (downloadsAllowed.value === allowed) return
	downloadsAllowed.value = allowed
	if (allowed) void resyncAfterSignIn()
}

/**
 * Runs the sync the sign-in gate held back.
 *
 * A sync may already be running when the player signs in; it is awaited first
 * because a sync that started without permission never downloads, and only the
 * run after it sees the sign-in state.
 */
async function resyncAfterSignIn(): Promise<void> {
	if (inFlight) await inFlight.catch(() => {})
	await syncNow()
}

/** Syncs once per launcher session, for pages that just need the current list. */
async function ensureSynced(): Promise<void> {
	if (syncedThisSession) {
		if (records.value.length === 0) {
			await refreshRecords().catch(() => {})
		}
		return
	}
	await syncNow()
}

async function refresh(): Promise<void> {
	await refreshRecords().catch(() => {})
}

/**
 * Downloads and installs one instance the player asked for.
 *
 * The panel keeps listing an instance that is not on disk yet, so this is what
 * its button calls. The records are re-read afterwards because installing binds
 * a new local instance to the society instance.
 */
async function installOptional(action: ManagedInstanceAction): Promise<void> {
	state.value = 'updating'
	updatingName.value = action.spec.name
	error.value = null

	try {
		await applyManagedInstanceAction(action)
		await refreshRecords()
		state.value = 'ready'
	} catch (installError) {
		error.value = installError instanceof Error ? installError.message : String(installError)
		state.value = 'error'
	} finally {
		updatingName.value = null
	}
}

export function useManagedInstances() {
	return {
		state,
		error,
		report,
		launcherPolicy,
		launcherUpdateRequired,
		downloadsAllowed,
		setDownloadsAllowed,
		records,
		activeRecords,
		retiredRecords,
		optionalActions,
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
		stopPeriodicSync,
	}
}
