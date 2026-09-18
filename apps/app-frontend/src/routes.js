import { createRouter, createWebHistory } from 'vue-router'

/**
 * Configures application routing. Add page to pages/index and then add to route table here.
 *
 * This launcher only serves the login, update, and launch surfaces: content
 * browsing, user-created instances, and the Lab tools are unrouted on purpose.
 * Their pages stay on disk so they can be re-registered later.
 */
export default new createRouter({
	history: createWebHistory(),
	routes: [
		{
			path: '/',
			name: 'Home',
			component: () => import('@/pages/Index.vue'),
			meta: {
				breadcrumb: [{ name: 'Home' }],
				discordActivity: 'Idling...',
			},
		},
		{
			path: '/worlds',
			name: 'Worlds',
			component: () => import('@/pages/Worlds.vue'),
			meta: {
				breadcrumb: [{ name: 'Worlds' }],
			},
		},
		{
			path: '/map',
			name: 'Map',
			component: () => import('@/pages/Map.vue'),
			meta: {
				breadcrumb: [{ name: 'Map' }],
			},
		},
		{
			path: '/downloads',
			name: 'Downloads',
			component: () => import('@/pages/Downloads.vue'),
			meta: {
				breadcrumb: [{ name: 'Downloads' }],
				discordActivity: 'Idling...',
			},
		},
		{
			path: '/settings',
			name: 'Settings',
			component: () => import('@/pages/Settings.vue'),
			meta: {
				breadcrumb: [{ name: 'Settings' }],
				discordActivity: 'Idling...',
			},
		},
		{
			path: '/help/drop',
			name: 'DropHelp',
			component: () => import('@/pages/help/DropHelp.vue'),
			meta: {
				breadcrumb: [{ name: 'Drop help' }],
			},
		},
		{
			path: '/screenshots',
			name: 'Screenshots',
			component: () => import('@/pages/Screenshots.vue'),
			meta: { breadcrumb: [{ name: 'Screenshots' }] },
		},
		{
			path: '/library',
			name: 'Library',
			component: () => import('@/pages/library/Index.vue'),
			meta: {
				breadcrumb: [{ name: 'Library' }],
				discordActivity: 'Browsing instances...',
				pageTransitionGroup: 'library',
			},
			children: [
				{
					path: '',
					name: 'Overview',
					component: () => import('@/pages/library/Overview.vue'),
				},
			],
		},
		{
			path: '/instance/:id',
			name: 'Instance',
			component: () => import('@/pages/instance/Index.vue'),
			props: true,
			meta: {
				discordActivity: 'Browsing instances...',
				pageTransitionGroup: 'instance',
			},
			children: [
				{
					path: '',
					name: 'InstanceOverview',
					component: () => import('@/pages/instance/Overview.vue'),
					meta: {
						useRootContext: true,
					},
				},
				{
					path: 'logs',
					name: 'Logs',
					component: () => import('@/pages/instance/Logs.vue'),
					meta: {
						renderMode: 'fixed',
						useRootContext: true,
						breadcrumb: [{ name: 'Logs' }],
					},
				},
			],
		},
	],
	linkActiveClass: 'router-link-active',
	linkExactActiveClass: 'router-link-exact-active',
	scrollBehavior(to, from) {
		if (to.path === from.path) return
		// Sometimes Vue's scroll behavior is not working as expected, so we need to manually scroll to top (especially on Linux)
		document.querySelector('.app-viewport')?.scrollTo(0, 0)
		return {
			el: '.app-viewport',
			top: 0,
		}
	},
})
