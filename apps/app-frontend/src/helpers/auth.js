/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { invoke } from '@tauri-apps/api/core'

// Example function:
// User goes to auth_url to complete flow, and when completed, authenticate_await_completion() returns the credentials
// export async function authenticate() {
//   const auth_url = await authenticate_begin_flow()
//   console.log(auth_url)
//   await authenticate_await_completion()
// }

/**
 * Check if the authentication servers are reachable, throwing an exception if
 * not reachable.
 */
export async function check_reachable() {
	await invoke('plugin:auth|check_reachable')
}

/**
 * Check the Mojang services mirrored by the Fallen proxy, returning their
 * individual reachability states.
 */
export async function check_mojang_services() {
	return await invoke('plugin:auth|check_mojang_services')
}

/**
 * Starts a sign-in with the SIT-Minecraft account site.
 *
 * The player approves `user_code` on `verification_uri_complete` in a browser.
 *
 * @returns {Promise<SitmcDeviceLoginFlow>} The code to show the player, plus the
 * `flow_id` used to poll and finish the sign-in.
 */
export async function begin_sitmc_device_login() {
	return await invoke('plugin:auth|begin_sitmc_device_login')
}

/**
 * Reports whether the player has approved the pending SIT-Minecraft sign-in.
 *
 * The account site asks for the character on its authorization page, so a
 * completed poll already carries the credentials.
 *
 * @param {string} flowId
 * @returns {Promise<{ status: 'pending', slow_down: boolean } | { status: 'complete', credentials: unknown }>}
 */
export async function poll_sitmc_device_login(flowId) {
	return await invoke('plugin:auth|poll_sitmc_device_login', { flowId })
}

export async function begin_yggdrasil_login(login, password) {
	return await invoke('plugin:auth|begin_yggdrasil_login', { login, password })
}

export async function finish_yggdrasil_login(flowId, profileId) {
	return await invoke('plugin:auth|finish_yggdrasil_login', { flowId, profileId })
}

export async function list_yggdrasil_saved_logins() {
	return await invoke('plugin:auth|list_yggdrasil_saved_logins')
}

export async function get_yggdrasil_password(login) {
	return await invoke('plugin:auth|get_yggdrasil_password', { login })
}

export async function set_yggdrasil_password(login, password) {
	return await invoke('plugin:auth|set_yggdrasil_password', { login, password })
}

export async function delete_yggdrasil_password(login) {
	return await invoke('plugin:auth|delete_yggdrasil_password', { login })
}

/**
 * Retrieves the default user
 * @return {Promise<UUID | undefined>}
 */
export async function get_default_user(offlineMode = false) {
	return await invoke('plugin:auth|get_default_user', { offlineMode })
}

/**
 * Updates the default user
 * @param {UUID} user
 */
export async function set_default_user(user) {
	return await invoke('plugin:auth|set_default_user', { user })
}

/**
 * Remove a user account from the database
 * @param {UUID} user
 */
export async function remove_user(user) {
	return await invoke('plugin:auth|remove_user', { user })
}

/**
 * Returns a list of users
 * @returns {Promise<Credential[]>}
 */
export async function users(offlineMode = false) {
	return await invoke('plugin:auth|get_users', { offlineMode })
}
