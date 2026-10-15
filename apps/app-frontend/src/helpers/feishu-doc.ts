import { invoke } from '@tauri-apps/api/core'

/**
 * Authentication for Feishu's document component.
 *
 * The signature covers a server-side ticket and the application secret, so it is
 * produced in the launcher backend and only the short-lived result reaches the
 * page. `url` is the string that was signed and has to be handed to the
 * component unchanged.
 */
export interface FeishuComponentAuth {
	signature: string
	timestamp: number
	nonceStr: string
	appId: string
	url: string
	retryAfterSecs: number
}

/** The document the component has to render, and whether it can. */
export interface FeishuDocumentRef {
	url: string
	supported: boolean
	title: string | null
}

/** One page in the documentation tree. */
export interface FeishuWikiNode {
	title: string
	/** Empty for a node the launcher cannot render, such as a sheet. */
	url: string
	depth: number
}

export interface FeishuWikiTree {
	nodes: FeishuWikiNode[]
	truncated: boolean
}

/** Asks the backend for a fresh signature. */
export async function feishuComponentAuth(): Promise<FeishuComponentAuth> {
	return await invoke<FeishuComponentAuth>('plugin:feishu|feishu_component_auth')
}

/**
 * Resolves a documentation address to the document behind it.
 *
 * A document address is returned as it is; a wiki address is resolved to the
 * document it points at, because Feishu's component does not render wiki nodes.
 */
export async function feishuResolveDocument(docsUrl: string): Promise<FeishuDocumentRef> {
	return await invoke<FeishuDocumentRef>('plugin:feishu|feishu_resolve_document', { docsUrl })
}

/**
 * Lists the pages below the documentation root.
 *
 * The component cannot move between pages, so the page draws its own navigation
 * from this.
 */
export async function feishuDocsTree(docsUrl?: string): Promise<FeishuWikiTree> {
	return await invoke<FeishuWikiTree>('plugin:feishu|feishu_docs_tree', {
		docsUrl: docsUrl ?? null,
	})
}
