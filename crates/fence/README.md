# fence

独立的 sandbox 命令；不依赖 corral、不读取它的状态。调用方通过 `check` 和 `run … --read/--write … -- <命令>` 接入。前端命令、JSON 与错误码见 `../../docs/CONTRACT.md`。

## 数据与来源

`data/{profiles,agents,services,mcp}/*.toml` 编进程序，构建脚本按目录收集；增加种类、模型服务或 MCP 服务器只需增加数据。四家内置数据源于 `docs/调研/R17a-srt小试验.md`，按 `docs/调研/R17b-fence方案.md` 定稿。不是从其他仓库迁入的程序。

`$FENCE_HOME` 默认 `~/.fence`；用户在其 `profiles/`、`agents/`、`services/`、`mcp/` 下放同名文件会整份替换内置数据。`local.toml` 是本机补充；工作区根目录的 `.fence.toml` 是仓库补充。规则来源可用 `fence profiles`、`fence check --profile dev --kind codex` 查看。

合并顺序：档案 → 种类 → 模型服务 → 仓库 → 本机 → 调用方。列表取并集；环境变量以后面的为准并报告覆盖。种类默认服务可由档案和本机的 `[services]` 表整份替换，本机优先；不猜各家配置。pi/omp 的 `dev` 默认服务是 DeepSeek 和 opencode，分别放行 `api.deepseek.com` 和 `opencode.ai`。

对 paddock：内置服务和派活技能兼容新增，命令、参数、JSON 格式和退出码不变，paddock 不用改；新版规则随 fence 部署，诊断说明随 corral、corral-dispatch 技能部署。

R30 将 R29 的 MCP 接入改为数据：`dev` 的 `mcp = ["exa"]` 默认仍用免 key、有限频的 Exa，搜索用 `web_search_exa`，已知网址用 `web_fetch_exa`，目标网页由 Exa 读取。Exa 的地址只在 `data/mcp/exa.toml`；所选 URL 的域名自动放行（非默认端口一并保留），GitHub 和 Rust 的八个域名仍在 `dev.domains`。fence 程序没有服务商分支；四家 `[mcp]` 模板决定每台服务器的参数和文件，资源只为当前种类生成，位于 `${tmp}/mcp/<服务器名字>/`，不写全局配置。同名种类或档案仍整份替换，旧自定义种类需自行补 `[mcp]` 模板。

Codex 的 server 名为 `fence_exa_${run_nonce}`，避免 `-c` 与用户同名表递归合并、把旧认证字段发给 Exa；fence 不读取 Codex 配置文件。工具名为 `mcp__fence_exa_<随机值>__web_search_exa` 和 `mcp__fence_exa_<随机值>__web_fetch_exa`，应从当次工具目录发现。Claude、pi、omp 没有同类认证字段递归继承，名称保持原样；pi 用户配置中的同名 server 仍优先于扩展。

用户 10-20 取消推送限制：GitHub 正常放行，Claude 的钥匙串读取照旧；是否推送由任务书约束，fence 不保证阻止 GitHub 写操作。

接入细节：pi 模板先设空 `Authorization`，再叠加该服务器的认证头，用于跳过默认 OAuth 文件加载；`authorization` 不分大小写归一成 `Authorization`，避免大小写不同产生两个认证值。扩展文件须用 `.ts` 后缀。Exa URL 在数据里显式请求 `web_search_exa,web_fetch_exa`，四家共用；否则 omp 会把无额外工具的 Exa MCP 配置过滤掉。沿用各家原生 MCP 客户端，不做 fence 网关。

对 paddock：R30 保留命令、参数和退出码；`profiles` 输出兼容新增 `mcp` 列表，`check/run` 来源列表可出现 `source:"mcp"`。现有界面不用改，随 fence 部署对新运行生效。

## 怎么换成别的搜索服务商

使用提供 **Streamable HTTP MCP** 地址的服务商；普通搜索 REST API 不是 MCP。以下 `research` 和 `.invalid` 地址只是示例，需要替换为服务商提供的名字和地址。

1. 在墙外创建 `~/.fence/mcp/research.toml`（名字只用 ASCII 字母、数字、`_`、`-`）：

   ```toml
   description = "我的搜索服务"
   url = "https://research.example.invalid/mcp"
   [auth]
   header = "Authorization"
   prefix = "Bearer "
   ```

   `auth` 可省略；如果服务商要 `X-API-Key: <key>`，改成 `header = "X-API-Key"`、`prefix = ""`。不要把 key 写在 URL、此数据文件或 `local.toml` 中。

2. 在 `~/.fence/local.toml` **顶层、任何 `[表]` 之前**加或修改这一行，保留原有其他内容：

   ```toml
   mcp = ["research"]
   ```

   这一列表整份替换档案的选择，四家一起生效，不用复制 `dev` 或各家种类文件；`mcp = []` 关闭，删掉该行恢复档案默认值，多个名字可同时选择。URL 域名自动放行，不必再写 `domains`。

3. 要 key 才做：在墙外创建 `~/.fence/keys/research`，文件只放 key 本身（单行，可有末尾换行）；建议 `keys/` 权限 0700、文件 0600。文件名与服务器数据名相同，`FENCE_HOME` 被改时这些路径也相应改变。没有该文件就按免 key 连接；有文件但缺少 `auth.header`、key 为空或无效则启动失败。

4. `fence check --profile dev --kind codex` 可查看选中的 `mcp` 来源和域名；随后新启动的四家套 `dev` 墙的 agent 使用新服务。`check` 不读取 key，参数里的认证头为空，不能用它验证 key 或连接；key 只在 `run` 时读取并装配。

key 只支持不含 `$` 的普通 ASCII HTTP 请求头字面值；组合后的 `prefix + key` 拒绝任何 `$`（包括 `$VAR`、`${VAR}`、`$$`），也拒绝以 `!` 开头的命令求值、空 key 和非法控制字符。各家只在对应 server 配置里收到认证：Claude 的 `--mcp-config` 只带 0600 文件路径；Codex 的 `env_http_headers` 只引用本次、该服务器专用的 `FENCE_MCP_<随机值>_<序号>_<头序号>` 变量，真实值由 fence 通过进程环境传入；pi／omp 写 0600 临时文件。真实 key 不进启动 argv、shell 参数、`check` 输出或 fence 运行记录，Codex 专用环境也不进入可输出的规则 `env`。

源 key 目录及其**所有条目**的原路径、符号链接真实目标强制禁读，与当前 MCP 选择无关；改选服务商或 `mcp = []` 不会解除旧 key 的保护。枚举及链接解析只查路径元数据，只有 `run` 会读取选中服务器的 key 内容。**启动后 agent 能看到专用环境或临时配置中的 key，也能主动复制外发；这不防 agent 窃取 key。** 客户端自己的会话日志和重定向行为由客户端负责。

准备阶段失败时立即清理本次新建的实例记录及临时目录（包括已经写出的凭证文件）；不会删除启动前已有的路径。若清理本身失败，错误里的 `cleanup_errors` 列出需人工处理的路径。成功交给 runner 的运行，须同时留下 `run.json` 和 `exit.json`，才会在以后启动 fence 时按结束超过七天回收；异常退出缺少完整记录的残留不保证自动回收。

内置 Exa 的认证头为 `x-api-key`，默认没有 key 文件，仍免 key；接入字段依据 [Exa MCP 官方说明](https://exa.ai/docs/get-started/exa-mcp)。本轮只以假 key 验证装配，没有使用真实 key。

| 键 | 可写来源 |
| --- | --- |
| `description` | 档案、种类、服务 |
| `deny_read` | 档案 |
| `read`、`write`、`domains` | 全部 |
| `deny_write`、`env`、`unix_sockets` | 档案、种类、本机 |
| `mkdir` | 档案、仓库、本机 |
| `args` | 种类 |
| `services` | 种类（数组）；档案、本机（按种类的表） |
| `mcp` | 档案、本机顶层（服务器名字数组，本机整份替换）；种类顶层（接入模板表） |
| `[macos]`、`[linux]` | 档案、种类、本机，含同来源允许的规则 |
| `allow_pty`、`weaker_network_isolation` | 只在 `[macos]` 内 |

路径只用具体绝对路径，不许 `* ? [`；`..` 按字面规整。写放行同时放行读。读取放行中的不存在路径会跳过，写放行保留。`unix_sockets` 沿用下面的逐组件符号链接解析，只输出最终真实路径；macOS 映射为 srt 的 `allowUnixSockets`，同时放行 bind 和 connect，Linux 忽略并给出 warning，不使用 `allowAllUnixSockets`。内置 `dev` 只放行 `${tmp}`；此键不自动放行读写。Linux 的分支 ref 与 pi 锁放行整个父目录，待 R17c 实测。

所有 `deny_write`（包括强制保护的 FENCE_HOME、`.fence.toml`）保留原路径、逐组件解析途中每一个链接节点及最终真实目标，避免通过改写目标或替换中间链接修改规则和 agent 配置；支持多跳链接及中间目录链接。不存在的末级路径按已解析的父目录补全；链接自身的目标须存在，断链、循环或其他解析错误拒绝启动。域名与端口在 `check/run` 共用的规则解析阶段校验，`invalid_data` 带来源与 `domains[索引]`，支持通配域名、带方括号的 IPv6 及 1–65535 的无前导零端口。

占位符用 `${home}`、`${cwd}`、`${worktree}`、`${repo}`、`${git_dir}`、`${git_common}`、`${branch}`、`${tmp}`、`${target}`、`${run_nonce}`；`${名字:json}` 生成 JSON/TOML 字符串字面量。Git 路径由 `git rev-parse` 取得，`${target}` 来自绝对路径的 `CARGO_TARGET_DIR`。`${run_nonce}` 为每次 `run` 从系统随机源生成的 128 位随机值（32 位十六进制），同一次展开保持一致，`check` 显示 `<运行时才有>`。路径引用缺值时跳过并列入 `skipped`；参数或环境变量引用缺值时失败。不在仓库里时 worktree/repo 都是 cwd。

种类 `[mcp]` 允许 `args` 字符串数组、`[mcp.files]` 的「相对文件名 = 内容模板」，以及可选的 `headers_via_env` 布尔值（默认 false）。文件名不允许绝对路径、`.`、`..` 或占位符，每台服务器的文件都写到自己的 `${mcp_dir}` 下。模板额外提供 `${mcp_name}`、`${mcp_url}`、`${mcp_dir}`（字符串，嵌入 JSON 时用 `:json`）、`${mcp_headers_json}`、`${mcp_headers_toml}`（已序列化的对象，直接嵌入，不再加引号）。`headers_via_env = true` 另外生成 `${mcp_env_headers_toml}` 的「请求头名 → 专用环境变量名」映射，Codex 只将这一引用映射放进参数；自定义模板也不要将含真实值的 headers 占位符放进 `args`。逐服务器展开，不经 shell、不递归展开插入值。`mcp/` 服务器文件仅允许 `description`、`url`、`auth.header`、`auth.prefix`，不能声明其他放行规则。

主控 10-18 决定：`${tmp}` 为 `${XDG_CACHE_HOME:-~/.cache}/fence/tmp/<实例>`，在起 agent 前创建；运行记录放在 `$FENCE_HOME/runs/<实例>`。srt 的禁止写优先于放行写，因此临时文件移到规则目录之外，保留整个 `$FENCE_HOME` 的写保护，在 Linux 上也按整目录保护。每次启动清理已结束超过 7 天的运行记录及其中记下的临时目录；缓存根目录后来改变也能清理原路径。`.fence.toml` 始终禁止写。

`dev` 同时设 `TMPDIR=${tmp}`、`CLAUDE_CODE_TMPDIR=${tmp}`，让 srt 为四家设置的墙内 `TMPDIR` 都是各实例的临时目录；否则 srt 在没有 `CLAUDE_CODE_TMPDIR` 或旧名 `CLAUDE_TMPDIR` 时会改用 `/tmp/claude`。`TMPPREFIX=${tmp}/zsh` 供 zsh heredoc 使用。

Codex 的 macOS 数据默认开放 trustd（`enableWeakerNetworkIsolation`），削弱网络隔离，已获用户同意。允许访问的应用状态目录范围较宽，不提供不同会话之间的隔离。worktree 只放行当前分支的 Git 写路径；档案用 `mkdir` 在启动前准备当前分支 ref/reflog 的父目录，避免 packed refs 清掉斜杠分支父目录后无法提交。`packed-refs.lock` 未放行，提交时可能报拒绝但提交成功，打包 refs、删除已打包的分支等操作会失败。

## 运行与打包

`resources/runner.mjs` 是 srt 的 Node 启动代码：调用库接口、保留 agent 的终端、收集拒绝事件、收尾代理并透传退出状态。它不持有规则。日志不保存 srt 事件的 `command` 或 `encodedCommand`。文件事件只有实际收到后才确认 `monitors.files:true`，因为 srt 0.0.80 没有监视器健康接口；未收到时为 false，不能把没有日志当成没有拒绝。true 也不保证逐条完整：本机验收中 Codex/pi 的主仓库写入已在终端明确被拒，但该条未出现在 srt 的文件事件中。

Linux 上游的 `deny <syscall> <路径>` 写尝试归为 `file-write`，保留路径并确认 `monitors.files:true`；这是诊断提示，路径可能不准，不代表实际文件写入的完整审计。两平台事件格式用合成事件源运行原 runner 回归，不需要启动真实 agent。

`srt/package.json` 与锁文件固定 srt 0.0.80。`ranch-package` 在版本目录安装依赖，不运行 npm 安装脚本；记录当时 node 的绝对路径和版本，要求 node ≥ 22.12。运行时不按 PATH 改选 node。Linux 额外要求系统安装 bwrap、socat、rg。升级 srt 单列任务，不自动升级。

版本目录包含 `bin/fence`、`share/fence/{runner.mjs,runtime.toml}` 与 `lib/srt/node_modules/`。`FENCE_RUNNER` 仅供合成测试，直接替换启动程序，不建立真实 sandbox，生产环境不要设置。

## 验证

普通 `cargo test -p fence` 用隔离 HOME/FENCE_HOME、合成程序；不调用真实 agent。真实 srt 测试默认忽略：

```sh
CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r17b-fence \
FENCE_TEST_SRT=/absolute/npm-prefix \
cargo test -p fence --test srt -- --ignored --nocapture
```

`FENCE_TEST_SRT` 指向含 `node_modules/@anthropic-ai/sandbox-runtime` 的目录。测试不安装或改变系统设置。
