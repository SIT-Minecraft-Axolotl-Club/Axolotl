# Axolotl Launcher 接口规范（SIT-Minecraft 专用版）

| 项目 | 内容 |
| --- | --- |
| 产品名 | Axolotl Launcher（上海应用技术大学 SIT-Minecraft 社团专用发行版） |
| 文档性质 | 接口规范（服务端契约），由社团后端开发者实现 |
| 文档范围 | 账户认证契约、实例清单契约、客户端行为保证、版本兼容、附录 |
| 客户端实现 | 基于 Axolotl Launcher fork（Rust / Tauri + Vue 3），复用现有 Modrinth 整合包安装管线 |
| 状态 | 草案。标注「现有」的部分可在当前代码中找到对应实现；标注「规划」的部分尚未实现 |

> 本文档只定义契约，不包含后端实现步骤，也不包含后端代码。文档中出现的所有 JSON、HTTP 头、URL 均为英文/ASCII，说明文字为简体中文。

## 目录

1. [概述与术语](#1-概述与术语)
2. [账户认证契约](#2-账户认证契约)
3. [实例清单契约](#3-实例清单契约)
4. [客户端行为规范](#4-客户端行为规范)
5. [版本与兼容](#5-版本与兼容)
6. [附录](#6-附录)

---

## 1. 概述与术语

### 1.1 目标形态

SIT-Minecraft 专用版启动器的运行流程被收敛为四个阶段：

```
登录 → 拉取实例清单 → 强制更新 / 安装 → 启动
```

- 启动器**不再提供**账号注册、皮肤编辑、实例自由创建与编辑等开放能力。
- 玩家打开启动器后必须先完成 SIT-Minecraft 账户登录，随后由社团后端下发的实例清单决定「能玩什么、装哪个版本、是否必须先更新」。
- 社团需要提供的能力被拆分为两个**互相独立**的服务端契约，任何一个不可用都不应导致另一个被误判为故障。

### 1.2 两个独立的服务端契约

| 编号 | 契约 | 提供方 | 是否需要社团再开发 | 说明 |
| --- | --- | --- | --- | --- |
| A | 账户认证 | 社团皮肤站 `https://skin.sitmc.club`（Blessing Skin Server + 插件 "Yggdrasil Connect for Blessing Skin by LittleSkin"） | **否**。启动器复用站点已有的 Yggdrasil / Yggdrasil Connect（OIDC 设备授权授予）接口，不实现账号体系 | 站点侧已实测提供规范所需的端点与作用域，仍需站点管理员确认的事项见 [2.4](#24-站点侧必须确认开启的事项检查清单) |
| B | 实例清单 | 社团自建后端（下称「清单服务」） | **是**。本规范第 [3](#3-实例清单契约) 章即为其接口定义 | 启动器**依赖此契约**；清单不可用时启动器保留上一次成功清单 |

启动器自身的更新（可执行文件级别）**不属于**上述两个契约，它沿用 Axolotl 既有的自更新服务 `https://update.axlmc.org/latest`，详见 [6.1](#61-与启动器现有机制的对应关系)。

### 1.3 术语表

| 术语 | 英文 / 字段对应 | 定义 |
| --- | --- | --- |
| 受管实例 | managed instance | 由实例清单下发、由启动器代为安装与维护的实例。玩家不能编辑、不能删除、不能改版本或加载器，只能启动。非受管实例不属于本规范范围 |
| 实例清单 | manifest | 清单服务返回的 JSON 文档，声明受管实例集合与启动器自身更新策略。端点见 [3.1](#31-端点) |
| revision | `instances[].revision` | 单个受管实例的整数版本号，**单调递增**。客户端判定「是否需要重新安装 / 更新」的**唯一依据**，不做内容哈希、不比较时间戳 |
| 整合包来源 | `pack.kind` / `pack.url` / `pack.sha1` | 受管实例内容的来源。当前客户端已实现 `mrpack`（Modrinth 整合包，按 `modrinth.index.json` 安装）。`zip`（整包覆盖）为**规划中/未实现** |
| 强制更新 | `launcher.force_update`、`launcher.min_version`、`instances[].required` | 两类强制：**启动器自身强制更新**（低于 `min_version` 时阻断一切操作，不可跳过、不可推迟）与**实例强制更新**（`required` 为真时玩家不可延迟） |
| 安装阶段 | `install_stage`（客户端本地状态） | 客户端实例的安装状态枚举：`not_installed` / `minecraft_installing` / `pack_installing` / `pack_installed` / `installed`。只有 `installed` 的实例才允许启动 |
| 客户端 | launcher | 运行在玩家机器上的 Axolotl Launcher 发行版 |
| 清单服务 | manifest service | 社团自建后端中负责下发实例清单的部分 |

---

## 2. 账户认证契约

> 本章描述**现状**：站点已经提供这些接口，社团**不需要**再开发认证后端。本章的作用是让社团确认站点侧配置，并让启动器实现者与后端维护者对同一套流程有共同理解。

### 2.1 基础地址与服务发现

| 用途 | 地址 | 说明 |
| --- | --- | --- |
| 站点首页 | `https://skin.sitmc.club/` | Blessing Skin Server 站点 |
| 注册页 | `https://skin.sitmc.club/auth/register` | 启动器只负责引导打开，不在内部实现注册表单 |
| 登录页 | `https://skin.sitmc.club/auth/login` | 密码登录兜底路径下可引导玩家在浏览器完成 |
| Yggdrasil API 根 | `https://skin.sitmc.club/api/yggdrasil` | 传统 Yggdrasil 接口根，下文记作 `{yggdrasil_root}`；**服务发现从这里开始** |
| OpenID 提供者元数据（OP 元数据） | `https://skin.sitmc.club/api/janus/.well-known/openid-configuration` | 由 `{yggdrasil_root}` 的 `meta.feature.openid_configuration_url` 指向；启动器由它解析端点、支持的 scope 与公用应用标识符 |

**服务发现（规范要求）**：客户端首先向 `{yggdrasil_root}` 发 `GET` 请求（Yggdrasil API 元数据），读取 `meta.feature.openid_configuration_url`。该字段即本站 OpenID 提供者的元数据 URL。**若 Yggdrasil API 元数据中不包含该字段，则应认为该站点不支持 Yggdrasil Connect**，客户端不得继续尝试 OIDC 流程，必须退回 [2.3](#23-兜底传统-yggdrasil-账号密码登录) 的账号密码登录。

实测（2026-09，只读探测）：

```http
GET https://skin.sitmc.club/api/yggdrasil HTTP/1.1
Accept: application/json
```

响应 `200`，节选：

```json
{
  "meta": {
    "serverName": "SIT-Minecraft",
    "feature.openid_configuration_url": "https://skin.sitmc.club/api/janus/.well-known/openid-configuration"
  }
}
```

- `meta.feature.openid_configuration_url` **实测存在**，故本站支持 Yggdrasil Connect。
- `meta.serverName` **实测为 `SIT-Minecraft`**，启动器用它作为账户来源的展示名（站点未返回时回退到常量 `SIT-Minecraft`）。

OP 元数据实测内容（节选，用于对齐；本站实际返回的字段多于下表列出者）：

```json
{
  "issuer": "https://skin.sitmc.club/api/janus",
  "authorization_endpoint": "https://skin.sitmc.club/api/janus/auth",
  "device_authorization_endpoint": "https://skin.sitmc.club/api/janus/device/auth",
  "token_endpoint": "https://skin.sitmc.club/api/janus/token",
  "userinfo_endpoint": "https://skin.sitmc.club/api/janus/userinfo",
  "jwks_uri": "https://skin.sitmc.club/api/janus/jwks",
  "scopes_supported": [
    "email",
    "profile",
    "Yggdrasil.PlayerProfiles.Select",
    "Yggdrasil.PlayerProfiles.Read",
    "Yggdrasil.Server.Join",
    "offline_access",
    "openid"
  ],
  "grant_types_supported": [
    "authorization_code",
    "implicit",
    "refresh_token",
    "urn:ietf:params:oauth:grant-type:device_code"
  ],
  "id_token_signing_alg_values_supported": ["PS256", "RS256"],
  "claims_supported": [
    "email",
    "email_verified",
    "nickname",
    "picture",
    "selectedProfile",
    "availableProfiles",
    "sub",
    "sid",
    "auth_time",
    "iss"
  ],
  "shared_client_id": "11"
}
```

要点（规范要求与本站实测逐项对照）：

- **必须提供的字段**：`issuer`、`token_endpoint`、`userinfo_endpoint`、`jwks_uri`、`scopes_supported`、`subject_types_supported`、`id_token_signing_alg_values_supported`。本站实测均提供。
- **`device_authorization_endpoint` 在规范中可选**：规范只要求它与 `authorization_endpoint` 至少提供其中一个。本站两者都提供（`https://skin.sitmc.club/api/janus/device/auth` 与 `https://skin.sitmc.club/api/janus/auth`），启动器因此可以完全不接触密码。
- **`scopes_supported` 必须包含 `openid`、`Yggdrasil.PlayerProfiles.Select`、`Yggdrasil.Server.Join` 三项**，缺少其中任意一项即视为该 OpenID 提供者不支持 Yggdrasil Connect。本站三项齐备（另含 `Yggdrasil.PlayerProfiles.Read`、`offline_access`、`email`、`profile`）。
- **`id_token_signing_alg_values_supported` 必须包含 `RS256`**；本站为 `["PS256", "RS256"]`。
- **`jwks_uri`** 用于获取校验 ID 令牌签名的 JWKS。**启动器不解析、不校验 ID 令牌**（见 [2.2 步骤 4](#步骤-4角色来源用户信息端点)），该字段按规范照常提供，但当前实现不使用。
- **`shared_client_id` 为 `"11"`**：这是站点预注册的**公用应用**标识符，启动器直接以该应用的身份申请授权，不在站点上注册自己的应用。规范规定公用应用必须被视为**公共客户端**（不持有 `client_secret`），因此启动器在设备代码流与刷新中都不发送 `client_secret`。
- 规范也定义了**授权代码流**（回调 URL、PKCE 等）与授权端点。本站提供 `authorization_endpoint`，但**启动器未实现该流程**，只使用设备授权授予（见 2.2 节）。

### 2.2 首选：Yggdrasil Connect（OIDC 设备授权授予）

适用场景：玩家在启动器内完成登录，无需输入密码到启动器进程内，也无需在启动器里实现浏览器回调页。完整流程遵循 RFC 8628，共 4 步。

启动器申请的 scope 固定为（顺序不敏感，用空格分隔）：

```
openid offline_access Yggdrasil.PlayerProfiles.Select Yggdrasil.Server.Join
```

#### 作用域约束（必须遵守）

- `openid` **必须**申请。规范规定：申请了任一 `Yggdrasil.*` 权限范围却未同时申请 `openid` 时，认证服务器应拒绝授权并返回 `invalid_scope`。
- `Yggdrasil.PlayerProfiles.Select` **必须支持**。申请后，认证服务器**应在询问用户授权时要求玩家选择角色**，并把最终颁发的访问令牌**绑定**到玩家选定的角色。
- `Yggdrasil.Server.Join` **必须支持，且必须与 `Yggdrasil.PlayerProfiles.Select` 同时申请**（换句话说，访问令牌必须绑定角色）。规范规定：未申请 `Yggdrasil.Server.Join` 时，会话服务器必须拒绝持有该访问令牌的客户端进入 Minecraft 多人游戏服务器。
- `Yggdrasil.PlayerProfiles.Read`（可选支持，返回账户名下**全部**角色）与 `Yggdrasil.PlayerProfiles.Select` **不允许同时申请**。启动器**既不申请 `Read`，也不读 `availableProfiles`**。早期版本的本文档曾选定 `Read` 方案，那是错误的，已废弃。
- `offline_access` 可选支持：只有申请了它，认证服务器才应随令牌颁发 `refresh_token`。启动器申请它，以支持无感续期。

#### 步骤 1：请求设备代码

请求（`Content-Type: application/x-www-form-urlencoded`）：

```http
POST https://skin.sitmc.club/api/janus/device/auth
Content-Type: application/x-www-form-urlencoded
Accept: application/json

client_id=11&scope=openid%20offline_access%20Yggdrasil.PlayerProfiles.Select%20Yggdrasil.Server.Join
```

成功响应 `200`（实测；`device_code` 的值省略）：

```json
{
  "device_code": "...",
  "user_code": "FPBQ-ZZZB",
  "verification_uri": "https://skin.sitmc.club/api/janus/device",
  "verification_uri_complete": "https://skin.sitmc.club/api/janus/device?user_code=FPBQ-ZZZB",
  "expires_in": 600
}
```

字段含义与规范性：

| 字段 | 类型 | 必填性 | 说明 |
| --- | --- | --- | --- |
| `device_code` | string | 必须提供 | 启动器保存，用于轮询令牌端点；**不得**展示给玩家 |
| `user_code` | string | 必须提供 | 展示给玩家的人类可读代码 |
| `verification_uri` | string | 必须提供 | 授权页面地址，玩家在此输入 `user_code` |
| `verification_uri_complete` | string | 可选提供 | 带 `user_code` 的完整地址；启动器直接打开它，玩家无需手输代码。实测本站提供 |
| `expires_in` | integer | 必须提供 | 本次授权请求的有效期（秒）；过期后 `device_code` 与 `user_code` 都失效，必须重新走步骤 1。实测本站为 `600` |
| `interval` | integer | 可选提供 | 最小轮询间隔（秒）。**如未提供，规范规定轮询间隔默认为 5 秒**。**实测本站未返回该字段**，因此启动器按 5 秒轮询；服务端返回 `slow_down` 时，客户端必须把间隔增加 5 秒 |

#### 步骤 2：展示代码并引导玩家授权

- 启动器在界面上展示 `user_code`，并提示玩家在浏览器中完成授权。
- 启动器用系统浏览器打开 `verification_uri_complete`；若站点未提供该字段，则打开 `verification_uri` 让玩家手动输入代码。
- 因为申请了 `Yggdrasil.PlayerProfiles.Select`，**认证服务器应在授权页要求玩家选择角色**。玩家选定的角色即访问令牌绑定的角色，也就是启动器最终使用的角色。
- 授权页由站点负责渲染，启动器不参与，也拿不到玩家密码。

#### 步骤 3：轮询授权结果并获得访问令牌

获取到 `device_code` 后，启动器以 `interval`（本站为缺省 5 秒）为间隔轮询令牌端点：

```http
POST https://skin.sitmc.club/api/janus/token
Content-Type: application/x-www-form-urlencoded
Accept: application/json

grant_type=urn:ietf:params:oauth:grant-type:device_code&client_id=11&device_code=...
```

玩家尚未完成授权时的响应（HTTP 400，实测）：

```json
{
  "error": "authorization_pending",
  "error_description": "authorization request is still pending as the end-user hasn't yet completed the user interaction steps"
}
```

| `error` | 规范规定的状态码 | 客户端行为 |
| --- | --- | --- |
| `authorization_pending` | 400 | 保持当前轮询间隔继续轮询 |
| `slow_down` | 400 | 将轮询间隔增加 5 秒后继续轮询 |
| `expired_token` | 400 | 停止轮询，回到步骤 1 重新获取 `device_code` |
| `access_denied` | 401 | 停止轮询，提示玩家/认证服务器已拒绝授权 |
| `invalid_scope` | 400 | 停止轮询，提示作用域配置错误（见 [作用域约束](#作用域约束必须遵守)） |

授权成功后响应 `200`：

```json
{
  "token_type": "Bearer",
  "expires_in": 3600,
  "access_token": "...",
  "refresh_token": "...",
  "id_token": "eyJ..."
}
```

| 字段 | 必填性 | 说明 |
| --- | --- | --- |
| `token_type` | 必须提供 | 固定为 `Bearer` |
| `expires_in` | 必须提供 | 访问令牌有效期（秒） |
| `access_token` | 必须提供 | 访问令牌 |
| `refresh_token` | 条件必须 | 申请了 `offline_access` 时必须提供 |
| `id_token` | 条件必须 | 申请了 `openid` 时必须提供 |

> **`access_token` 本身就是 Minecraft 访问令牌，不需要任何"换取"步骤。**
> 会话服务器直接校验这个访问令牌；authlib-injector 启动游戏时把它交给游戏本体，与经典 Yggdrasil 登录拿到的令牌在使用方式上完全一致。因为申请了 `Yggdrasil.PlayerProfiles.Select` 且授权页已让玩家选好角色，**该访问令牌已绑定角色**，所以可以直接用于进服（`Yggdrasil.Server.Join` 的语义）。
>
> **已删除的旧流程**：早期版本的本文档描述了 `POST {yggdrasil_root}/authserver/oauth`「用 OIDC 令牌换取 Minecraft 令牌」。那是**非规范扩展，本站并不提供，也不需要**——该流程已从本文档中删除，不得再作为契约的一部分实现。

#### 步骤 4：角色来源：用户信息端点

访问令牌绑定的是玩家在授权页选定的角色。启动器用该访问令牌请求**用户信息端点**，读取这个角色：

```http
GET https://skin.sitmc.club/api/janus/userinfo
Authorization: Bearer ...
Accept: application/json
```

成功响应 `200` 的 JSON 是 ID 令牌声明的**超集**（但不含 `iss`、`iat`、`exp`）：

```json
{
  "sub": "user_id",
  "aud": "client_id",
  "selectedProfile": {
    "id": "f702c5d39d5c457f80c691c664757092",
    "name": "SSSSSteven"
  }
}
```

| 字段 | 必填性 | 说明 |
| --- | --- | --- |
| `selectedProfile` | 申请了 `Yggdrasil.PlayerProfiles.Select` 时必须具有 | 访问令牌**绑定的角色**，`{"id": "<32 位无符号 UUID>", "name": "<角色名>"}`（不含角色属性）。启动器据此决定用哪个角色启动游戏 |
| `availableProfiles` | 申请了 `Yggdrasil.PlayerProfiles.Read` 时才具有 | 账户名下全部角色。**启动器不申请 `Read`，因此不读取该字段** |

客户端行为：

- **用访问令牌请求用户信息端点成功，即证明访问令牌有效**；返回 `invalid_token` 即说明访问令牌已失效（规范明确以这两种结果判定访问令牌有效性）。启动器据此在每次登录/刷新后确认会话仍然可用。
- **`selectedProfile` 缺失时才回退解析 ID 令牌负载**：启动器把 `id_token` 的 JWT 负载解码后取其中的 `selectedProfile`，仅作为兜底来源。
- **启动器不校验 ID 令牌**：不拉取 `jwks_uri`、不验证签名，也不校验 `iss` / `aud` / `exp`。选择哪个角色只影响"用哪个角色启动"，访问令牌本身仍由会话服务器校验，因此这里不做 JWT 校验是有意的取舍。
- 站点可以出于隐私考虑省略部分声明；**省略不等于错误**（规范要求缺声明时直接省略，而不是置空或 `null`），启动器只在 `selectedProfile` 完全缺失且 ID 令牌兜底也失败时报错。
- 现有实现在该请求中附带 `Accept-Language: zh-CN`，以便站点返回本地化内容；该头不是契约要求。

实测（2026-09）：带**伪造令牌**请求该端点返回 `401`，响应头为 `WWW-Authenticate: Bearer error="invalid_token", error_description="The access token expired or is invalid.", realm="openid"`，响应体为 `{"error":"invalid_token",...}`；响应还带 `X-Authlib-Injector-API-Location: https://skin.sitmc.club/api/yggdrasil`。该端点会 `302` 到 `/yggc/userinfo`，客户端跟随重定向后 `Authorization` 头**仍然保留**——已实测验证。

#### 令牌的刷新

为了延长单次授权的有效期，访问令牌过期后可用刷新令牌换取新的访问令牌：

```http
POST https://skin.sitmc.club/api/janus/token
Content-Type: application/x-www-form-urlencoded
Accept: application/json

grant_type=refresh_token&client_id=11&refresh_token=...
```

| 参数 | 必填性 | 说明 |
| --- | --- | --- |
| `grant_type` | 必须提供 | 固定为 `refresh_token` |
| `client_id` | 必须提供 | 先前请求访问令牌时使用的应用标识符（本站为 `11`） |
| `client_secret` | 条件必须 | **仅当先前的访问令牌是用授权代码流取得时才必须提供**。启动器使用设备授权授予，属于**公共客户端**，**不发送 `client_secret`** |
| `refresh_token` | 必须提供 | 先前获取到的刷新令牌 |

响应与步骤 3 的成功响应同构：返回**新的** `access_token` 与**新的** `refresh_token`（申请了 `offline_access` 时必须提供）。刷新令牌**只能使用一次**，客户端必须用响应中的新值覆盖保存。

启动器在访问令牌过期后走该端点续期；续期成功后重新请求用户信息端点（步骤 4）刷新本地角色，若用户信息端点未应答则保留已保存的角色。

> 注意：这里没有"两层令牌"。**OIDC 访问令牌就是 Minecraft 访问令牌**，续期就走本节的 `{token_endpoint}`；经典 Yggdrasil 的 `{yggdrasil_root}/authserver/refresh` 只用于 [2.3](#23-兜底传统-yggdrasil-账号密码登录) 密码路径取得的令牌，两条路径的令牌与刷新令牌不可混用。

#### ID 令牌

- 规范规定：ID 令牌由认证服务器用其签名密钥签名，应用在使用前**必须**校验 `iss`（与 OpenID 提供者标识符一致）、`aud`（与应用标识符一致）、`iat`/`exp` 与签名（经 `jwks_uri` 取 JWKS，按 `kid` 选键）。允许的算法为 RS256/RS384/RS512、ES 系列与 EdDSA，且**必须**至少支持 `RS256`。
- **启动器不解析、不校验 ID 令牌**：它只把 ID 令牌作为 `selectedProfile` 的兜底来源（见步骤 4），角色与访问令牌的有效性都以用户信息端点为准。因此站点轮换签名密钥不会影响启动器登录。
- 若将来启动器改为依赖 ID 令牌，上述校验全部必须补齐 —— **当前未实现**。

#### 授权代码流

- 规范同样定义了授权代码流：客户端在 `authorization_endpoint` 上拼接 `client_id`、`redirect_uri`、`response_type=code`、`scope`、`state`；回调取得 `code` 后在令牌端点以 `grant_type=authorization_code`、`code`、`redirect_uri` 与 `client_secret` 换取令牌。该流程的客户端属于**机密客户端**（不使用 PKCE 时），其刷新请求同样必须带 `client_secret`。
- **启动器未实现授权代码流**，只使用设备授权授予：设备流不需要注册回调 URL，也不需要 `client_secret`，更适合桌面启动器。规范支持、**启动器未实现**——这是有意的实现范围。

### 2.3 兜底：传统 Yggdrasil 账号密码登录

适用场景：站点**未启用 Yggdrasil Connect**（Yggdrasil API 元数据中没有 `meta.feature.openid_configuration_url`、OP 元数据缺少规范要求的三项 scope，或未提供设备授权端点），或玩家主动选择用账号密码登录。此路径要求玩家把密码输入启动器，安全性弱于设备代码流，因此仅作为**兜底**；**密码只写入系统凭据管理器**（玩家可随时删除），不落到启动器数据库。该路径**已实测可用**。

#### 登录

```http
POST https://skin.sitmc.club/api/yggdrasil/authserver/authenticate
Content-Type: application/json
Accept: application/json

{
  "agent": { "name": "Minecraft", "version": 1 },
  "username": "steve@example.com",
  "password": "secret",
  "clientToken": "3d6f1a90c2b84e57a1d0f3e8b7c45612",
  "requestUser": true
}
```

成功响应 `200`：含 `accessToken`、`clientToken`、`availableProfiles`、`selectedProfile`（`requestUser` 为真时额外含 `user`）。这里的 `accessToken` 同样是**标准 Minecraft 访问令牌**，与 OIDC 路径拿到的访问令牌在使用方式上完全一致（见 [2.2 步骤 3](#步骤-3轮询授权结果并获得访问令牌)）。该端点**已实测可用**（`GET` 返回 `405`，说明路径存在且仅接受 POST）。

当 `availableProfiles` 含多个角色时，**由启动器内实现选择**。这与 OIDC 路径不同：OIDC 路径的角色由**站点授权页**选定（`Yggdrasil.PlayerProfiles.Select` 已把访问令牌绑定到该角色），密码路径没有授权页，只能由启动器选择，并把选中的角色作为 `selectedProfile` 传给刷新接口。

#### 刷新

```http
POST https://skin.sitmc.club/api/yggdrasil/authserver/refresh
Content-Type: application/json
Accept: application/json

{
  "accessToken": "0f1c8f2b4c1d4a2e9b7f5c3d1e2a4b6c",
  "clientToken": "3d6f1a90c2b84e57a1d0f3e8b7c45612",
  "requestUser": true
}
```

#### 校验

```http
POST https://skin.sitmc.club/api/yggdrasil/authserver/validate
Content-Type: application/json

{
  "accessToken": "0f1c8f2b4c1d4a2e9b7f5c3d1e2a4b6c",
  "clientToken": "3d6f1a90c2b84e57a1d0f3e8b7c45612"
}
```

有效返回 `204 No Content`；无效返回 `403 Forbidden`，客户端应改为刷新令牌或要求重新登录。

#### 档案

```http
GET https://skin.sitmc.club/api/yggdrasil/sessionserver/session/minecraft/profile/069a79f444e94726a5befca90e38aaf5?unsigned=false
Accept: application/json
```

用于展示角色名与皮肤信息。皮肤与披风由皮肤站管理，**启动器不提供皮肤编辑功能**。

### 2.4 站点侧必须确认/开启的事项（检查清单）

> **实测结论（2026-09，对 `https://skin.sitmc.club` 的只读探测）**
>
> 探测方法：对只允许 POST 的已知端点发 `GET`。该站点对"路径存在但方法不对"返回 **405**，对"路径不存在"返回 **404**，两者可区分。
>
> | 端点 | 结果 | 含义 |
> | --- | --- | --- |
> | `GET /api/yggdrasil` | 200 | 含 `meta.feature.openid_configuration_url` 与 `meta.serverName=SIT-Minecraft`，**服务发现可用** |
> | `GET /api/janus/.well-known/openid-configuration` | 200 | OP 元数据可用：`device_authorization_endpoint=https://skin.sitmc.club/api/janus/device/auth`、`token_endpoint=https://skin.sitmc.club/api/janus/token`、`userinfo_endpoint=https://skin.sitmc.club/api/janus/userinfo`、`shared_client_id="11"`、`scopes_supported` 含 `openid`/`offline_access`/`Yggdrasil.PlayerProfiles.Select`/`Yggdrasil.PlayerProfiles.Read`/`Yggdrasil.Server.Join` |
> | `POST /api/janus/device/auth`（`client_id=11`，上方 scope） | 200 | **设备授权授予可用**：返回 `device_code`、`user_code`（形如 `FPBQ-ZZZB`）、`verification_uri`、`verification_uri_complete`、`expires_in=600`；**未返回 `interval`**，客户端按规范缺省为 5 秒 |
> | `POST /api/janus/token`（`device_code` 尚未授权） | 400 + `authorization_pending` | 轮询语义符合规范 |
> | `GET /api/janus/userinfo`（带伪造令牌） | 401 + `WWW-Authenticate: Bearer error="invalid_token", ...` + `{"error":"invalid_token",...}` | 用户信息端点**可用**，并按规范拒绝无效访问令牌；响应带 `X-Authlib-Injector-API-Location: https://skin.sitmc.club/api/yggdrasil`；该端点会 302 到 `/yggc/userinfo`，跟随重定向后 `Authorization` 头仍保留（已验证） |
> | `GET /api/yggdrasil/authserver/authenticate` | 405 | 经典密码登录**存在**，可用（仅 POST） |
> | `GET /api/yggdrasil/authserver/validate` | 405 | 令牌校验**存在**，可用（仅 POST） |
>
> **结论：本站符合 Yggdrasil Connect 规范，规范流程所需端点齐备。启动器以 OIDC 设备码流为主、账号密码为兜底。**
>
> 早期版本的本文档曾以**两个非规范端点**（`POST {yggdrasil_root}/authserver/oauth` 与 `GET {yggdrasil_root}/sessionserver/session/minecraft/profile`）作为契约依据，并据此得出了关于本站可用性的错误结论。本站不提供、也不需要这两个端点：兑换端点并非规范的一部分，角色来自**用户信息端点**。**该结论已作废，不得再作为契约依据。**
>
> 仍待站点管理员确认的项：
>
> - 游戏服务端是否已配置 authlib-injector 外置登录（这是账户闭环的前提）。
> - 公用应用（`client_id=11`）是否对设备码流有额外限制，例如强制每次授权确认、缩短令牌有效期。规范允许认证服务器对公共客户端施加此类限制；本站是否施加**未验证**（需要完成一次真实授权才能观察）。
> - 申请 `offline_access` 后是否确实随令牌颁发 `refresh_token`（规范要求必须颁发）。**未验证**（同样需要完成一次真实授权）。

| # | 检查项 | 为什么重要 | 若未满足的后果 |
| --- | --- | --- | --- |
| 1 | `GET {yggdrasil_root}` 是否提供 `meta.feature.openid_configuration_url` | 这是 Yggdrasil Connect 的服务发现入口；规范规定没有该字段即视为站点不支持 Yggdrasil Connect | **实测已提供**。缺失时启动器直接退回 [2.3](#23-兜底传统-yggdrasil-账号密码登录) 密码登录 |
| 2 | OP 元数据的 `scopes_supported` 是否含 `openid`、`Yggdrasil.PlayerProfiles.Select`、`Yggdrasil.Server.Join` | 规范规定缺少任意一项即视为该 OpenID 提供者不支持 Yggdrasil Connect | **实测三项齐备**；缺失时启动器报错并退回密码登录 |
| 3 | OP 元数据是否提供 `device_authorization_endpoint` | 启动器用设备授权授予登录，才能完全不让玩家把密码交给启动器 | **实测提供**（`https://skin.sitmc.club/api/janus/device/auth`）。缺失时只能走密码兜底 |
| 4 | `client_id=11`（公用应用）是否被允许使用设备授权授予，是否有额外限制 | 规范允许认证服务器对公共客户端限流或缩短令牌有效期 | 步骤 1 直接失败（如 `invalid_client`），OIDC 流程不可用。**实测设备授权端点返回 200，未观察到额外限制；更细的策略未验证** |
| 5 | 访问令牌是否确实绑定玩家选定的角色（`Select` 与 `Server.Join` 同时生效） | 规范规定 `Yggdrasil.Server.Join` 必须与 `Yggdrasil.PlayerProfiles.Select` 同时申请，且访问令牌必须绑定角色，否则会话服务器必须拒绝该令牌进服 | 授权成功但进不了服务器（表现由会话服务器/服务端决定） |
| 6 | 游戏服务端是否已配置 authlib-injector 外置登录 | 账户闭环的前提：访问令牌必须在游戏与服务端两侧被同一 Yggdrasil 源验证 | 客户端能登录、能启动，但进不了服务器。**未验证** |
| 7 | 皮肤与披风是否全部由皮肤站管理 | 启动器本期不提供皮肤编辑入口 | 玩家找不到换肤入口（预期行为，但需在玩家侧说明） |
| 8 | 站点错误响应体格式是否统一为 `error` / `error_description` | 客户端按此结构解析并本地化提示 | 客户端只能显示通用错误，排障困难。**实测 device/token/userinfo 三处均符合** |
| 9 | 站点是否允许启动器与 authlib-injector 使用同一 `{yggdrasil_root}` | 客户端与游戏共用同一 API 根，避免出现两套令牌域；用户信息端点实测已回带 `X-Authlib-Injector-API-Location: {yggdrasil_root}` | 访问令牌无法在游戏侧复用 |

---

## 3. 实例清单契约

> 本章是**社团自建后端必须实现**的部分。启动器**依赖此契约**：它决定主界面上有哪些受管实例、每个实例当前应该是什么内容、以及启动器自身是否必须先更新。
>
> 本章定义的请求头使用 `X-Launcher-*` 前缀，与启动器自更新服务使用的 `X-Axolotl-*` 前缀刻意区分（见 [6.1](#61-与启动器现有机制的对应关系)），两者互不影响。

### 3.1 端点

```http
GET {content_base}/v1/launcher/manifest
Accept: application/json
```

`{content_base}` 由启动器构建时配置（例如 `https://content.sitmc.club`）。契约只要求路径固定为 `/v1/launcher/manifest`。

#### 请求头

| 头 | 必填 | 取值 | 说明 |
| --- | --- | --- | --- |
| `Accept` | 是 | `application/json` | 固定值 |
| `X-Launcher-Version` | 否（建议支持） | 语义化版本，如 `1.3.0` | 当前启动器版本。用于按版本灰度 |
| `X-Launcher-Channel` | 否（建议支持） | `release` / `beta` | 当前更新通道。取值与启动器既有通道枚举一致 |
| `X-Launcher-Platform` | 否（建议支持） | `windows-x86_64` / `linux-x86_64` / `linux-aarch64` / `darwin-x86_64` / `darwin-aarch64` | 取值与启动器既有平台标识**完全一致**，便于复用 |

行为要求：

- 三个头都是**可选**的。若启动器未发送或发送了服务端不认识的值，服务端必须回退到默认（release / 通用平台）清单，**不得**返回错误。
- 若服务端对同一个 URL 因请求头不同而返回不同内容，必须返回 `Vary: X-Launcher-Version, X-Launcher-Channel, X-Launcher-Platform`，避免中间缓存串号。

### 3.2 响应示例

状态码 `200`，`Content-Type: application/json`：

```json
{
  "schema": 1,
  "generated_at": "2026-02-01T00:00:00Z",
  "launcher": {
    "min_version": "1.2.0",
    "latest_version": "1.3.0",
    "force_update": false
  },
  "instances": [
    {
      "id": "survival",
      "name": "生存服",
      "description": "长期生存，白名单开放",
      "icon_url": "https://content.sitmc.club/icons/survival.png",
      "sort": 10,
      "revision": 12,
      "minecraft": {
        "game_version": "1.20.1",
        "loader": "fabric",
        "loader_version": "0.15.11"
      },
      "pack": {
        "kind": "mrpack",
        "url": "https://content.sitmc.club/packs/survival-12.mrpack",
        "sha1": "9f2c1a7b4e5d6f8091a2b3c4d5e6f708192a3b4c",
        "size": 12345678
      },
      "server": {
        "address": "mc.sitmc.club",
        "port": 25565
      },
      "required": true
    }
  ]
}
```

> 示例中的 `sha1` 为形式示例，必须是真实的 40 位十六进制字符串。

### 3.3 顶层字段

| 字段 | 类型 | 必填 | 含义与约束 |
| --- | --- | --- | --- |
| `schema` | integer | 是 | 清单版本。当前为 `1`。客户端遇到**高于**自身支持版本的 `schema` 必须拒绝该清单并提示玩家更新启动器；遇到等于或低于自身支持版本的 `schema` 正常处理 |
| `generated_at` | string | 否（建议提供） | 清单生成时间，RFC 3339 / ISO 8601，**必须带时区且推荐使用 UTC**（形如 `2026-02-01T00:00:00Z`）。仅用于排障展示，客户端**不得**用它判定新旧 |
| `launcher` | object | 是 | 启动器自身更新策略，见 [3.4](#34-launcher-对象启动器自身更新策略) |
| `instances` | array | 是 | 受管实例数组，**可以为空数组**。元素结构见 [3.5](#35-instances-元素) |

### 3.4 `launcher` 对象：启动器自身更新策略

该对象描述**启动器自身**（可执行文件）的更新强制策略，与实例更新无关。

| 字段 | 类型 | 必填 | 含义与约束 |
| --- | --- | --- | --- |
| `min_version` | string | 是 | 允许运行的最低启动器版本，语义化版本（`MAJOR.MINOR.PATCH`）。**低于该版本时必须阻断所有操作**（包括查看实例、启动游戏、进入设置），只允许执行启动器自身更新 |
| `latest_version` | string | 是 | 当前推荐的最新启动器版本，语义化版本。仅用于提示「有新版本可用」，`force_update` 为 `false` 时玩家可以推迟 |
| `force_update` | boolean | 否（默认 `false`） | 是否**强制**更新到 `latest_version`。为 `true` 时玩家不可跳过、不可推迟、不可关闭提示，必须完成更新后才能继续使用 |

强制语义的边界（务必按此实现，客户端的阻断行为由这些规则推导）：

- `min_version` 是**硬阻断**：即使 `force_update` 为 `false`，只要当前版本 < `min_version`，也一律阻断，且**不可暂停、不可推迟**。
- `force_update: true` 是**软阻断中的硬要求**：当前版本 < `latest_version` 且 `force_update` 为 `true` 时，同样不可跳过、不可推迟。
- 版本比较必须按语义化版本规则进行，**不得**做字符串比较（`"1.10.0"` 必须大于 `"1.9.0"`）。
- 该策略的判定结果与实例清单内容无关：即使 `instances` 为空，`launcher` 的强制更新依然生效。

> 启动器自更新走 `https://update.axlmc.org/latest`（现有机制，见 [6.1](#61-与启动器现有机制的对应关系)）。本对象只负责**下发策略**，不负责下发安装包。

### 3.5 `instances` 元素

| 字段 | 类型 | 必填 | 含义与约束 |
| --- | --- | --- | --- |
| `id` | string | 是 | 服务端侧**稳定标识**。建议匹配 `[a-z0-9_-]{1,64}`。客户端据此对应本地受管实例。**服务端改变该值会被视为新实例**：客户端会新建一个实例，原实例保留但被标记为已下线（见 [3.6](#36-实例集合的增删语义)） |
| `name` | string | 是 | 面向玩家的实例名，用于界面展示。不含格式约束，但建议长度不超过 64 字符 |
| `description` | string | 否 | 面向玩家的简介，可含换行，界面做纯文本展示 |
| `icon_url` | string | 否 | 实例图标 URL。必须是 https 且可被客户端直接下载。**不要**指向需要交互式登录的地址，否则图标加载失败 |
| `sort` | integer | 否（默认 `0`） | 展示排序权重，**数值越小越靠前**。相同值时的顺序由客户端决定，服务端不得依赖 |
| `revision` | integer | 是 | 该实例的内容版本号，**单调递增**。**客户端唯一的新旧判定依据**（不做内容哈希、不比较时间）。约束见下方「revision 规则」 |
| `minecraft` | object | 是 | 游戏与加载器信息，见 [3.5.1](#351-minecraft-对象) |
| `pack` | object | 是 | 整合包来源，见 [3.5.2](#352-pack-对象)。**清单中必须同时包含 `revision` 与 `pack`** |
| `server` | object | 否 | 服务器直连信息，见 [3.5.3](#353-server-对象)。若提供，客户端启动时自动连接该服务器 |
| `required` | boolean | 否（默认 `true`） | `true` 表示强制实例：客户端必须完成安装/更新到清单指定的 `revision` 才允许启动，不可延迟。`false` 表示非强制实例，客户端可允许玩家延迟更新（延迟期间通常只能启动旧版本或提示后再更新，具体由客户端交互决定） |

#### revision 规则（契约核心）

1. `revision` 为整数，**单调递增**。服务端一旦为某实例发布了 `revision = 12`，之后只能发布更大的值。
2. **`revision` 变化时，`pack` 必须同时变化**（`url` 或 `sha1` 至少一项不同）。禁止出现「`revision` 变了但包内容与地址完全相同」或「包变了但 `revision` 没变」的情况。
3. 客户端仅在 `本地 revision != 清单 revision` 时触发重新安装/更新；`pack` 内容是否真的变化由服务端负责保证。
4. 发布新包时应使用**新的 URL 或新的文件名**（例如 `survival-13.mrpack`），避免 CDN 与客户端缓存返回旧内容。旧 URL 若继续提供，应至少保留一个客户端更新周期。

#### 3.5.1 `minecraft` 对象

| 字段 | 类型 | 必填 | 含义与约束 |
| --- | --- | --- | --- |
| `game_version` | string | 是 | Minecraft 版本号，如 `1.20.1`。必须是真实存在的正式版本号 |
| `loader` | string（枚举） | 是 | 取值集合：`vanilla`、`fabric`、`forge`、`neoforge`、`quilt`。其他值视为服务端错误 |
| `loader_version` | string | 是（`loader` 为 `vanilla` 时可省略或忽略） | 加载器版本，**必须是精确版本号**。**不接受 `latest` / `stable` 这类别名**：客户端需要可复现的构建结果，别名会导致不同时间安装出不同内容 |

#### 3.5.2 `pack` 对象

| 字段 | 类型 | 必填 | 含义与约束 |
| --- | --- | --- | --- |
| `kind` | string（枚举） | 是 | `mrpack`（**现有，已实现**）或 `zip`（**规划中/未实现**），见下表 |
| `url` | string | 是 | 整合包下载地址。必须能被客户端**直接下载**：**不要** 302 到需要交互式登录的页面、不要返回 HTML 登录页、不要要求 Cookie 或 Referer |
| `sha1` | string | 是 | 包内容的 SHA-1，**40 位小写十六进制**。客户端会强校验 |
| `size` | integer | 是 | 包字节数。客户端会强校验，用于进度显示与下载前空间检查 |

`pack.kind` 取值：

| 取值 | 状态 | 客户端行为 |
| --- | --- | --- |
| `mrpack` | **现有 / 已实现** | Modrinth 整合包格式。客户端按包内 `modrinth.index.json` 安装；更新时会**删除上一版本包内已有、而新版本已移除的文件**，**玩家自己添加的文件会保留** |
| `zip` | **规划中 / 未实现** | 语义为「整包覆盖」：解压到实例目录，服务端须保证包内是完整的实例内容，客户端会清理包外多余文件。**当前客户端只实现了 `mrpack`**，收到 `zip` 时应拒绝该实例并在 UI 中提示「该实例类型暂不支持」 |

完整性校验要求：

- 客户端下载完成后会校验 `sha1` 与 `size`。**任意一项不匹配即判定为安装失败并重试**。
- 因此服务端在替换包文件时必须同步更新 `sha1` 与 `size`，且**先上传新文件、确认可下载后再切换清单**，避免出现清单指向的文件尚不可用或内容不一致的窗口期。

#### 3.5.3 `server` 对象

| 字段 | 类型 | 必填 | 含义与约束 |
| --- | --- | --- | --- |
| `address` | string | 是（若提供 `server`） | 服务器地址，域名或 IP |
| `port` | integer | 否（默认 `25565`） | 端口，取值 `1`–`65535` |

若提供 `server`，客户端启动游戏时会自动连接该服务器。若省略整个 `server` 对象，客户端正常启动但不自动连接。

### 3.6 实例集合的增删语义

| 情况 | 客户端策略 |
| --- | --- |
| 清单中**新出现**的实例 | 视为新受管实例，创建本地实例并执行安装 |
| 清单中已存在、`revision` 未变 | 视为最新，不重复下载 |
| 清单中已存在、`revision` 变大 | 执行强制安装/更新到新 `revision` |
| 清单中已存在、`revision` **变小** | 服务端错误。客户端应拒绝该实例的更新（避免回滚破坏玩家数据），并在 UI 中提示 |
| **未在清单中出现**的受管实例 | **保留但标记为已下线、不再可启动**。**不删除**——客户端无法确认该实例内是否含有玩家存档与截图，误删代价不可逆。这是明确的取舍：以「占用磁盘空间」换取「绝不丢失玩家数据」 |
| `instances` 为空数组 | **合法响应**。客户端展示「暂无可用实例」而不是报错 |

### 3.7 缓存与条件请求

服务端建议：

| 项 | 建议值 | 说明 |
| --- | --- | --- |
| `ETag` | 强 ETag，例如 `"m1-12"`（`schema` + 最大 `revision`） | 内容有任何变化时必须改变 |
| `Cache-Control` | `no-cache` 或 `max-age=60` | `no-cache` 表示允许缓存但每次必须校验；不要给长时间 `max-age`，否则客户端长时间看不到新实例 |
| `Last-Modified` | 可选 | 客户端可选用；**不**作为新旧判定依据 |
| `304 Not Modified` | 当 `If-None-Match` 与当前 ETag 匹配时返回 | 响应**不得带 body**；客户端沿用上一次成功清单 |

客户端行为（供后端评估负载）：客户端使用 `If-None-Match` 做条件请求，并自行节流轮询（建议最短间隔数分钟级别，不做秒级高频轮询）。服务端不必为「短时间内大量相同请求」做特殊优化，但应正确返回 `304` 以降低带宽。

### 3.8 错误响应与客户端处理

#### 建议的错误响应体

```json
{
  "error": "invalid_schema",
  "message": "Unsupported manifest schema version"
}
```

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `error` | string | 是 | 机器可读的稳定错误码（see 下表），**只使用 `snake_case` 英文字符** |
| `message` | string | 否 | 面向开发者/排障的可读信息。**不要**把面向玩家的最终文案写在这里；客户端会结合自己的本地化文案展示 |

建议错误码：

| `error` | 建议状态码 | 含义 |
| --- | --- | --- |
| `invalid_schema` | 400 | 请求参数或版本不受支持 |
| `unauthorized` | 401 | 需要认证（若清单服务对客户端做鉴权） |
| `forbidden` | 403 | 已认证但无权访问该清单 |
| `not_found` | 404 | 端点或清单不存在 |
| `internal_error` | 500 | 服务端内部错误 |

#### 客户端处理策略（非 2xx）

1. **保留上一次成功清单**：客户端会持久化最后一次成功解析的清单，并在请求失败时继续沿用，UI 上给出提示。因此**短暂 5xx 不会让玩家失去实例**。
2. **不覆盖缓存**：错误响应体**永远不得**被当作清单解析，客户端只按 `error` / `message` 处理。服务端不要用 `200` 携带错误对象，也不要返回结构不完整的清单。
3. **连续失败计数**：客户端记录连续失败次数，超过 N 次（N 由客户端实现决定，建议为 3）后停止静默重试，向玩家展示明确的网络错误提示与「重试」入口。
4. **启动器自身强制更新不受影响**：强制更新策略与安装包来自 `launcher` 对象与 `https://update.axlmc.org/latest`，清单服务不可用期间**启动器仍能完成自身更新**。这是刻意的降级设计：后端故障不应导致玩家无法修复客户端。

---

## 4. 客户端行为规范

> 本章是「客户端保证」：写清楚客户端一定会做什么、一定不做什么，便于后端作者理解自己下发的字段实际会引发什么行为。**本章描述的是客户端行为，不是后端实现要求。**

1. **登录先于一切**：启动游戏前必须已登录 SIT-Minecraft 账户（[第 2 章](#2-账户认证契约)的任一流程成功并持有有效的 Minecraft 令牌）。未登录时不进入主界面。
2. **启动前三项校验**：启动游戏前必须校验 —— ①实例存在；②`install_stage == installed`；③本地 `revision` 等于清单 `revision`。任一项不满足，则**先执行强制安装/更新**，该过程**不可跳过、不可取消**。
3. **受管实例只读**：受管实例用户**不可编辑、不可删除、不可改版本、不可改加载器**。玩家的自由度限于启动、查看内容与游戏内行为。
4. **中断可恢复**：安装中断（崩溃、断电、断网、手动结束进程）后，下次启动**自动恢复**。客户端有安装任务的持久化与断点重试机制，不会要求玩家从头重下整个包。
5. **版本硬门槛**：启动器版本低于 `launcher.min_version` 时，**阻断一切游戏启动**，只允许执行启动器自身更新（[3.4](#34-launcher-对象启动器自身更新策略)）。该阻断不可暂停、不可推迟。
6. **完整性强制校验**：整合包下载完成后强校验 `sha1` 与 `size`，不匹配即判失败并重试；重试仍失败则在 UI 明确报错，不静默放过。
7. **玩家自加内容保留**：`mrpack` 更新时只清理「上一版本包内已有、新版本已移除」的文件，**玩家自己添加的文件保留**。
8. **实例下线不删数据**：清单中消失的受管实例只被标记为已下线、不再可启动，**不会被删除**（[3.6](#36-实例集合的增删语义)）。
9. **空清单可容忍**：`instances: []` 展示为「暂无可用实例」，不视为错误。
10. **清单失败可降级**：清单请求失败时沿用上一次成功清单并提示，连续失败达阈值后给出明确的网络错误（[3.8](#38-错误响应与客户端处理)）。

---

## 5. 版本与兼容

### 5.1 清单 `schema` 演进规则

| 规则 | 说明 |
| --- | --- |
| **只增不改** | 在同一 `schema` 内**只允许新增**可选字段，**不得**修改已有字段的类型或语义。客户端必须忽略自己不认识的字段（前向兼容），因此新增字段不会破坏旧客户端 |
| **删除字段需升 `schema`** | 删除字段、修改字段类型、修改字段语义（例如把 `revision` 从整数改成字符串，或改变 `required` 的默认值）都属于破坏性变更，**必须**提升 `schema` |
| **枚举扩展** | 新增枚举取值（如新的 `loader`、新的 `pack.kind`）属于兼容变更，可不升 `schema`：客户端遇到不认识的枚举值时应**跳过该实例并提示**，而不是丢弃整个清单 |
| **未知更高版本拒绝** | 客户端遇到高于自身支持的 `schema` 必须拒绝整个清单并提示更新启动器，不得「尽力解析」 |
| **降级处理** | 客户端遇到低于自身支持的 `schema` 时按该版本的语义尽可能处理；建议服务端在过渡期内保持向后兼容 |

### 5.2 端点版本

- 端点路径包含版本段 `/v1/`。未来不兼容的路径级变更应通过 `/v2/` 引入，**不要**在 `/v1/` 内改变响应形态。
- `schema` 与路径版本是两套独立机制：路径版本表示「端点的迁移」，`schema` 表示「文档结构的迁移」。两者可以同时使用。

### 5.3 本规范对应的启动器实现版本

| 项 | 值 |
| --- | --- |
| 首次实现本规范的启动器版本 | TODO |
| 本规范最近一次修订对应的版本 | TODO |
| 支持的清单 `schema` 上限 | TODO |
| 客户端构建标识（`X-Launcher-Platform` 取值来源） | 现有：`windows-x86_64` / `linux-x86_64` / `linux-aarch64` / `darwin-x86_64` / `darwin-aarch64` |

> 上表中的 `TODO` 需要在实际实现落地后由启动器维护者补齐；在补齐之前，本文档描述的是**目标契约**，不代表已有对应发行版本。

---

## 6. 附录

### 6.1 与启动器现有机制的对应关系

本规范不是从零设计，而是复用启动器既有机制。以下对照关系为**现有实现**，供后端作者理解字段的来源与命名习惯。

| 本规范中的概念 | 启动器现有机制 | 位置 | 说明 |
| --- | --- | --- | --- |
| 启动器自身更新策略 `launcher.*` | 自更新服务 `https://update.axlmc.org/latest` | `apps/app/src/updater_impl.rs` | 现有清单中已含 `force_update` 布尔字段（缺省视为 `false`），并含 `version`、`body`、`published_at`。本规范的 `launcher` 对象是该策略在实例清单中的镜像：`force_update` 语义一致，`min_version` 是本规范**新增**的硬门槛 |
| 灰度请求头（`X-Launcher-*`） | 现有请求头 `X-Axolotl-Channel`、`X-Axolotl-Platform`、`X-Axolotl-Version` | `apps/app/src/updater_impl.rs` | 现有三个头的取值语义（通道 `release`/`beta`、平台标识串、当前版本）与本规范一致，仅前缀不同。沿用取值可以保证客户端两侧逻辑一致 |
| 客户端身份 | `launcher_user_agent()` → `garbage-human-studio/axolotl/{version} ({os})` | `packages/app-lib/src/brand.rs` | 清单服务可在日志中把该 User-Agent 作为客户端标识；它不含联系方式，不要依赖它做鉴权 |
| 产品与品牌 | `PRODUCT_NAME = "Axolotl Launcher"`、`BUNDLE_IDENTIFIER = "red.ghs.axolotl"`、`DEEP_LINK_SCHEME = "axolotl"` | `packages/app-lib/src/brand.rs` | 若社团发行版需要改名/改 bundle id，由构建期配置决定；接口契约不受影响 |
| 账户认证（Yggdrasil Connect 设备授权授予与角色来源） | 设备代码流轮询 + 用户信息端点取角色 + OIDC 刷新 | `packages/app-lib/src/state/minecraft_auth/oidc.rs`（`begin_device_login` / `poll_device_login` / `refresh_credentials_with_oidc`）、`packages/app-lib/src/state/minecraft_auth/yggdrasil.rs`（`openid_configuration_url`、`refresh_yggdrasil_credentials` 的 OIDC 分支）、`packages/app-lib/src/sitmc.rs`（`OIDC_SCOPE` / `OIDC_CLIENT_ID` / `SITE_LABEL`） | **现有实现**：[第 2 章](#2-账户认证契约)的 OIDC 路径已经落地——先用 `openid_configuration_url()` 从 `GET {api_root}` 的 `meta.feature.openid_configuration_url` 取得 OP 元数据，校验 `scopes_supported` 是否含规范要求的三项，再走 RFC 8628 设备代码流；轮询成功后角色来自 `userinfo_endpoint` 的 `selectedProfile`。**访问令牌本身就是 Minecraft 访问令牌，没有换取步骤**；续期走 `refresh_credentials_with_oidc`（`grant_type=refresh_token`）。**启动器不解析、不校验 ID 令牌**，仅在 `selectedProfile` 缺失时解码其 JWT 负载作兜底。`yggdrasil.rs` 中的 `fetch_profiles_with_oauth_token` / `authenticate_with_oauth_token` 是旧实现遗留的死代码，已不再被调用 |
| 密码兜底登录 | 传统 Yggdrasil `POST {api_root}/authserver/authenticate` | `packages/app-lib/src/state/minecraft_auth/yggdrasil.rs`、`apps/app-frontend/src/components/ui/login/SitmcLoginGate.vue` | 站点未启用 Yggdrasil Connect 或玩家主动选择时使用。密码只写入系统凭据管理器（`set_yggdrasil_password` / `get_yggdrasil_password`），不落库。登录界面由 `SitmcLoginGate.vue` 提供：浏览器授权（设备代码流）为主、账号密码兜底，**角色在站点授权页选择**，启动器不再自行列出角色供玩家挑选 |
| 公告接口 | `https://admin.axlmc.org/api/public/announcements` | `apps/app-frontend/src/components/ui/RemoteAnnouncements.vue` | 现有实现以 `?version=<版本>&channel=stable\|beta` 查询（内部通道 `release` 映射为 `stable`），响应形如 `{ "announcements": [...] }`，客户端限 10 秒超时、响应体超 4.5 MB 丢弃、失败静默重试。**公告与实例清单是两条独立链路**：公告走现有 `admin.axlmc.org`，实例清单走社团新后端 |
| 受管实例安装 | 整合包安装管线 `CreatePackLocation`（`FromVersionId` / `FromFile`）与 `mrpack` 安装实现 | `packages/app-lib/src/api/pack/install_from.rs`、`packages/app-lib/src/api/pack/install_mrpack.rs` | 受管实例**复用现有整合包安装管线**：`pack.url` 下载后的本地文件走 `FromFile` 路径安装，因此 `pack.kind = "mrpack"` 能直接落地。`zip` 对应的「整包覆盖」管线尚未实现 |
| 实例安装状态 | `InstanceInstallStage`：`not_installed` / `minecraft_installing` / `pack_installing` / `pack_installed` / `installed` | `packages/app-lib/src/state/instance_types.rs` | [第 4 章](#4-客户端行为规范)中的 `install_stage == installed` 即此枚举 |
| 实例链接类型 | `InstanceLink` 枚举（含 `Unmanaged`、`ModrinthModpack`、`ImportedModpack` 等） | `packages/app-lib/src/state/instances/model/link.rs` | 受管实例的链接类型属于**规划**：当前枚举中没有「社团受管实例」这一变体，需要后续新增（该新增不影响本契约的字段设计） |

### 6.2 域名与 CSP

新后端域名必须加入 `apps/app/tauri.conf.json` 的 `connect-src`，**`devCsp` 与 `csp` 两处都要改**（分别为该文件第 113 行与第 124 行所在的字符串）。遗漏会导致前端请求被 CSP 拦截，表现为「网络错误」而没有任何服务端日志。

| 域名 | 用途 | 是否已在 `connect-src` | 备注 |
| --- | --- | --- | --- |
| `https://content.sitmc.club`（示例，以实际后端域名为准） | **新增**：实例清单 `GET /v1/launcher/manifest` 与整合包/图标下载 | 否，**必须新增** | 至少需要清单域名；若包与图标在其他域名（如 CDN），那些域名同样需要加入 |
| `https://skin.sitmc.club` | 账户认证（Janus 与 Yggdrasil） | 否，**必须新增** | OIDC 设备流与 Yggdrasil 调用都由此发起 |
| `https://admin.axlmc.org` | 现有：远程公告 | 是 | 已存在，无需改动 |
| `https://update.axlmc.org` | 现有：启动器自更新 | 是（通过 Tauri updater 与 capability 配置） | 自更新下载不走前端 `connect-src`；其域名已在 `apps/app/capabilities/plugins.json` 的 http 插件作用域中登记 |

补充提醒（参考，非本契约强制要求）：

- 若新增域名需要由前端直接 fetch，除 CSP 外还需确认 `apps/app/capabilities/plugins.json` 中 http 插件的 URL 作用域是否覆盖该域名（现有条目中可见 `https://update.axlmc.org/*`、`https://api.papermc.io/*` 等）。
- 图片类资源（如 `icon_url`）由 `img-src` 约束，现有策略为 `https:`，因此 https 图标默认可加载；`http://` 图标会被拦截。
- 若后端早期只有测试域名，请同时把测试域名加入两处 CSP，避免开发期反复改动发布配置。

### 6.3 客户端如何指向后端

社团必须对外发布的地址都是**构建期常量**，不提供运行时入口：

| 变量 | 用途 | 未设置时 |
| --- | --- | --- |
| `VITE_SITMC_MANIFEST_URL` | 受管实例清单（§2） | 不请求任何清单，首页显示"未配置社团实例清单地址" |
| `VITE_SITMC_ANNOUNCEMENTS_URL` | 远程公告接口（可选，`?version=&channel=` 由客户端追加） | 不请求任何公告；旧变量 `VITE_AXO_ANNOUNCEMENTS_URL` 仍被兼容 |
| `VITE_SITMC_UPDATE_URL` | 自更新服务基址，客户端请求 `{基址}/latest?channel=` | 设置页不查询最新版本 |

```bash
VITE_SITMC_MANIFEST_URL=https://content.sitmc.club/v1/launcher/manifest \
VITE_SITMC_ANNOUNCEMENTS_URL=https://content.sitmc.club/v1/announcements \
VITE_SITMC_UPDATE_URL=https://update.sitmc.club \
pnpm app:build
```

- 三个变量都**默认为空并关闭对应功能**：客户端绝不会回退到 Axolotl 上游的服务器。指向后端失败时也不会阻断登录与启动。
- 未设置清单变量时，客户端不会请求任何清单，首页会显示"未配置社团实例清单地址"的提示，其余功能（登录、设置、下载、世界、截图）不受影响。
- 之所以不做成设置项：客户端一旦可被随意指向其他服务端，"实例由社团下发"就不再成立。变更后端地址请重新发版。
- 客户端不直接 fetch 清单（请求由 Rust 侧发出，见 `packages/app-lib/src/api/managed.rs`），因此**清单域名不需要加入前端 CSP**；但整合包与图标的下载域名必须在 CSP 的 `connect-src`（下载）与 `img-src`（图标）中可达。

---

*文档结束。*
