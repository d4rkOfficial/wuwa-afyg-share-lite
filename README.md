# 椰果工坊 · WUWA-AFYG-SHARE-LITE

配合 [椰果工具箱](https://wuwa-afyg-tool.200503.xyz/) 使用的《鸣潮》工程分享平台 —— **SQLite 版**。

只提供 **TUI / CLI 界面**（无 Web 页面），HTTP API 与 [wuwa-afyg-share](https://github.com/d4rkOfficial/wuwa-afyg-share) 完全一致；**不含 AI 工具与 bilibili-toy 相关功能**。

- 用户可以上传椰果工具箱导出的工程 JSON，生成带有效期的分享链接；访客可浏览、克隆（下载）他人的拉表排轴工程
- 工程广场（最新/最热排序、搜索、只看登录用户）、分享码、详情预览、下载原始 JSON
- 登录上传（本地账号，**无邮件系统**）、我的工程（编辑简介标签、设过期、换源、换码、发布/隐藏、删除、保护）
- Buff 集（角色/武器/声骸/套装的固定增益库）、公开浏览、管理员编辑、全量 SQL 导出
- **上游名录**——角色/武器/首位声骸/套装列表直接从 [nanoka.cc](https://static.nanoka.cc) 获取（与椰果工具箱同一上游，随游戏版本自动更新；上游不可达时回退本地缓存）
- Buff 集快照（根 + 版本链：创建/对比/恢复/删除，单快照模型，差异现算不落库）
- 工程保护（豁免批量删除、单条删除与过期清理）、清空内容（本人/管理员）
- 管理员权限链（按用户名授权，仅授出者可撤销，连坐收回；首账号为根管理员）

## 技术栈

| 层   | 技术 |
| ---- | ---- |
| 语言 | Rust（edition 2021） |
| 数据库 | SQLite（`rusqlite` bundled，单文件，零外部依赖） |
| HTTP | axum + tokio（公开接口与原版一致，含 CORS / 限频 / 状态码 / 错误文案） |
| CLI | clap（全功能命令） |
| TUI | dialoguer（方向键交互菜单） |
| 压缩 | brotli（工程文件原始 ≤5MB，压缩后 ≤0.5MB，与原版一致） |

## 快速开始

```bash
# 构建（需要 Rust 工具链，MSVC）
cargo build --release
# 二进制：target/release/wuwa-afyg-share-lite.exe

# 1. 启动服务器（默认 0.0.0.0:3000，数据库 ~/.wuwa-afyg-share-lite/share.db）
target\release\wuwa-afyg-share-lite.exe serve

# 2. 本机使用免登录：localhost 连接直接以根管理员 root_admin 身份操作
wuwa-afyg-share-lite demo                       # 写入示例数据（无需注册）
wuwa-afyg-share-lite publish examples\demo-project.json   # 直接发布

# 3. 启动交互式 TUI（本机同样免登录）
wuwa-afyg-share-lite tui
```

> 服务器与 CLI 分离：`serve` 在后台跑，CLI/TUI 通过 HTTP 访问。
> 默认端口 3000（与原版本地开发一致），可用 `serve --port <端口>` 修改，
> 客户端用 `--server http://localhost:<端口>` 或 `config set server ...` 指定。

## 免登录（localhost → root_admin）

**从本机回环地址（localhost / 127.0.0.1 / ::1）连接服务器时，无需注册登录，
自动以根管理员 `root_admin` 身份操作**（首次访问自动创建该账号，随机密码不可被登录）：

```bash
wuwa-afyg-share-lite whoami        # → 当前用户：root_admin（管理员）
wuwa-afyg-share-lite me            # → 我的工程（root_admin 的工程）
wuwa-afyg-share-lite admin users   # → 管理员列表
```

- 仅**回环地址**生效：服务器中间件按 TCP 对端 IP 判定，非回环请求会被移除该标记（无法伪造）
- 局域网 / 远程客户端仍需 `register` / `login` 获取 token（token 保存在 `~/.wuwa-afyg-share-lite/config.json`）
- `register` 注册的首个**非本机**账号不再是根管理员（本机 root_admin 已存在）；
  需要多管理员时由 root_admin 通过 `admin grant <用户名>` 授权

## 环境变量

| 变量 | 说明 | 默认 |
| ---- | ---- | ---- |
| `PORT` | serve 监听端口 | `3000` |
| `HOST` | serve 监听地址 | `0.0.0.0` |
| `WUWA_AFYG_SHARE_DB` | SQLite 数据库文件路径 | `~/.wuwa-afyg-share-lite/share.db` |
| `WUWA_AFYG_SHARE_SITE_URL` | 分享链接前缀（POST 返回的 url 字段） | 请求 Host |
| `WUWA_AFYG_SHARE_SERVER` | CLI 默认服务器地址 | `http://localhost:3000` |
| `WUWA_AFYG_SHARE_WW_VERSION` | 强制指定上游游戏版本（调试/离线用） | manifest 最新版 |

## CLI 命令参考

```
wuwa-afyg-share-lite <COMMAND>

服务：
  serve [--port] [--host] [--db] [--site-url]   启动 HTTP API 服务器
  tui                                          启动交互式 TUI
  config get | set <key> <value>               查看/修改配置（server / token）

账号：
  register <用户名> [--password]   注册（本机免登录；首个非本机账号为普通用户）
  login <用户名> [--password]      登录并保存 token（本机可跳过）
  logout / whoami                  本机 whoami 显示 root_admin

公开功能：
  browse [--page] [--per-page] [--q] [--sort newest|heat] [--exclude-anon]
  info <分享码>                    分享详情（浏览 +1）
  download <分享码> [-o 文件]      下载工程 JSON
  share <工程.json>               匿名分享（10 分钟有效）
  buff list [--entity-type] [--q] / buff get <类型> <实体> / buff export [-o]
  catalog version                 上游版本与条目统计（nanoka.cc）
  catalog list [--type characters|weapons|echoes|sets] [--q]   上游名录
  announcement list

我的工程（需登录）：
  publish <工程.json> [--desc] [--tag ..] [--days N] [--expires ISO]
  me
  project <id> show | update | set-expiry --at ISO|--clear | replace-file <文件>
              | regenerate-code | toggle-publish | protect | unprotect | delete

管理员：
  admin users | grant <用户名> | revoke <用户名>
  admin projects [--q] [--page] [--page-size]
  admin summary <用户名> | delete-user <用户名>（清空其全部工程）
  buff upsert / delete / entity-update / entity-delete
  snapshot list | create [--note] | diff <id> | restore <id> | delete <id>
  announcement create/update/delete
  cleanup（清理过期工程，服务器每 5 分钟自动执行）

所有返回 JSON 的命令可用 --json 输出原始 JSON；全局参数 --server / --token。
```

Buff 增益项格式（`buff upsert --zone`）：

```
--zone "zoneId=数值[;ref=引用区;pct=比例;threshold=..;lower=..;upper=..;discrete;divisor=..;multiplier=..;override;refOwner=self|owner]"
示例：--zone "atkPct=10;ref=totalAtk;pct=0.05;override"
条件：--condition '{"chain":3,"elements":["导电"]}'（chain 0-6 / refinement 1-5 / elements / damageTypes）
```

> 提示：Windows PowerShell 向原生程序传含双引号的 JSON 参数时会被剥引号，
> 请用反斜杠转义（`'{\"chain\":3}'`）或改用 cmd / 配置文件（`buff entity-update`）。

## HTTP API（与原版完全一致）

| 方法 | 路径 | 说明 |
| ---- | ---- | ---- |
| GET | `/api/public/projects` | 工程广场：`page` `perPage`(≤50) `q` `sort(newest/heat)` `excludeAnon=1` → `{projects, total, page, perPage}` |
| POST | `/api/public/projects` | 匿名分享：`{fileText}` → `201 {code, url}`；按 IP 限频 20 次/分钟；`400/413/429` |
| GET | `/api/buff-sets` | Buff 集：`entity_type` `entity_name` `q` → `{buffSets, total}` |
| GET | `/api/buff-sets/export` | Buff 集全量 SQL 导出（优先最新快照，无快照用实时数据），`Content-Disposition` 下载 |
| GET | `/share/{code}/download` | 下载工程 JSON（克隆 +1）；不存在/已过期 → `404 分享已失效` |
| GET | `/api/catalog` | 上游名录（nanoka.cc）：`{version, source, stale, counts, characters[], weapons[], echoes[], sets[]}` |
| GET | `/api/catalog/{characters\|weapons\|echoes\|sets}` | 单项名录：`{version, source, stale, <key>: [...]}` |
| OPTIONS | 上述路径 | `204` + CORS 头（`Access-Control-Allow-*`） |

上游名录条目结构：角色 `{name, star, element, weaponType}`、武器 `{name, star, weaponType}`、
声骸（首位声骸）`{name, sets[], cost}`、套装 `{name, pieces[]}`；
数据来自 `https://static.nanoka.cc/ww/{version}/character.json / weapon.json / echo.json / sonata.json`（manifest 取最新版本），
内存缓存 30 分钟并持久化到 SQLite，上游不可达时返回缓存数据（`stale: true`）。

响应字段与原版逐一对齐（`authorName`、`gameVersion`、`teamPreview`、`downloads`、`createdAt`、`buffSets`、`entity_type` 等）。

### 扩展接口（本地认证与管理，原版由 Supabase / Server Actions 承担）

| 方法 | 路径 | 说明 |
| ---- | ---- | ---- |
| POST | `/api/auth/register` / `login` / `logout`；GET `/api/auth/me` | 本地账号（username + password，Bearer token 会话 30 天） |
| GET | `/api/public/projects/{code}` | 分享详情（原版为 SSR 页面，此处补 JSON 接口，浏览 +1） |
| GET | `/api/me`、`/api/me/projects`、`/api/me/projects/{id}` 及 `expiry/file/regenerate-code/toggle-publish/protected`、`/api/me/username`、`/api/me/clear-content` | 我的工程（对应原版 server actions） |
| GET/PATCH/DELETE | `/api/admin/projects`、`/api/admin/users`（grant/revoke/summary/content）、`/api/admin/buff-sets`、`/api/admin/buff-entities`、`/api/admin/snapshots`、`/api/admin/announcements`、`/api/admin/cleanup`、`/api/admin/demo` | 管理员功能（对应原版 server actions + RPC） |
| GET | `/api/announcements` | 公告列表（公开） |

## 与原版 wuwa-afyg-share 的差异

| 项 | 原版 | 本版 |
| ---- | ---- | ---- |
| 界面 | Next.js Web（React） | 仅 TUI / CLI |
| 数据库 | Supabase（Postgres + RLS + Auth + pg_cron） | SQLite 单文件（逻辑等价：RLS → 服务层显式校验，pg_cron → 定时任务，Auth → 本地账号） |
| 认证 | GitHub OAuth / 邮箱魔法链接 | 用户名 + 密码；**本机（localhost）免登录直接为 root_admin**（无邮件系统） |
| AI 工具 | `/api/ai/stream`、deepseek 生成/润色 | ❌ 移除 |
| bilibili-toy | 工具箱实例认证（toy-auth） | ❌ 移除 |
| Buff 集实体名录 | 手工录入 | 直接取自上游 nanoka.cc（与椰果工具箱同一数据源） |
| 部署 | Cloudflare Workers | 任意可运行 Rust 的机器 |

业务规则对齐：10 分钟匿名有效期、非匿名过期一周宽限、保护工程豁免删除与清理、
标签清洗（≤12 字符、≤8 个）、标题 60 / 简介 500 / 用户名 20 字符截断、
快照根 + 版本链（差异现算）、管理员权限链不动点级联收回等，均与原版一致。

## 数据库

- 默认位置：`~/.wuwa-afyg-share-lite/share.db`（WAL 模式）
- 表结构与原版 `supabase/migrations/0001_init.sql` 对齐：
  `profiles`（+password_hash）、`projects`、`buff_sets`、`announcements`、
  `buff_set_snapshot`（根 + 版本链）、`admin_grants`（权限链），另加 `sessions`（本地会话）与 `meta`
- `project_blob` 存 brotli 压缩字节；`tags` / `team_preview` / `buff_set` / `condition` / 快照 `state` / `diff` 为 JSON 文本列

## 开发

```bash
cargo build          # 调试构建
cargo build --release
cargo run -- serve --port 3456 --db ./test.db
```

（本仓库 `tools/net-proxy.mjs` 是受限网络环境下给 cargo 用的本地 CONNECT 代理，正常环境不需要。）
