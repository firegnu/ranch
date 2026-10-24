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
  2. 写工作区外、读 HOME 里的私有文件、改 Kimi 的配置都被拦，记进 `fence denials`；
  3. 能用 Exa 查资料；
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
