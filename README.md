# 椰果工坊 · WUWA-AFYG-SHARE-LITE

《鸣潮》工程分享平台（SQLite 版）——配合 [椰果工具箱](https://wuwa-afyg-tool.200503.xyz/) 使用。
有 **TUI / CLI / 极简 Web** 三种界面，数据存在本地 SQLite 文件里，HTTP API 与线上版 wuwa-afyg-share 完全一致（不含 AI 工具与 bilibili-toy 功能）。

**一句话：启动服务器 → 浏览器打开或敲命令 → 上传/浏览/分享工程、管理 Buff 集。**

---

## 一、安装与启动（约 2 分钟）

### 1. 构建

需要安装 [Rust](https://www.rust-lang.org/tools/install)（MSVC 工具链），然后：

```bash
cd wuwa-afyg-share-lite
cargo build --release
```

生成的可执行文件：`target\release\wuwa-afyg-share-lite.exe`（下文用 `lite` 指代它）。

### 2. 启动服务器

```bash
lite serve
```

- 默认监听 `0.0.0.0:3000`，数据库文件在 `~/.wuwa-afyg-share-lite/share.db`（自动创建）
- 改端口：`lite serve --port 8080`；改数据库位置：`lite serve --db D:\data\share.db`
- 看到 `wuwa-afyg-share-lite 服务已启动` 即成功；`Ctrl+C` 停止

### 3. 开始使用

```bash
lite tui        # 打开交互式界面（方向键选择、回车确认）
lite --help     # 查看全部命令
```

> **本机免登录**：从 localhost 连接时不需要注册/登录，自动以根管理员 `root_admin` 身份操作。
> 局域网/远程访问才需要账号（见第五节）。

### 4. Web 界面（浏览器）

服务器启动后，**浏览器直接打开 http://localhost:3000** 即可使用：

- 工程广场（搜索 / 最新·最热排序 / 分页）、分享详情、下载 JSON
- 匿名分享与登录后上传（登录后还可填简介、标签、过期天数）
- 我的工程（发布/隐藏、设过期、删除）
- Buff 集浏览、上游名录（角色/武器/首位声骸/套装，来自 nanoka.cc）、公告
- 登录 / 注册（右上角；**本机打开自动一键登录 root_admin（免登录）**，远程才需账号）

实现为**单文件原生 Web Components**（`<lite-header>`/`<lite-modal>`/`<lite-toast>` 等），
无框架、无 Tailwind、无构建步骤，直接调用本服务 HTTP API，内嵌在可执行文件里。

---

## 二、TUI 界面使用教程

进入 `lite tui` 后是主菜单，用 **↑↓ 移动、回车确认**：

```
主菜单
  工程广场        ← 浏览/搜索/下载别人的分享
  我的工程        ← 自己上传的工程管理
  Buff 集         ← 增益库数据 + 上游名录
  管理（管理员）   ← 用户授权、公告、快照等
  公告
  上传工程        ← 上传新工程
  退出
```

### 1. 工程广场（浏览别人的分享）

- 进入后是工程列表，`↑↓` 选择、回车查看详情
- 列表顶部四个选项：**上一页 / 下一页 / 搜索 / 排序**（最新 ↔ 最热）
- 详情页可以 **下载 JSON**（保存为文件）

### 2. 上传工程

主菜单选「上传工程」：

1. 输入工程 JSON 文件路径（椰果工具箱导出的文件，可用项目里的示例 `examples\demo-project.json`）
2. 依次输入：简介（可留空）、标签（逗号分隔，可留空）、过期天数（留空 = 永久）
3. 完成：匿名分享 10 分钟有效；本机 root_admin 上传则按填写的有效期

### 3. 我的工程（管理自己的分享）

- 列出你的工程与统计（总数/已过期/总浏览/总克隆）
- 选择某个工程后可：**编辑标题/简介/标签、设置过期、换源（替换文件）、重新生成分享码、发布/隐藏切换、保护/解除保护、删除**

### 4. Buff 集（固定增益库）

- **Buff 集数据**：按实体类型（角色/武器/声骸/1-5件套）浏览已录入的增益条目
- **上游名录**：角色 / 武器 / 首位声骸 / 套装列表**直接来自上游 nanoka.cc**（与椰果工具箱同一数据源），支持按名称搜索，声骸可看所属套装、套装可看件数

### 5. 管理（管理员菜单）

- 管理员列表 / 授权管理员 / 撤销管理员（仅授权者可撤销，连坐收回）
- 工程管理（管理员视角改标题/作者名、删除）
- 清空用户工程（按用户名，保护工程豁免）
- 快照管理：创建快照（无根则建根、有根则追加版本）、对比差异、恢复、删除最新版本
- 公告管理：新建/删除公告
- 清理过期工程、写入示例数据

---

## 三、CLI 常用命令

> 所有命令都通过 HTTP 访问服务器；连本机默认免登录。
> 指定服务器：`lite --server http://localhost:8080 <命令>`（或 `lite config set server http://localhost:8080` 一劳永逸）。
> 加 `--json` 可输出原始 JSON。

### 公开功能（无需登录）

```bash
lite browse                          # 工程广场（--page --per-page --q 搜索 --sort heat 最热 --exclude-anon 只看登录用户）
lite info <分享码>                    # 查看分享详情（浏览 +1）
lite download <分享码> -o 工程.json    # 下载工程 JSON
lite share 工程.json                  # 匿名分享（10 分钟有效）
lite catalog version                 # 上游数据版本与条目统计
lite catalog list --type characters  # 上游角色名录（--type weapons/echoes/sets，--q 搜索）
lite buff list --entity-type character   # 本库 Buff 条目
lite buff get character 今汐          # 查看某实体已录入的 Buff
lite buff export -o buff.sql         # 导出全量 SQL
lite announcement list               # 公告
```

### 本机管理（root_admin，免登录）

```bash
lite demo                            # 写入示例数据（7 条 Buff + 1 条公告），快速体验
lite me                              # 我的工程与统计
lite publish 工程.json --desc 简介 --tag 标签 --days 30   # 发布（--days 过期天数，缺省永久）
lite project <工程ID> show           # 工程详情（ID 用 lite me --json 查）
lite project <工程ID> set-expiry --at 2030-01-01T00:00:00.000Z
lite project <工程ID> regenerate-code # 换分享码
lite project <工程ID> protect        # 保护（删除/清理豁免）
lite project <工程ID> delete         # 删除（有二次确认）
lite admin users                     # 管理员列表
lite admin grant <用户名>            # 授权管理员
lite admin revoke <用户名>           # 撤销（连坐收回子树）
lite admin projects                  # 管理员工程列表
lite admin summary <用户名>          # 用户工程统计
lite admin delete-user <用户名>      # 清空某用户全部工程（保护豁免）
lite snapshot create --note 备注     # 创建快照（无根建根 / 有根追加版本）
lite snapshot list                   # 快照列表（根 + 版本链）
lite snapshot diff <快照ID>          # 对比当前差异
lite snapshot restore <快照ID>       # 恢复（级联删除比其新的版本）
lite snapshot delete <快照ID>        # 删除版本快照（仅最新版可删）
lite buff upsert character 今汐 新增益 --zone atkPct=12 --scope self   # 录入/更新 Buff
lite buff delete character 今汐 新增益
lite announcement create 标题 内容
lite cleanup                         # 手动清理过期工程（服务器每 5 分钟自动执行）
lite wipe [--force]                  # 清空全部本地数据（仅本机 localhost；二次确认，--force 跳过）
lite reset-templates [--tui|--web]   # 恢复默认 TUI 脚本 / Web 页面（覆盖用户修改）
lite sql [--path <文件>] [--db <仓库>] # 直接对本机数据库执行 SQL（本机可用，管理员可写）
```

### SQL 命令（`lite sql`）

直接对本机 SQLite 数据库执行语句，方便紧急维护 / 批量导入：

- `lite sql --path 脚本.sql [--db <仓库>]`：执行 SQL 文件（原生 SQLite 语法）
- `lite sql --db <仓库>`：进入交互式输入缓冲区，多行粘贴 SQL，**空行结束执行**
- 仅本机（localhost）可用，本机即 root_admin（管理员）可执行任意 SQL（`UPDATE`/`DELETE` 等）；非本机连接拒绝执行
- **自动识别 Buff 导出格式**：SQL 文本含 `::jsonb` 或 `public.buff_sets`（即 `/api/buff-sets/export` 的导出）时，
  自动走 Buff 集批量导入（等价于 TUI 的「SQL 导入 Buff 集」），无需手工转换 Postgres 语法

### 可定制界面（TUI 脚本 + Web 页面）

Web 页面（`index.html`）与 TUI 交互脚本（`tui-script.json`）**外置**在数据目录，缺省与数据库同一位置：

- **默认数据目录**：`~/.wuwa-afyg-share-lite/`；若 `serve --db` / `tui --db` 指定了其它仓库，则在该仓库的 db 同级目录
- 首次启动（`serve` 或 `tui`）自动从内置默认模板写入；**用户可直接编辑**这两个文件
- `lite wipe` 会**一并清除**外置的 `index.html` 与 `tui-script.json`（下次访问自动重建默认）
- `lite reset-templates [--tui|--web] [--db <仓库>]` 恢复默认模板（覆盖用户修改）

**TUI 脚本是 JSON，可定义每个菜单项在选中的动作**——直连 SQLite 跑 SQL、调用本服务 HTTP API、或执行本机命令：

```jsonc
{
  "title": "椰果工坊 Lite", "author": "root_admin",
  "menus": [
    { "label": "查看工程数", "action": { "type": "sql",
        "sql": "SELECT COUNT(*) AS 工程数, SUM(view_count) AS 总浏览 FROM projects" } },
    { "label": "按分享码查工程", "action": { "type": "sql",
        "sql": "SELECT title, author_name, code FROM projects WHERE code = {分享码}",
        "params": ["分享码"] } },
    { "label": "广场最新", "action": { "type": "http",
        "method": "GET", "path": "/api/public/projects?perPage=5" } },
    { "label": "打开网页", "action": { "type": "command", "cmd": "start http://localhost:3000" } },
    { "label": "退出", "exit": true }
  ]
}
```

- `action.type`：`sql` / `http` / `command` / `import-buff-sql`
  - `sql` 直接对项目 SQLite 数据库执行（author 为 root_admin 时管理员可写，否则只读），结果以表格打印
  - `http` 通过当前连接调用服务器（`method` + `path` + 可选 `body`）
  - `command` 执行本机命令（仅限本机 localhost 连接）
  - `import-buff-sql` 输入一个**导出格式 SQL 文件**（/api/buff-sets/export），批量导入 Buff 集
- `{占位名}` 或显式 `params` 里的变量，会在动作执行前提示输入，留空则忽略该占位
- `author` 若非 `root_admin`，动作交由该身份执行；sql / import-buff-sql 写操作仅 root_admin 可用

Buff 增益项（`--zone`）格式：`"zoneId=数值;ref=引用区;pct=比例;override"`，例如
`--zone "atkPct=10;ref=totalAtk;pct=0.05;override"`；
生效条件 `--condition '{"chain":3,"elements":["导电"]}'`（链 0-6 / 精炼 1-5 / 属性 / 伤害类型）。

> PowerShell 里给 `--condition` 传 JSON 会把双引号吃掉，请用反斜杠转义：
> `--condition '{\"chain\":3}'`，或改用 cmd。

---

## 四、HTTP API（供程序调用，与原版一致）

| 接口 | 说明 |
| ---- | ---- |
| `GET /api/public/projects` | 工程广场（`page` `perPage` `q` `sort` `excludeAnon`）→ `{projects, total, page, perPage}` |
| `POST /api/public/projects` | 匿名分享 `{fileText}` → `201 {code, url}`；限频 20 次/分钟 |
| `GET /api/buff-sets` | Buff 集（`entity_type` `entity_name` `q`）→ `{buffSets, total}` |
| `GET /api/buff-sets/export` | 全量 SQL 导出（优先快照） |
| `GET /share/{code}/download` | 下载工程 JSON |
| `GET /api/catalog` | 上游名录（角色/武器/声骸/套装 + version/stale/counts） |
| `GET /api/catalog/{characters\|weapons\|echoes\|sets}` | 单项名录 |
| `GET /api/announcements` | 公告 |
| `POST /api/auth/register` `login` `logout`；`GET /api/auth/me` | 本地账号（局域网/远程用） |
| `GET/POST/PATCH/DELETE /api/me/**`、`/api/admin/**` | 我的工程 / 管理员功能 |

带 `Authorization: Bearer <token>` 访问受保护接口；本机（回环地址）自动为 root_admin，无需 token。

---

## 五、多用户 / 局域网使用

服务器默认监听 `0.0.0.0`，局域网内其他机器可访问：

```bash
# 服务器上：注册账号（局域网连接需要登录）
lite register <用户名> --password <密码>     # 本机创建的账号是普通用户

# 本机 root_admin 授权
lite admin grant <用户名>

# 其他机器上：
lite --server http://<服务器IP>:3000 login <用户名> --password <密码>
lite --server http://<服务器IP>:3000 publish 工程.json
```

- token 保存在 `~/.wuwa-afyg-share-lite/config.json`；`logout` 清除
- 安全说明：免登录只对**回环地址**生效，非本机请求无法伪造（服务器按 TCP 对端 IP 判定并剥离该标记）

---

## 六、日常维护

```bash
lite cleanup                # 手动清理过期工程（服务端每 5 分钟自动做一次）
lite config get             # 查看配置（server/token/数据库路径）
# 备份 = 复制数据库文件（先停止服务器，或直接复制 WAL 模式下的 .db 文件）
```

常见问题：

- **端口被占用**：换端口 `lite serve --port 8080`，客户端 `lite --server http://localhost:8080 ...`
- **上游名录拿不到**：检查能否访问 `https://static.nanoka.cc`；首次成功获取后数据会缓存到 SQLite，
  断网时自动返回缓存（响应带 `stale: true`），也可用 `WUWA_AFYG_SHARE_WW_VERSION=3.6.7` 固定版本
- **TUI 需要真实终端**：在 PowerShell / Windows Terminal 等交互终端里运行；管道/重定向下会提示 `not a terminal`
- **数据库在哪**：`~/.wuwa-afyg-share-lite/share.db`（可用 `lite serve --db 自定义路径` 换位置）

---

## 七、与原版 wuwa-afyg-share 的差异

| 项 | 原版 | 本版 |
| ---- | ---- | ---- |
| 界面 | Next.js Web | TUI / CLI + 单文件原生 Web（Web Components，零构建） |
| 数据库 | Supabase（Postgres + RLS + Auth + pg_cron） | SQLite 单文件（RLS→服务层校验，pg_cron→定时任务，Auth→本地账号） |
| 登录 | GitHub / 邮箱魔法链接 | 用户名+密码；本机 localhost 免登录直接为 root_admin |
| AI 工具 / bilibili-toy | 有 | ❌ 移除 |
| Buff 集实体名录 | 手工录入 | 直接取自上游 nanoka.cc（与椰果工具箱同一数据源） |

业务规则与原版对齐：匿名分享 10 分钟有效、非匿名过期一周宽限、保护工程豁免删除与清理、
分享码 8 位、标题 60 / 简介 500 字符截断、快照根+版本链（差异现算）、管理员权限链连坐收回等。

技术栈一句话：Rust + rusqlite(bundled) + axum + clap + dialoguer + brotli，零外部运行时依赖。
