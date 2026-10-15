<script setup lang="ts">
import {
	Checkbox,
	defineMessages,
	injectNotificationManager,
	Slider,
	StyledInput,
	useVIntl,
} from '@modrinth/ui'
import { platform } from '@tauri-apps/plugin-os'
import { computed, readonly, ref, watch } from 'vue'

import JavaArgumentsInput from '@/components/ui/JavaArgumentsInput.vue'
import MemoryAllocationDisplay from '@/components/ui/MemoryAllocationDisplay.vue'
import useMemorySlider from '@/composables/useMemorySlider'
import { collectGcContext, extractJavaMajorVersion } from '@/helpers/gc/context'
import type { GcContext } from '@/helpers/gc/types'
import { edit, get_content_snapshot, get_optimal_jre_key } from '@/helpers/instance'
import { get } from '@/helpers/settings'
import { injectInstanceSettings } from '@/providers/instance-settings'

import type { AppSettings } from '../../../helpers/types'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	javaInstallation: {
		id: 'instance.settings.tabs.java.java-installation',
		defaultMessage: 'Java installation',
	},
	customJavaInstallation: {
		id: 'instance.settings.tabs.java.custom-java-installation',
		defaultMessage: 'Use a custom Java installation for this instance',
	},
	javaPathPlaceholder: {
		id: 'instance.settings.tabs.java.java-path-placeholder',
		defaultMessage: '/path/to/java',
	},
	javaMemory: {
		id: 'instance.settings.tabs.java.java-memory',
		defaultMessage: 'Memory allocated',
	},
	customMemoryAllocation: {
		id: 'instance.settings.tabs.java.custom-memory-allocation',
		defaultMessage: 'Custom memory allocation',
	},
	automaticMemory: {
		id: 'instance.settings.tabs.java.automatic-memory',
		defaultMessage: 'Automatically allocate memory at launch',
	},
	optimizeMemoryBeforeLaunch: {
		id: 'instance.settings.tabs.java.optimize-memory-before-launch',
		defaultMessage: 'Optimize memory before launching the game',
	},
	optimizeMemoryBeforeLaunchDescription: {
		id: 'instance.settings.tabs.java.optimize-memory-before-launch-description',
		defaultMessage: 'Waits for Windows memory optimization to finish before starting the game.',
	},
	javaArguments: {
		id: 'instance.settings.tabs.java.java-arguments',
		defaultMessage: 'Java arguments',
	},
	customJavaArguments: {
		id: 'instance.settings.tabs.java.custom-java-arguments',
		defaultMessage: 'Custom Java arguments',
	},
	enterJavaArguments: {
		id: 'instance.settings.tabs.java.enter-java-arguments',
		defaultMessage: 'Enter Java arguments...',
	},
	javaEnvironmentVariables: {
		id: 'instance.settings.tabs.java.environment-variables',
		defaultMessage: 'Environment variables',
	},
	customEnvironmentVariables: {
		id: 'instance.settings.tabs.java.custom-environment-variables',
		defaultMessage: 'Custom environment variables',
	},
	enterEnvironmentVariables: {
		id: 'instance.settings.tabs.java.enter-environment-variables',
		defaultMessage: 'Enter environmental variables...',
	},
})

const { instance } = injectInstanceSettings()
const supportsMemoryOptimization = (await platform()) === 'windows'

const globalSettings = (await get().catch(handleError)) as unknown as AppSettings

/**
 * Java the launcher will actually run this instance with.
 *
 * The launcher only ever starts the game on the runtime it installs itself, so
 * this instance has no say in which Java is used and the page offers none. The
 * value is still read to describe the runtime when explaining memory or garbage
 * collection choices.
 */
const runJava = readonly(await get_optimal_jre_key(instance.value.id).catch(handleError))

const overrideJavaArgs = ref((instance.value.extra_launch_args?.length ?? 0) > 0)
const javaArgs = ref(
	(instance.value.extra_launch_args ?? globalSettings?.extra_launch_args ?? []).join(' '),
)

const overrideEnvVars = ref((instance.value.custom_env_vars?.length ?? 0) > 0)
const envVars = ref(
	(instance.value.custom_env_vars ?? globalSettings?.custom_env_vars ?? [])
		.map((x: string[]) => x.join('='))
		.join(' '),
)

const defaultMemory = { maximum: 2048, automatic: true, optimize_before_launch: false }
const overrideMemorySettings = ref(!!instance.value.memory)
const memory = ref({
	...defaultMemory,
	...(instance.value.memory ?? globalSettings?.memory),
})
const effectiveMemory = computed(() =>
	overrideMemorySettings.value ? memory.value : { ...defaultMemory, ...globalSettings?.memory },
)
const memData = useMemorySlider()
const maxMemory = memData.maxMemory
const snapPoints = memData.snapPoints

const gcContext = ref<GcContext | null>(null)

async function updateGcContext() {
	const javaMajorVersion = extractJavaMajorVersion(runJava?.parsed_version)
	let modCount = 0
	try {
		const snapshot = await get_content_snapshot(instance.value.id)
		modCount = snapshot.items.filter(
			(item) => item.projectType === 'mod' && item.materializationState === 'present',
		).length
	} catch {
		modCount = 0
	}
	gcContext.value = await collectGcContext(
		memory.value.maximum,
		instance.value.loader,
		javaMajorVersion,
		modCount,
	)
}

await updateGcContext()

watch([memory, () => instance.value.loader], updateGcContext)

const editInstanceObject = computed(() => ({
	extra_launch_args: overrideJavaArgs.value
		? javaArgs.value.trim().split(/\s+/).filter(Boolean)
		: null,
	custom_env_vars: overrideEnvVars.value
		? envVars.value
				.trim()
				.split(/\s+/)
				.filter(Boolean)
				.map((x: string) => x.split('=').filter(Boolean))
		: null,
	memory: overrideMemorySettings.value ? memory.value : null,
}))

watch(
	[overrideJavaArgs, javaArgs, overrideEnvVars, envVars, overrideMemorySettings, memory],
	async () => {
		await edit(instance.value.id, editInstanceObject.value).catch(handleError)
	},
	{ deep: true },
)
</script>

<template>
	<div>
		<h2 class="mb-1 block text-base font-extrabold text-contrast">
			{{ formatMessage(messages.javaMemory) }}
		</h2>
		<Checkbox
			v-model="overrideMemorySettings"
			:label="formatMessage(messages.customMemoryAllocation)"
			class="mb-2"
		/>
		<Checkbox
			v-if="overrideMemorySettings"
			v-model="memory.automatic"
			:label="formatMessage(messages.automaticMemory)"
			class="mb-2"
		/>
		<div
			v-if="supportsMemoryOptimization && overrideMemorySettings"
			class="mb-2 flex flex-col gap-1"
		>
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
			:disabled="!overrideMemorySettings || memory.automatic"
			:min="512"
			:max="maxMemory"
			:step="64"
			:snap-points="snapPoints"
			:snap-range="512"
			unit="MB"
		/>
		<MemoryAllocationDisplay :instance-id="instance.id" :memory="effectiveMemory" />

		<h2 class="mb-1 mt-4 block text-base font-extrabold text-contrast">
			{{ formatMessage(messages.javaArguments) }}
		</h2>
		<Checkbox
			v-model="overrideJavaArgs"
			:label="formatMessage(messages.customJavaArguments)"
			class="my-1"
		/>
		<JavaArgumentsInput
			id="java-args"
			v-model="javaArgs"
			:disabled="!overrideJavaArgs"
			:gc-context="gcContext"
			:show-auto-details="true"
			:placeholder="formatMessage(messages.enterJavaArguments)"
		/>

		<h2 class="mb-1 mt-4 block text-base font-extrabold text-contrast">
			{{ formatMessage(messages.javaEnvironmentVariables) }}
		</h2>
		<Checkbox
			v-model="overrideEnvVars"
			:label="formatMessage(messages.customEnvironmentVariables)"
			class="mb-2"
		/>
		<StyledInput
			id="env-vars"
			v-model="envVars"
			autocomplete="off"
			:disabled="!overrideEnvVars"
			:placeholder="formatMessage(messages.enterEnvironmentVariables)"
			wrapper-class="w-full"
		/>
	</div>
</template>
