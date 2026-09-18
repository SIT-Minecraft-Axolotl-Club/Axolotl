# 启动器更新与使用指南

面向两类人:**玩家**(怎么用/怎么更新)和**社团管理员**(怎么发布一次更新)。

自更新已经接到社团自己的服务:`https://skin.sitmc.club/api/launcher/update`(由 `SITUserCenter` 的 `sitmc-launcher` 插件提供),安装包用**社团自己的 minisign 密钥**签名,公钥内置在启动器里。

---

## 一、玩家侧

### 1. 登录

启动器只接受社团账户。首次打开会看到登录页:

- **使用 SIT-Minecraft 账户登录** → 自动打开浏览器,页面里选好角色并授权;启动器会显示一串授权码,**点一下即可复制**,也可以点「打开授权页」再粘进浏览器。
- 若站点未开放浏览器授权,或你更习惯账号密码:点「**改用账号密码登录**」,用户名密码与皮肤站一致。
- 登录成功后启动器才会进入主界面;换号、登出在**顶栏账户组件**(头像 + 游戏名)里操作。

### 2. 游戏实例

实例由**服务端发布**,分两种:

| 标记 | 行为 |
| --- | --- |
| **强制下发** | 打开启动器即自动下载/更新到服务端指定版本,你**不能删除**,启动前必须是最新版 |
| **按需下载** | 服务端只在列表里列出它,**不会自动下载**;点「开始游戏」时才下载并接着启动;可以随时「从本机删除」释放磁盘,实例仍留在列表里,想玩时再下一次 |

首页面板会显示每个实例的版本、服务器地址与状态;「检查更新」可手动同步一次清单。

### 3. 更新启动器

- 启动器**启动时**会检查一次更新,之后每 5 分钟检查一次,也可以在设置 → **更新** 里手动检查。
- 发现新版本时:启动器后台下载(带进度)→ 校验签名 → 静默安装 → 重启后生效。玩家不需要手动下载安装包。
- 服务端可以要求最低版本:低于要求时启动器会**全屏阻断**,并提供更新入口。
- 若某次更新被登记为**强制更新**,或「暂停更新」失效,仍会被要求安装。

---

## 二、社团管理员

### 1. 改版本号并写更新说明

按仓库约定,玩家看到的「更新后公告」只能写在 `apps/app-frontend/src/announcements/catalog.ts`:

1. 用确切版本号新增一条 `launcher-<version>` 条目(最新放最前),`publishedAt` 为 `YYYY-MM-DD`;
2. 只能使用 `added / changed / deprecated / removed / fixed / security` 类别;
3. 标题与每条改动都要写 `en-US` 与 `zh-CN` 两份;
4. 版本号同时要写进 `apps/app-frontend/package.json`(tauri 的 `version` 指向它),可用仓库脚本:

```powershell
node scripts/axolotl/set-version.mjs v2.0.1      # 同步 package.json 与两个 Cargo.toml
```

### 2. 签名密钥(只需生成一次)

```powershell
pnpm --filter @modrinth/app exec tauri signer generate -w "$env:USERPROFILE\.tauri\sitmc-updater.key" -p "<你自己设的密码>"
```

- **私钥**:`%USERPROFILE%\.tauri\sitmc-updater.key`,**绝不能进仓库**,请离线备份(丢了就无法再给已安装的客户端推送更新,只能让玩家重新下载安装包)。
- **公钥**:`...key.pub` 的**文件内容**(那一整行 `dW50cnVzdGVk…`)就是 `apps/app/tauri-release.conf.json` → `plugins.updater.pubkey` 的值,换密钥时同步替换。
- 当前仓库内置的是社团密钥 `minisign public key: ABED6FC749650B4F`(2026-09-18 生成),对应的私钥与密码:
  - 私钥 `%USERPROFILE%\.tauri\sitmc-updater.key`
  - 密码 `%USERPROFILE%\.tauri\sitmc-updater.key.password`(**请抄进你们的密码管理器/CI 密钥后再删掉这个明文文件**)
- 想换成你们自己的密码:重新生成一对密钥 → 把新的 `.pub` 内容替换进配置 → 重新构建即可(发布第一个正式版之前换是免费的,发布之后就换不动老客户端了)。

### 3. 构建安装包(带签名)

```powershell
# 三个地址是构建期变量,不设的话客户端不会拉清单/公告/更新
$env:VITE_SITMC_MANIFEST_URL       = 'https://skin.sitmc.club/api/launcher/manifest'
$env:VITE_SITMC_ANNOUNCEMENTS_URL  = 'https://skin.sitmc.club/api/launcher/announcements'
$env:VITE_SITMC_UPDATE_URL         = 'https://skin.sitmc.club/api/launcher/update'

# 签名:私钥内容 + 密码都要给(当前 CLI 不认 TAURI_SIGNING_PRIVATE_KEY_PATH)
$env:TAURI_SIGNING_PRIVATE_KEY          = Get-Content "$env:USERPROFILE\.tauri\sitmc-updater.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content "$env:USERPROFILE\.tauri\sitmc-updater.key.password" -Raw

pnpm --filter @modrinth/app build                 # = tauri build --config tauri-release.conf.json
```

> `turbo` 的严格环境模式会**丢弃未声明的环境变量**,这三个 `VITE_SITMC_*` 已加入 `turbo.jsonc` 的 `globalEnv`。
> 构建时若看到 `A public key has been found, but no private key`,就是私钥变量没设;若停在 `Decrypting updater signing key, expect a prompt for password`,就是密码变量是空的(空值会让 CLI 转为交互式提问,CI/后台脚本会一直卡住)。

产物(都在 `target/release/bundle/nsis/`):

| 文件 | 用途 |
| --- | --- |
| `Axolotl Launcher_<版本>_x64-setup.exe` | 给新玩家直接下载安装;**同时就是自更新制品**(后台「制品地址」填它的直链) |
| `Axolotl Launcher_<版本>_x64-setup.exe.sig` | 制品签名:把**整个文件内容**(那一长串 base64)贴进后台「签名」字段 |

> 更新器也接受「包含安装程序的 zip」,但本次构建产出的是安装包本体 + `.sig`,按上表填即可。
> 文件名里的空格不影响使用;要放进 CDN 建议改成 `Axolotl_Launcher_<版本>_x64-setup.exe` 这种不带空格的写法(记得 URL 编码或直接改文件名)。

### 4. 发布到社团更新服务

把 `*-setup.exe` 与其 `.sig` 上传到你们的制品存储(如 CDN)拿到 https 直链,然后后台 → **启动器版本** → 新建:

| 字段 | 填写 |
| --- | --- |
| 渠道 | `release`(正式)或 `beta`(测试) |
| 版本号 | 与安装包一致,例如 `2.0.1`,必须**高于**玩家当前版本 |
| 发布日期 | 实际发布时间(客户端用它计算「发布延迟」) |
| 更新说明 | 可留空;玩家看到的更新公告来自 `catalog.ts` |
| 制品地址 | `*-setup.exe` 的直链 |
| 签名 | 同名 `*-setup.exe.sig` 的**完整内容**(一整串 base64) |
| 目标 / 架构 | `windows` / `x86_64`(留空表示适用于所有平台) |
| 启用 | 勾选 |

客户端随后会请求(启动时 + 每 5 分钟一次,请求头带 `X-Axolotl-Channel: release|beta`):

```
GET https://skin.sitmc.club/api/launcher/update/windows/x86_64/<玩家当前版本>
```

- 有更高版本 → `{"version":…,"notes":…,"pub_date":…,"url":…,"signature":…}` → 启动器下载、验签、静默安装并重启;
- 没有更高版本、记录未启用或缺签名 → `204 No Content` → 启动器认定为「已是最新」。

设置 → 更新 里显示的「最新版本」来自 `GET /api/launcher/update/latest?channel=release|beta`,这个接口只返回版本号/说明,不参与安装。

自测(把 `<版本>` 换成比后台记录低的版本):

```powershell
curl.exe -i "https://skin.sitmc.club/api/launcher/update/windows/x86_64/1.0.0" `
  -H "X-Axolotl-Channel: release"
```

### 5. 后台三个菜单分别管什么

| 菜单 | 作用 |
| --- | --- |
| **启动器实例** | 实例目录:标识/名称/图标/游戏与加载器版本/服务器地址;**强制下发**勾选与整合包上传(上传自动算 SHA-1 并发布新版) |
| **启动器公告** | 启动器内公告:标题、摘要、正文(Markdown)、类型(弹窗/通知)、优先级、生效与结束时间、按钮文案与链接 |
| **启动器版本** | 自更新元数据:渠道、版本号、更新说明、制品地址、minisign 签名、目标平台 |

`revision` 语义:整合包每发布一次(上传或登记外链)版本号 +1,所有客户端下次启动**强制重装**该实例;服务端把实例从清单里删掉,客户端会**连带删除**本地实例与文件。

### 6. 服务端实现要点(插件 `sitmc-launcher`)

- 两个更新路由都接收渠道:优先 `?channel=`,其次请求头 `X-Axolotl-Channel`(启动器的更新器只发请求头,因为它要在路径里带上平台与当前版本);`release` 与 `stable` 视为同一渠道。
- 动态端点(`update/{target}/{arch}/{currentVersion}`)只认**同时填了制品地址与签名**的记录;没有更高版本时返回 204,不要返回空 JSON。
- `latest?channel=` 只用于界面提示,不参与安装。
- 制品下载走客户端自己的 HTTP 客户端:CDN 需要支持 `HEAD`(用于取体积)与 `GET`,并允许任意 User-Agent。

---

## 三、常见问题

| 现象 | 原因与处理 |
| --- | --- |
| 首页只显示「未配置社团实例清单地址」 | 构建时没设 `VITE_SITMC_MANIFEST_URL` |
| 公告页/首页公告卡片没有内容 | 接口是通的(`/api/launcher/announcements`);检查后台是否有已生效的公告,以及构建时是否设了 `VITE_SITMC_ANNOUNCEMENTS_URL` |
| 自更新一直提示「已是最新」 | 依次检查:构建时是否设了 `VITE_SITMC_UPDATE_URL`;`tauri-release.conf.json` 的 `createUpdaterArtifacts` 是否为 `true`;后台记录的版本号是否高于当前版本、是否勾选启用、是否填了制品地址与签名 |
| 下载完更新但安装失败 | 制品地址与签名不是同一对(重新复制 `.sig` 全文,注意别丢字符);或 CDN 对 `.exe` 返回了 HTML 错误页 |
| 更新下载报网络错误 | CDN 不支持 `HEAD`、证书无效、或文件地址需要登录 |
| 换过一次签名密钥后老客户端收不到更新 | 老客户端内置的是旧公钥,只能用旧私钥签名推送**最后一次**更新;新版本再从新公钥起算 |
| 实例点开始没反应 | 该实例是「按需下载」,首次点击会先下载;下载失败会在面板上给出错误文本 |
| 整合包上传失败 | PHP 的 `upload_max_filesize` / `post_max_size` 太小,后台会显示当前限额;大包建议用「登记外链整合包」 |
| 地图页空白 | 地图是站内 iframe,站点不可达时会留空白;右上「在浏览器中打开」可直接跳转 |
