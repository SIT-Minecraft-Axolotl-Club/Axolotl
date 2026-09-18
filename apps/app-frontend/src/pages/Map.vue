<script setup lang="ts">
import { ExternalIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, ref, watch } from 'vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: { id: 'app.map.title', defaultMessage: 'Map' },
	pixelsit: { id: 'app.map.tabs.pixelsit', defaultMessage: 'PixelSIT' },
	survival: { id: 'app.map.tabs.survival', defaultMessage: 'Survival Satellite' },
	openInBrowser: { id: 'button.open-in-browser', defaultMessage: 'Open in browser' },
})

type SocietyMapId = 'pixelsit' | 'survival'

type SocietyMap = {
	id: SocietyMapId
	label: string
	url: string
}

const MAPS: SocietyMap[] = [
	{
		id: 'pixelsit',
		label: messages.pixelsit.id,
		url: 'https://www.sitmc.club/pixelsit/',
	},
	{
		id: 'survival',
		label: messages.survival.id,
		url: 'https://survival.sitmc.club/',
	},
]

const STORAGE_KEY = 'axolotl-map-selected-map'

function storedMapId(): SocietyMapId {
	try {
		const stored = localStorage.getItem(STORAGE_KEY)
		if (stored === 'pixelsit' || stored === 'survival') return stored
	} catch {
		// Storage can be unavailable; the first map is a fine default then.
	}
	return 'pixelsit'
}

const selectedId = ref<SocietyMapId>(storedMapId())
const selectedMap = computed(() => MAPS.find((map) => map.id === selectedId.value) ?? MAPS[0])

watch(selectedId, (id) => {
	try {
		localStorage.setItem(STORAGE_KEY, id)
	} catch {
		// Ignore a storage failure: the choice simply does not persist.
	}
})
</script>

<template>
	<div class="flex h-full min-h-0 flex-col gap-4 p-6">
		<header class="flex flex-wrap items-center gap-3">
			<h1 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.title) }}</h1>

			<div
				data-onboarding-id="map-tabs"
				class="flex items-center gap-1 rounded-full bg-surface-4 p-1"
				role="tablist"
			>
				<button
					v-for="map in MAPS"
					:key="map.id"
					type="button"
					role="tab"
					:aria-selected="selectedId === map.id"
					class="cursor-pointer rounded-full border-none px-4 py-1.5 text-sm transition-colors"
					:class="
						selectedId === map.id
							? 'bg-surface-1 text-contrast'
							: 'bg-transparent text-secondary hover:text-contrast'
					"
					@click="selectedId = map.id"
				>
					{{ formatMessage({ id: map.label, defaultMessage: map.id }) }}
				</button>
			</div>

			<button
				type="button"
				class="ml-auto flex cursor-pointer items-center gap-2 rounded-full bg-surface-4 px-4 py-1.5 text-sm text-secondary transition-colors hover:text-contrast"
				@click="openUrl(selectedMap.url)"
			>
				<ExternalIcon class="size-4" />
				{{ formatMessage(messages.openInBrowser) }}
			</button>
		</header>

		<!--
			The society removed the X-Frame-Options header from these map paths, so the
			pages can be embedded directly and the switcher stays usable above them.
			The sandbox keeps an embedded map from capturing the pointer or opening
			itself fullscreen, which used to leave the rest of the launcher dead.
		-->
		<iframe
			:key="selectedMap.id"
			:src="selectedMap.url"
			:title="selectedMap.url"
			class="min-h-96 w-full flex-1 rounded-[var(--radius-lg)] border border-solid border-divider bg-surface-4"
			sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
			referrerpolicy="strict-origin-when-cross-origin"
		/>
	</div>
</template>
