import { invoke } from '@tauri-apps/api/core'

/**
 * First-start move of an installation from the launcher this build was forked
 * from.
 *
 * The identifier changed with the rebrand, so an existing installation's data
 * lives in the directory keyed by the old identifier. Both directories sit under
 * the same application data root, so the move is a rename and completes
 * instantly; the boot screen only waits for it before opening the database.
 */
export async function legacyMigrationPending(): Promise<boolean> {
	return await invoke<boolean>('legacy_migration_pending')
}

export async function runLegacyMigration(): Promise<void> {
	return await invoke<void>('legacy_migrate')
}
