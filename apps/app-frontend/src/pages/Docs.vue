<script setup lang="ts">
import { ChevronRightIcon, ExternalIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import { SitmcConfig } from '@/config'
import type { FeishuWikiNode } from '@/helpers/feishu-doc'
import { feishuComponentAuth, feishuDocsTree, feishuResolveDocument } from '@/helpers/feishu-doc'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: { id: 'app.docs.title', defaultMessage: 'Documentation' },
	openInBrowser: { id: 'button.open-in-browser', defaultMessage: 'Open in browser' },
	loading: { id: 'app.docs.loading', defaultMessage: 'Loading the documentation…' },
	failed: {
		id: 'app.docs.failed',
		defaultMessage: 'The documentation could not be loaded. Use "Open in browser" above.',
	},
	contents: { id: 'app.docs.contents', defaultMessage: 'Contents' },
	overview: { id: 'app.docs.overview', defaultMessage: 'Overview' },
})

/** The documentation root, and the page the entry point opens. */
const rootUrl = SitmcConfig.docsUrl

/** Element Feishu's component renders the document into. */
const mount = ref<HTMLElement | null>(null)

const loading = ref(true)
const error = ref<string | null>(null)

/**
 * The document's own colour scheme, taken from the launcher's.
 *
 * The launcher marks the scheme on the root element, which is also how its skin
 * editor reads it, so the document matches whatever the player picked instead of
 * forcing one scheme.
 */
const theme = ref(documentTheme())

function documentTheme(): 'light' | 'dark' {
	const root = document.documentElement
	return root.classList.contains('dark-mode') || root.classList.contains('oled-mode')
		? 'dark'
		: 'light'
}

/** Pages below the root, flattened with their nesting level. */
const tree = ref<FeishuWikiNode[]>([])
/** Address of the page on show. */
const currentUrl = ref(rootUrl)
/** List positions whose children are folded away. */
const collapsed = ref(new Set<number>())

const hasTree = computed(() => tree.value.length > 0)

/**
 * The pages whose parent chain is open.
 *
 * The platform returns the tree already flattened, so a folded position hides
 * every later entry nested below it and nothing else.
 */
const visibleNodes = computed(() => {
	const visible: { node: FeishuWikiNode; index: number }[] = []
	let hiddenBelow: number | null = null

	for (const [index, node] of tree.value.entries()) {
		if (hiddenBelow !== null) {
			if (node.depth > hiddenBelow) continue
			hiddenBelow = null
		}
		visible.push({ node, index })
		if (node.hasChild && collapsed.value.has(index)) {
			hiddenBelow = node.depth
		}
	}

	return visible
})

function toggle(index: number) {
	const next = new Set(collapsed.value)
	if (next.has(index)) {
		next.delete(index)
	} else {
		next.add(index)
	}
	collapsed.value = next
}

/**
 * Shows a page.
 *
 * Feishu's component cannot move between documents on its own — a read-only
 * component running as the application follows no links — so the navigation is
 * the launcher's and this is what a click calls.
 */
function show(url: string) {
	if (!url || url === currentUrl.value) return
	currentUrl.value = url
	void render()
}

/** The SDK entry point, loaded on demand so the page costs nothing until used. */
const SDK_URL =
	'https://sf1-scmcdn-cn.feishucdn.com/obj/feishu-static/docComponentSdk/lib/1.0.13.js'

interface DocComponent {
	start(): Promise<unknown>
	destroy(): void
	/** Swaps in another document without rebuilding the component. */
	replace(src: string): Promise<unknown>
	/** Re-applies presentation options; only valid once the document mounted. */
	setFeatureConfig(config: Record<string, unknown>): void
}

interface DocComponentConstructor {
	new (options: Record<string, unknown>): DocComponent
}

declare global {
	interface Window {
		DocComponentSdk?: DocComponentConstructor
	}
}

let component: DocComponent | null = null
/** Colour scheme the mounted component was built with. */
let builtTheme: 'light' | 'dark' | null = null
let authTimer: ReturnType<typeof setTimeout> | null = null
let sdkPromise: Promise<DocComponentConstructor> | null = null
let themeObserver: MutationObserver | null = null

/**
 * Loads the component SDK once.
 *
 * The library is served from Feishu's CDN and published as a plain script that
 * assigns its constructor to the window, so it is injected rather than imported.
 */
function loadSdk(): Promise<DocComponentConstructor> {
	if (window.DocComponentSdk) return Promise.resolve(window.DocComponentSdk)
	if (sdkPromise) return sdkPromise

	sdkPromise = new Promise((resolve, reject) => {
		const script = document.createElement('script')
		script.src = SDK_URL
		script.async = true
		script.onload = () => {
			if (window.DocComponentSdk) {
				resolve(window.DocComponentSdk)
			} else {
				reject(new Error('The Feishu component SDK loaded without its constructor'))
			}
		}
		script.onerror = () => reject(new Error(`Failed to load ${SDK_URL}`))
		document.head.appendChild(script)
	}).catch((reason) => {
		sdkPromise = null
		throw reason
	})

	return sdkPromise
}

function describe(reason: unknown): string {
	if (reason instanceof Error) return reason.message
	if (typeof reason === 'string') return reason
	return JSON.stringify(reason)
}

/**
 * Authenticates and renders the document on show.
 *
 * The first call builds the component; every later call swaps the document into
 * it, so the page's own session and the layout the component has already painted
 * survive a page switch. Feishu expires a signature ten minutes after it was
 * issued, so both paths ask for a fresh one, and the same path serves a
 * signature the component reports as rejected.
 */
async function render() {
	if (!mount.value) return
	error.value = null
	loading.value = true

	try {
		const [Sdk, auth, document] = await Promise.all([
			loadSdk(),
			feishuComponentAuth(),
			feishuResolveDocument(currentUrl.value),
		])

		if (!document.supported) {
			throw new Error(`The documentation address ${currentUrl.value} is not a document`)
		}

		const credentials = {
			signature: auth.signature,
			appId: auth.appId,
			timestamp: auth.timestamp,
			nonceStr: auth.nonceStr,
			url: auth.url,
			jsApiList: ['DocsComponent'],
		}

		/**
		 * Builds the component around a document, with its presentation options.
		 *
		 * Presentation options are a constructor argument: the document header and
		 * footer are dropped because the launcher supplies its own frame around it,
		 * the outline is asked for because Feishu keeps it off by default, and the
		 * like button is dropped because it belongs to an editor the player is not
		 * in.
		 */
		const create = (src: string) => {
			const created = new Sdk({
				src,
				mount: mount.value,
				// Feishu asks for a fixed height; an auto height loads the whole
				// document at once.
				size: { height: '100%' },
				theme: theme.value,
				auth: credentials,
				config: {
					extensions: {
						suiteNavBar: { disable: true },
						directory: { disable: false, pin: true },
						like: { disable: true },
						footer: { enable: false },
					},
				},
				// `pin` only counts when the document is initialised, so the outline
				// is asked for again once it has mounted. That is what leaves the
				// article's outline open instead of collapsed.
				onMountSuccess: () => {
					created.setFeatureConfig({
						extensions: { directory: { disable: false, pin: true } },
					})
				},
				onAuthError: () => void render(),
				onError: (componentError: unknown) => {
					error.value = describe(componentError)
				},
			})
			return created
		}

		// A colour scheme is fixed when a component is built, so a launcher theme
		// the player switched since then means the component has to be rebuilt.
		// That is also the one case where the document is not swapped in place: it
		// has to be handed to the new component instead.
		const themeChanged = component !== null && builtTheme !== theme.value
		if (themeChanged) {
			component?.destroy()
			component = null
		}

		if (component) {
			// Feishu's documented way to move between pages. The mounted component
			// keeps its session, so the signature it was built with still holds; a
			// swap that fails falls back to rebuilding it, which re-authenticates.
			try {
				await component.replace(document.url)
				// A swapped document mounts on its own, so the outline is asked for
				// again: the article that is now on show should start open too.
				component.setFeatureConfig({
					extensions: { directory: { disable: false, pin: true } },
				})
			} catch (swapFailure) {
				console.warn('Could not swap the document; rebuilding the component:', swapFailure)
				component.destroy()
				component = null
			}
		}

		if (!component) {
			component = create(document.url)
			builtTheme = theme.value
			await component.start()
		}

		if (authTimer) clearTimeout(authTimer)
		authTimer = setTimeout(() => void render(), Math.max(60, auth.retryAfterSecs) * 1000)
	} catch (reason) {
		error.value = describe(reason)
	} finally {
		loading.value = false
	}
}

onMounted(async () => {
	await render()
	// The navigation is optional: a plain document address has no tree, and a
	// failed walk leaves the page readable. The failure is reported so a missing
	// list is never mistaken for a space without pages.
	tree.value = await feishuDocsTree(rootUrl)
		.then((result) => result.nodes)
		.catch((reason) => {
			console.warn('Could not list the documentation pages:', reason)
			return []
		})

	// The launcher switches its scheme by swapping a class on the root element, so
	// that class is what the document follows.
	themeObserver = new MutationObserver(() => {
		const next = documentTheme()
		if (next === theme.value) return
		theme.value = next
		void render()
	})
	themeObserver.observe(document.documentElement, {
		attributes: true,
		attributeFilter: ['class'],
	})
})

onBeforeUnmount(() => {
	if (authTimer) clearTimeout(authTimer)
	authTimer = null
	themeObserver?.disconnect()
	themeObserver = null
	component?.destroy()
	component = null
	builtTheme = null
})
</script>

<template>
	<div class="flex h-full min-h-0 flex-col gap-4 p-6">
		<header class="flex flex-wrap items-center gap-3">
			<h1 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.title) }}</h1>

			<button
				type="button"
				class="ml-auto flex cursor-pointer items-center gap-2 rounded-full bg-surface-4 px-4 py-1.5 text-sm text-secondary transition-colors hover:text-contrast"
				@click="openUrl(currentUrl)"
			>
				<ExternalIcon class="size-4" />
				{{ formatMessage(messages.openInBrowser) }}
			</button>
		</header>

		<p v-if="error" class="m-0 rounded-lg bg-surface-3 px-4 py-3 text-sm text-secondary">
			{{ formatMessage(messages.failed) }}
			<span class="text-xs opacity-70">({{ error }})</span>
		</p>

		<div class="flex min-h-0 flex-1 gap-4">
			<!--
				Feishu's component renders one document and cannot move between
				pages, so the navigation beside it is the launcher's own.
			-->
			<nav
				v-if="hasTree"
				:aria-label="formatMessage(messages.contents)"
				class="flex w-60 shrink-0 flex-col overflow-y-auto rounded-[var(--radius-lg)] border border-solid border-divider bg-surface-2 p-2"
			>
				<p class="m-0 px-2 py-1 text-xs font-semibold text-secondary">
					{{ formatMessage(messages.contents) }}
				</p>

				<button
					type="button"
					class="cursor-pointer truncate rounded-[var(--radius-sm)] border-none bg-transparent px-1 py-1 text-left text-sm hover:text-contrast"
					:class="currentUrl === rootUrl ? 'bg-surface-3 text-contrast' : 'text-secondary'"
					@click="show(rootUrl)"
				>
					{{ formatMessage(messages.overview) }}
				</button>

				<ul class="m-0 list-none p-0">
					<li v-for="{ node, index } in visibleNodes" :key="index">
						<div
							class="flex items-center gap-1 rounded-[var(--radius-sm)]"
							:class="node.url && node.url === currentUrl ? 'bg-surface-3' : ''"
						>
							<button
								v-if="node.hasChild"
								type="button"
								class="flex size-5 shrink-0 cursor-pointer items-center justify-center border-none bg-transparent p-0 text-secondary"
								:aria-expanded="!collapsed.has(index)"
								:aria-label="node.title"
								@click="toggle(index)"
							>
								<ChevronRightIcon
									class="size-3.5 transition-transform"
									:class="collapsed.has(index) ? '' : 'rotate-90'"
								/>
							</button>
							<span v-else class="size-5 shrink-0" />

							<button
								type="button"
								:disabled="!node.url"
								class="min-w-0 flex-1 cursor-pointer truncate border-none bg-transparent px-1 py-1 text-left text-sm disabled:cursor-default"
								:class="
									node.url && node.url === currentUrl
										? 'text-contrast'
										: 'text-secondary hover:text-contrast'
								"
								@click="show(node.url)"
							>
								{{ node.title }}
							</button>
						</div>
					</li>
				</ul>
			</nav>

			<!--
				Feishu's document component renders into this element; it is an
				ordinary part of the page, so nothing outside it can be affected by
				what the document does.
			-->
			<div
				ref="mount"
				class="relative min-h-96 min-w-0 flex-1 overflow-hidden rounded-[var(--radius-lg)] border border-solid border-divider bg-surface-4"
			>
				<p v-if="loading" class="m-0 p-4 text-sm text-secondary">
					{{ formatMessage(messages.loading) }}
				</p>
			</div>
		</div>
	</div>
</template>
