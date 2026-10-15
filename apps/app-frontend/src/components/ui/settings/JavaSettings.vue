<script setup>
import {
	Checkbox,
	defineMessages,
	injectNotificationManager,
	Slider,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { platform } from '@tauri-apps/plugin-os'
import { ref, watch } from 'vue'

import JavaArgumentsInput from '@/components/ui/JavaArgumentsInput.vue'
import MemoryAllocationDisplay from '@/components/ui/MemoryAllocationDisplay.vue'
import useMemorySlider from '@/composables/useMemorySlider'
import { collectGcContext } from '@/helpers/gc/context'
import { getJavaArgumentPresets } from '@/helpers/java-argument-presets'
import { get, set } from '@/helpers/settings.ts'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	autoHighPerformanceMode: {
		id: 'app.settings.java.auto-high-performance-mode',
		defaultMessage: 'Automatically use high-performance GPU for Java',
	},
	autoHighPerformanceModeDescription: {
		id: 'app.settings.java.auto-high-performance-mode-description',
		defaultMessage:
			'Uses the high-performance GPU for Minecraft when it launches. Supported on Windows and Linux.',
	},
	memory: {
		id: 'app.settings.defaults.memory',
		defaultMessage: 'Memory allocated',
	},
	memoryDescription: {
		id: 'app.settings.defaults.memory-description',
		defaultMessage: 'The memory allocated to each instance when it is run.',
	},
	automaticMemory: {
		id: 'app.settings.defaults.automatic-memory',
		defaultMessage: 'Automatically allocate memory at launch',
	},
	automaticMemoryDescription: {
		id: 'app.settings.defaults.automatic-memory-description',
		defaultMessage: 'Adjusts memory for each launch based on available RAM and installed mods.',
	},
	optimizeMemoryBeforeLaunch: {
		id: 'app.settings.defaults.optimize-memory-before-launch',
		defaultMessage: 'Optimize memory before launching the game',
	},
	optimizeMemoryBeforeLaunchDescription: {
		id: 'app.settings.defaults.optimize-memory-before-launch-description',
		defaultMessage: 'Waits for Windows memory optimization to finish before starting the game.',
	},
	javaArguments: {
		id: 'app.settings.defaults.java-arguments',
		defaultMessage: 'Java arguments',
	},
	javaArgumentsPlaceholder: {
		id: 'app.settings.defaults.java-arguments-placeholder',
		defaultMessage: 'Enter Java arguments...',
	},
})

const currentPlatform = await platform()
const supportsHighPerformanceMode = ['windows', 'linux'].includes(currentPlatform)
const supportsMemoryOptimization = currentPlatform === 'windows'
const settings = ref(await get().catch(handleError))
const autoHighPerformanceMode = ref(settings.value?.auto_set_java_high_performance_mode ?? false)

const javaArgs = ref((settings.value?.extra_launch_args ?? []).join(' '))

const memory = ref(
	settings.value?.memory
		? { optimize_before_launch: false, ...settings.value.memory }
		: { maximum: 2048, automatic: true, optimize_before_launch: false },
)

let shouldApplyDefaultAuto = (settings.value?.extra_launch_args?.length ?? 0) === 0

const memorySlider = useMemorySlider()
const maxMemory = memorySlider?.maxMemory ?? 4096
const snapPoints = memorySlider?.snapPoints ?? []

const gcContext = ref(undefined)

async function updateGcContext() {
	gcContext.value = await collectGcContext(memory.value.maximum, null, null, 0)

	if (shouldApplyDefaultAuto) {
		const autoPreset = getJavaArgumentPresets(gcContext.value).find(
			(preset) => preset.id === 'gc-auto',
		)

		if (autoPreset) {
			javaArgs.value = autoPreset.resolveArgs
				? autoPreset.resolveArgs(gcContext.value)
				: autoPreset.args
		}

		shouldApplyDefaultAuto = false
	}
}

await updateGcContext()

watch(() => memory.value.maximum, updateGcContext)

watch(
	[autoHighPerformanceMode, memory, javaArgs],
	async () => {
		if (!settings.value) return

		settings.value = {
			...settings.value,
			auto_set_java_high_performance_mode: autoHighPerformanceMode.value,
			memory: memory.value,
			extra_launch_args: javaArgs.value.trim().split(/\s+/).filter(Boolean),
		}

		await set(settings.value).catch(handleError)
	},
	{ deep: true },
)
</script>

<template>
	<!--
		No Java picker here on purpose: the launcher only ever starts the game on
		the runtime it installs itself, so offering a choice of installations would
		only let a player pick something that is then ignored. Memory and launch
		arguments are the parts of this page that still take effect.
	-->
	<div class="settings-page flex flex-col gap-6">
		<div v-if="supportsHighPerformanceMode" class="border-0 border-solid border-button-border pb-5">
			<div class="flex items-center justify-between gap-4">
				<div class="flex min-w-0 flex-col gap-1">
					<span class="text-sm font-semibold text-contrast">
						{{ formatMessage(messages.autoHighPerformanceMode) }}
					</span>
					<span class="text-xs text-secondary">
						{{ formatMessage(messages.autoHighPerformanceModeDescription) }}
					</span>
				</div>
				<Toggle id="auto-java-high-performance-mode" v-model="autoHighPerformanceMode" />
			</div>
		</div>

		<div class="flex flex-col gap-6 border-0 border-t border-solid border-button-border pt-5">
			<div class="flex flex-col gap-2.5">
				<h2
					id="settings-target-java-memory"
					tabindex="-1"
					class="m-0 text-lg font-semibold text-contrast"
				>
					{{ formatMessage(messages.memory) }}
				</h2>

				<Checkbox v-model="memory.automatic" :label="formatMessage(messages.automaticMemory)" />
				<div v-if="supportsMemoryOptimization" class="flex flex-col gap-1">
					<Checkbox
						v-model="memory.optimize_before_launch"
						:label="formatMessage(messages.optimizeMemoryBeforeLaunch)"
					/>
					<p class="m-0 text-xs leading-tight text-secondary">
						{{ formatMessage(messages.optimizeMemoryBeforeLaunchDescription) }}
					</p>
				</div>

				<Slider
					id="max-memory"
					v-model="memory.maximum"
					:disabled="memory.automatic"
					:min="512"
					:max="maxMemory"
					:step="64"
					:snap-points="snapPoints"
					:snap-range="512"
					unit="MB"
				/>

				<p class="m-0 mt-1 leading-tight">
					{{
						formatMessage(
							memory.automatic ? messages.automaticMemoryDescription : messages.memoryDescription,
						)
					}}
				</p>

				<MemoryAllocationDisplay :memory="memory" show-optimize-button />
			</div>

			<div class="flex flex-col gap-2.5">
				<h2
					id="settings-target-java-arguments"
					tabindex="-1"
					class="m-0 text-lg font-semibold text-contrast"
				>
					{{ formatMessage(messages.javaArguments) }}
				</h2>

				<JavaArgumentsInput
					id="java-args"
					v-model="javaArgs"
					:gc-context="gcContext"
					:placeholder="formatMessage(messages.javaArgumentsPlaceholder)"
				/>
			</div>
		</div>
	</div>
</template>

<style scoped>
.settings-page > div {
	padding: var(--gap-lg);
	border: 1px solid
		var(--settings-card-border, color-mix(in srgb, var(--surface-4) 72%, transparent));
	border-radius: var(--radius-md);
	background: var(--surface-2);
}

.settings-page > div:has(.border-warning) {
	padding: var(--gap-md);
	background: var(--color-orange-bg);
}

@media (max-width: 700px) {
	.settings-page :deep(.flex.items-center.justify-between) {
		align-items: flex-start;
		flex-direction: column;
	}
}
</style>
