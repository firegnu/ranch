# fence

独立的 sandbox 命令；不依赖 corral、不读取它的状态。调用方通过 `check` 和 `run … --read/--write … -- <命令>` 接入。前端命令、JSON 与错误码见 `../../docs/CONTRACT.md`。

## 数据与来源

`data/{profiles,agents,services}/*.toml` 编进程序，构建脚本按目录收集；增加种类或模型服务只需增加数据。四家内置数据源于 `docs/调研/R17a-srt小试验.md`，按 `docs/调研/R17b-fence方案.md` 定稿。不是从其他仓库迁入的程序。

`$FENCE_HOME` 默认 `~/.fence`；用户在其 `profiles/`、`agents/`、`services/` 下放同名文件会整份替换内置数据。`local.toml` 是本机补充；工作区根目录的 `.fence.toml` 是仓库补充。规则来源可用 `fence profiles`、`fence check --profile dev --kind codex` 查看。

合并顺序：档案 → 种类 → 模型服务 → 仓库 → 本机 → 调用方。列表取并集；环境变量以后面的为准并报告覆盖。种类默认服务可由档案和本机的 `[services]` 表整份替换，本机优先；不猜各家配置。pi/omp 的 `dev` 默认服务是 DeepSeek。

| 键 | 可写来源 |
| --- | --- |
| `description` | 档案、种类、服务 |
| `deny_read` | 档案 |
| `read`、`write`、`domains` | 全部 |
| `deny_write`、`env` | 档案、种类、本机 |
| `mkdir` | 档案、仓库、本机 |
| `args` | 种类 |
| `services` | 种类（数组）；档案、本机（按种类的表） |
| `[macos]`、`[linux]` | 档案、种类、本机，含同来源允许的规则 |
| `allow_pty`、`weaker_network_isolation` | 只在 `[macos]` 内 |

路径只用具体绝对路径，不许 `* ? [`；`..` 按字面规整。写放行同时放行读。读取放行中的不存在路径会跳过，写放行保留。没有 Unix socket 配置键。Linux 的分支 ref 与 pi 锁放行整个父目录，待 R17c 实测。

所有 `deny_write`（包括强制保护的 FENCE_HOME、`.fence.toml`）保留原路径并补上真实目标，避免通过链接目标修改规则或 agent 配置。不存在的末级路径按已解析的父目录补全；断链、循环或其他解析错误拒绝启动。域名与端口在 `check/run` 共用的规则解析阶段校验，`invalid_data` 带来源与 `domains[索引]`，支持通配域名、带方括号的 IPv6 及 1–65535 的无前导零端口。

占位符用 `${home}`、`${cwd}`、`${worktree}`、`${repo}`、`${git_dir}`、`${git_common}`、`${branch}`、`${tmp}`、`${target}`；`${名字:json}` 生成 JSON/TOML 字符串字面量。Git 路径由 `git rev-parse` 取得，`${target}` 来自绝对路径的 `CARGO_TARGET_DIR`。路径引用缺值时跳过并列入 `skipped`；参数或环境变量引用缺值时失败。不在仓库里时 worktree/repo 都是 cwd。

主控 10-18 决定：`${tmp}` 为 `${XDG_CACHE_HOME:-~/.cache}/fence/tmp/<实例>`，在起 agent 前创建；运行记录放在 `$FENCE_HOME/runs/<实例>`。srt 的禁止写优先于放行写，因此临时文件移到规则目录之外，保留整个 `$FENCE_HOME` 的写保护，在 Linux 上也按整目录保护。每次启动清理已结束超过 7 天的运行记录及其中记下的临时目录；缓存根目录后来改变也能清理原路径。`.fence.toml` 始终禁止写。

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
