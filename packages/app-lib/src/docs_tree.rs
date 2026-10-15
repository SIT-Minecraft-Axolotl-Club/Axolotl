//! The knowledge-space tree behind a Feishu wiki node.
//!
//! Feishu's document component renders exactly one document and offers no way to
//! move between pages: a read-only component running as the application cannot
//! follow a link, and an in-document sub-page list stays empty because the
//! component never learns where its node sits in the tree. The launcher
//! therefore draws the navigation itself, and this walks the nodes below a known
//! node to say what that navigation should contain.
//!
//! The walk starts from the page the Documentation entry already opens, so
//! nothing here needs the space-level read permission — the platform refuses
//! that one even when every document permission has been granted.

use crate::feishu::{FeishuApp, feishu_body, feishu_token};
use crate::sitmc;
use serde::Serialize;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Open platform endpoints used to walk the tree.
const WIKI_NODE_URL: &str = "https://open.feishu.cn/open-apis/wiki/v2/spaces/get_node";
const WIKI_CHILDREN_URL: &str =
    "https://open.feishu.cn/open-apis/wiki/v2/spaces/{space}/nodes";

/// How long a walked tree is reused.
///
/// The tree changes rarely and the platform throttles application-identity
/// requests at 100 per minute, so switching pages does not re-walk it.
const TREE_TTL: Duration = Duration::from_secs(600);

/// Deepest level walked. A documentation space is a couple of levels deep, and a
/// cap keeps a pathological tree from spending the request budget.
const MAX_DEPTH: usize = 6;

/// Most nodes collected for one tree.
const MAX_NODES: usize = 200;

static CACHE: parking_lot::Mutex<Option<CachedTree>> = parking_lot::Mutex::new(None);

struct CachedTree {
    url: String,
    fetched_at: Instant,
    nodes: Vec<WikiNode>,
    truncated: bool,
}

/// One page of the documentation.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WikiNode {
    pub title: String,
    /// Page the document component renders for this node. Empty for a node that
    /// is not a document this launcher can show.
    pub url: String,
    /// Nesting level, the root's direct children being one.
    pub depth: usize,
    /// Whether more pages sit below this one, so the page can offer to expand it.
    pub has_child: bool,
}

/// The pages below `wiki_url`, in the order the platform returns them.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WikiTree {
    pub nodes: Vec<WikiNode>,
    pub truncated: bool,
}

/// Walks the pages below a wiki node, reusing a recent walk.
///
/// The node itself is not part of the result: it is the page the Documentation
/// entry already opens. A plain document address yields an empty tree rather
/// than an error, so the page simply shows no navigation.
#[tracing::instrument(skip(app, wiki_url))]
pub async fn docs_tree(
    app: &FeishuApp,
    wiki_url: &str,
) -> crate::Result<WikiTree> {
    if let Some(cached) = CACHE.lock().as_ref()
        && cached.url == wiki_url
        && cached.fetched_at.elapsed() < TREE_TTL
    {
        return Ok(WikiTree {
            nodes: cached.nodes.clone(),
            truncated: cached.truncated,
        });
    }

    let app_access_token = crate::feishu::app_access_token(app).await?;
    let root = match node_detail(&app_access_token, wiki_url).await? {
        Some(root) => root,
        None => {
            let empty = WikiTree {
                nodes: Vec::new(),
                truncated: false,
            };
            cache(wiki_url, &empty);
            return Ok(empty);
        }
    };

    let mut walk = Walk {
        nodes: Vec::new(),
        seen: HashSet::new(),
        truncated: false,
    };
    walk.collect(
        &app_access_token,
        &root.space_id,
        &root.node_token,
        &address_host(wiki_url),
        0,
    )
    .await?;

    let tree = WikiTree {
        nodes: walk.nodes,
        truncated: walk.truncated,
    };
    cache(wiki_url, &tree);
    Ok(tree)
}

fn cache(url: &str, tree: &WikiTree) {
    *CACHE.lock() = Some(CachedTree {
        url: url.to_string(),
        fetched_at: Instant::now(),
        nodes: tree.nodes.clone(),
        truncated: tree.truncated,
    });
}

struct Walk {
    nodes: Vec<WikiNode>,
    seen: HashSet<String>,
    truncated: bool,
}

impl Walk {
    /// Appends the children of `parent_node_token`, then their children.
    #[allow(clippy::too_many_arguments)]
    async fn collect(
        &mut self,
        app_access_token: &str,
        space_id: &str,
        parent_node_token: &str,
        host: &str,
        parent_depth: usize,
    ) -> crate::Result<()> {
        if parent_depth >= MAX_DEPTH {
            self.truncated = true;
            return Ok(());
        }

        for child in child_nodes(space_id, parent_node_token, app_access_token).await? {
            if self.nodes.len() >= MAX_NODES {
                self.truncated = true;
                return Ok(());
            }
            if !self.seen.insert(child.node_token.clone()) {
                continue;
            }

            let depth = parent_depth + 1;
            let url = match child.obj_type.as_str() {
                "docx" => node_detail_for(app_access_token, &child.node_token, host)
                    .await?
                    .map(|detail| detail.url)
                    .unwrap_or_default(),
                _ => String::new(),
            };
            self.nodes.push(WikiNode {
                title: child.title,
                url,
                depth,
                has_child: child.has_child,
            });

            if child.has_child {
                Box::pin(self.collect(
                    app_access_token,
                    space_id,
                    &child.node_token,
                    host,
                    depth,
                ))
                .await?;
            }
        }

        Ok(())
    }
}

/// A node listing entry. The listing carries no document address.
struct ChildNode {
    node_token: String,
    title: String,
    has_child: bool,
    obj_type: String,
}

/// A node's own document address.
struct NodeDetail {
    url: String,
    space_id: String,
    node_token: String,
}

/// The host a wiki address belongs to.
fn address_host(address: &str) -> String {
    url::Url::parse(address)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "feishu.cn".to_string())
}

/// Looks a node up from its token.
///
/// The host travels with the token because the document address handed to the
/// component has to sit on the tenant that owns the node.
async fn node_detail_for(
    app_access_token: &str,
    token: &str,
    host: &str,
) -> crate::Result<Option<NodeDetail>> {
    let response = crate::util::fetch::INSECURE_REQWEST_CLIENT
        .get(WIKI_NODE_URL)
        .query(&[("token", token), ("obj_type", "wiki")])
        .bearer_auth(app_access_token)
        .send()
        .await?;
    let body = feishu_body(response, WIKI_NODE_URL).await?;

    let Some(node) = body.get("data").and_then(|data| data.get("node")) else {
        return Ok(None);
    };
    let obj_token = string_field(node, "obj_token");
    let obj_type = string_field(node, "obj_type");

    Ok(Some(NodeDetail {
        url: if obj_type == "docx" && !obj_token.is_empty() {
            format!("https://{host}/docx/{obj_token}")
        } else {
            String::new()
        },
        space_id: string_field(node, "space_id"),
        node_token: string_field(node, "node_token"),
    }))
}

/// Looks a node up from a wiki address. A plain document address yields `None`.
async fn node_detail(
    app_access_token: &str,
    address: &str,
) -> crate::Result<Option<NodeDetail>> {
    let Some((kind, token)) = feishu_token(address) else {
        return Err(crate::ErrorKind::InputError(format!(
            "The documentation address {address} is not a Feishu page"
        ))
        .as_error());
    };
    if kind != "wiki" {
        return Ok(None);
    }

    node_detail_for(app_access_token, &token, &address_host(address)).await
}

async fn child_nodes(
    space_id: &str,
    parent_node_token: &str,
    app_access_token: &str,
) -> crate::Result<Vec<ChildNode>> {
    let url = WIKI_CHILDREN_URL.replace("{space}", space_id);
    let mut nodes = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut request = crate::util::fetch::INSECURE_REQWEST_CLIENT
            .get(&url)
            .query(&[
                ("parent_node_token", parent_node_token),
                ("page_size", "50"),
            ])
            .bearer_auth(app_access_token);
        if let Some(token) = &page_token {
            request = request.query(&[("page_token", token.as_str())]);
        }

        let response = request.send().await?;
        let body = feishu_body(response, &url).await?;
        let data = body.get("data");

        for item in data
            .and_then(|data| data.get("items"))
            .and_then(|items| items.as_array())
            .into_iter()
            .flatten()
        {
            nodes.push(ChildNode {
                node_token: string_field(item, "node_token"),
                title: string_field(item, "title"),
                has_child: item
                    .get("has_child")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                obj_type: string_field(item, "obj_type"),
            });
        }

        let has_more = data
            .and_then(|data| data.get("has_more"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        page_token = data
            .and_then(|data| data.get("page_token"))
            .and_then(|value| value.as_str())
            .map(str::to_string);
        if !has_more || page_token.is_none() {
            return Ok(nodes);
        }
    }
}

fn string_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

/// The page the Documentation entry opens when nothing else is configured.
///
/// Held here so the tree can be walked for a build that only sets the page's
/// address in the frontend.
pub const DEFAULT_DOCS_URL: &str = sitmc::DOCS_URL;
