# 任务：墙里允许 agent 在自己的临时目录建、连 unix socket；顺带让四家的 TMPDIR 都是自己的临时目录

10-19，ranch/main 交给 ranch/dev-r20（Codex，gpt-6-astra / high）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.35）、要、碰要害）
类型：功能变更
依据：paddock P5-142 实测（10-19），paddock/main 起草、经用户转给 ranch/main；ranch/main 查了 srt 0.0.80 的源码后定了做法（见「要做的」）。本轮改 fence 的数据、代码和 CONTRACT 里 fence 那一节；Linux 上不放开（见下）。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话
- “选1，先合并，需求写好转给ranch”。
- 用户选的第 1 条，paddock/main 当时的说法：“请 ranch 放行建 socket：只放行 agent 自己临时目录里的 unix socket（本机进程间通信，不连外网），墙基本不会因此变松。”

## 现象（paddock P5-142，fence `versions/eff7a40`，dev 档案）
- paddock 有 14 个测试要在临时目录里建 unix socket（`UnixListener::bind`，路径形如 `<TMPDIR>/pc-<16 位>/<16 位>.sock`、`<TMPDIR>/pctl-<16 位>/paddock-ctl/<16 位>.sock`），再由同一测试进程或它起的子进程连上去。
- 套 `--sandbox dev` 的 Claude Code（`TMPDIR=~/.cache/fence/tmp/<实例>`）和 Codex（`TMPDIR=/tmp/claude`）里，这 14 个都在 bind 时报 `Operation not permitted (os error 1)`；`fence denials` 记 `deny(1) network-bind <路径>`。目录能建能写，只是不许 bind。

## ranch/main 查到的（srt 0.0.80 源码）
- macOS：`network.allowUnixSockets` 是路径列表，每条按子路径同时放行 bind 和 connect（`macos-sandbox-utils.js` 生成 `network-bind (local unix-socket (subpath …))` 和 `network-outbound (remote unix-socket (subpath …))`），并会规范化路径（`/tmp` → `/private/tmp`）。
- Linux：`allowUnixSockets` 被忽略（seccomp 认不出路径），只能 `allowAllUnixSockets` 全开。
- srt 会改掉墙里的 `TMPDIR`：取 `CLAUDE_CODE_TMPDIR`（或旧名 `CLAUDE_TMPDIR`），都没有就用 `/tmp/claude`（`sandbox-utils.js`）。fence 只在 Claude 的数据里设了 `CLAUDE_CODE_TMPDIR=${tmp}`，所以 Codex、pi、omp 在墙里的 `TMPDIR` 实际是各墙共用的 `/tmp/claude`，不是 `dev` 档案想给的 `${tmp}`。

## 要做的
1. **四家的 `TMPDIR` 都是自己的 `${tmp}`**：让 srt 给出的 `TMPDIR` 对所有种类都是这个 agent 自己的 `${tmp}`，不再落到共用的 `/tmp/claude`（做法：`dev` 档案的 `env` 设 `CLAUDE_CODE_TMPDIR=${tmp}`，或同等效果；Claude 数据里那条可保留或并掉）。
2. **放行自己临时目录的 unix socket**：fence 数据加一个放行 unix socket 的键（名字由开发定，写进 CONTRACT），值为路径、可用占位符；`dev` 档案默认写 `${tmp}`。macOS 映射到 srt 的 `allowUnixSockets`，bind 和 connect 都放行，只限这些路径下。
   - 别处照旧拦：worktree、HOME、`/tmp` 其他位置（包括 `/tmp/claude`）、系统 socket 都不因此放开；对外网络规则不变；不用 `allowAllUnixSockets`。
   - 路径照现有写规则那样解析符号链接；只认具体路径、不许通配符。
   - 本机补充 `local.toml` 能用这个键；仓库补充 `.fence.toml` 不开放（paddock 只需要 `${tmp}`）。
3. **Linux 不放开**：Linux 上这个键不生效、不改成全开，`fence check` 在 `warnings` 里照实说明（如「Linux 不能按路径放行 unix socket，已忽略」）。Linux 实测仍留 R17c。
4. `fence check` 的 `settings` 里照实显示合并出的这项。
5. CONTRACT 写清：新键、默认值、平台差异、`TMPDIR` 的实际取值；都是兼容新增。对 paddock 的影响：不用改 `.fence.toml`；部署后 paddock/main 用 Claude Code、Codex 测试 agent 复验。DESIGN 有要改的在完成记录里写，不要自己改。

## 在哪里干活
- worktree：`../ranch-worktrees/r20-unix-socket`，分支 `r20-unix-socket`（已从 main 建好）。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r20-unix-socket`。
- 会动：`crates/fence/`、`docs/CONTRACT.md`。

## 怎么算做完
- 用户原话：“选1，先合并，需求写好转给ranch”。
- 用户 10-19 同意（“三条都按你的建议”）：Linux 先不放开；顺带让四家 `TMPDIR` 都是自己的 `${tmp}`；验收再加 paddock/main 提的那条——套 `--sandbox dev` 的 agent 在 `${tmp}` 下能 bind、connect unix socket，在 worktree、HOME 下照旧被拦；部署后 paddock/main 用 Claude Code、Codex 测试 agent 跑 paddock 全量测试复验（这一条在合并部署之后，不在本任务里做）。
- 验证只做这些：
  - 针对的测试，先写、确认它因为现在的问题失败（隔离的 HOME、FENCE_HOME，合成程序）：合并规则得出的 srt 设置（新键、`CLAUDE_CODE_TMPDIR` 或同等项、Linux 下忽略并给 warning）；边角：路径含符号链接、`/tmp` 与 `/private/tmp`、不存在的路径、`local.toml` 能加、`.fence.toml` 写了报 `invalid_data`。
  - 真 srt：加进 `crates/fence/tests/srt.rs` 那组 `#[ignore]` 测试，本机跑一次。用一个小的合成程序（Rust 测试里起子进程，或 `nc -U`、`python` 以外的现成工具），在墙里：`${tmp}` 下 bind 并 connect 成功；工作目录下、`/tmp/claude` 下 bind 被拦；`TMPDIR` 等于这个 agent 的 `${tmp}`（用 `--kind codex` 或 `--kind pi` 的数据验，不止 claude）。
  - 标准测试跑一次：`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。
  - 觉得不够，在回复里说，不要自己加。

## 不要做
- 不改 corral，不改技能，不改 paddock。
- 不用 `allowAllUnixSockets`，不在 Linux 上放开 unix socket，不放宽读写和网络规则。
- 不往真实的 `~/.fence/` 写东西（测试用隔离的 `FENCE_HOME`），不动用户的 `~/.fence/local.toml`。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`、`child-sandbox`，不 attach 打字。要起真实 agent 验证时，自己开 `ranch/test-r20-<名字>`（`--label role=test`、`--env CAIRN_DISABLE=1`），用完 `corral stop`。
- 不靠删测试、放宽断言、缩短超时来通过。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- srt 的 `allowUnixSockets` 实测和上面说的不一样（比如 bind 放行了 connect 没放行、路径规范化后对不上），停下来报告，不要自己换别的办法。
- 不合并到 main，不推送。只在 `r20-unix-socket` 上提交。

## 做完
在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：新增路径数组 `unix_sockets`，允许档案、种类和 `local.toml` 使用，仓库 `.fence.toml` 使用时报 `invalid_data`。复用写保护的逐组件符号链接解析，保留不存在的后缀，拒绝通配符、相对路径、断链和循环；macOS 输出 `network.allowUnixSockets`，Linux 不输出该设置并给 warning。`dev` 默认只放行 `${tmp}`，同时给四家设 `CLAUDE_CODE_TMPDIR=${tmp}`。更新了 CONTRACT 的 fence 一节及 fence README；paddock 不用改 `.fence.toml`。
- 验证了什么：先确认四家环境测试因 Codex 缺少 `CLAUDE_CODE_TMPDIR` 失败、默认设置测试因缺少 `allowUnixSockets` 失败、真实 srt 测试因 `${tmp}` 下 bind 报 EPERM 失败，再实现并通过。针对测试覆盖规则合并、符号链接、`/tmp` 与 `/private/tmp` 去重、不存在路径、local 可添加、repo 拒绝和两平台映射。新增的 macOS ignored 测试使用隔离 HOME/FENCE_HOME、pi 数据及 Rust 合成程序，实测 TMPDIR 等于实例临时目录，子目录内 bind、子进程 connect 和传输成功，worktree、HOME、`/tmp/claude` 下 bind 被拒；用只读的已安装 srt 0.0.80 依赖，未启动真实 agent。最终 `cargo test --all-targets` 为 158 passed、11 ignored，`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 及合成程序的 rustfmt 检查通过。所有命令前台执行并等到结束。
- 拿主意的地方：新键沿用现有路径数组与分层并集机制，不自动放行文件读写。Linux 的 settings 只显示实际生效规则，省略 socket 放行项并用 warnings 解释。保留 Claude 种类原有的 `CLAUDE_CODE_TMPDIR`，避免改变用户整份替换 dev 档案时 Claude 的既有行为。
- 没做的事：未改 corral、技能、paddock 或用户配置，未部署、合并、推送；未跑 Linux 真实 sandbox（留 R17c）或部署后的 paddock 全量测试（由 paddock/main 用 Claude Code、Codex 复验）。未改 DESIGN；建议主控在 §8 补充 R20 的默认 `${tmp}` unix socket 放行、Linux 忽略，以及四家墙内 TMPDIR 的实际取值。
- 踩过的坑：最初环境测试没准备种类程序，先报 `program_not_found`；补了隔离 PATH 下的合成程序后才取得有效 RED。真实 srt 中 bind/connect 已成功，但探针额外设置读取超时返回 EINVAL；改为等待客户端写完退出后非阻塞读取，未改 sandbox 规则或放宽断言。首次全量检查发现既有四家设置快照尚未包含新项，精确补上 macOS `allowUnixSockets` 预期后全量重跑通过。
- 要主控决定的事：没有新增实现决策；交主控审查，并同步上述 DESIGN 内容、安排合并部署后的 paddock 复验。
