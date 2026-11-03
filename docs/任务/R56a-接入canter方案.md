# 任务：R56a 定 corral 接 canter 的方案（只出设计，不写代码）

2026-11-03，ranch/main 交给 ranch/dev-r56a（Claude Code，高档「界面和设计」：opus[1m] / xhigh）。
路由：高 / 交叉审查不要 / 影响面：看得见（路由：重（置信 0.97）、要、碰要害；本轮是不改代码的设计，按 corral-dispatch 第 3 节一律算看得见、不交叉审查；路由判「要」是因为摘要提到并发的订阅线程）
类型：设计
依据：本轮只定 corral、pony、派活表接 canter 的做法和给 paddock 的公开约定，写成方案交用户定；不写 corral 代码，不改 canter、paddock 仓库。
提示：围绕已知约束给出推荐及关键取舍；有实质备选时再比较，不凑方案数量。
你是被委派的 agent：照本文件做。除了下面写明的测试用 canter 外，不要再开别的 agent。

## 先读

- `CLAUDE.md`「规矩」。
- `docs/任务/R56-接入canter.md`：用户要的 10 条、这一版不做的、规矩。**以它为准。**
- `docs/DESIGN.md` §7「agent 适配器」整节，重点：结构和加一家的规矩、R6 `Global`／`CORRAL_KIND`、R7 读屏、R13 认领会话和信任、R22–R26 被打断、R43 提问题目、R48 Kimi（主 wire 读取、cursor、停止跟踪子孙）。§6 暂停、§8 sandbox 里「不支持的家」怎么报。
- `docs/CONTRACT.md`：被人打断（R22）、提问与批准（R43、R48g2）、原始命令（R47c）、`integration status`（R4、R6、R48c）、sandbox（R17b、R18）、stop 退出记录（R51）。
- corral 代码：`crates/corral/src/agents/mod.rs`（`Adapter`）、`agents/kimi.rs`（最接近：读主会话文件出状态、回复、题目、命令）、`pen.rs`、`pen/upgrade.rs`（升级快照）、`events.rs`（状态机）、`cli.rs`（status/reply/wait/stop/pause）、`state.rs`（`FILES`）、`integration.rs`、`freeze.rs`、`sandbox.rs`、`child_sandbox.rs`。
- pony：`crates/pony/src/corral.rs` 和 Kimi 的拒绝、打断按钮怎么接的（`docs/任务/R47a-…`、`R48g2-…`）。
- 派活表：`crates/dispatch/resources/dispatch.toml` 里 `kimi` 的家和格子，`crates/dispatch/resources/corral-dispatch/` 技能里讲 Kimi 和「卡在信任框」的段落；`docs/任务/R48d-Kimi-Code派活.md`。
- canter 的公开接口（只读这些，不读 canter 的别的源码当依据；需要核对行为时用下面「实测」的办法）：
  - `canter --help`（本机已装 `0.1.0 · 16fb98c`）；
  - `~/Developer/personal_projs/canter/docs/DESIGN.md`「接 corral、paddock、pony（P2-83）」一节；
  - `~/Developer/personal_projs/canter/crates/canter-protocol/README.md`（HTTP 对照、`Managed terminal lifetime (P2-192)`、事件、归并、错误与 SSE）；
  - 调研报告 `~/Developer/personal_projs/canter/docs/调研/P2-83-接corral和paddock调研.md` §1、§3.2（参考，以 R56 的 10 条为准）。
  - 起假模型服务端、写测试配置只看：`crates/canter-server/README.md` 的 `config.json` 那段（约 33–50 行，`prompt_suggestions` 约 79 行、`sandbox` 约 109 行）、`crates/canter-model/README.md`；`crates/canter/tests/tui.rs`（约 795–830、1230–1256 行）和 `crates/canter/tests/exec.rs`（约 240–290 行）只当「怎么起」的参考，不当行为依据。

## canter 主控补的要点（11-03，canter/main 回 ranch/main，设计里要写清）

- 会话文件要等会话真正建成后才写：没有会话文件可能是在启动，也可能卡在信任框上（`--yolo` 不跳过它），两种都报 starting。会话文件字段以后只增不改，读时容忍多出来的字段。
- SSE 每帧带 `seq`，断线后用 `Last-Event-ID` 或 `after` 补发；`seq` 为 0 的帧（如 `session.promptSuggestionsReduced`）不进补发，翻译时跳过。
- 子 agent 的批准和提问作为 `agent.request.opened` 转发到父会话的流，快照里是 `agentRequests`，和 `pendingRequests` 分开；两处都算 blocked。
- 「一轮结束」不等于空闲：目标续跑、后台命令跑完叫醒模型都会再开一轮。
- `--close-on-exit`：终端正常退出（含 HUP/TERM）立刻发收尾，服务端回 202 后自己关会话、不等关完；要「停完了」的确认可另调同步的 `DELETE /v1/sessions/{id}`；关闭原因 `closeReason: clientRequest`，和服务端停机区分。
- canter 自己升级（`serve --replace`）时旧服务端用 `closeReason: serverShutdown` 关会话，终端在新服务端上自动恢复同一个 ID 并重写会话文件；看到 `serverShutdown` 不能当用户停止，要重读 `server.json`、重连、用 `after` 续订。
- corral 的订阅一律带 `observe=true`，否则会被算成 canter 的客户端，改变它「最后一个客户端离开 10 秒后收尾」的规则。
- 版本和能力从 `GET /v1/info` 的 `protocolVersion`、`version`、`features` 判断（受管模式要有 `managedClients`）；缺了就报 canter 版本太旧，不猜。
- canter 的沙箱开关是服务端全局配置，不是每个会话一份；`--yolo` 只是不弹批准框，命令仍在自带沙箱里跑。
- 隔离的 `CANTER_HOME` 没有信任记录，会弹信任框：测试里可以在终端按键确认，也可以经公开接口 `POST /v1/folderTrust {cwd}` 先记信任（canter DESIGN P2-168，只对那个隔离目录生效）。corral 正式使用时要不要替用户记信任，是要用户定的事，列进「要用户定的」，不要自己定成默认。
- 一个 `CANTER_HOME` 只能跑一个服务端，多个测试各用各的目录。
- canter 已知没修、联调会碰到的两点（canter HANDOFF）：服务端排空期间受管终端显式退出不会触发收尾，由停机路径关会话；受管终端租约过期后那个终端会一直显示 `Reconnecting`。
- 终端标题格式是 `⠋ canter · <目录名>`，第一个字符是转圈动画。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r56a`，分支 `r56a`（已从 main 建好）。只写 `docs/调研/R56a-接入canter方案.md` 和本任务文件末尾。
- 编译目录（要编 corral 看行为时）：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r56a`。

## 要做的

写 `docs/调研/R56a-接入canter方案.md`，开头「结论先说」。每个设计点写推荐做法和理由；有实质备选的写出取舍；依据标明是 canter 公开文档哪一节、corral 代码哪里、还是实测看到的。要覆盖：

1. **通用挂点**：corral 公共代码加什么（用户定的：某一家可以带一个盯状态的后台线程，pen 负责起停）。接口长什么样、在 `Adapter` 上怎么声明；线程的产出怎么进现有的状态、回复、题目、命令的读取路径（复用 v1 事件和状态机，还是像 Kimi 那样由读取方现算，或别的）；pen 退出、线程出错、订阅断开时怎么办。别的家行为不变。
2. **开和会话跟随**（第 1 条）：canter 的启动参数怎么拼（首句、`--session-file` 放哪、`--close-on-exit`、`--tag` 要不要带 corral 的名字或标签）；怎么从 `server.json` 找服务端、`GET /v1/info` 核对什么；会话文件改写（`/new`、`/resume`、分叉、交接）后怎么切订阅；会话文件还没出现（信任框、还没进会话）时报什么。
3. **状态、打断、回复、提问、命令**（第 2、3 条）：canter 的快照和事件（`turn.*`、`request.*`、后台命令、目标续跑、`input.queued` 等）一条条怎么对到 corral 的 starting / working / idle / blocked、`interrupted`、`reply`、`questions`、`command`；`seq` 补发、去重、即时事件怎么处理；`sandboxApproval`、`planApproval` 怎么报；子会话的请求怎么算。
4. **送话、按键、attach、read**（第 4 条）：确认照现有终端方式不变需要注意什么（例如 send 的空闲检查依据）。
5. **停**（第 5 条）：用 `--close-on-exit` 还是另调 `DELETE`，或两者都用；`quit_steps` 怎么写；`corral stop` 怎么确认服务端上的工作真停了；服务端不可达时怎么报。
6. **升级**（第 6 条）：`corral upgrade` 换 pen 时订阅位置怎么交给新 pen（`pen/upgrade.rs` 的快照），不丢、不重报旧回复、不误判停止；canter 服务端 `serve --replace`／`canter upgrade` 时怎么重连、怎么不误判。
7. **不支持的明确报**（第 7 条）：暂停、继续、显式和继承的 `--sandbox` 对 canter 各报什么错误码、在哪一步拦；和 paddock 现在把 `unsupported` 显示成「needs corral upgrade」的关系（要不要新码区分「这一家不支持」和「旧 pen 不支持」）。canter 自带沙箱在 corral 的输出里要不要提、怎么提才不和 fence 混。
8. **`integration status`**（第 8 条）：canter 一行的程序、版本、接法（新的接法名？状态来源说明？）、技能 target。
9. **pony**（第 9 条）：拒绝、打断按钮对 canter 怎么做，认标题的规则。
10. **派活表和技能**（第 10 条）：`dispatch.toml` 加的家和格子、标签；corral-dispatch 技能、corral 技能里要改的段落（照 Kimi 的写法）。
11. **给 paddock 的公开约定**：把上面所有对外可见的变化（`integration status` 新字段或新值、能力怎么报、错误码、`status`/`ls` 有没有新字段）单列一节，写成 paddock 能照着做的样子；标明哪些是兼容新增、有没有不兼容的。
12. **对 canter 接口的疑问**：照公开接口做不到、或要猜的地方，单列「要 canter 改的或要用户定的」，每条写清缺什么、为什么、可选办法和你的建议。不要在 corral 里绕过去读 canter 的私有文件。
13. **测试怎么做**：怎么在不调真实模型、不碰 `~/.canter` 和用户正在跑的 canter 服务端的情况下测（隔离 `CANTER_HOME`、假模型服务端；canter 仓库里现成的做法只看它的公开测试说明或 README）；测试要不要依赖本机装了 canter，没装时怎么办。
14. **实现怎么拆**：拆成几件、每件改哪些文件、哪些能并行（文件不重叠）、哪件碰要害；每件大概要验什么。

最后一节「要用户定的」：只列真有分歧、要用户拿主意的，每条给选项和你的建议。

### 实测（可选）

要核对 canter 的实际行为（会话文件什么时候写、事件长什么样、`--close-on-exit` 退出时服务端发什么）可以真跑 canter，但只能这样：

- 隔离的 `HOME` 和 `CANTER_HOME`（放在 `/tmp/r56a-*` 之类的临时目录），不连用户真实的 `~/.canter` 和正在跑的服务端；不调真实模型（用假模型服务端，照上面「先读」里的说明起，`config.json` 0600、`prompt_suggestions:false`）。做不到隔离或要用真实模型才能看的，不跑，停下问主控。
- 要看终端画面时可以用 corral 开测试 agent：名字 `ranch/test-r56a-<几个字>`，带 `--label role=test`、`--env CAIRN_DISABLE=1` 和隔离的 `--env CANTER_HOME=…`，用完 `corral stop`。最多同时开一个。
- 自己起的 canter 服务端记下 PID，用完按 PID 停掉；临时目录用完删掉。

## 怎么算做完

- 方案 `docs/调研/R56a-接入canter方案.md` 写完，上面 14 条和「要用户定的」都有，每条有推荐和依据。
- 验证只做这些：`git diff --check`；方案里写到的 canter 参数、接口、字段，挑关键的几条对过公开文档或实测一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 corral、pony、dispatch 的代码和资源，不改 DESIGN、CONTRACT（方案定了由主控改）。
- 不改 canter、paddock 仓库；不读 canter 状态目录里 `server.json` 以外的文件，不读、不改用户真实的 `~/.canter`；不连用户正在跑的 canter 服务端。
- 不调真实模型，不花钱。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade` 它们，也不 attach。只操作自己开的 `ranch/test-r56a-*`。
- 不按项目名或路径批量杀进程（`pkill -f canter`、`pkill -f corral` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，用户自己也开着 canter。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事（包括要真实模型或真实 `~/.canter` 才能看清的行为），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r56a` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、查了哪些来源、实测了什么、拿主意的地方、没查清的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-11-03，ranch/dev-r56a。

### 做了什么

- 写了 `docs/调研/R56a-接入canter方案.md`，结构如下：
  - 开头「结论先说」；
  - 「实测记录」：19 条看到的事实，外加一份没实测的清单；
  - 第 1–14 节，每节都有推荐做法，并标明依据：canter 公开文档的行号、corral 代码的 `文件:行`，或者【实测 n】；
  - 最后一节「要用户定的」，两条。
- 没改代码、资源、DESIGN、CONTRACT，也没改 canter、paddock 仓库。

### 查了哪些来源

- ranch：R56 总任务、本任务文件、DESIGN §6–§10、CONTRACT。
- corral 代码：`agents/mod.rs`、`agents/kimi.rs`、`events.rs`、`events/raw.rs`、`hooks.rs`、`cli.rs`、`pen.rs`、`pen/upgrade.rs`、`state.rs`、`integration.rs`、`environment.rs`、`child_sandbox.rs`、fence 的 `data.rs`。
- pony：`corral.rs`、`core/engine.rs`、`core/engine/buttons.rs`。
- 派活：`dispatch.toml`、corral-dispatch 技能、corral 技能和 `AGENT_USAGE.md`。
- canter：`canter --help`、`canter --version`；canter DESIGN 的 P2-83 节及其中引到的 P2-168、P2-178、P2-191、P2-192；协议 README 全文；canter-server README 的 config 段；canter-model README；调研报告 P2-83 §1、§3.2。
- paddock：为确认现在怎么显示，只读了两处，没改：
  - `app/src/integrations.rs`：怎么显示 `hooks` 的值；
  - `app/src/pause.rs`：怎么把 `unsupported` 显示成「needs corral upgrade」。

### 实测了什么

环境：
- 隔离的 `HOME`、`CANTER_HOME`，放在 `/tmp/r56a-probe`。
- 一次性的假 Chat Completions 服务（Node 脚本，放在 scratchpad，不进仓库），`prompt_suggestions:false`。
- 用 corral 开了一个测试 agent `ranch/test-r56a-probe`（`--label role=test`、`--env CAIRN_DISABLE=1`、隔离的 HOME 和 CANTER_HOME），用完 `corral stop`。

看了这些：
- `server.json`、`/v1/info`、会话文件、SSE 帧的样子；
- 正常的一轮、出错的一轮、批准、提问、两种 Esc；
- 标题、框上重画；
- `/new`、`serve --replace`；
- `corral stop`（HUP）和 `kill -9` 之后服务端怎么关会话；
- 终端自己起的服务端的进程关系；
- `folderTrust`、快照。

自己起的服务端和假模型都按 PID 停掉，临时目录删了，`corral ls` 里没有留下 `ranch/test-r56a*`。没调真实模型，没碰 `~/.canter` 和用户的服务端。

### 拿主意的地方

这些都写成了方案里的推荐，没有落地：

- 线程把事件翻译成 v1 事件写进 `events`。没选「读取方现算」（Kimi 那样），也没选 v2 原始事件。
- 换 pen 时，fork 前先停线程。
- 不加 `--tag`。
- 停只靠 `--close-on-exit` 加上 12 秒的确认，不调 DELETE。
- `hooks` 仍写 `"launch"`，不加新的接法名。
- 暂停和 sandbox 共用新错误码 `unsupported_kind`，暂停在 CLI 和 pen 两头拦。
- canter 也报 `interaction`，pony 改用结构化核对，不比屏幕。
- 断开 20 秒报 `unknown`。
- 实现拆成 R56b、R56c 两件。

### 没查清的事

- 子 agent 的 `agent.request.*`。
- `sandboxApproval`、`planApproval`。
- 目标续跑、后台命令叫醒。
- `/resume`、分叉、交接。
- 提问框上按 Esc。
- 服务端 replace 时正好有一轮在跑。
- `session.status error`。
- 多行送话后 `turn.input` 是什么样。
- SSE 是不是 chunked。
- canter 跑的命令里 `CORRAL_NAME` 对不对：按文档写的，没实测。

这些都写进了方案的「没实测的」和第 13 节联调。

### 踩过的坑

- 假模型让 shell 工具带了 `justification`、没带 `sandboxRequest`，canter 回工具错误「sandboxRequest requires a nonempty justification」。只给 `command` 才行。
- `echo` 在只读白名单里，不弹批准框；换成 `mkdir … && touch …` 才看到批准框。
- 假模型回 400 的第一版，先写了 200 的头，再改状态码，没生效。改成先判断再写头。
- `POST /v1/sessions` 不带 `"model":""` 会报 `invalid_request`。
- macOS 没有 `timeout` 命令，订阅 SSE 改用 `curl --max-time`。
- `/v1/info` 的版本字段叫 `serverVersion`，和主控转述的 `version` 不一样。已写进方案第 12 节。
- 框开着时 canter 每 2 秒重画一次，pony 现有的「比屏幕」核对对 canter 永远不成立。设计因此改成结构化核对。
- 有一次误把假模型的 `/push` 请求发到了 canter 的 socket，回了 404，没有影响。
