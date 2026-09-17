# SIT-Minecraft 专用化改造:状态与验收说明

本文档记录"把 Axolotl Launcher 改造成 SIT-Minecraft 专用启动器"的当前状态、验收方式、站点侧前提与剩余工作。
接口契约见 [`launcher-protocol.md`](./launcher-protocol.md)。

> ⚠️ **重要**:本次改动的开发环境**没有 Rust 工具链、没有 pnpm/node_modules**,因此**没有经过编译、类型检查或 lint 验证**。
> 落地前请先按下方「验收」一节执行 `cargo check` 与前端 `prepr`。

---

## 1. 目标形态

```
启动 → 登录闸门(只有 SIT-Minecraft 账户) → 拉取服务端实例清单 → 强制更新/安装 → 启动
```

- 账户**有且只有** `https://skin.sitmc.club`(Blessing Skin / Yggdrasil);注册跳转皮肤站。
- 游戏实例**全部**由社团自建服务端下发,客户端按 `revision` 对账并强制更新,启动前有闸门。
- 界面收敛为「登录 / 更新 / 启动」,同时保留多实例、设置、下载、实例列表、联机、世界/截图,皮肤改为跳转皮肤站。

## 2. 已完成

### 2.1 账户(阶段 0 / 1)

| 文件 | 内容 |
| --- | --- |
| `packages/app-lib/src/sitmc.rs` | 新增。集中固定站点、注册/登录地址、Yggdrasil API 根、OpenID 提供者(Janus)基址、`client_id=11`、`OIDC_SCOPE`(`openid offline_access Yggdrasil.PlayerProfiles.Select Yggdrasil.Server.Join`)、`SITE_LABEL`;`is_yggdrasil_api_root()` |
| `packages/app-lib/src/state/minecraft_auth/oidc.rs` | 新增(后按 Yggdrasil Connect 规范重写)。**设备授权授予**登录:先从 Yggdrasil API 元数据的 `meta.feature.openid_configuration_url` 取 OP 元数据并校验规范要求的三项 scope → 申请上述 scope → 轮询令牌端点 → 用访问令牌请求 `userinfo_endpoint`,以 `selectedProfile` 决定角色。**访问令牌本身就是 Minecraft 访问令牌,没有"换取"步骤**;仅在 `selectedProfile` 缺失时解码 ID 令牌负载兜底。另含用 `offline_access` 刷新令牌续期的 `refresh_credentials_with_oidc` |
| `packages/app-lib/src/state/minecraft_auth/yggdrasil.rs` | 新增 `openid_configuration_url()`(读 `meta.feature.openid_configuration_url`,缺失即判定站点不支持 Yggdrasil Connect);`refresh_yggdrasil_credentials()` 在存在站点刷新令牌时改走 `refresh_credentials_with_oidc()`。旧实现遗留的 `fetch_profiles_with_oauth_token()` / `authenticate_with_oauth_token()` 已不再被调用(死代码,待清理) |
| `packages/app-lib/src/api/minecraft_auth.rs` | 新增 `begin_sitmc_device_login` / `poll_sitmc_device_login`;`begin_yggdrasil_login` 不再接受 `api_root`(硬锁站点);`check_reachable()` 由探测 Mojang 会话服改为探测皮肤站 |
| `packages/app-lib/src/state/minecraft_auth.rs` | 新增 `Credentials::is_supported_provider()`;`get_default_credential()` 只接受受支持账户 |
| `apps/app/src/api/auth.rs`、`apps/app/build.rs` | 新命令注册;keyring 命令去掉 `api_root` 参数 |
| `apps/app-frontend/src/helpers/auth.js` | invoke 包装同步新签名 |
| `apps/app-frontend/src/components/ui/login/SitmcLoginGate.vue` | 新增。全屏登录闸门:浏览器授权(设备代码流)为主、账号密码兜底、记住密码、注册/皮肤站跳转。**OIDC 路径的角色在站点授权页选定**;只有密码路径可能需要在启动器内选角色。支持 `overlay` 供侧栏复用 |
| `apps/app-frontend/src/components/ui/AccountsCard.vue` | 精简(1238 → 610 行):只保留账户列表/切换/登出/皮肤站,登录交给闸门 |
| `apps/app-frontend/src/App.vue` | 未登录即全屏阻断;登录/切换/登出后自动刷新账户状态 |

**「有且只能使用 SIT-Minecraft 账户」如何强制**:`users()` 与 `get_default_user()` 过滤掉非受支持账户,启动取号路径 `get_default_credential()` 同样过滤;遗留的微软/离线/他站 Yggdrasil 账户因此无法被选中、无法用于启动,但其数据行保留以便玩家自行删除。

### 2.2 服务端实例(阶段 2,后端已落地)

| 文件 | 内容 |
| --- | --- |
| `packages/app-lib/migrations/20260917090000_managed-instances.sql` | 新增 `managed_instances`(受管实例记录:已应用/目标 revision、名称、排序、上下架、自动连接地址)与 `managed_manifest_cache`(清单快照) |
| `packages/app-lib/src/api/managed.rs` | 新增。清单类型与校验、`fetch_manifest`、`sync_manifest`(与本地对账并写回 `target_revision`/`retired`)、`list_managed_instances`、`prepare_pack`(下载 `.mrpack` 到缓存并校验 sha1/size,`.part` + 原子重命名)、`mark_instance_installed`、`register_instance`、`ensure_instance_runnable`(启动闸门,只读本地库) |
| `apps/app/src/api/instance.rs`、`apps/app/build.rs` | `managed_*` 命令注册 |
| `packages/app-lib/src/api/instance/run.rs` | 启动前调用 `ensure_instance_runnable`;离线模式下找不到离线账户时回退到已登录的 SIT-Minecraft 账户 |

受管实例的**安装**复用既有整合包管线:后端只负责"下载并校验好本地文件路径",安装任务仍走 `install_create_modpack_instance` / `install_pack_to_existing_instance`,因此进度、重试、断点恢复、回滚都直接复用。

前端接线(本轮新增):

| 文件 | 内容 |
| --- | --- |
| `apps/app-frontend/src/helpers/managed.ts` | 新增。清单/记录/动作类型,`managed_*` invoke 包装,以及 `applyManagedInstanceAction()`:先 `prepare_pack` 取本地包 → 新建走 `install_create_modpack_instance`、更新走 `install_pack_to_existing_instance` → `wait_for_install_job` 成功后登记 revision |
| `apps/app-frontend/src/composables/useManagedInstances.ts` | 新增。会话内单例状态:同步 + 顺序执行待更新动作 + 刷新本地记录;暴露 `activeRecords` / `retiredRecords` / `updateProgress` / `launcherUpdateRequired` |
| `apps/app-frontend/src/components/home/ManagedInstancesPanel.vue` | 新增。首页顶部的社团实例面板:同步/更新进度、失败可重试、每个实例一张卡片(图标、名称、版本与加载器、服务器地址、开始游戏),已下线实例折叠展示 |
| `apps/app-frontend/src/pages/Index.vue` | 首页挂载该面板 |

**同步时机与"强制更新"的边界(重要)**

启动闸门只能对比**已经同步到本地**的 `target_revision`,因此同步时机决定了"旧版本还能被启动"的窗口。当前实现有四重触发:

1. **启动时**一次(`App.vue` 初始化完成、隐私同意之后);
2. **首页面板挂载时**一次(幂等,由 `useManagedInstances` 内部去重);
3. **每 5 分钟**一次(启动器持续运行时);
4. **窗口重新获得焦点 / 标签页重新可见**时一次——玩家切回启动器准备点"开始游戏"的那一刻;
5. 此外,**面板上的"开始游戏"会先强制同步再启动**:若服务端已升 revision,玩家会先看到更新进度,更新完成后自动继续启动。

> 边界说明:若玩家始终停留在某个不使用该面板的页面、且网络不可达,客户端无法得知服务端的新 revision,此时启动的是**本地已安装且本地记录一致**的版本(离线可玩优先)。这是刻意的取舍:闸门只读本地库,绝不因联网失败而拒绝启动已安装的实例。若需要"完全不允许离线启动",应改为在闸门里强制联网校验(revision 需要联网才能确认),但那会让断网玩家彻底无法游戏。

### 2.3 启动器自身强制更新(阶段 4)

- `apps/app-frontend/src/providers/app-update.ts`:新增 `isUpdateForced`;强制更新**不再被"暂停更新"隐藏**。
- `apps/app-frontend/src/App.vue`:`force_update` 的更新**不能被暂停或推迟**——更新检查在暂停状态下仍会运行,强制更新跳过 24 小时发布延迟与"计量网络不自动下载"的限制。另外新增 `launcher.min_version` 的**硬阻断**:低于最低版本时全屏阻断(优先级高于登录闸门),并提供一个"立即更新并重启"按钮,该路径刻意忽略"暂停更新"偏好。
- `apps/app-frontend/src/components/ui/app-update-button/index.vue`:强制更新时始终显示提示(文案「必须更新」)。

> 硬阻断依赖社团后端在清单里下发 `launcher.min_version`;未下发时不会触发。版本比较用既有 `compareSemanticVersions()`,无法解析的版本号会记警告并放行(fail-open),避免把玩家锁死。

### 2.4 文档

- `docs/launcher-protocol.md`:账户认证契约 + 实例清单契约(`GET {base}/v1/launcher/manifest` 全字段表、`revision` 语义、`ETag`/304、错误体、增删语义)+ 客户端"强制"行为规范 + 版本兼容 + CSP 提醒。

### 2.5 界面收敛与社团化(按你的确认执行)

- **删除「皮肤」与「多人游戏」**:`routes.js` 去掉 `/skins`、`/multiplayer`(含 servers/rooms/studio 子路由);`App.vue` 去掉两个导航按钮与 `onSkinsPage` 判断;`shortcut-actions.ts` 去掉 `Ctrl+4`/`Ctrl+5` 动作与文案;`settings-search-index.ts`、`onboardingConfig.ts`、`Breadcrumbs.vue` 同步清理;删除 `pages/Skins.vue`、`pages/Multiplayer.vue`。服务器管理组件(`components/multiplayer/**`)保留在磁盘但不可达,日后加回路由即可恢复。
- **删除首页「每日挑战」**:`pages/Index.vue` 侧栏不再渲染,并删除 `components/home/HomeDailyChallenge.vue` 与 `data/daily-challenges.ts`。
- **社团化**:官网指向 `https://www.sitmc.club/`,隐私政策指向 `https://www.sitmc.club/privacy`(已实测为真实页面);QQ 群/频道、爱发电赞助、飞书问卷这些上游社群入口**置空并在界面上隐藏**(有值就自动显示,无需改代码)。唯一保留的上游链接是 `repositoryUrl`:About 页的 LICENSE / COPYING / 第三方许可证都从它拼接,是 AGPL 合规所需的源码与许可出处。
- **公告与自更新改为构建期配置**:远程公告、自更新版本查询分别由 `VITE_SITMC_ANNOUNCEMENTS_URL` / `VITE_SITMC_UPDATE_URL` 控制,**默认空 = 关闭**,不再有指向 `admin.axlmc.org` / `update.axlmc.org` 的默认回退(见 `docs/launcher-protocol.md` §6.3)。

### 2.6 受管实例不可删除 + 同步自愈

实测中暴露过两个问题,已在实例层与同步层修掉(不是靠界面藏按钮):

- **受管实例不能被删除**:`theseus::instance::remove` 会先查 `managed_instances`,命中就直接报错 `Instance … is managed by the club server as "…" and cannot be deleted`。界面同步收敛:实例右键菜单、批量删除、实例设置里的「删除实例」入口对受管实例一律隐藏(`isManagedInstance()`,数据来自 `managed_list`),批量删除只删可删的那些;即使集合过期,删除请求也会被上面的实例层拒绝,不会真的删掉。要让玩家看不到某个实例,应把它从后端清单里移除(客户端会标记为 `retired`,数据保留但不可启动)。
- **删掉实例后点启动"没反应"**:清单同步过去只比较 `revision`,于是本地实例已被删除时它仍判定为「已是最新」,闸门随后抛 `InstanceNotReady`。现在同步会同时检查本地实例是否存在且 `install_stage == Installed`:**实例不存在 → 动作退化为 `create`(重装一个新实例)**;存在但安装未完成 → `update`(装进原实例);只有"存在 + 已安装 + revision 一致"才算 `current`。因此即使玩家此前已经把受管实例删掉,下次同步或点「检查更新」就会自动装回来。

## 3. 验收

下面这组命令已经在本机用完整工具链跑通(`rustup 1.95.0` + MSVC + Node 24 + pnpm),结论附在每条之后;你可以用同一组命令复跑。CI(`.github/workflows/axolotl-ci.yml`)执行的就是它们:

```bash
pnpm install --frozen-lockfile

# 1) CI 的 guardrails 作业(其中两个脚本已在本环境跑通)
node scripts/axolotl/brand-guard.mjs            # 已在本环境通过
node scripts/axolotl/i18n-check.mjs
node scripts/axolotl/downgrade-app-db.test.mjs  # 已在本环境通过
node scripts/axolotl/check-migrations.mjs --base HEAD^   # 需要 git 历史
node scripts/axolotl/check-migrations.mjs --release      # 需要 gh CLI + 网络
cargo fmt --all --check

# 2) CI 的 desktop 作业(关键:前端这条内含 vue-tsc 类型检查)
pnpm --filter @modrinth/app-frontend build   # = contributors:sync && vue-tsc --noEmit && vite build
cargo test --package theseus writing_version_info_creates_missing_parent_directory
cargo check --package theseus_gui --features updater
```

- 前端类型检查**只能**通过 `pnpm --filter @modrinth/app-frontend build`(或 `pnpm prepr:frontend:app`)进行;仓库禁止单独跑 `tsc`/`typecheck`。
- `pnpm prepr:frontend:app` 的 turbo 任务是 `dependsOn: ["fix", "intl:extract", "intl:prune-local"]`,即先 `eslint --fix && prettier --write`(自动修格式),再**重新提取** `src/locales/en-US/index.json`。因此前端格式与 en-US 目录不需要手工维护。
- 若后续新增文案:源码写**英文** `defaultMessage`,中文写进 `zh-CN/index.json`(en-US 由 extract 生成)。i18n-check 要求 zh-CN 覆盖 en-US 的每个 id、ICU 参数一致、且中英文本不能相同(含 2 个以上连续拉丁字母时视为未翻译,专有名词走脚本内的白名单)。
- 开发桌面端前先按 CLAUDE.md 复制 `packages/app-lib/` 的 `.env` 模板;最终功能验证用 `pnpm app:dev`。

**已在本环境完成的验证**(这些不需要 Rust 工具链):

- 把 `packages/app-lib/migrations/**` 的**全部 107 个迁移**按文件名顺序在一个内存 SQLite 中执行:**全部成功**;`managed_instances` 与 `managed_manifest_cache` 的表结构与本设计逐字段一致。
- 用真实 SQLite **解析** `packages/app-lib/src/api/managed.rs` 的全部 **10 条 SQL**:全部通过 prepare(说明表名、列名、语句均正确),且每条的 `?` 数量与 `.bind()` 数量**一一匹配**。
- **Rust 定界符平衡**:对 13 个被改动/新增的 Rust 文件做词法分析(剥离注释、普通/原始字符串、字符字面量)后检查 `()`/`[]`/`{}` 配对,全部通过——可排除"编辑截断"这类错误。
- **前端 import 路径**:586 个文件中所有相对路径与 `@/` 别名导入**全部解析到真实文件**(0 处未解析);9 个重度改动文件的**未使用导入为 0**。
- **i18n**:源码 4019 个消息 id 全部存在于 `zh-CN`;套用 CI 自身白名单后,`i18n-check` 的失败数为 **0**。
- **路由残留**:活路径上 0 处指向已停用路由的跳转/命名路由;残留仅存在于已停用页面(死代码)。
- **引导锚点**:所有 `targetId` 均有对应锚点(设置页 4 个通过 `settings-category-definitions.ts` 的 `onboardingId` 声明)。
- **CI 脚本**:`brand-guard.mjs`、`downgrade-app-db.test.mjs` 已直接跑通;`check-migrations` 因工作区无 `.git` 无法运行(它只审计"已发布迁移是否被改动",本改动仅**新增**迁移,不影响)。
- **站点端点**:见 §4.1。
- `auth` 插件的 `invoke_handler` 与 `apps/app/build.rs` 白名单两侧一致(各 20 项);`add_offline_user` / `parse_custom_uuid` / `MinecraftLoginModal` 全仓零残留。

**最可能的编译风险点**(优先排查):

1. `packages/app-lib/src/api/managed.rs` 是新增的大模块(约 950 行),SQL 列名与迁移的对应关系、`sqlx::FromRow` 派生、`reqwest` 流式下载的类型。
2. `packages/app-lib/src/state/minecraft_auth/oidc.rs` 的类型与可见性(`pub(super)` 交叉引用)。
3. `apps/app/src/api/instance.rs` 新命令的参数名与 `apps/app/build.rs` 白名单是否一致(不一致会在运行时报 `command not found`)。
4. 前端:新命令的 IPC 前缀必须与实际注册的插件一致(`plugin:instance|managed_*`)。

## 4. 站点侧前提(已实测,见 §4.1)

站点符合 Yggdrasil Connect 规范,规范流程所需端点齐备。客户端因此以 **OIDC 设备码流为主、账号密码为兜底**。

1. **服务发现**:`GET https://skin.sitmc.club/api/yggdrasil` 的 `meta.feature.openid_configuration_url` **实测存在**,指向 `https://skin.sitmc.club/api/janus/.well-known/openid-configuration`;`meta.serverName=SIT-Minecraft`。规范规定**没有该字段即视为站点不支持 Yggdrasil Connect**。
2. **OP 元数据**:`token_endpoint`、`userinfo_endpoint`、`device_authorization_endpoint` 均已提供;`scopes_supported` 含 `openid`、`offline_access`、`Yggdrasil.PlayerProfiles.Select`、`Yggdrasil.PlayerProfiles.Read`、`Yggdrasil.Server.Join`(规范要求其中 `openid`、`Yggdrasil.PlayerProfiles.Select`、`Yggdrasil.Server.Join` 三项必须齐备);`shared_client_id="11"`。
3. **公用应用 `client_id=11`**:实测可正常调用设备授权端点。规范允许认证服务器对公共客户端施加额外限制(强制每次授权确认、缩短令牌有效期),**更细的策略未验证**。
4. **访问令牌即 Minecraft 令牌**:会话服务器直接校验该访问令牌,authlib-injector 启动游戏时把它交给游戏本体;**本站不提供、也不需要**"用 OIDC 令牌换 Minecraft 令牌"的非规范兑换端点。
5. **仍待确认**:游戏服务端是否已用 authlib-injector + 本皮肤站做外置登录(账户闭环的前提);申请 `offline_access` 后是否确实颁发 `refresh_token`(**未验证**,需完成一次真实授权)。

客户端申请的 scope 固定为 `openid offline_access Yggdrasil.PlayerProfiles.Select Yggdrasil.Server.Join`:角色由**站点授权页**选定,访问令牌因此绑定角色(规范要求 `Yggdrasil.Server.Join` 必须与 `Yggdrasil.PlayerProfiles.Select` 同时申请,否则会话服务器必须拒绝该令牌进服)。规范**不应允许**应用同时申请 `Select` 与 `Read`,启动器既不申请 `Read` 也不读 `availableProfiles`;启动器**不解析、不校验 ID 令牌**,仅在 `selectedProfile` 缺失时解码其 JWT 负载兜底。

### 4.1 站点端点实测结果(只读探测,2026-09)

探测方法:对只允许 POST 的已知端点发 `GET`——该站点"路径存在但方法不对"返回 **405**,"路径不存在"返回 **404**,两者可区分。

| 端点 | 结果 | 含义 |
| --- | --- | --- |
| `GET /api/yggdrasil` | 200 | 含 `meta.feature.openid_configuration_url` 与 `meta.serverName=SIT-Minecraft`,服务发现**可用** |
| `GET /api/janus/.well-known/openid-configuration` | 200 | OP 元数据**可用**:`device_authorization_endpoint`/`token_endpoint`/`userinfo_endpoint` 齐备,`shared_client_id="11"`,`scopes_supported` 含规范要求的三项 |
| `POST /api/janus/device/auth`(`client_id=11`,上文 scope) | 200 | 设备授权授予**可用**:返回 `device_code`/`user_code`/`verification_uri`/`verification_uri_complete`/`expires_in=600`;**未返回 `interval`**(客户端按规范缺省 5 秒) |
| `POST /api/janus/token`(device_code 尚未授权) | 400 + `authorization_pending` | 轮询语义符合规范 |
| `GET /api/janus/userinfo`(带伪造令牌) | 401 + `WWW-Authenticate: Bearer error="invalid_token",...` + `{"error":"invalid_token",...}` | 用户信息端点**可用**,并按规范拒绝无效令牌;响应带 `X-Authlib-Injector-API-Location: https://skin.sitmc.club/api/yggdrasil`;该端点 302 到 `/yggc/userinfo`,跟随重定向后 `Authorization` 头仍保留(已验证) |
| `GET /api/yggdrasil/authserver/authenticate` | 405 | 经典密码登录存在,**可用**(仅 POST) |
| `GET /api/yggdrasil/authserver/validate` | 405 | 令牌校验存在,**可用**(仅 POST) |

**结论:本站符合 Yggdrasil Connect 规范,规范流程所需端点齐备。启动器以 OIDC 设备码流为主、账号密码为兜底**——账号密码路径已实测可用,作为站点未启用 Yggdrasil Connect 时的兜底。

旧版本本文档曾以**两个非规范端点**(`POST /api/yggdrasil/authserver/oauth` 与 `GET /api/yggdrasil/sessionserver/session/minecraft/profile`)作为依据,得出了 OIDC 登录在本站不可用的错误结论。本站不提供、也不需要这两个端点:兑换端点并非规范的一部分,角色来自**用户信息端点**。**该结论已作废。**

## 5. 剩余工作

| 项 | 内容 |
| --- | --- |
| 配置 | 后端清单地址写进构建变量 `VITE_SITMC_MANIFEST_URL`(未设置时首页会显示"未配置"提示,其余功能不受影响) |
| 阶段 3 | **已完成**,见下方 §5.0 |
| 阶段 5 | 见下方"已清理"与"有意延后" |

### 5.0 阶段 3 实际结果

- `apps/app-frontend/src/routes.js`:只保留 `/`、`/worlds`、`/downloads`、`/settings`、`/skins`、`/screenshots`、`/help/drop`、`/multiplayer/**`、`/library`(仅概览)、`/instance/:id`(仅 `InstanceOverview` 与 `logs`)。
- `App.vue`:移除「发现 / Lab / 新建实例」三个导航项与 Modrinth 账户入口;`OpenSeedMap` / `OpenDiscovery` 事件不再跳转到已停用页面。
- `pages/instance/Index.vue`:标签裁到 **Overview + Logs**。
- `onboardingConfig.ts`:删除目标已不存在的步骤(仅剩 7 步)。
- `packages/app-lib/src/api/handler.rs`:深链 `discovery` / `seed-map` / `mod/{id}` / `version/{id}` / `modpack/{id}` / `server/{id}` 与 `.mrpack` / `.zip` 文件打开一律拒绝,`launch?instance_id=` 保留;并**同步更新了该文件的单元测试**。
- `pages/Skins.vue`:改为跳转皮肤站的小页面(保留 `data-onboarding-id="skins-page"` 与角色名/UUID 展示)。
- 活路径残留清理:还额外修掉了 7 处会跳到已停用路由的地方——`pages/library/Index.vue`(标签与"新建实例"按钮)、`pages/Index.vue`(首页新建动作改为触发服务端清单同步)、`pages/Downloads.vue`(移除"新建下载"按钮)、`components/GridDisplay.vue`(被保留的 `/library` 使用的"新建实例"按钮)、`helpers/shortcut-actions.ts`(删除 Discover/Lab/新建实例三个全局导航快捷键)、`components/ui/settings/settings-search-index.ts`(对应搜索项)、`composables/useDropImport.ts`(改为回首页)。

### 5.1 本轮已清理(阶段 5 的安全子集)

- 删除孤儿组件 `apps/app-frontend/src/components/ui/MinecraftLoginModal.vue`(微软设备码登录弹窗,已无任何引用)。
- `add_offline_user` 端到端移除:`apps/app-frontend/src/helpers/auth.js` 包装、`apps/app/src/api/auth.rs` 命令与 `parse_custom_uuid`(含其单元测试)、`apps/app/build.rs` 白名单、`packages/app-lib/src/api/minecraft_auth.rs` 的 API 函数。
- 移除前端不再使用的微软登录包装:`login`、`browser_login`、`begin_device_login`、`poll_device_login`。

### 5.2 有意延后(需要能编译/能跑迁移测试的环境再做)

「账户有且只能使用 SIT-Minecraft」**在行为上已经强制**:`Credentials::is_supported_provider()` 让非 SIT-Minecraft 账户既不出现在列表、也不被认作已登录、更不能用于启动(见 §2.1)。剩下的只是**删除不可达代码**,而其中一部分风险过高:

1. `MinecraftAccountType::{Microsoft, Offline}` 与 `minecraft_users.account_type` 的 `CHECK` 约束——SQLite 无法直接改约束,需要"建新表 → 搬数据 → 改名 → 重建索引"的重建型迁移。按 `CLAUDE.md`,这类迁移必须有针对"全新库"与"含旧数据的升级库"的测试,而本环境没有 Rust 工具链、跑不了迁移测试。
2. 离线皮肤管线(`api/minecraft_skins/offline.rs`、`offline_minecraft_skins` 表、`minecraft_skins.rs` 里 12 处 `is_offline()` 分支、启动时的离线皮肤资源包注入)与微软 OAuth/Xbox/XSTS 流程(`state/minecraft_auth.rs` 中约数百行 + `apps/app/src/api/auth.rs` 的 `signin` 窗口)。删除它们牵涉面广且不影响已实现的行为强制。
3. ~~**拖拽整合包仍可创建实例**~~ **已修复**:`composables/useDropImport.ts` 现在在四个位置拒绝"整合包 / 其他启动器实例目录"的拖入——分类完成后(`continueWithClassification`)、确认弹窗确认时(`handleDropConfirm`,防止玩家在弹窗里手动改类型)、批量分组确认弹窗(`showBatchGroupConfirmModal`,并从"选项"里过滤掉整合包)、批量确认(`onBatchGroupConfirm`)。拖入存档/资源包/数据包等仍然可用(用户保留了"世界/截图")。新增两条文案 `app.drop.error.unmanaged-title` / `-text` 已按 i18n 约定同时写入源码(英文)与 `zh-CN/index.json`(中文)。

建议:等本地能跑 `cargo check` / `cargo test` 后,再用一次专门的重构把它删干净;届时请为迁移补上升级库测试。

## 6. 关键取舍(便于日后 review)

- **账户复用 Yggdrasil 而不是新造类型**:角色枚举、令牌刷新、authlib-injector 注入(启动时自动加 `-javaagent` 与 prefetched 元数据)全部现成,新增的只有"取令牌"的方式。
- **用 `Select` 让站点授权页选角色,而不是 `Read` + 启动器列角色**:规范不允许同时申请两者,且 `Yggdrasil.Server.Join` 必须与 `Select` 同时申请(访问令牌必须绑定角色);角色改从 `userinfo_endpoint` 的 `selectedProfile` 读取。**不解析、不校验 ID 令牌**,仅在 `selectedProfile` 缺失时解码其 JWT 负载兜底,避免自实现 JWT 签名校验。
- **`managed_instances` 用独立表而不是扩展 `InstanceLink`**:避免改动实例持久化层(运行时查询 + `.sqlx` 缓存),把受管语义收在一个新模块里。
- **清单地址由前端传入 Rust 命令**:配置只有一处(前端 `SitmcConfig`),且后端域名无需加入 Tauri CSP。
- **启动器更新"强制"的两层含义**:`force_update` 让更新不可暂停/不可推迟;`launcher.min_version` 则在低于要求时阻断一切操作。两者都可被社团后端独立控制。
- **受管实例的 revision 记录归 sync 与安装两方协作**:`sync_manifest` 只写 `target_revision`,`register_instance` / `mark_instance_installed` 只写已应用 `revision`,因此安装期间清单若继续前进,客户端仍会再次强制更新。
- **新文案按仓库 i18n 约定**:源码英文 `defaultMessage` + `zh-CN/index.json` 中文翻译(本轮新增 48 条),以满足 CI 的 `i18n-check`。

## 7. 本地联调:在后端写好之前跑通整条链路

目标:用你自己的机器上的一份静态清单 + 一个真实整合包,验证「登录(浏览器授权设备码流,站点授权页选角色)→ 拉清单 → 强制安装 → 启动」以及「改 revision → 强制更新」。

### 7.1 准备整合包与它的 sha1

从 Modrinth 任意整合包页面下载一个 `.mrpack`(或先手工做一个最小 `.mrpack`:`modrinth.index.json` + `overrides/`),然后算 sha1(必须是小写十六进制):

```powershell
certutil -hashfile .\pack.mrpack SHA1
```

### 7.2 写一份清单

在同一个文件夹里新建 `manifest.json`:

```json
{
  "schema": 1,
  "generated_at": "2026-01-01T00:00:00Z",
  "launcher": { "min_version": null, "latest_version": null, "force_update": false },
  "instances": [
    {
      "id": "survival",
      "name": "生存服",
      "description": "本地联调用的受管实例",
      "icon_url": null,
      "sort": 10,
      "revision": 1,
      "minecraft": { "game_version": "1.20.1", "loader": "fabric", "loader_version": "0.15.11" },
      "pack": {
        "kind": "mrpack",
        "url": "http://127.0.0.1:8000/pack.mrpack",
        "sha1": "把上一步算出的 sha1 填在这里",
        "size": 12345678
      },
      "server": { "address": "mc.sitmc.club", "port": 25565 },
      "required": true
    }
  ]
}
```

> `size` 可以省略;`sha1` 必填(客户端强校验,不匹配会拒绝安装)。`minecraft` 仅用于界面展示与人工核对——**实际版本/加载器以整合包内的 `modrinth.index.json` 为准**。
>
> `revision` 是唯一的新旧判定依据:把它从 `1` 改成 `2`,下次启动就会**强制重装/更新**这个实例(旧包里已有、新包没有的文件会被删除,玩家自加的文件保留)。

### 7.3 起一个本地静态服务

```powershell
# 在存放 manifest.json 与 pack.mrpack 的目录里执行
python -m http.server 8000
```

### 7.4 带着清单地址构建/启动

清单地址是**构建期常量**,必须在构建前设置环境变量:

```powershell
$env:VITE_SITMC_MANIFEST_URL = 'http://127.0.0.1:8000/manifest.json'
$env:VITE_SITMC_ANNOUNCEMENTS_URL = ''   # 留空即不请求公告
$env:VITE_SITMC_UPDATE_URL = ''          # 留空即不查询最新版本
pnpm app:dev          # 开发运行
# 或：pnpm --filter @modrinth/app-frontend build  然后 cargo check/tauri build
```

### 7.5 预期表现与已知限制

- **登录**:站点已符合 Yggdrasil Connect 规范(见 §4.1),默认走浏览器授权(设备代码流),角色由站点授权页选定;仅当站点未启用该特性、或玩家主动点击「改用账号密码登录」时,才走密码兜底。
- **启动即同步**:客户端在初始化完成后会拉一次清单;有 `create`/`update` 的实例会自动下载并安装,首页面板显示进度(也可在「下载」页看任务)。
- **受管实例不可改**:实例列表里这些实例的版本/加载器由服务端决定;启动前会再校验一次 revision 与安装状态,过期会拒绝启动并要求先更新。
- **已下线实例**:从 `instances` 数组里删掉某条后,该实例在本地被标记 `retired`:**不再可启动,但数据保留**(不会删玩家存档)。
- **已知限制**:`icon_url` 用 `http://` 的图片会被前端 CSP 拦掉(`img-src` 只放行 `https:`),本地联调时图标不显示属正常现象;整合包下载本身不受影响(Rust 侧直接请求)。

### 7.6 验证强制更新

1. 首次启动完成安装后,能正常进入游戏。
2. 把 `manifest.json` 里的 `revision` 改成 `2`(并换一个整合包或改动包内容,同时更新 `sha1`),重启启动器:面板应显示"正在更新",完成后才能启动。
3. 直接把 `revision` 改成 `2` 但**不改包**:客户端仍会重装一次(这是刻意的强一致语义,服务端若确定内容未变就不要动 `revision`)。
4. 在 `launcher.min_version` 里填一个高于当前版本的值(如 `99.0.0`):重启后应出现全屏"需要更新启动器"阻断,且不受"暂停更新"设置影响。

## 8. 上线前必须替换的上游遗留项

品牌与联系方式已在 §2.5 处理完毕;下表是**仍然残留**的上游项,社团发布前需要处理:

| 位置 | 当前值(上游) | 建议 |
| --- | --- | --- |
| `packages/app-lib/src/brand.rs` | `WEBSITE`(`ghs.red`,目前无引用点)、`user_agent`(`garbage-human-studio/axolotl/...`)、`PRODUCT_NAME`/`SHORT_PRODUCT_NAME`、`BUNDLE_IDENTIFIER`(`red.ghs.axolotl`)、`DEEP_LINK_SCHEME`(`axolotl`) | `WEBSITE` 可直接改成社团官网;改 `user_agent` 需同步该文件里的单测。**不要**改 `BUNDLE_IDENTIFIER` 或数据目录:玩家数据目录会随之改变(等于全新安装) |
| `apps/app/tauri.conf.json` | `productName`(`Axolotl Launcher`)、`identifier`、`shortDescription`/`longDescription`(空)、CSP 里的上游域名(`admin.axlmc.org`、`mod.mcimirror.top`、`mod.tianpao.top`) | 按需替换描述与域名;**清单域名不需要加进 CSP**(请见 §6.3),但整合包/图标域名要 |
| `apps/app/tauri-release.conf.json` + `apps/app/src/updater_impl.rs` | 自更新服务 `https://update.axlmc.org/*` 与上游 minisign 公钥 | 这两处**只在带 `updater` 特性的发布变体里编译**。社团自建更新服务之前,建议用 CI 里的 native 变体出包(`pnpm --filter @modrinth/app tauri build`):它不编译更新器,因此不会去连上游更新服务;需要自更新时再同时改这两处与 `apps/app/capabilities/plugins.json` 的 http 作用域 |
| `apps/app-frontend/src/announcements/catalog.ts` | 上游 Axolotl 的历史发布说明 | 这是**启动器更新后弹给玩家的更新公告**;社团应写入自己的条目(需要你提供确切版本号) |
| `packages/app-lib/src/sitmc.rs` | 站点/注册/登录/Yggdrasil API 根/Janus 基址/`client_id`/scope 串 | 站点域名或 `client_id` 变化时改这里;启动器的其他部分都引用这些常量 |
| `apps/website`(Nuxt) | 上游政策页与介绍页 | 若社团不部署这个站点可忽略;若要部署,隐私政策应改指社团自己的条款(启动器内已指向 `https://www.sitmc.club/privacy`) |
| 遥测 | `capabilities.ghsTelemetry: false`(已关闭) | 若将来要开,需先补隐私政策 |
| 构建变量 | `VITE_SITMC_MANIFEST_URL`、`VITE_SITMC_ANNOUNCEMENTS_URL`、`VITE_SITMC_UPDATE_URL`(均未设置) | 至少设置清单地址,否则首页只显示"未配置社团实例清单地址";后两个留空即关闭对应功能(详见 §6.3) |

> 关于「更新公告」:按仓库约定,启动器发版公告只能写在 `apps/app-frontend/src/announcements/catalog.ts`,并且需要确切的版本号;`scripts/axolotl/create-release-notes.mjs` 会用它生成 GitHub Release 正文,标签没有对应条目时发布会失败。
