<script setup lang="ts">
import { BellRingIcon, ExternalIcon } from '@modrinth/assets'
import { ButtonStyled, commonMessages, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { renderString } from '@modrinth/utils'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onMounted, ref } from 'vue'

import type { HomeWidgetSize } from '@/components/home/home-dashboard'
import { useLatestAnnouncement } from '@/composables/useLatestAnnouncement'
import { safeAnnouncementUrl } from '@/helpers/remote-announcements'

const props = defineProps<{ dashboardSize?: HomeWidgetSize | null }>()

const { formatMessage, locale } = useVIntl()
const { announcement, loading, refresh } = useLatestAnnouncement()
const modal = ref<InstanceType<typeof NewModal>>()

const messages = defineMessages({
	title: { id: 'app.home.announcement.title', defaultMessage: 'Latest announcement' },
	empty: { id: 'app.home.announcement.empty', defaultMessage: 'No announcements right now.' },
	loading: { id: 'app.home.announcement.loading', defaultMessage: 'Loading announcements...' },
	view: { id: 'app.home.announcement.view', defaultMessage: 'Read' },
	published: { id: 'app.home.announcement.published', defaultMessage: 'Published {date}' },
	important: { id: 'app.home.announcement.priority-high', defaultMessage: 'Important' },
	urgent: { id: 'app.home.announcement.priority-critical', defaultMessage: 'Urgent' },
})

const compact = computed(() => props.dashboardSize === '1x1')
const html = computed(() => renderString(announcement.value?.content ?? ''))
const actionUrl = computed(() => safeAnnouncementUrl(announcement.value?.action_url))
const summary = computed(() => announcement.value?.summary?.trim() ?? '')
const priority = computed(() => {
	if (announcement.value?.priority === 'critical') return formatMessage(messages.urgent)
	if (announcement.value?.priority === 'high') return formatMessage(messages.important)
	return ''
})
const publishedAt = computed(() => {
	const value = announcement.value?.published_at
	if (!value || !Number.isFinite(Date.parse(value))) return ''
	return formatMessage(messages.published, {
		date: new Intl.DateTimeFormat(locale.value, { dateStyle: 'medium' }).format(new Date(value)),
	})
})

async function openLink(value: string | null) {
	if (!value) return
	try {
		await openUrl(value)
	} catch {
		// The opener rejects unknown schemes; the card simply stays as it is.
	}
}

function contentClick(event: MouseEvent) {
	const link = event.target instanceof Element ? event.target.closest('a') : null
	if (!link) return
	event.preventDefault()
	event.stopPropagation()
	void openLink(safeAnnouncementUrl(link.getAttribute('href')))
}

function show() {
	if (!announcement.value) return
	modal.value?.show()
}

onMounted(() => {
	void refresh()
})
</script>

<template>
	<section class="flex h-full min-h-0 min-w-0 flex-col gap-2 overflow-hidden p-2">
		<header class="flex h-8 min-w-0 flex-none items-center gap-2">
			<BellRingIcon class="size-5 shrink-0 text-brand" aria-hidden="true" />
			<h2 class="m-0 min-w-0 truncate text-sm font-bold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
			<span
				v-if="priority"
				class="ml-auto shrink-0 rounded-full bg-brand-highlight px-2 py-0.5 text-xs font-semibold text-brand"
			>
				{{ priority }}
			</span>
		</header>

		<p v-if="!announcement" class="m-0 text-sm text-secondary">
			{{ formatMessage(loading ? messages.loading : messages.empty) }}
		</p>

		<template v-else>
			<button
				type="button"
				class="min-w-0 cursor-pointer border-0 bg-transparent p-0 text-left text-sm font-semibold text-contrast hover:underline"
				@click="show"
			>
				<span class="line-clamp-2">{{ announcement.title }}</span>
			</button>

			<p v-if="!compact && summary" class="m-0 line-clamp-3 text-xs leading-5 text-secondary">
				{{ summary }}
			</p>

			<footer class="mt-auto flex min-w-0 flex-none items-center gap-2">
				<span class="min-w-0 truncate text-xs text-secondary">{{ publishedAt }}</span>
				<ButtonStyled size="small" class="ml-auto shrink-0">
					<button @click="show">{{ formatMessage(messages.view) }}</button>
				</ButtonStyled>
			</footer>
		</template>

		<NewModal ref="modal" :header="announcement?.title" max-width="640px" scrollable>
			<div
				class="markdown-body break-words"
				@click="contentClick"
				@auxclick="contentClick"
				v-html="html"
			/>
			<template #actions>
				<div class="flex flex-wrap justify-end gap-2">
					<ButtonStyled v-if="actionUrl" color="brand">
						<button @click="openLink(actionUrl)">
							<ExternalIcon />
							{{ announcement?.action_label || formatMessage(messages.view) }}
						</button>
					</ButtonStyled>
					<ButtonStyled>
						<button @click="modal?.hide()">
							{{ formatMessage(commonMessages.closeButton) }}
						</button>
					</ButtonStyled>
				</div>
			</template>
		</NewModal>
	</section>
</template>
