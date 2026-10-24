# R48：Kimi Code 接入 corral 摸底

2026-10-24，ranch/dev-r48。只调研，不含适配器实现、安装或部署。

## 结论先说

1. **【查证】能启动、能从终端送话，有可利用的事件和会话原文，但不能直接照搬 amp 的接法。** 本机 `kimi` 是 2.3.1；新交互窗口收到第一句话才创建 session，`-p/--prompt` 则是执行完退出的非交互模式。现有 corral 尚不认识它，实测状态为 `unknown`。首句触发、启动就绪和正常收发需要另定接法（§2、§4，实测 T1–T4）。
2. **【查证】没有查到“沿用现有账号配置、只为本次附加 hooks”的 CLI 参数。** `KIMI_CODE_HOME` 能整体换数据根，但配置、登录、会话也一起换。原生 hooks 在 `config.toml`；插件 hooks 还要登记共享的 `plugins/installed.json`。现有 `Global` 只安装自己拥有的文件/目录，不能直接安全地改共享注册表（§2，S2、S3）。
3. **【查证】不是所有等待和 Esc 都相同。** 有 `PermissionRequest`、`Interrupt` 等 hooks；提问框另走 question interaction。真实提问框按第一次 Esc 只是取消问题，模型继续；再在运行中 Esc，主会话才留下 `turn.ended reason=cancelled`。不能看见 Esc 就断言整轮被打断（§3，T3）。
4. **【查证】现有 `~/.agents/skills` 可以共用，不必再装一份技能。** 默认不扫描 `~/.claude/skills`；额外传 `--skills-dir` 会替换自动发现目录（§5，S4）。
5. **【推测】可以设计 fence 支持，但本轮不能称为“已能套墙”，也不足以把 Kimi 放进默认分工表。** 网络、登录刷新、文件写入、子进程退出和 MCP 注入还需正式验收；四次短输入不是编码能力评测（§6、§7）。
6. **【查证】真实运行有超出“仅留几条会话”的自动写入。** 没有手工编辑用户配置，但运行期间 `config.toml` 校验和改变，凭证文件的 mtime 也改变。发现后停止真跑、清理测试进程，没有回写用户文件。后续是否允许这种自动刷新，需要主控交用户定（§1、末节）。

标记：**【查证】** 本机命令、实测或所列版本源码直接支持；源码支持不等于实际触发已经测过。**【推测】** 根据证据提出的接入判断。**【未知】** 本轮未验证。以下路径中的 `~` 指运行该程序的用户 HOME；不包含任何凭证、token 或设备标识内容。

## 依据与实测账

**【查证】版本边界：** 本机 `~/.kimi-code/bin/kimi --version` 为 `2.3.1`，文件为 macOS arm64 单文件可执行程序。上游按发布标签 `@moonshot-ai/kimi-code@2.3.1` 固定到提交 `684bae9ea6a437f3bae98bba1dcb797a917d500b`，没有把最新 main 当作已安装版本。网页于 10-24 查看；版本相关判断以本机和该提交为准。

源码索引（下面的 S 编号均指固定版本）：

- S1：[CLI commands.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/apps/kimi-code/src/cli/commands.ts)、[main.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/apps/kimi-code/src/main.ts)、[kimi-tui.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/apps/kimi-code/src/tui/kimi-tui.ts)。
- S2：[bootstrap.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/bootstrap/bootstrap.ts)、[OAuth storage.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/oauth/src/storage.ts)、[oauth-manager.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/oauth/src/oauth-manager.ts)、[managed-kimi-code.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/oauth/src/managed-kimi-code.ts)。
- S3：[externalHooks 目录](https://github.com/MoonshotAI/kimi-code/tree/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/features/externalHooks)、[plugin manager.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/plugin/manager.ts)、[store.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/plugin/store.ts)、[插件文档](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/docs/en/customization/plugins.md)。
- S4：[skillRoots.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/features/skill/catalog/skillRoots.ts)、[Skills 文档](https://moonshotai.github.io/kimi-code/en/customization/skills)。
- S5：[workspaceTrustStore.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/workspace/workspaceTrust/workspaceTrustStore.ts)、[环境变量文档](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/docs/en/configuration/env-vars.md)、[配置文档](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/docs/en/configuration/config-files.md)。
- S6：[interactionOps.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/agent/interaction/interactionOps.ts)、[AskUserQuestion 实现](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/agent/tools/ask-user-question/askUserQuestionTool.ts)、[批准框](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/apps/kimi-code/src/tui/components/dialogs/approval-panel.ts)、[提问框](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/apps/kimi-code/src/tui/components/dialogs/question-dialog.ts)。
- S7：[region.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/oauth/src/region.ts)、[server instanceRegistry.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/kap-server/src/instanceRegistry.ts)、[persistentToken.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/kap-server/src/services/auth/persistentToken.ts)、[server start.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/kap-server/src/start.ts)。

**【查证】本仓库对照：** 已读 `CLAUDE.md`、`HANDOFF.md`、DESIGN §7 整节及 §8、R5 调研、派活路由规则分工表、`agents/mod.rs`、`claude.rs`、`amp.rs`。以下适配建议不代表已修改 DESIGN 或实现。

**【查证】实测只在 `/tmp/r48-research.00FO6W/work`；没有在仓库中让 Kimi 工作。** 真账号共 **4 次用户输入**，均沿用默认 `kimi-code/k3`、high，没有切换模型或重新登录。T3 被取消后模型继续的同轮调用也包含在这次记录里；不把它算成额外用户输入来掩盖用量。先后只开了 `ranch/test-r48-isolated`、`ranch/test-r48-live`，不同时存活，均带 `role=test` 和 `CAIRN_DISABLE=1`，均已 stop。

| 编号 | 操作与结果 |
|---|---|
| T0，无真账号调用 | `KIMI_CODE_HOME=<临时目录> kimi doctor` 接受含 SessionStart、TurnStarted、StopFailure、SessionEnd 的探针配置；`--prompt=-R48-isolation-probe --output-format stream-json` 解析成功，输出版本后因没有模型配置退出 1，没有 hooks 记录。再开临时数据根的 TUI，`--auto` 仍弹信任框；未通过框，HUP 退出 129。 |
| T1，第 1 次真输入 | 真实 HOME，设 `KIMI_CODE_TRUST_WORKSPACE=1`、`KIMI_CODE_NO_AUTO_UPDATE=1`、`KIMI_DISABLE_TELEMETRY=1`，直接进入编辑器，尚无 session。送入以 `-` 开头的短句后收到 `R48_OK`，主 wire 有 turn 0 completed。 |
| T2，第 2 次真输入 | 请求 Bash 写当前目录 marker.txt，模型只回 `TOOL_OK`；没有该文件，也没有 tool.call。**不能把这次当作工具或批准测试成功。** |
| T3，第 3 次真输入 | 请求 AskUserQuestion，实际出现 A/B/Other 提问框。第一次 Esc：`interaction.resolved response=null`，模型继续；运行中再 Esc：显示 Interrupted by user，wire 的 turn 2 记 cancelled。 |
| T4，第 4 次真输入 | 请求前台 `/bin/sleep 60`，为退出测试准备。发现配置已自动改变后停止；屏幕曾显示 Running a command，但 wire 尚未落下对应 tool.call，进程快照没有直接子进程。HUP 结束 Kimi，退出 129。**没有证据证明 sleep 真正启动，不能据此宣称子进程清理通过。** |

**【查证】操作坑：** `corral keys` 要小写 `enter`，`Enter` 被拒绝；`corral send` 的位置文本以 `-` 开头被当作选项，本次改用 `keys 'text:-…'`，紧跟的一次 Enter 只留下换行，稍后单独 Enter 才提交。后续普通文本 `send --force` 能送入，返回 `confirmed:false`，因为没有 Kimi 钩子。未改 corral 修这些现象。

## 1. 安装、登录、隔离与用户文件影响

**【查证】Kimi Code 是 MoonshotAI 的终端 coding agent，当前这一版是 TypeScript 项目的单文件发行物，不是旧 Python `kimi-cli` 的参数集合。** 命令叫 `kimi`，官方 macOS/Linux 安装入口为 `curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash`。这里只核对文档和现装程序，没有运行安装脚本。[官方安装说明](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/README.md)

**【查证，源码 S2】该账号登录实现走文件存储，不依赖 macOS 系统钥匙串。** OAuth token store 是 `credentials/<name>.json`，目录 0700、文件 0600；本机存在 `credentials/kimi-code.json`。`oauth/kimi-code` 是刷新锁使用的目标路径，锁路径加 `.lock`，不能把这个名字直接当作 token 文件。`server.token` 是另一种本地服务器访问凭证，见 §6。未读取以上文件内容，也没有读 `device_id`。

**【查证，T0、S2】`KIMI_CODE_HOME` 是整体数据根覆盖，不是单独的配置覆盖。** 空临时根不继承现有登录或模型；配置探针和诊断日志留在临时根。**【推测，S2】** 不设置该变量时，临时 HOME 经 `os.homedir()` 改变默认 `.kimi-code` 和 `.agents` 位置，可隔开文件登录；本轮没有单独运行临时 HOME 组合测试。只改 `KIMI_CODE_HOME` 不会隔开真实 HOME 下通用的 `.agents` 资源。**【未知】** 在不读取、复制或链接用户凭证的边界内，尚未验证“临时配置 + 原账号”的组合；不能声称二者天然兼容。

**【查证】真实 HOME 运行前后按文件名、mtime、大小做快照，仅对 `config.toml`、`tui.toml` 算校验和。观察到：**

- 新建 `sessions/wd_work_52e55d296c13/session_<本次会话>/state.json`、`agents/main/wire.jsonl`、`notify/state.json`、`logs/kimi-code.log`，以及本次目录的 `user-history/<目录哈希>.jsonl`。
- 改变 `session_index.jsonl`、`workspaces.json`、`sessions/.index-cache/scan.json`、全局 `logs/kimi-code.log`；`cache/query-store/` 的若干 WAL/索引/代际文件新增或更新。
- `config.toml` 从 2416 到 2418 字节，mtime 与校验和改变；`credentials/kimi-code.json` 大小未变、mtime 改变。没有保存或比较凭证内容，不能判定其值是否变化。
- `tui.toml` 校验和未变；快照中 `server.token`、`server/` 既有文件、`workspace-trust/`、`oauth/kimi-code`、`device_id` 的文件元数据未变。元数据未变不是内容逐字相同的证明。

**【推测，S2】** 模型目录刷新可能解释配置变化，OAuth 自动刷新可能解释凭证 mtime；运行账号可能同时被别的客户端使用，单凭快照不能把每一笔共享文件写入归因给本次 CLI。**【查证】** 本次没有手工改写全局文件、登出、登录、装插件或技能；发现后没有擅自“恢复”配置或删会话。不能保证真实运行对用户配置零写入，后续需用户重新定边界。

## 2. 钩子怎么挂，和现有适配器的距离

**【查证，S1–S3】** `--help`、CLI option 定义和 hooks loader 支持的路径如下：

| 路径 | 结论 |
|---|---|
| 启动参数直接附加 hooks/配置文件 | 未找到公开参数；`--agent-file` 是 agent 定义，`--skills-dir` 是技能，不是 hook 配置。SDK 内部有 configPath，不等于 CLI 支持。 |
| `KIMI_CODE_HOME=<corral 状态下的目录>` | 能加载该目录的 config.toml，但同时迁走认证和所有状态。T0 只证实配置合法，未验证真实 hook 回调。 |
| 原有 config.toml 的 `[[hooks]]` | 支持；会改用户共享文件，不能用现有 Global 整份覆盖。 |
| 插件 `kimi.plugin.json` 内的 `hooks` | 支持，但 manager 从 `plugins/installed.json` 读安装和 enabled 记录；光把目录放进 plugins 下不等于安装。源码没有该目录的自动扫描捷径。 |

**【查证，S3】** hooks 输入为 stdin JSON；公共字段含 `hook_event_name`、`session_id`、`session_title`、`client_type`、`cwd`，内部字段转 snake_case。用户 hook 的 cwd 是工作目录，插件 hook 的 cwd 是插件根；插件另注入 `KIMI_CODE_HOME`、`KIMI_PLUGIN_ROOT`，进程启动默认继承环境。默认超时 30 秒，可配 1–600 秒；`PreToolUse`、`UserPromptSubmit`、`Stop` 可阻断，其他是观察事件。实现 observer 时应空 stdout、成功退出，不回 allow/deny 指令，也不把 `{}` 或诊断文字加入上下文。

**【未知】** 真账号的 hook stdin、父进程链、并发触发顺序本轮没实测。探针只写临时 config，未在全局挂钩子；没有为了继续测试而复制登录文件。

**【推测，和 `agents/mod.rs`、`claude.rs`、`amp.rs` 对照】接入方案有三条：**

1. 原生全局插件：最贴近已有 Global 思路，但必须另处理共享注册表中“只属于 corral 的一条”的安装/卸载、冲突和 enabled；现有 Global 文件所有权模型不够。先获准，再改 DESIGN，不能把 `installed.json` 当 corral 自有文件。
2. 每次生成完整临时配置根：需明确如何保留模型、认证、MCP、技能和用户偏好，以及 token 刷新归属。复制/链接 credentials 都不是本轮已批准、已验证的方案；不建议仅为挂 hooks 就克隆用户目录。
3. 不装全局插件，读取 Kimi 自己的主 wire：有状态事实可用，但要新增发现本次 session、增量读取、启动前就绪和身份归属机制，不是只加一个 `kimi.rs` 就完成。可以作为不愿改全局配置时的独立方案调研。

**【推测】** 若选择插件，仍须沿用 corral 的环境门控与父子运行隔离；还要避免 Kimi 内部子 agent 把主会话置 idle。S3 中部分 agent hook 没有统一附加 `agent_id`，不能假定所有事件都带足够的主/子身份信息；主 wire 的 `agentId=main` 是另一条可核对的依据。

## 3. 状态、回复、等待与 Esc

**【查证，S3、S6；“实测”列只写本轮证据】**

| corral 所需 | Kimi 2.3.1 的证据 | 实测范围及限制 |
|---|---|---|
| 会话开始 | `SessionStart`，source/model/profile；wire 有本会话记录 | T1：第一句前没有会话，不能用此 hook 提前宣告输入就绪。 |
| 收到输入、工作中 | `UserPromptSubmit`、`TurnStarted`，后者有 turn_id/origin_kind/prompt；wire `turn.prompt` | T1–T4 的用户输入在主 wire 可定位。排队输入还有 `UserPromptQueued`。 |
| 工具 | `PreToolUse`、`PostToolUse`、`PostToolUseFailure`；wire `context.append_loop_event` 内 tool.call/result | T3 实际调用 AskUserQuestion；T2 并未调用 Bash。 |
| 正常结束 | `Stop`；wire `turn.ended reason=completed` | T1、T2 有 completed。Stop 是可阻断的“将结束”检查，其他 Stop hook 可要求继续；不能不加判断就当最终结束。 |
| 出错 | `StopFailure`，error_type/error_message；wire 失败结尾 | 未真跑错误场景。 |
| 等批准 | `PermissionRequest`、`PermissionResult`，tool_name/tool_input/decision 等 | 本轮未捕获真正的批准框；不能声称已验证事件与画面一致。 |
| 等回答 | AskUserQuestion 内部产生 `interaction.request kind=question`，对应 resolved；没有专门 Question hook | T3 捕获框和 question 原文；该工具默认获准，不能依赖 PermissionRequest 判等回答。 |
| 整轮被 Esc | hook `Interrupt`；wire `turn.ended reason=cancelled` | T3 运行中 Esc 得到 cancelled；第一次框上 Esc 仅 resolved null，继续运行。 |
| 进程结束 | `SessionEnd` 在正常 session close 时运行 | HUP 走紧急退出，不能保证 SessionEnd；见 §4。 |

**【查证，T1、T3、S3】回复原文不在 Stop payload 里。** 原始记录在 `$KIMI_CODE_HOME/sessions/<workDirKey>/<sessionId>/agents/main/wire.jsonl`。本轮文本是 `context.append_loop_event.event` 中 `type=content.part`、`part.type=text`、`part.text`；同一记录体系有 `turnId`、step 和工具事件，外层 `turn.ended` 标记结束。只取当前主会话当前轮的文字，排除 think、工具参数、工具结果及旧轮；读取时须考虑未写完的一行和多段文本。这里验证了样例格式，未实现完整 extractor。

**【查证，T3、S6】真实提问框底部为：** `↑↓ select  1-3 / ↵ choose  ←/→/tab switch  esc cancel`，上方有 question、题目标签和 Submit。**【查证，仅源码】** 批准框底部构造为 `↑/↓ select · <数字范围> choose · ↵ confirm`，有可预览内容时加 `· ctrl+e preview`；反馈输入时换成 `Type feedback · ↵ submit.`。T0 的信任框是 `Trust this folder?` 和 `↑↓ navigate · Enter select · Esc exit`，与工具批准不同。

**【推测】** R7 可以补等待：提问框用底部导航、switch、esc cancel 的组合，加面板语境，避免正文引用同样文字误判；批准框仍需真实画面后定规则和行数。R7 目前只把 working/idle 改 blocked，不能从 unknown/starting 猜出整个生命周期，更不能靠读屏猜 cancelled。

**【查证，T1、T3、S1】会设终端标题，但不是可靠的忙闲信号。** 初始无 session 时标题为空；建立会话后是会话标题。T3 等问题时仍是此前的标题，没有 amp 的 Input needed 前缀。源码 `updateTerminalTitle` 取 sessionTitle，空则产品名，没有按批准状态拼标题。

## 4. 首句、免确认、信任与退出

**【查证，S1、T0】** `kimi -p <text>` / `--prompt=<text>` 是非交互单次执行；`--output-format stream-json` 也是这个模式的输出选项。`--prompt=-R48-isolation-probe` 已通过本机解析，随后才因无模型失败。CLI 定义没有交互式首句参数，也没有为默认命令声明位置 prompt。不能把 corral 默认的 `-- <首句>` 或 agy 的 prompt-interactive 方式照抄过去。

**【推测】** 若继续做 PTY 常驻接入，第一期可像 amp 一样令 `prompt()=false`，但必须同时设计首句 bootstrap：Kimi 的 SessionStart 发生在第一句之后，而普通 send 在 starting 会受保护。不能单写“等 idle 再送”形成死等，也不宜让用户长期用 `--force`。另选 `--session <id>` 的提前创建路线需单独核对，未验证。

**【查证，`--help`、S1、S5】** `--yolo` 是 Ask When Needed，仍会问风险动作、问题和计划；`--auto` 是 Never Ask，自动处理动作和决策，源码会禁止 AskUserQuestion 等待。它们不等于信任目录：T0 的 `--auto` 仍弹信任框。`KIMI_CODE_TRUST_WORKSPACE=1` 在本进程视为信任且不写持久信任记录，T1 已验证可直接进编辑器。正式派活可考虑 `--auto` 加该变量，是否默认信任由用户定。

**【查证，T0、T4、S1】HUP 能结束本轮两个 Kimi 进程，退出码 129。** 源码明确 HUP 用 emergency exit，跳过正常清理；TERM 调 `stop(143)`，关闭 session/harness 并恢复终端。**【未知】** TERM 的实际耗时、MCP 和真正运行中的 Bash 子树是否全停，均未测到。T4 不是成功的子进程退出证据，不能把“父进程退出”写成“停干净”。**【推测】** 适配器退出顺序应优先研究 TERM，再以 HUP/KILL 兜底，不能凭 amp 的 HUP→TERM 默认顺序定案。

## 5. 技能位置

**【查证，S4、S1】默认自动发现：**

| 范围 | 位置 |
|---|---|
| 用户专用 | `$KIMI_CODE_HOME/skills/`，默认 `~/.kimi-code/skills/` |
| 用户通用 | `~/.agents/skills/`，跟 OS HOME，不跟 KIMI_CODE_HOME |
| 项目 | 最近 `.git` 所在根的 `.kimi-code/skills/`、`.agents/skills/`；无 `.git` 则工作目录 |
| 其他 | config 的 extra_skill_dirs、启用插件、内置技能；`--skills-dir` 可重复，覆盖自动发现的用户/项目目录 |

**【查证，S4】** `~/.claude/skills` 不在默认发现列表。**【推测，对照 ranch 的 skills targets】** 两份现有技能共用 `~/.agents/skills` 即可；Kimi 适配器可声明现有 `codex` 位置，不必新增安装目标。本轮未安装技能，也未让模型调用 corral-dispatch；如果用户显式覆盖 skills-dir 或同名技能被项目覆盖，则另行核对。

## 6. fence 可行性：只做路径和网络摸底

**【查证】本轮没有运行 fence/srt。** DESIGN §8 的按种类数据、默认禁读 HOME、按需开放网络/MCP、独立临时目录仍是边界；以下是准备数据所需的候选，不是通过验收的 allowlist。

| 项目 | 证据和候选 |
|---|---|
| 模型与认证 | **【查证，S2、S7】** 默认中国区 `api.kimi.com`、`auth.kimi.com`；全球区 `api.kimi.ai`、`auth.kimi.ai`。现有配置/环境可覆盖端点，正式接入按实际 provider 定，不把四个域名一律打开。 |
| 搜索/抓网页 | **【查证，S2、S5】** 登录用户可走 managed API 的 `/search`、`/fetch`；也能配服务或走其他访问方式。**【未知】** Kimi 这套内置工具是否满足 fence 现有 Exa 要求，及临时 MCP 注入怎样沿用用户设置；本机 CLI 没有 `--mcp-config` 参数，不能照抄其他家。 |
| 更新/可选服务 | **【查证，S7、S5】** 有 code.kimi.com / code.kimi.ai 的发行 CDN、cdn.kimi.com 资源、telemetry-logs.kimi.com、code-rc.kimi.com / code-rc.kimi.ai。T1–T4 用环境关闭自动更新、遥测；墙内可先不用更新和 remote 功能，不能因为字符串存在就全部放行。 |
| 读路径 | **【推测，S2、S4、S5】** 可执行文件、config.toml、tui.toml、credentials、oauth/region、所选技能和 AGENTS.md、MCP/插件配置，以及工作区、Rust/Git 工具链。通用 `.agents` 若放行会扩大可见内容，要遵守原 fence 数据边界。 |
| 写路径 | **【查证，T1–T4】** sessions、session_index、workspaces、user-history、logs、cache；运行时可能写 config 与 credentials。**【推测，S2】** OAuth 刷新锁及会话锁也需写。不能只开放 sessions，也不能为省事允许写整个 HOME。 |
| 本地网络/IPC | **【查证，T1】** 对本次 Kimi PID 的 lsof 快照没有 TCP LISTEN；有出站 TLS 和一条无路径的 Unix socket 端点。**【未知】** 单次快照不能证明没有短暂监听，Unix 端点用途未定，代理映射 IP 不能可靠还原域名。 |

**【查证，S7】`server/` 与 `server.token` 的用途：** `server/instances/*.json` 是本地 Kimi Web/server 实例注册，含 PID、host、port、heartbeat；`server/events/` 是服务器事件存储；`server.token` 为服务器读入或新建的持久随机访问 token，不是模型账号 OAuth。Web server 的源码会监听 HTTP/WS；普通 TUI 的 `run-shell.ts` 用进程内 harness，显式 `/web` 才转到 server。**【推测】** 首期 PTY 接入不需要启用已有 Web server，也不应默认开放其 token/端口。**【未知】** 本轮没有连接已有用户 server、没有检查它的 token，也没有覆盖 Desktop、remote、MCP 的网络行为。

**【推测】** R17b 的最小后续实验应先限定普通 TUI、一个已选 provider、更新/遥测关闭，分别确认身份刷新和配置写入，再定最小路径；本轮不把任何推断写成已能进墙。Linux Unix socket 限制仍沿用 DESIGN §8，不能从 macOS 推定 Linux 可行。

## 7. 模型、强度与派活分工表

**【查证，T1 主 wire】** 实际请求为 alias `kimi-code/k3`、模型 `k3`、thinkingEffort `high`；这比只看欢迎页的 K3 名字更具体。未切换到 highspeed、K3.1 或更贵模型。

**【查证，S1、S5】** `--model <alias>` 选择本次模型；未指定取 default_model。发行版配置例子还列 `kimi-code/kimi-for-coding`、`kimi-code/kimi-for-coding-highspeed`，可配置其他兼容 provider；列表能随服务端模型目录刷新，例子不是本账号可用模型的保证。K3 示例 support_efforts 为 low/high/max，配置 `[thinking].effort` 和交互 `/effort` 支持强度；CLI 没有独立 `--effort`。`KIMI_MODEL_THINKING_EFFORT` 是 kimi provider 的本次覆盖，绕过客户端声明的档位检查，不等于服务端接受任意档位。

**【未知】** 本账号模型全集、不同强度的成本/效果、服务端是否接受 xhigh 均未测；没有把 Claude/Codex 的 xhigh 直接映射给 Kimi。

**【推测，对照 R31/R34 分工表】** 建议先做点名才能派的候选，明确默认模型或用户选定 alias；若先不分强度，不加 effort 标签。要进入默认格子应另做代表性编码任务评估：本轮只测试控制接口，T2 甚至只口头确认而未执行工具，不能据此判断“适合中档”或替换哪家。技术上分工表可新增家模板与选项，但还依赖可靠的启动、状态和收发，不能单靠加数据让当前 unknown 的 Kimi 变成可派活适配器。

## 对 paddock 的影响

**【查证】本轮只加调研文档，paddock 不用动。** **【推测】** 正式接入若沿用现有公开约定，`corral integration status` 会多一项 `kind=kimi`、路径/版本、技能位置和 hook 方式；如果用全局插件，还需准确显示注册与 enabled 状态，不能仅检查插件文件是否存在就报 same。已有命令和状态值可保留，新的家是兼容新增。

**【推测】** 如新增启动就绪方式、首句限制、安装共享注册表的行为或分工表模型/强度选项，要在自己的任务中写清公开协议，paddock 单独适配展示、安装确认和动态选项。本轮不要求 paddock 读取 Kimi 内部状态目录，不改 paddock 仓库；旧 corral 不认识 Kimi，不能承诺新旧命令都能操作这一家，需正式任务界定版本边界。

## 要用户定的

以下都是**【推测：建议】**，不是已经授权或已经实现的决定。

1. **接到什么程度。** A：只做点名使用的 corral 启停、状态、回复与送话；B：同时进派活分工表；C：再加 fence。建议先 A，首句 bootstrap、主/子身份和最终完成判定一起说清楚；B 先作为点名候选，默认格子另评估；C 单独真机验收。依据：§2–§7 的缺口。
2. **允许怎样接 hooks。** A：允许独立插件加共享注册表中的 corral 条目；B：允许在 config.toml 合并 corral hook；C：不改全局，另做 wire 观察方案。建议愿意安装则先论证 A 的窄注册/卸载机制；不接受共享写入则选 C。不要把临时 HOME 当作既保留用户登录又零影响的现成解法。正式开发前改 DESIGN，本轮不安装、不修改全局。
3. **真实账号自动写入边界。** A：接受 Kimi 自己刷新 config、credentials 和缓存，继续复用原账号；B：坚持原目录零写入，另准备独立数据根及测试登录；C：先停在本报告。建议先确认 A 或 B 再真跑。本轮已发现 config 校验和及 credential mtime 变化，因此停止了继续试验；没有擅自复制凭证、回滚文件或绕过登录。关闭自动更新没有阻止这种写入。
4. **正式接入的实测安排。** A：在上条边界确认后，另给少量真跑额度，测正常回复/工具/批准/提问/框上与运行中 Esc、TERM/HUP 子树清理、启动身份及已选 hooks 的实收；B：先只用合成输入做解析，真实行为保持未知。建议 A，并把 sandbox 留在独立实测阶段；不再复用本轮额度继续探索。全局探针若需要安装，也须在同一授权里明确范围和卸载办法。

**【查证】本报告的验证仅做关键本机参数/路径核对和 `git diff --check`，没有跑 Cargo 测试、没有实现适配器或进入 sandbox。** 以上未知项不构成“已能正式接入”的验收证据。
