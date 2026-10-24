# 任务：R48f Kimi Code 进 fence 的 sandbox（Kimi 阶段 3）

2026-10-24，ranch/main 交给 ranch/dev-r48f（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.85）、要、碰要害）
类型：功能变更
依据：让 Kimi 和 Claude Code、Codex、pi、omp 一样能套 `dev` 墙跑、接 Exa 查资料、派活时能套墙；不做 Linux，不改用户的 Kimi 配置。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent（下面写明的测试 agent 除外）。

## 用户原话

- 10-24：「1: 我觉得要做到现在已经接入的agent的所有功能（但是可以分阶段做，有几个阶段你要给我列一下）……」。主控列的阶段：0 补测 → 1 基本接入 → 2 派活 → 3 sandbox → 4 Slack 按钮（可选）。0、1、2 已做完（R48b、R48c、R48d，R48e 补了阶段 1 的两条建议）。
- 10-24：「阶段3和阶段4新建两个draft task」；「先做 R48f」。
- 10-24，主控问三件事，用户都选了推荐：验收照下面「怎么算做完」五条；真 Kimi 实测给少量几轮（约 5–10 轮短任务），超了先停下来问；查资料 MCP 不改全局配置就注入不进去时，停下来问。

## 先读

- `docs/DESIGN.md` §8 sandbox 全节（R17b 边界、R18 `child-sandbox`、R19、R20、R29、R30）。
- `crates/fence/README.md`；`crates/fence/data/agents/*.toml`、`profiles/dev.toml`、`services/`、`mcp/`：四家怎么写的。
- `docs/调研/R48-接入Kimi-Code.md` §6（Kimi 的域名、路径摸底，都是推测）；`docs/调研/R48b-Kimi-Code补测.md`（Kimi 的配置、钩子、`KIMI_CODE_TRUST_WORKSPACE`）。
- `docs/CONTRACT.md` 里 fence、`--sandbox`、`child-sandbox` 和 Kimi 的部分。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r48f`，分支 `r48f`（已从 main 建好）。只动 `crates/fence/`、`crates/corral/` 里和 Kimi 套墙直接有关的部分（如果要改）、两份技能里派 Kimi 那段（`crates/dispatch/resources/corral-dispatch/SKILL.md`、`crates/corral/resources/SKILL.md`）、`docs/DESIGN.md`、`docs/CONTRACT.md`，以及本任务文件末尾的记录。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r48f`。实测用这里编出的 corral、fence（`CORRAL_FENCE` 指向新 fence），不用 PATH 上装好的。

## 要做的

让 Kimi 和其他四家一样能套 `dev` 墙跑：

1. 写 Kimi 的 fence 种类数据（必要时加模型服务数据），`corral start --sandbox dev … -- kimi` 能开、能正常干活；登录身份、用户的模型和配置照常用，墙里改不了 Kimi 的配置和凭证。Kimi 需要的域名、路径按真 Kimi 实测定最小放行，不照推测整片放开。
2. 查资料接上 Exa MCP（和四家一样走 `dev` 的 `mcp` 选择、种类数据的 `[mcp]` 模板），不改用户的 Kimi 全局配置。
3. 被拦的读写、网络进 `fence denials`；套墙的 Kimi 在墙里开不了别的 agent（和四家一样）。
4. 派活：主控设了 `child-sandbox` 后派 Kimi 会套墙；技能里派 Kimi 那段、DESIGN §8、CONTRACT 跟着写，写明对 paddock 的影响。

## 怎么算做完

- 用真 Kimi `corral start --sandbox dev`：
  1. 能登录、用用户的默认模型完成一个小改动（写工作区里的文件）；
  2. 写工作区外、读 HOME 里的私有文件、改 Kimi 的 config.toml 都被拦，记进 `fence denials`（用户 10-24 改用 config.toml，不要求凭证禁写）；
  3. 能用 Kimi 自带的搜索/读网页查资料（用户 10-24 决定不注入 Exa）；
  4. 墙里开不了别的 agent；
  5. 主控设了 `child-sandbox dev` 后派 Kimi 会套墙。
- 验证只做这些：上面五条用真 Kimi 实测（记下每条怎么看出来的）；fence 数据和改动的代码各有针对的测试（照四家已有测试的样子，先写测试确认现在失败）；`cargo test --all-targets --no-fail-fast`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次。可以加边角用例：Kimi 写自己的会话、日志目录；`server.token`、credentials 是否必须可读。觉得不够，在回复里说，不要自己加。

## 真 Kimi 实测的边界

- 额度：少量几轮，约 5–10 轮短任务（每轮一两句话的小改动或一次查资料），超了先停下来问主控。
- 测试 agent 名字用 `ranch/test-r48f-<用途>`，带 `--label role=test`，cwd 用 worktree 外的临时目录（一个临时 git 仓库），用完 `corral stop`。用隔离的 `CORRAL_HOME`。
- Kimi 用用户的真实 `~/.kimi-code`（登录态要用到），不改任何配置和凭证；不读、不往仓库里抄凭证、token、`device_id`、`server.token` 的内容。不指定模型。信任框用 `KIMI_CODE_TRUST_WORKSPACE=1`。
- fence 可以用用户真实的 `~/.fence`（运行记录照常写进 `runs/`），但不改 `~/.fence/local.toml`、`~/.fence/` 下的档案、种类和 keys。Kimi 如果因为本机的 IP Guardian 之类被拦，停下来问，不改 `local.toml`。
- 测「读 HOME 私有文件被拦」时用自己建的诱饵文件（放 HOME 下一个临时文件，用完删），不去读用户真实的私密文件。

## 不要做

- 不做 Linux（5090），沿用 R17c 暂缓。
- 不改别家的种类数据和 `dev` 档案已有的放行；Kimi 需要的域名加在 Kimi 自己的数据或模型服务里。
- 不改用户的 `~/.kimi-code`、`~/.fence/local.toml`、`~/.corral`、`~/.ranch`、`~/.pony`。
- 不改 PATH 上装好的 `corral`、`ranch`、`fence`，不打包、不部署，不装技能，不切共用 helper。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。只操作自己开的 `ranch/test-r48f-*`。
- 不按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f kimi` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉；用户自己可能也开着 kimi。停自己起的进程用记下的 PID。
- 不放宽现有测试的断言、不缩短超时、不删测试来过。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。包括：查资料 MCP 不改全局配置就注入不进去；Kimi 要放行的东西明显比四家多（例如整个 `~/.kimi-code` 可写、要读 HOME 别处）；要改 fence 的 Rust 代码才能支持 Kimi；额度不够。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r48f` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么（五条各怎么看出来的、真跑了几轮）、Kimi 最终放行了哪些域名和路径及理由、拿主意的地方、没做的事、对 paddock 的影响、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录（本轮停在 MCP 注入决策，任务未完成）

2026-10-24，ranch/dev-r48f。

- **做了什么**：读完任务要求的设计、约定、四家 fence 数据及 R48/R48b 调研，核对本机 Kimi 与固定版本源码。尚未改功能代码、种类数据或技能；本次提交只保存停点。
- **验证了什么**：用临时 `KIMI_CODE_HOME`，关闭自动更新及遥测，前台运行本机 `kimi --version`、`--help`，版本仍是 **2.3.1**；`kimi --mcp-config <临时路径>` 在参数解析阶段退出 **1**，原文为 `error: unknown option '--mcp-config'`，未启动模型。这是能力核对，不是功能测试的 RED。真账号输入 **0 轮**，未开测试 agent；五条真 Kimi 验收（默认模型写文件、越界访问和 denials、Exa、墙内禁止派活、继承 child-sandbox）均未开始。没有实现变更，未添加行为测试，也未消耗任务规定的全量 test、clippy、fmt 各一次验证。
- **阻塞依据**：沿用 R48 固定的官方提交 `684bae9ea6a437f3bae98bba1dcb797a917d500b`。[CLI 参数定义](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/apps/kimi-code/src/cli/commands.ts) 没有一次性 MCP 配置或插件目录参数；[MCP 配置加载器](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/mcpConfig/configLoader.ts) 的 `resolveMcpJsonPaths` 固定读取 `$KIMI_CODE_HOME/mcp.json`、Git 工作区根的 `.mcp.json`、cwd 下的 `.kimi-code/mcp.json`；[注册服务](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/mcpRegistry/mcpRegistryService.ts) 还合入已安装插件。`KIMI_CODE_HOME` 同时决定配置与状态根，不能仅用它改 MCP 路径并保持当前账号和配置。现有 fence `[mcp.files]` 只在 `${tmp}/mcp/<名字>/` 写资源，文件名不能绝对路径或含 `..`，`[mcp.args]` 也没有可传给 Kimi 的对应入口。因此尚未找到符合本任务边界、仅靠种类模板的注入方式；不能把另写项目级配置或重建 Kimi 数据根当成现有模板机制直接实施。
- **最终放行**：尚未确定或新增任何域名、路径；没有照 R48 的候选清单直接放开。未读取用户配置或 credentials、token、device_id、server.token 的内容，未修改用户目录、全局配置或已装程序。
- **要主控决定**：任务书明确要求遇到「查资料 MCP 不改全局配置就注入不进去」或「要改 fence 的 Rust 代码才能支持 Kimi」时停下。本轮没有自行选择替代办法。选项 A：允许另行设计项目级 MCP 临时注入，并明确授权所需 fence Rust 改动；须先定已有配置、同目录并发与异常退出后的清理方式。选项 B：保持当前实现边界，暂停 R48f，等 Kimi 支持一次性 MCP 配置入口。**建议 A 先做方案评审再实施**，继续保留不改用户全局配置、不复制凭证的限制；不能直接覆写项目已有 MCP 文件。主控尚未决定。
- **没做的事**：尚未实现 Kimi 套墙或完成五项验收；没有改变 DESIGN/CONTRACT 对已支持能力的描述，没有跑 Linux、安装、部署、合并或推送。临时目录只用于下载固定源码和无账号 CLI 参数核对，未留下运行中的命令或测试 agent。
- **对 paddock 的影响**：本次只有任务记录，paddock 不用动；Kimi sandbox 仍未支持。
- **踩过的坑**：不能照抄 Claude 的 `--mcp-config`，Kimi 会在解析阶段拒绝。Kimi 的 MCP 文档只列用户和 cwd 两层，固定版本源码另读 Git 工作区根的 `.mcp.json`；即便有项目级入口，也不等于 fence 已支持把临时配置注入其中。

### 主控决定（用户 10-24 定，恢复实施）

不注入 Exa；Kimi 种类不写 `[mcp]`，墙内查资料用 Kimi 自带的服务端搜索、读网页。验收第 3 条改为「能用 Kimi 自带的搜索/读网页查资料」，其余四条不变。域名按真实运行确认，仅加在 Kimi 种类或模型服务，不改 `dev` 和别家数据，不开放任意互联网。允许为没有 `[mcp]` 的种类跳过 MCP 做最小 fence Rust 改动并给出说明、补测试；更大的改动仍须停下问主控。DESIGN §8、CONTRACT 记录不走 Exa 的原因和上述源码依据；额度及其他边界照原任务。

### 继续实施记录（停在 Kimi 工作区原子写，任务仍未完成）

- **已做**：`rules.rs` 对明确指定但没有 `[mcp]` 的种类跳过所选 MCP，给 `mcp_skipped` warning，不生成 MCP 参数／文件、不额外开放其域名、不列生效来源；所有 key 源及链接目标的禁读仍保留，无种类的档案检查仍预览 MCP。新增 Kimi 种类及模型服务的**试验草案**，只用于找最小路径；DESIGN §8、CONTRACT 记了不接 Exa 的决定及未验收状态。没有改 `dev` 和别家数据，也没有改 corral 或派活技能。
- **RED → GREEN**：先加公开 `fence check` 测试，旧实现因仍出现 `mcp.exa.ai` 而失败；最小实现后 fence CLI 23 项通过，含四家 MCP 的默认接入、切换和认证回归。再加 Kimi 配置保护测试，先实收 `unsupported_kind`，加入草案数据后通过。最终 `cargo test -p fence --test cli` **24 项全部通过**，`git diff --check` 通过。编译 `cargo build -p corral-core -p fence` 成功；只运行了针对测试，任务规定的全量 test、clippy、fmt check 各一次仍留到完成实施后。未删测试或放宽断言；原自定义无模板种类的域名精确断言随新约定去掉 Exa，已有四家断言不变。
- **真跑账**：仅启动一次 `ranch/test-r48f-bootstrap`，实例 `b657b96a0817`，带 `role=test`、`KIMI_CODE_TRUST_WORKSPACE=1`、`CAIRN_DISABLE=1`，使用 `.target/r48f/debug/{corral,fence}`；`CORRAL_FENCE` 指向新 fence。cwd 为 worktree 外的临时 Git 仓库，`CORRAL_HOME` 隔离，使用真实 Kimi home 和现有 `~/.fence`。**模型用户输入 0 轮**，没有到编辑器或调用模型；不指定模型。测试 agent 已 `stop`，`exit_code:-15`、`stopped_by:SIGTERM`，隔离 `corral ls` 为空，fence 记录 `running:false`。`config.toml` 和 `tui.toml` 前后 SHA-256 相同；未读取凭证、token、device_id、server.token 内容。
- **五项验收进度**：① `start --sandbox dev` 接受 Kimi，但停在信任画面 `Could not inspect project settings.`，尚不能完成小改动；② 启动中已有读写拒绝进入 `fence denials`（结束时 file-read 20、file-write 6、network 0），但尚未运行三种指定诱饵操作，不能算验收通过；③ 自带搜索／读网页未测；④ 墙内开 agent 未测；⑤ `child-sandbox dev` 继承未测。
- **新阻塞及源码**：启动实收 `workspaces.json.tmp.<PID>.<随机值>` 的 `file-write-create` 拒绝。固定版本的 [sdk-rpc-client-v2.ts `getWorkspaceTrustInfo`](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/node-sdk/src/sdk-rpc-client-v2.ts#L699) 先创建工作区实例再读取信任状态；[workspaceInstanceManagerService.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/workspace/workspaceInstance/workspaceInstanceManagerService.ts#L131) 调 `createOrTouch`，[workspaceService.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/workspace/workspaceService.ts#L61) 无论新建还是已有工作区都保存目录；[fileWorkspacePersistence.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/workspace/fileWorkspacePersistence.ts) 固定写 home 根的 `workspaces.json`；[fs.ts `atomicWrite`](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/_base/utils/fs.ts#L74) 必须先创建同目录的 `.tmp.<PID>.<随机值>` 再 rename。只放行 `workspaces.json` 不够，`TMPDIR` 不改变这个同目录临时文件；当前 fence 只接受具体路径，不能列出未来随机文件，也不支持通配规则。检查失败被 TUI 捕获为不可信并显示上述警告，信任环境变量无法绕过之前的写失败。另观察到 `.agents/skills` watcher 和 Kimi 原生扩展缓存被拒，尚未为它们扩大权限。
- **待主控决定**：这是原任务明确要求停下的「可能要整个 `~/.kimi-code` 可写」／超出已授权 MCP 小改的路径规则问题。A：另行设计并授权只允许所需原子写临时文件的窄规则，涉及 fence 路径机制，需要先评审；B：授权 `.kimi-code` 父目录可写、另严禁配置凭证等路径写入，范围较宽且需重新审查；C：保持当前边界暂停。**建议 A**，本轮没有执行任一替代方案，也没有替 Kimi 点信任框。
- **放行状态及未做**：没有最终 allowlist。草案仅列原调研见过的 sessions、logs、cache、user-history、session_index.jsonl、workspaces.json 写路径，以及 config.toml、tui.toml、credentials/kimi-code.json、region 读路径；配置及 credentials 禁写。Kimi 模型服务 `domains=[]`，尚未走到网络确认阶段，没有放开任意互联网，也没有放开 server.token 或整个 Kimi home。没有扩展路径规则，没有改本机配置、装技能、部署、合并或推送。paddock 不用动；该草案不能视为 Kimi 已能正常套墙。
- **踩过的坑**：Kimi 在信任检查前就写工作区目录，因原子写临时文件被拒仍会弹信任框；这不是缺少 `KIMI_CODE_TRUST_WORKSPACE`。另为真 srt 实测在独立编译目录临时放了 runner/runtime 和现装 srt 的只读链接，导致已有「没有打包 runtime」测试不再报 `runtime_missing`；撤掉这些实测辅助文件后 24 项通过，未改断言。源码下载、临时仓库和隔离 corral 目录在记录后清理，fence 运行记录按其正常留存规则保留。

### 主控决定（用户 10-24 定，选择 B）

Kimi 种类照 Claude Code、Codex：`write` 收成整个 `${home}/.kimi-code`，`deny_write` 保护 config.toml、tui.toml、credentials 目录，以及按源码判断的其他配置和身份文件（如 region、server.token、device_id）；必须可写的说明理由。不改 fence Rust 路径机制，不加临时文件通配规则。继续五项验收，墙里确认保护文件改不了并记进 denials；额度和其他边界不变。

### 选择 B 后的实施记录（停在凭证刷新，任务仍未完成）

- **已做**：Kimi 的零散 `read/write` 收成 `write = ["${home}/.kimi-code"]`（写放行包含读），不改路径机制；`deny_write` 包含 config.toml、tui.toml、credentials、region、server.token、device_id、mcp.json、plugins、AGENTS.md、skills、agents、workspace-trust、bin。前六项为用户配置、账号或本机身份；MCP／插件／指令／技能／agent 定义及信任记录也属配置；bin 包括已装的 Kimi 和托管工具，不允许墙内更新。依据固定版本 [data-locations.md](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/docs/en/configuration/data-locations.md)、`packages/oauth/src/{region,identity,storage}.ts`、`packages/kap-server/src/services/auth/persistentToken.ts`。未改用户文件，也未读取身份和凭证内容。
- **保留写权限的理由**：sessions、session_index.jsonl、workspaces.json（含同目录随机临时文件）、user-history、logs、cache、file-history、search-index 是会话及索引运行数据；locks 是会话占用锁；oauth 是刷新互斥锁及空 sentinel，真正 token 在 credentials 下，仍禁写。这里不提供不同 Kimi 会话之间的隔离。没有把整个 `.kimi-code` 改成只读，也没有放宽配置／身份文件保护。
- **处理上一轮附带拒绝**：通过已有环境入口将原生缓存移到本次 `${tmp}/kimi-cache`，设置 `KIMI_CODE_CACHE_DIR`；`KIMI_CODE_WATCH=0` 只在墙内关文件 watcher，避免 `.agents/skills` watch 的 EPERM。源码分别为 `apps/kimi-code/src/native/native-assets.ts:getNativeCacheBase`、`packages/agent-core-v2/src/human/utils/watch.ts:isWatchEnabled`。未放行 `~/Library/Caches`、`.agents` 或 shell 配置。自动更新、遥测继续仅在这次运行关闭。
- **域名实测**：零 Kimi 域名的实例 `585a96c9cc41` 已进入正常编辑器，显示用户默认 **K2.8 Preview / max**；没有送模型输入。`fence denials` 实收 `api.kimi.com:443`、`auth.kimi.com:443` 未在 allow list，共 network 5。仅将这两个域名加入 `data/services/kimi.toml` 后重开，实例 `68865097dbb4` 的 network 拒绝为 0，并确实进入 OAuth 刷新保存步骤。没有开放 .ai、CDN、遥测或任意互联网，也没有改变 `dev` 或四家数据。模型／搜索端到端尚未跑通，不能把这当最终网络验收。
- **验证和额度**：配置保护测试先实收新增 region 等保护缺失的 RED，修改后通过；域名断言也先在未放行时 RED，加入实测域名后 GREEN。`cargo build -p corral-core -p fence` 成功，最新 fence CLI **24 项全部通过**，`git diff --check` 通过。全量 test、clippy、fmt check 各一次仍未跑，留待实施完成。本轮两次启动同名 `ranch/test-r48f-native`（均 `role=test`），只有第二次 `--prompt` 提交了「创建 hello.txt 并回读」的一条短任务；**累计用户模型输入 1 轮，因认证失败未完成**，此前启动共 0 轮输入。没有额外花额度试替代登录方式。
- **五项验收**：① 启动、信任、编辑器已通过，但首轮认证失败，hello.txt 未完成；② 本轮确实拦住 credentials 目录 chmod 和 `credentials/kimi-code.json.tmp.<PID>.<随机值>` 创建，已进 denials，但指定的工作区外写／HOME 诱饵读／配置禁写整组操作仍未执行；③ 自带搜索／读网页未测；④ 墙内禁止派活未测；⑤ child-sandbox 继承未测。`config.toml`、`tui.toml` 校验和前后相同。两次测试均已 stop（SIGTERM），最后隔离 `corral ls` 为空；等待命令也已结束，无遗留测试 agent。
- **新阻塞**：Kimi 首轮显示 `Skipped refreshing managed:kimi-code: EPERM` 及 `[internal] EPERM`，对象正是受保护 credentials 下的临时文件。固定版本 [oauth-manager.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/oauth/src/oauth-manager.ts#L364) 的刷新流程先 `refreshImpl`，再 `storage.save`，保存成功才返回 access token；[storage.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/oauth/src/storage.ts#L90) 必须创建 credentials 目录内的临时文件并 rename。目录禁写确实生效，也因此在需要刷新时无法继续使用模型。保存前已请求服务端刷新；是否影响原有登录态未验证，没有自行重试登录、复制凭证或放开写权限。
- **待主控决定**：A：保留凭证禁写，由用户在墙外先刷新登录，再在 token 有效期间继续验收；之后到期仍需墙外刷新，必须把这一限制写清。B：修改原验收边界，允许 credentials 写入以支持自动刷新；同一进程权限下不能只允许 Kimi 自己而禁止它的工具写入。C：保持当前状态暂停。**建议 A，先保留既定保护边界**；本轮没有替用户执行墙外刷新，也没有改掉 credentials 禁写。主控尚未决定。
- **没做／影响／踩坑**：不改 fence Rust 路径机制，不增加通配规则，不改其他仓库、用户全局配置、已装程序或技能，不部署、不合并、不推送。paddock 不用动，Kimi 数据仍标记试验草案。坑是“能到编辑器”不代表模型可用，自动刷新在首轮因凭证落盘被拒而失败；原生缓存和 watcher 则已用上述本次环境设置解决。临时源码、仓库、隔离 corral 目录和运行时辅助链接清理，fence 记录正常保留。

### 主控决定（用户 10-24 定，允许凭证自动续期）

从 `deny_write` 去掉 `${home}/.kimi-code/credentials`，允许墙内自动续期；config.toml、tui.toml 等其余配置和身份路径仍禁写。验收第 2 条用 config.toml 测配置禁写，不再要求凭证禁写。墙内本来就要读凭证，开放写入增加的是可能改坏登录的风险；用户接受，与 Claude Code 在墙内经钥匙串续期的使用目标一致。若上轮服务端刷新却未保存导致登录失效、Kimi 要求重新登录，停下告知主控，不自行登录、不碰凭证内容。之前已用 1 轮，累计额度仍为 5–10 轮短任务。

## 完成记录

2026-10-24，ranch/dev-r48f。前面的停点保留作为过程记录；最终权限与验收按上述三次主控决定执行。

- **做了什么**：新增 Kimi 种类和模型服务；未带 `[mcp]` 的种类跳过所选 MCP 并给 warning，保留所有 key 源的禁读。Kimi 使用自己的 WebSearch、FetchURL，不改用户配置、不注入 Exa。credentials 从禁写表移除，其余配置／身份保护保留。修复围栏内 Kimi 全局 hook 的父进程校验：只在 Kimi 且 `FENCE_RUN == CORRAL_INSTANCE` 时容许多一级 runner，macOS 用 `proc_pidinfo` 读取两级父 PID，仍过滤嵌套 Kimi。同步 DESIGN §8、CONTRACT、fence README 和两份技能的 Kimi 派活段落。
- **五条真实验收**：使用 Kimi 2.3.1、真实 srt 0.0.80 和本分支 debug corral/fence，`CORRAL_FENCE` 指向新 fence；所有 agent 带 `role=test`，cwd 在 worktree 外的临时 Git 仓库、CORRAL_HOME 隔离，不指定模型。

  | 项目 | 实测证据 |
  | --- | --- |
  | ① 现有登录、默认模型、小改动 | 放开凭证写入后未要求重新登录。实例 `b94c87402c8f` 用默认 K2.8 Preview / max 创建 `hello.txt`，墙外回读为 `R48F_OK\n`；后续实例均可正常调用模型。没有读取、复制或手改凭证。 |
  | ② 越界读写、配置禁写与 denials | 实例 `f98495fc6a6f` 由 Kimi 的 Bash 前台运行工作区 Rust 探针：读 HOME 自建诱饵、创建 HOME 另一个临时文件、以不截断的写方式打开 config.toml，均为 EPERM；`fence denials --agent ranch/test-r48f-native --instance f98495fc6a6f` 分别记下 `file-read-data`、`file-write-create`、`file-write-data`。tui.toml、region、server.token、device_id 的同样打开操作也返回 EPERM，不读其内容；这些快速连续拒绝没有全部出现在 srt 文件日志中，不把日志当完整审计。config.toml、tui.toml 前后 SHA-256 相同，诱饵内容未变、越界目标未创建。 |
  | ③ 自带搜索与读网页 | `f98495fc6a6f` 的真实终端显示 `Used WebSearch (Rust 2024 edition official release date)`，返回发布日期与 Rust 官方博客链接；`b5fd99c38b95` 的 `status.last_tool` 为 `FetchURL`、`wait` 返回 idle、`reply` 返回 Example Domain 标题及摘要。搜索／读网页未出现新增网络拒绝。此前同一围栏中的 curl 直连 example.com 为 HTTP 403，`denials` 明记 `example.com:443` 不在 allow list，证明未额外开放目标网页。 |
  | ④ 墙内不能再开 agent | 上述 Rust 探针执行新 corral 的 `start ranch/test-r48f-nested --no-sandbox --label role=test -- /bin/cat`，返回退出码 6、`error:sandbox`、`running inside fence`，没有创建嵌套 agent；探针总退出码 0。 |
  | ⑤ 继承 child-sandbox | 隔离主控 `ranch/test-r48f-controller`（`/bin/cat`，实例 `bb0116c149ae`）设 `child-sandbox dev`；以该主控的 CORRAL_NAME／CORRAL_INSTANCE 调 `start`，未带 `--sandbox` 或 `--no-sandbox`，新 Kimi `ranch/test-r48f-child` 实例 `6ee901baf6b1` 的 start、status 均为 `sandbox:{profile:dev}`，fence 记录 kind=kimi。它创建 `child.txt` 为 `CHILD_WALLED\n`，再前台运行同一边界探针，退出码 0；wait、reply 正常。 |

- **额度和收尾**：总计 **6 轮短任务**：第 1 轮为上次凭证保存被拒的失败；第 2 轮写 hello；第 3 轮边界探针；第 4 轮 WebSearch；第 5 轮 FetchURL；第 6 轮继承围栏后写 child 并复用探针。无额外模型调试轮。原 hook 失效时 send 因 starting 未提交，第 4 轮用测试 agent 的 keys 输入并提交一次；诊断退出用 `/exit`，不计模型轮。所有测试 agent 均已退出（native 退出码 0，其余按任务 stop），隔离 `corral ls` 为空，三个最终 fence 记录均 `running:false`；HOME 诱饵已删除。仅为定位 hook 临时替换隔离 CORRAL_HOME 的 helper 链接，已还原，未切用户共用 helper。实测用的 runner/runtime/srt 辅助链接已撤掉；fence 运行记录正常保留。
- **最终放行与理由**：Kimi 自己只新增 `api.kimi.com`、`auth.kimi.com`，由前轮零域名启动的拒绝记录确认；`dev` 既有 GitHub／Rust 域名不变，不加入任意互联网、CDN、遥测或目标网页域名。写路径只有 `${home}/.kimi-code` 一条，支持 `workspaces.json` 的同目录随机临时文件以及 sessions、session_index、user-history、logs、cache、file-history、search-index、locks、oauth 的运行数据；凭证目录按用户决定可写，供 OAuth 刷新保存。config.toml、tui.toml、region、server.token、device_id、mcp.json、plugins、AGENTS.md、skills、agents、workspace-trust、bin 禁写，理由及源码位置见前述 B 记录。原生缓存移到本次 `${tmp}/kimi-cache`，仅本次禁用 watcher、自动更新和遥测；不额外读 HOME 技能、shell 配置或 Library 缓存，不提供会话间隔离。
- **拿主意的地方**：完整执行三次主控决定：无一次性 MCP 入口，所以不接 Exa；选择 B 开整个 Kimi 根目录写并保护配置／身份；为自动续期放开 credentials 写入。墙内本来就能读凭证，增加的风险是可改坏登录，用户已接受；Claude Code 墙内也能经钥匙串续期。fence Rust 只加已获准的缺模板跳过逻辑，没有改路径机制、进程权限或加通配规则。Kimi hook 的小改位于任务已允许的 corral 范围，其他家行为不变，没有新增待主控决定的事项。
- **针对验证**：fence 种类、无模板 MCP 跳过与 key 源保护均先 RED 后 GREEN，四家原有 MCP 回归通过；credentials 可写断言在仍禁写时先失败，再移除禁写后通过。hook 最初以多一级 runner 的合成测试复现 starting；真机进一步发现 macOS `/bin/ps` 执行被拒，用 `sandbox-exec` 明确禁止该命令补强测试，实收 RED 后改用系统接口，再 GREEN。最新 corral 针对测试为 Kimi 10 通过（1 个原有合成子进程 fixture 按原样 ignored）、sandbox 16 通过，包含围栏主会话落事件、嵌套会话无事件与 hook 空输出。没有缩短现有超时、删除测试或放宽断言。
- **全量检查**：按任务各跑一次，均在前台结束、退出码 0：`cargo test --all-targets --no-fail-fast` 共 **477 通过、0 失败、16 按原设定 ignored**；`cargo clippy --all-targets -- -D warnings` 零警告；`cargo fmt --check` 通过。均使用独立 `.target/r48f`；`git diff --check` 也通过。自查没有越过任务允许的文件范围，没有留下诊断代码、临时探针或运行中的测试 agent。
- **没做的事**：未做 Linux；未改其他家的数据、dev 档案、fence 路径机制、用户 Kimi 配置、fence 本机配置或 keys；未读取凭证、token、device_id、server.token 内容。凭证写入仅由 Kimi 运行时按用户授权自动续期。未改其他仓库、安装技能、打包、部署、合并或推送；只在 r48f 提交。
- **对 paddock 的影响**：新增可用的 Kimi sandbox 种类、模型服务和技能说明，保持命令、参数、JSON 字段和退出码兼容；paddock 不用动。需由后续部署新 corral/fence 才在用户日常启动生效，本任务没有部署。
- **踩过的坑**：工作区索引必须在根目录原子写，零散文件放行不够；只读 credentials 能进入编辑器却在首轮自动续期保存时失败；macOS 的 setuid `/bin/ps` 在墙内无法执行，单纯补 runner 层数的合成测试不足，加入实际拒绝条件后才锁住 bug。原生缓存／watcher 已用官方环境入口处理。实测的开发目录 runtime 会干扰已有 `runtime_missing` 测试，已先撤掉再跑全量检查；没有改变断言来容纳实测环境。
- **审查后补文档（用户 10-24 定）**：按交叉审查的三条「建议改」补清所有 Kimi 会话可读写及墙外恢复风险、身份与跨项目数据的读暴露和种类不支持 `deny_read` 的限制，并修正有 `[mcp]` 模板的种类及自定义无模板种类的域名约定，仅改文字，验证只运行 `git diff --check`，未重跑全套。
