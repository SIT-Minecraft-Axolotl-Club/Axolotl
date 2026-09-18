<script setup lang="ts">
import { CheckIcon, CopyIcon, ExternalIcon } from '@modrinth/assets'
import { ButtonStyled, defineMessages, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onUnmounted, ref } from 'vue'

import minecraftTitle from '@/assets/sitmc/minecraft-title.png'
import {
	begin_sitmc_device_login,
	begin_yggdrasil_login,
	delete_yggdrasil_password,
	finish_yggdrasil_login,
	get_yggdrasil_password,
	list_yggdrasil_saved_logins,
	poll_sitmc_device_login,
	set_yggdrasil_password,
} from '@/helpers/auth'

/** The club's player guide, opened in the system browser from the sign-in card. */
const PLAY_GUIDE_URL = 'https://www.sitmc.club/guide'

/** How long the "copied" confirmation stays visible after the code is copied. */
const COPY_FEEDBACK_MS = 2000

type SitmcProfile = {
	id: string
	name: string
}

type SitmcDeviceLoginFlow = {
	flow_id: string
	user_code: string
	verification_uri: string
	verification_uri_complete: string
	expires_in: number
	interval: number
}

type YggdrasilLoginResult =
	| { status: 'complete'; credentials: unknown }
	| { status: 'select_profile'; flow_id: string; profiles: SitmcProfile[] }

/**
 * The account site picks the character on its authorization page, so this flow
 * only ever reports pending or complete.
 */
type SitmcDeviceLoginPoll =
	{ status: 'pending'; slow_down: boolean } | { status: 'complete'; credentials: unknown }

type SavedLogin = {
	api_root: string
	login: string
}

const emit = defineEmits<{
	complete: [credentials: unknown]
	close: []
}>()

const props = withDefaults(defineProps<{ overlay?: boolean }>(), { overlay: false })

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: { id: 'sitmc-login.title', defaultMessage: 'Sign in to SIT-Minecraft' },
	subtitle: { id: 'sitmc-login.subtitle', defaultMessage: 'Sign in to begin your adventure' },
	signInWithSite: {
		id: 'sitmc-login.sign-in-with-site',
		defaultMessage: 'Sign in with a SIT-Minecraft account',
	},
	signInHint: {
		id: 'sitmc-login.sign-in-hint',
		defaultMessage: 'Opens your browser to authorize',
	},
	starting: {
		id: 'sitmc-login.starting',
		defaultMessage: 'Requesting an authorization code from the account site...',
	},
	deviceDescription: {
		id: 'sitmc-login.device-description',
		defaultMessage: 'If the browser did not open, visit the address below and enter this code:',
	},
	copyCode: { id: 'sitmc-login.copy-code', defaultMessage: 'Copy the code' },
	codeCopied: { id: 'sitmc-login.code-copied', defaultMessage: 'Copied' },
	openVerification: {
		id: 'sitmc-login.open-verification',
		defaultMessage: 'Open the authorization page again',
	},
	waitingApproval: {
		id: 'sitmc-login.waiting-approval',
		defaultMessage: 'Waiting for you to finish authorizing in the browser...',
	},
	selectProfile: {
		id: 'sitmc-login.select-profile',
		defaultMessage: 'Choose a character',
	},
	selectProfileOnSite: {
		id: 'sitmc-login.select-profile-on-site',
		defaultMessage:
			'The authorization page asks which character to play as, and the launcher starts the game with that character.',
	},
	selectProfileHint: {
		id: 'sitmc-login.select-profile-hint',
		defaultMessage: 'This account owns several characters. Choose the one to play as.',
	},
	deviceExpired: {
		id: 'sitmc-login.device-expired',
		defaultMessage: 'The authorization code expired. Start the sign-in again.',
	},
	passwordFallback: {
		id: 'sitmc-login.password-fallback',
		defaultMessage: 'Use username and password instead',
	},
	deviceFallback: {
		id: 'sitmc-login.device-fallback',
		defaultMessage: 'Back to browser sign-in',
	},
	passwordHint: {
		id: 'sitmc-login.password-hint',
		defaultMessage:
			'When browser sign-in is unavailable, use your skin site account name and password directly.',
	},
	accountLabel: { id: 'sitmc-login.account-label', defaultMessage: 'Account (email or username)' },
	passwordLabel: { id: 'sitmc-login.password-label', defaultMessage: 'Password' },
	rememberPassword: {
		id: 'sitmc-login.remember-password',
		defaultMessage: 'Remember this password (stored by the system credential manager)',
	},
	signIn: { id: 'sitmc-login.sign-in', defaultMessage: 'Sign in' },
	savedAccounts: { id: 'sitmc-login.saved-accounts', defaultMessage: 'Remembered accounts' },
	forgetAccount: { id: 'sitmc-login.forget-account', defaultMessage: 'Remove' },
	playGuide: { id: 'sitmc-login.play-guide', defaultMessage: 'Play guide' },
	close: { id: 'sitmc-login.close', defaultMessage: 'Not now' },
})

const mode = ref<'device' | 'password'>('device')
const busy = ref(false)
const error = ref<string | null>(null)

const flow = ref<SitmcDeviceLoginFlow | null>(null)
const pendingFlowId = ref<string | null>(null)
const profiles = ref<SitmcProfile[]>([])
const codeCopied = ref(false)

const account = ref('')
const password = ref('')
const rememberPassword = ref(true)
const savedLogins = ref<SavedLogin[]>([])

let pollTimer: ReturnType<typeof setTimeout> | undefined
let pollGeneration = 0
let expiresAt = 0
let copyFeedbackTimer: ReturnType<typeof setTimeout> | undefined

const verificationUrl = computed(
	() => flow.value?.verification_uri_complete || flow.value?.verification_uri || '',
)

function reportError(value: unknown) {
	error.value = value instanceof Error ? value.message : String(value)
}

/**
 * The account site may lack the two endpoints the device flow finishes with.
 * When it does, the password form is the only way in, so move the player there
 * instead of leaving them with an error they cannot act on.
 */
function offerPasswordFallback(value: unknown) {
	const text = value instanceof Error ? value.message : String(value)
	if (
		text.includes('/authserver/oauth') ||
		text.includes('/sessionserver/session/minecraft/profile')
	) {
		mode.value = 'password'
		void loadSavedLogins()
	}
}

function stopPolling() {
	pollGeneration += 1
	if (pollTimer !== undefined) {
		clearTimeout(pollTimer)
		pollTimer = undefined
	}
}

function clearCopyFeedback() {
	codeCopied.value = false
	if (copyFeedbackTimer !== undefined) {
		clearTimeout(copyFeedbackTimer)
		copyFeedbackTimer = undefined
	}
}

function resetFlow() {
	stopPolling()
	clearCopyFeedback()
	flow.value = null
	pendingFlowId.value = null
	profiles.value = []
}

function resetAll() {
	resetFlow()
	error.value = null
	busy.value = false
	account.value = ''
	password.value = ''
}

async function complete(credentials: unknown) {
	resetAll()
	emit('complete', credentials)
}

async function openUrlSafely(url: string) {
	await openUrl(url).catch(() => {})
}

async function openVerification() {
	if (!verificationUrl.value) return
	await openUrlSafely(verificationUrl.value)
}

async function copyUserCode() {
	const code = flow.value?.user_code
	if (!code) return
	try {
		await navigator.clipboard.writeText(code)
	} catch (copyError) {
		reportError(copyError)
		return
	}
	codeCopied.value = true
	if (copyFeedbackTimer !== undefined) clearTimeout(copyFeedbackTimer)
	copyFeedbackTimer = setTimeout(() => {
		codeCopied.value = false
		copyFeedbackTimer = undefined
	}, COPY_FEEDBACK_MS)
}

async function pollDeviceLogin(generation: number) {
	if (generation !== pollGeneration) return
	const currentFlowId = pendingFlowId.value ?? flow.value?.flow_id
	if (!currentFlowId) return
	if (Date.now() >= expiresAt) {
		error.value = formatMessage(messages.deviceExpired)
		return
	}

	try {
		const result = (await poll_sitmc_device_login(currentFlowId)) as SitmcDeviceLoginPoll
		if (generation !== pollGeneration) return

		if (result.status === 'complete') {
			await complete(result.credentials)
			return
		}

		const delay = Math.max(flow.value?.interval ?? 5, 1) * 1000
		pollTimer = setTimeout(
			() => void pollDeviceLogin(generation),
			result.slow_down ? delay + 5000 : delay,
		)
	} catch (pollError) {
		if (generation !== pollGeneration) return
		reportError(pollError)
		offerPasswordFallback(pollError)
	}
}

async function startDeviceLogin() {
	if (busy.value) return
	resetFlow()
	error.value = null
	busy.value = true
	try {
		const started = (await begin_sitmc_device_login()) as SitmcDeviceLoginFlow
		flow.value = started
		expiresAt = Date.now() + Math.max(started.expires_in, 60) * 1000
		await openVerification()
		const generation = (pollGeneration += 1)
		const delay = Math.max(started.interval, 1) * 1000
		pollTimer = setTimeout(() => void pollDeviceLogin(generation), delay)
	} catch (startError) {
		reportError(startError)
		offerPasswordFallback(startError)
	} finally {
		busy.value = false
	}
}

async function loadSavedLogins() {
	const loaded = (await list_yggdrasil_saved_logins().catch(() => [])) as SavedLogin[] | null
	savedLogins.value = (loaded ?? []).filter((entry) => entry.login)
}

async function showPasswordLogin() {
	mode.value = 'password'
	resetFlow()
	error.value = null
	await loadSavedLogins()
}

async function useSavedLogin(saved: SavedLogin) {
	account.value = saved.login
	const stored = (await get_yggdrasil_password(saved.login).catch(() => null)) as string | null
	if (stored) password.value = stored
}

async function forgetSavedLogin(saved: SavedLogin) {
	await delete_yggdrasil_password(saved.login).catch(handlePasswordError)
	if (account.value === saved.login) password.value = ''
	await loadSavedLogins()
}

function handlePasswordError(value: unknown) {
	reportError(value)
}

async function submitPasswordLogin() {
	if (busy.value) return
	if (!account.value.trim() || !password.value) return
	busy.value = true
	error.value = null
	try {
		const result = (await begin_yggdrasil_login(
			account.value.trim(),
			password.value,
		)) as YggdrasilLoginResult

		if (result.status === 'select_profile') {
			pendingFlowId.value = result.flow_id
			profiles.value = result.profiles
			return
		}

		if (rememberPassword.value) {
			await set_yggdrasil_password(account.value.trim(), password.value).catch(() => {})
		} else {
			await delete_yggdrasil_password(account.value.trim()).catch(() => {})
		}
		await complete(result.credentials)
	} catch (loginError) {
		reportError(loginError)
	} finally {
		busy.value = false
	}
}

async function selectPasswordProfile(profileId: string) {
	if (busy.value || !pendingFlowId.value) return
	busy.value = true
	error.value = null
	try {
		const credentials = await finish_yggdrasil_login(pendingFlowId.value, profileId)
		if (rememberPassword.value) {
			await set_yggdrasil_password(account.value.trim(), password.value).catch(() => {})
		}
		await complete(credentials)
	} catch (selectError) {
		reportError(selectError)
	} finally {
		busy.value = false
	}
}

onUnmounted(() => {
	stopPolling()
	clearCopyFeedback()
})

defineExpose({ resetAll })
</script>

<template>
	<div
		class="fixed inset-0 z-[100] flex items-center justify-center overflow-y-auto p-6"
		:class="props.overlay ? 'bg-black/70 backdrop-blur-sm' : 'bg-surface-1'"
	>
		<div class="w-full max-w-lg rounded-xl border border-divider bg-surface-2 p-8 shadow-2xl">
			<div class="flex flex-col gap-6">
				<header class="flex flex-col items-center gap-3 text-center">
					<img :src="minecraftTitle" alt="" class="h-auto w-full max-w-xs" />
					<h1 class="m-0 text-2xl font-semibold text-contrast">
						{{ formatMessage(messages.title) }}
					</h1>
					<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.subtitle) }}</p>
				</header>

				<template v-if="profiles.length > 0">
					<div class="flex flex-col gap-3">
						<h2 class="m-0 text-base font-medium text-contrast">
							{{ formatMessage(messages.selectProfile) }}
						</h2>
						<p class="m-0 text-sm text-secondary">
							{{ formatMessage(messages.selectProfileHint) }}
						</p>
						<ButtonStyled v-for="profile in profiles" :key="profile.id" class="w-full">
							<button :disabled="busy" @click="selectPasswordProfile(profile.id)">
								{{ profile.name }}
							</button>
						</ButtonStyled>
					</div>
				</template>

				<template v-else-if="mode === 'device'">
					<div class="flex flex-col gap-2">
						<ButtonStyled color="brand" class="w-full">
							<button :disabled="busy" @click="startDeviceLogin">
								{{ formatMessage(messages.signInWithSite) }}
							</button>
						</ButtonStyled>
						<p class="m-0 text-center text-xs text-secondary">
							{{ formatMessage(messages.signInHint) }}
						</p>
					</div>

					<div v-if="flow" class="flex flex-col gap-3 rounded-lg bg-surface-3 p-4">
						<p class="m-0 text-sm text-secondary">
							{{ formatMessage(messages.deviceDescription) }}
						</p>
						<div class="flex flex-col items-center gap-2">
							<button
								class="flex w-full cursor-pointer select-text items-center justify-center gap-3 rounded-lg border border-solid border-divider bg-surface-2 px-4 py-3 transition-colors hover:border-brand"
								type="button"
								:title="formatMessage(messages.copyCode)"
								:aria-label="formatMessage(messages.copyCode)"
								@click="copyUserCode"
							>
								<code class="select-text text-2xl font-bold tracking-[0.2em] text-contrast">
									{{ flow.user_code }}
								</code>
								<CheckIcon v-if="codeCopied" class="size-4 text-brand" />
								<CopyIcon v-else class="size-4 text-secondary" />
							</button>
							<p class="m-0 h-4 text-xs" role="status">
								<span v-if="codeCopied" class="inline-flex items-center gap-1 text-brand">
									<CheckIcon class="size-3.5" />
									{{ formatMessage(messages.codeCopied) }}
								</span>
							</p>
						</div>
						<ButtonStyled class="w-full">
							<button @click="openVerification">
								<ExternalIcon /> {{ formatMessage(messages.openVerification) }}
							</button>
						</ButtonStyled>
						<p class="m-0 text-xs text-secondary">
							{{ formatMessage(messages.waitingApproval) }}
						</p>
						<p class="m-0 text-xs text-secondary">
							{{ formatMessage(messages.selectProfileOnSite) }}
						</p>
					</div>
					<p v-else-if="busy" class="m-0 text-center text-sm text-secondary">
						{{ formatMessage(messages.starting) }}
					</p>

					<button
						class="m-0 self-center text-xs text-secondary underline"
						type="button"
						@click="showPasswordLogin"
					>
						{{ formatMessage(messages.passwordFallback) }}
					</button>
				</template>

				<template v-else>
					<p class="m-0 text-xs text-secondary">
						{{ formatMessage(messages.passwordHint) }}
					</p>

					<div v-if="savedLogins.length > 0" class="flex flex-col gap-2">
						<h2 class="m-0 text-sm font-medium text-contrast">
							{{ formatMessage(messages.savedAccounts) }}
						</h2>
						<div
							v-for="saved in savedLogins"
							:key="saved.login"
							class="flex items-center justify-between gap-2 rounded-lg bg-surface-3 px-3 py-2"
						>
							<button
								class="m-0 flex-1 truncate text-left text-sm text-contrast"
								type="button"
								@click="useSavedLogin(saved)"
							>
								{{ saved.login }}
							</button>
							<button
								class="m-0 text-xs text-secondary underline"
								type="button"
								@click="forgetSavedLogin(saved)"
							>
								{{ formatMessage(messages.forgetAccount) }}
							</button>
						</div>
					</div>

					<label class="flex flex-col gap-1 text-sm text-secondary">
						{{ formatMessage(messages.accountLabel) }}
						<input
							v-model="account"
							class="rounded-lg border border-divider bg-surface-1 px-3 py-2 text-contrast outline-none"
							type="text"
							autocomplete="username"
							@keyup.enter="submitPasswordLogin"
						/>
					</label>
					<label class="flex flex-col gap-1 text-sm text-secondary">
						{{ formatMessage(messages.passwordLabel) }}
						<input
							v-model="password"
							class="rounded-lg border border-divider bg-surface-1 px-3 py-2 text-contrast outline-none"
							type="password"
							autocomplete="current-password"
							@keyup.enter="submitPasswordLogin"
						/>
					</label>
					<label class="flex items-center gap-2 text-xs text-secondary">
						<input v-model="rememberPassword" type="checkbox" />
						{{ formatMessage(messages.rememberPassword) }}
					</label>

					<ButtonStyled color="brand" class="w-full">
						<button :disabled="busy || !account.trim() || !password" @click="submitPasswordLogin">
							{{ formatMessage(messages.signIn) }}
						</button>
					</ButtonStyled>

					<button
						class="m-0 self-center text-xs text-secondary underline"
						type="button"
						@click="mode = 'device'"
					>
						{{ formatMessage(messages.deviceFallback) }}
					</button>
				</template>

				<p v-if="error" class="m-0 text-sm text-red">{{ error }}</p>

				<footer
					class="flex flex-wrap items-center justify-between gap-3 border-0 border-t border-solid border-divider pt-4"
				>
					<button
						class="m-0 flex items-center gap-1 text-sm text-brand underline"
						type="button"
						@click="openUrlSafely(PLAY_GUIDE_URL)"
					>
						<ExternalIcon /> {{ formatMessage(messages.playGuide) }}
					</button>
					<ButtonStyled v-if="props.overlay">
						<button @click="emit('close')">{{ formatMessage(messages.close) }}</button>
					</ButtonStyled>
				</footer>
			</div>
		</div>
	</div>
</template>
