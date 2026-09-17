<template>
	<div
		v-if="offline"
		class="flex flex-col gap-1 bg-highlight-orange border border-solid border-orange rounded-xl p-3 mt-2"
	>
		<span class="font-semibold text-contrast">{{ formatMessage(messages.offlineMode) }}</span>
		<span class="text-sm text-secondary">{{ formatMessage(messages.offlineModeDescription) }}</span>
		<ButtonStyled>
			<button class="mt-1" :disabled="refreshingNetwork" @click="refreshNetworkStatus()">
				<SpinnerIcon v-if="refreshingNetwork" class="animate-spin" />
				<RefreshCwIcon v-else />
				{{ formatMessage(messages.refreshNetworkStatus) }}
			</button>
		</ButtonStyled>
	</div>
	<div
		v-if="accounts.length === 0"
		class="flex flex-col gap-3 bg-button-bg border border-solid border-surface-5 rounded-xl p-3 mt-2"
	>
		<span>{{ formatMessage(messages.notSignedIn) }}</span>
		<ButtonStyled v-if="!offline" color="brand">
			<button color="primary" :disabled="loginDisabled" @click="login()">
				<LogInIcon v-if="!loginDisabled" />
				<SpinnerIcon v-else class="animate-spin" />
				{{ formatMessage(messages.signInToSitmc) }}
			</button>
		</ButtonStyled>
		<ButtonStyled v-if="!offline">
			<button :disabled="loginDisabled" @click="openSitmcRegister()">
				<ExternalIcon />
				{{ formatMessage(messages.registerSitmcAccount) }}
			</button>
		</ButtonStyled>
		<ButtonStyled>
			<button :disabled="loginDisabled" @click="openSitmcSite()">
				<ExternalIcon />
				{{ formatMessage(messages.openSitmcSite) }}
			</button>
		</ButtonStyled>
	</div>
	<Accordion
		v-else
		class="w-full mt-2 bg-button-bg border border-solid border-surface-5 rounded-xl overflow-clip"
		button-class="button-base w-full bg-transparent px-3 py-2 border-0 cursor-pointer"
		:open-by-default="false"
	>
		<template #title>
			<div class="flex gap-2 w-full min-w-0">
				<Avatar
					size="36px"
					:src="selectedAccount ? avatarUrl : axolotlLogo"
					:pixelated="Boolean(selectedAccount)"
					:unframed-natural-width="72"
				/>
				<div class="flex flex-col items-start w-full min-w-0">
					<span class="truncate w-full text-left">{{
						selectedAccount ? selectedAccount.profile.name : formatMessage(messages.selectAccount)
					}}</span>
					<span class="text-secondary text-xs">{{ SitmcConfig.serverLabel }}</span>
				</div>
			</div>
		</template>
		<div class="bg-button-bg pt-1 pb-2 border-0 border-t border-solid border-surface-5">
			<div v-for="account in accounts" :key="account.account_id" class="flex gap-1 items-center">
				<button
					class="flex items-center flex-shrink flex-grow overflow-clip gap-2 p-2 border-0 bg-transparent cursor-pointer button-base min-w-0"
					@click="setAccount(account)"
				>
					<RadioButtonCheckedIcon
						v-if="selectedAccount && selectedAccount.account_id === account.account_id"
						class="w-5 h-5 text-brand shrink-0"
					/>
					<RadioButtonIcon v-else class="w-5 h-5 text-secondary shrink-0" />
					<Avatar
						:src="getAccountAvatarUrl(account)"
						size="24px"
						pixelated
						:unframed-natural-width="72"
					/>
					<div class="flex flex-1 min-w-0 flex-col text-left">
						<p
							class="m-0 truncate text-left"
							:class="
								selectedAccount && selectedAccount.account_id === account.account_id
									? 'text-contrast font-semibold'
									: 'text-primary'
							"
						>
							{{ account.profile.name }}
						</p>
						<p
							v-if="duplicateAccountNames.has(account.profile.name)"
							class="m-0 truncate text-left text-xs text-secondary"
						>
							{{ account.profile.id }}
						</p>
					</div>
					<span class="text-secondary text-xs shrink-0">{{ SitmcConfig.serverLabel }}</span>
				</button>
				<div class="flex shrink-0 items-center">
					<button
						v-tooltip="formatMessage(messages.copyUuid)"
						type="button"
						class="button-base border-0 bg-transparent p-1.5 cursor-pointer text-secondary hover:text-brand"
						@click="copyAccountUuid(account)"
					>
						<CopyIcon />
					</button>
					<button
						v-tooltip="formatMessage(messages.removeAccount)"
						type="button"
						class="button-base border-0 bg-transparent p-1.5 cursor-pointer text-secondary hover:text-red"
						@click="logout(account)"
					>
						<TrashIcon />
					</button>
				</div>
			</div>
			<div class="flex flex-col gap-2 px-2 pt-2">
				<ButtonStyled v-if="!offline" class="w-full">
					<button :disabled="loginDisabled" @click="login()">
						<PlusIcon />
						{{ formatMessage(messages.addSitmcAccount) }}
					</button>
				</ButtonStyled>
				<ButtonStyled class="w-full">
					<button :disabled="loginDisabled" @click="openSitmcSite()">
						<ExternalIcon />
						{{ formatMessage(messages.openSitmcSite) }}
					</button>
				</ButtonStyled>
			</div>
		</div>
	</Accordion>
	<Teleport to="body">
		<SitmcLoginGate
			v-if="showSitmcLogin"
			overlay
			@complete="onSitmcLoginComplete"
			@close="closeSitmcLogin"
		/>
	</Teleport>
</template>

<script setup lang="ts">
import {
	CopyIcon,
	ExternalIcon,
	LogInIcon,
	PlusIcon,
	RadioButtonCheckedIcon,
	RadioButtonIcon,
	RefreshCwIcon,
	SpinnerIcon,
	TrashIcon,
} from '@modrinth/assets'
import {
	Accordion,
	Avatar,
	ButtonStyled,
	defineMessages,
	injectNotificationManager,
	useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { Ref } from 'vue'
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import axolotlLogo from '@/assets/axolotl.png'
import steveSkinTexture from '@/assets/skins/steve.png?inline'
import SitmcLoginGate from '@/components/ui/login/SitmcLoginGate.vue'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { SitmcConfig } from '@/config'
import { compareMinecraftAccounts } from '@/helpers/accounts'
import { trackEvent } from '@/helpers/analytics'
import { get_default_user, remove_user, set_default_user, users } from '@/helpers/auth'
import { process_listener } from '@/helpers/events'
import { getPlayerHeadUrl } from '@/helpers/rendering/batch-skin-renderer.ts'
import type { Skin } from '@/helpers/skins'
import { get_available_skins } from '@/helpers/skins'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { offline, refreshBrowserOffline } = useNetworkStatus()
const queryClient = useQueryClient()
const route = useRoute()
const refreshingNetwork = ref(false)

/**
 * Re-checks session server reachability on demand so users can leave offline
 * mode without restarting the launcher. Goes through the shared
 * `authServerReachability` query so the reachability state and the auth
 * warning banner stay consistent.
 */
async function refreshNetworkStatus() {
	if (refreshingNetwork.value) return
	refreshingNetwork.value = true
	try {
		refreshBrowserOffline()
		await nextTick()
		await queryClient.refetchQueries({ queryKey: ['authServerReachability'] })
	} finally {
		refreshingNetwork.value = false
	}
}

const emit = defineEmits<{
	change: []
}>()

type MinecraftCredential = {
	account_id: string
	account_type: 'microsoft' | 'offline' | 'yggdrasil'
	profile: {
		id: string
		name: string
		skins?: Array<{
			state: string
			url: string
			variant: Skin['variant']
			textureKey?: string
		}>
	}
}

const accounts: Ref<MinecraftCredential[]> = ref([])
const loginDisabled = ref(false)
const defaultUser = ref<string | undefined>()
const equippedSkin = ref<Skin | null>(null)
const accountChangeRevision = ref(0)
const showSitmcLogin = ref(false)
const headUrlCache = ref(new Map<string, string>())
const accountHeadUrlCache = ref(new Map<string, string>())
const accountHeadTextureKeyCache = ref(new Map<string, string>())
let refreshGeneration = 0
let headRefreshTimer: ReturnType<typeof setTimeout> | undefined
let defaultUserUpdateQueue = Promise.resolve()

function createSkinHeadDataUrl(textureUrl: string) {
	const escapedTextureUrl = textureUrl
		.replaceAll('&', '&amp;')
		.replaceAll('"', '&quot;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 8 8" shape-rendering="crispEdges"><image href="${escapedTextureUrl}" x="-8" y="-8" width="64" height="64" style="image-rendering:pixelated"/><image href="${escapedTextureUrl}" x="-40" y="-8" width="64" height="64" style="image-rendering:pixelated"/></svg>`

	return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

const defaultSteveHeadUrl = createSkinHeadDataUrl(steveSkinTexture)

const HEAD_REFRESH_RETRY_DELAYS = [1500, 5000, 15000, 30000] as const
const HEAD_REFRESH_CONTINUOUS_DELAY = 60_000

function hasResolvedAccountHead(account: MinecraftCredential) {
	const skin = getAccountSkin(account)
	return Boolean(
		skin &&
		accountHeadUrlCache.value.has(account.account_id) &&
		accountHeadTextureKeyCache.value.get(account.account_id) === skin.texture_key,
	)
}

function hasMissingAccountHeads() {
	return accounts.value.some(
		(account) => account.account_type !== 'offline' && !hasResolvedAccountHead(account),
	)
}

function clearHeadRefreshRetry() {
	if (headRefreshTimer !== undefined) {
		clearTimeout(headRefreshTimer)
		headRefreshTimer = undefined
	}
}

function scheduleHeadRefreshRetry(generation: number, attempt: number) {
	if (offline.value || generation !== refreshGeneration) return
	const delay = HEAD_REFRESH_RETRY_DELAYS[attempt] ?? HEAD_REFRESH_CONTINUOUS_DELAY

	clearHeadRefreshRetry()
	headRefreshTimer = setTimeout(() => {
		headRefreshTimer = undefined
		if (generation !== refreshGeneration) return
		void refreshValues(Math.min(attempt + 1, HEAD_REFRESH_RETRY_DELAYS.length)).catch((error) => {
			console.warn('Failed to refresh account heads:', error)
		})
	}, delay)
}

async function refreshValues(headRefreshAttempt = 0) {
	clearHeadRefreshRetry()
	const generation = ++refreshGeneration
	const selectedUser = await get_default_user(offline.value).catch(handleError)
	if (generation !== refreshGeneration) return

	defaultUser.value = selectedUser
	if (offline.value && selectedUser) {
		await persistDefaultUser(selectedUser)
		if (generation !== refreshGeneration) return
	}
	const userList = await users(offline.value).catch(handleError)
	if (generation !== refreshGeneration) return
	// The Rust backend returns a plain array that structurally matches
	// MinecraftCredential but the TS types from the Tauri IPC bridge do not
	// carry this refinement. The double cast is deliberate — the shape is
	// correct and verified at runtime by the backend.
	accounts.value = Array.isArray(userList)
		? [...(userList as unknown as MinecraftCredential[])]
		: []
	accounts.value.sort(compareMinecraftAccounts)
	await renderAccountHeads(accounts.value, generation)
	if (generation !== refreshGeneration) return
	try {
		const skins = await get_available_skins()
		if (generation !== refreshGeneration) return
		equippedSkin.value = skins.find((skin) => skin.is_equipped) ?? null

		if (equippedSkin.value) {
			try {
				const headUrl = await getPlayerHeadUrl(equippedSkin.value)
				if (generation !== refreshGeneration) return
				headUrlCache.value = new Map(headUrlCache.value).set(
					equippedSkin.value.texture_key,
					headUrl,
				)
				if (selectedUser) {
					const selectedAccountSkin = getAccountSkin(
						accounts.value.find((account) => account.account_id === selectedUser),
					)
					cacheAccountHead(selectedUser, selectedAccountSkin ?? equippedSkin.value, headUrl)
				}
			} catch (error) {
				console.warn('Failed to get head render for equipped skin:', error)
			}
		}
	} catch {
		equippedSkin.value = null
	}

	if (hasMissingAccountHeads()) {
		scheduleHeadRefreshRetry(generation, headRefreshAttempt)
	}
}

async function setEquippedSkin(skin: Skin) {
	const selectedUser = defaultUser.value
	equippedSkin.value = skin

	try {
		const headUrl = await getPlayerHeadUrl(skin)
		headUrlCache.value = new Map(headUrlCache.value).set(skin.texture_key, headUrl)
		if (selectedUser) {
			cacheAccountHead(selectedUser, skin, headUrl)
		}
	} catch (error) {
		console.warn('Failed to get head render for equipped skin:', error)
	}
}

function setLoginDisabled(value: boolean) {
	loginDisabled.value = value
}

defineExpose({
	accountChangeRevision,
	accounts,
	login,
	refreshValues,
	setEquippedSkin,
	setLoginDisabled,
	loginDisabled,
})

await refreshValues()

watch(offline, async () => {
	await refreshValues()
	notifyAccountChange()
})

const selectedAccount = computed(() =>
	accounts.value.find((account) => account.account_id === defaultUser.value),
)

function notifyAccountChange() {
	accountChangeRevision.value += 1
	emit('change')
}

const duplicateAccountNames = computed(() => {
	const counts = new Map<string, number>()
	for (const account of accounts.value) {
		counts.set(account.profile.name, (counts.get(account.profile.name) ?? 0) + 1)
	}
	return new Set([...counts].filter(([, count]) => count > 1).map(([name]) => name))
})

watch(
	() => route.fullPath,
	() => {
		if (!hasMissingAccountHeads()) return
		void refreshValues().catch((error) => {
			console.warn('Failed to refresh account heads after navigation:', error)
		})
	},
)

function getAccountSkin(account: MinecraftCredential | undefined): Skin | undefined {
	if (!account || account.account_type === 'offline') return undefined
	const skin =
		account.profile.skins?.find((skin) => skin.state === 'ACTIVE') ?? account.profile.skins?.[0]
	if (!skin?.url) return undefined

	return {
		texture_key: skin.textureKey ?? `${account.profile.id}:${skin.url}`,
		variant: skin.variant ?? 'UNKNOWN',
		texture: skin.url,
		source: 'custom_external',
		is_equipped: true,
	}
}

function cacheAccountHead(accountId: string, skin: Skin, headUrl: string) {
	accountHeadUrlCache.value = new Map(accountHeadUrlCache.value).set(accountId, headUrl)
	accountHeadTextureKeyCache.value = new Map(accountHeadTextureKeyCache.value).set(
		accountId,
		skin.texture_key,
	)
}

async function renderAccountHeads(accountList: MinecraftCredential[], generation: number) {
	await Promise.all(
		accountList.map(async (account) => {
			const skin = getAccountSkin(account)
			if (!skin) return

			try {
				const headUrl = await getPlayerHeadUrl(skin)
				if (generation !== refreshGeneration) return
				cacheAccountHead(account.account_id, skin, headUrl)
			} catch (error) {
				console.warn(`Failed to render head for account ${account.account_id}:`, error)
			}
		}),
	)
}

const avatarUrl = computed(() => {
	if (selectedAccount.value) {
		const cachedHeadUrl = accountHeadUrlCache.value.get(selectedAccount.value.account_id)
		if (cachedHeadUrl) return cachedHeadUrl
	}
	if (equippedSkin.value?.texture_key) {
		const cachedUrl = headUrlCache.value.get(equippedSkin.value.texture_key)
		if (cachedUrl) {
			return cachedUrl
		}
	}
	return selectedAccount.value ? defaultSteveHeadUrl : axolotlLogo
})

function getAccountAvatarUrl(account: MinecraftCredential) {
	const cachedHeadUrl = accountHeadUrlCache.value.get(account.account_id)
	if (cachedHeadUrl) {
		return cachedHeadUrl
	}
	if (
		account.account_id === selectedAccount.value?.account_id &&
		equippedSkin.value?.texture_key
	) {
		const cachedUrl = headUrlCache.value.get(equippedSkin.value.texture_key)
		if (cachedUrl) {
			return cachedUrl
		}
	}
	return defaultSteveHeadUrl
}

function persistDefaultUser(userId: string) {
	const update = defaultUserUpdateQueue.then(async () => {
		await set_default_user(userId).catch(handleError)
	})
	defaultUserUpdateQueue = update.catch(() => {})
	return update
}

async function setAccount(account: MinecraftCredential) {
	const userId = account.account_id
	refreshGeneration += 1
	defaultUser.value = userId
	equippedSkin.value = null

	await persistDefaultUser(userId)
	if (defaultUser.value !== userId) return
	await refreshValues()
	if (defaultUser.value === userId) notifyAccountChange()
}

function login() {
	if (offline.value) return
	showSitmcLogin.value = true
}

function closeSitmcLogin() {
	showSitmcLogin.value = false
}

async function onSitmcLoginComplete() {
	closeSitmcLogin()
	loginDisabled.value = true
	try {
		await refreshValues()
		notifyAccountChange()
		trackEvent('AccountLogIn')
	} catch (error) {
		handleError(error as Error)
	} finally {
		loginDisabled.value = false
	}
}

async function openSitmcRegister() {
	await openUrl(SitmcConfig.registerUrl).catch(() => {})
}

async function openSitmcSite() {
	await openUrl(SitmcConfig.site).catch(() => {})
}

async function logout(account: MinecraftCredential) {
	await remove_user(account.account_id).catch(handleError)
	await refreshValues()
	if (!selectedAccount.value && accounts.value.length > 0) {
		await setAccount(accounts.value[0])
	} else {
		notifyAccountChange()
	}
	trackEvent('AccountLogOut')
}

async function copyAccountUuid(account: MinecraftCredential) {
	try {
		await navigator.clipboard.writeText(account.profile.id)
	} catch (error) {
		handleError(error as Error)
	}
}

const unlisten = await process_listener(async (e) => {
	if (e.event === 'launched') {
		await refreshValues()
	}
})

onUnmounted(() => {
	clearHeadRefreshRetry()
	unlisten()
})

const messages = defineMessages({
	offlineMode: {
		id: 'minecraft-account.offline-mode',
		defaultMessage: 'Offline mode',
	},
	offlineModeDescription: {
		id: 'minecraft-account.offline-mode.description',
		defaultMessage:
			'Only offline accounts are available. You can launch fully downloaded instances.',
	},
	refreshNetworkStatus: {
		id: 'minecraft-account.offline-mode.refresh',
		defaultMessage: 'Refresh connection status',
	},
	notSignedIn: {
		id: 'minecraft-account.not-signed-in',
		defaultMessage: 'Not signed in',
	},
	signInToSitmc: {
		id: 'sitmc-account.sign-in',
		defaultMessage: 'Sign in to SIT-Minecraft',
	},
	registerSitmcAccount: {
		id: 'sitmc-account.register',
		defaultMessage: 'Register a SIT-Minecraft account',
	},
	openSitmcSite: {
		id: 'sitmc-account.open-site',
		defaultMessage: 'Open the skin site',
	},
	addSitmcAccount: {
		id: 'sitmc-account.add',
		defaultMessage: 'Add a SIT-Minecraft account',
	},
	copyUuid: {
		id: 'minecraft-account.copy-uuid',
		defaultMessage: 'Copy UUID',
	},
	removeAccount: {
		id: 'minecraft-account.remove-account',
		defaultMessage: 'Remove account',
	},
	selectAccount: {
		id: 'minecraft-account.select-account',
		defaultMessage: 'Select account',
	},
})
</script>
