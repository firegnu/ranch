# 任务：R58 pony 在 Slack 上回答 canter 的提问框、批准 canter 的框

2026-11-11，ranch/main 交给 ranch/dev-r58（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.73）、要、碰要害）
类型：功能变更
依据：canter 主控转来、用户 11-11 同意的需求：canter 的提问框在 Slack 上给选项按钮和写回答，批准框加「允许这一次」，和 Claude Code（R47b、R47c）对齐。走 canter 的公开回答接口，不按键。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent（下面写明的测试 agent 除外）。

## 用户原话

- 11-11（对 canter 主控）：「canter-desktop我觉得得有slack的频道。你觉得呢？」
- canter 主控说明 canter 的选择框在 Slack 里答不了（pony 对 canter 只有「拒绝」「打断」，碰到提问回「无法核对 canter 的框，没按；请回电脑处理」）后：「那个缺口好做吗？如果要做的话得在哪个repo中做？」「为什么要等canter-desktop 真在 iPad 上用起来」「写吧」
- 看了草稿后：「不要作的第一条和第三条可以做吗？我想和claude code以及codex对齐啊」（第一条是「不往 canter 的终端里按键来答题」，第三条是「这次不做 canter 批准框的『允许』按钮」）
- 11-11（对 ranch 主控）：问「这个不涉及到核心吧？我记得当时定的是每一个接入的社交软件都是一个类似插件的东西」；主控说明三层（见下面「用户定的」第 5 条）后：「可以，都按你的建议，补进去派吧」。

## 用户定的（11-11，主控的建议，用户全部同意）

1. 「允许这一次」只给跑命令（工具 `shell`）和改文件（工具 `write`、`edit`）的 `approval` 框，且 `availableDecisions` 里有 `accept`。放宽 canter 自带沙箱的 `sandboxApproval`、MCP 等其他工具只给「拒绝」。
2. 「拒绝」「打断」对 canter 照旧按一次 Esc，这次不改。
3. 同时开着几个框时只答最前面那个；canter 子 agent 的框（`agent.request.opened`）这次不做，照旧「回电脑处理」；计划框只给「批准，之后的操作仍逐个问」，不给清空上下文、继续计划。
4. 真 canter 实测：开一个测试 canter，用 canter 服务端配置的模型，4 轮短任务：选项回答、写一句话回答、允许一条命令、批准一个计划。真 Slack 验收部署后由用户和主控做。
5. 插件式的边界：
   - pony 的社交软件插件（`crates/pony/src/chats/`）不加任何 canter 特例；按钮、确认框、线程回答用 R47 已有的接口。
   - corral 的公共代码只加通用挂点（适配器接口一个可选方法，默认不支持）和命令入口，照 R57e `steer_api` 的样子；canter 的协议细节全放 `crates/corral/src/agents/canter.rs`。
   - corral、pony 已有的命令、参数、输出不变，只做兼容新增。

## 要的结果（canter 主控转来的原文）

1. 频道绑定的仓库主控是 canter 时，canter 弹出提问（`ask_user`），pony 在 Slack 里照 Claude Code 那样（R47b）显示题目和每个选项的按钮，也能自己写一句话回答。
2. 点了按钮或写了回答，canter 那边的框关掉，模型收到这个回答接着做；Slack 上那条消息改成显示答了什么，按钮去掉。
3. 同一道题已经在电脑上答过了，Slack 上再点不生效，消息改成说已经在别处答过。
4. 批准框照 Claude Code（R47c）对齐：canter 要批准命令或改文件时，除了现有的「拒绝」「打断」，也给「允许这一次」；计划批准给和 Claude Code 对应的那一项（批准计划、之后的操作仍逐个问）。持久放行这类选项照 R47c 一样不给。
5. Claude Code、Kimi 的答题和批准照旧；机器上没有 canter 时什么都不变。
6. 怎么答（走 canter 的公开接口，还是按键）由 ranch 定，只要上面这些做得到、不会答错框。

## 先读

- canter 的公开接口（权威）：`/Users/firegnu/Developer/personal_projs/canter/crates/canter-protocol/README.md` 里批准和提问那几段（「批准和提问先到先得」起、计划模式 P2-103）、`/Users/firegnu/Developer/personal_projs/canter/crates/canter-protocol/src/types.rs` 的 `RequestKind`、`ApprovalRequest`、`UserInputRequest`、`UserInputAnswer`、`PlanAnswer`、`RequestResolveParams`、`RequestResolveResponse`；`/Users/firegnu/Developer/personal_projs/canter/docs/DESIGN.md` 第 8 条「提问（P2-4）」。只读，不改。
- `docs/DESIGN.md` §7 接入 canter（R56）和 canter 插话（R57e）那两段，§10 pony。
- `docs/CONTRACT.md` 里 `status` 的 `questions`、`command`、`interaction`，canter 的部分，R57e 那一段。
- `docs/任务/R47b-pony提问框选项按钮.md`、`R47c-pony允许这一次按钮.md`、`R48g2-pony在Slack上回答Kimi的提问和批准框.md` 的完成记录和审查：Slack 上的样子、核对、过期、连点、重启的规矩照这几件。
- `crates/corral/src/agents/canter.rs`（`Translation::dialog`、`steer_input`）、`crates/corral/src/pen/steer.rs`：R57e 怎么经公开接口送话。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r58`，分支 `r58`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r58`。
- 只动 `crates/corral/`（canter 适配器、回答的入口）、`crates/pony/`、`docs/DESIGN.md`、`docs/CONTRACT.md`、`resources/` 里的技能说明（要提新命令时）、pony README，以及本任务文件末尾。

## 要做的

答法 ranch 定为走 canter 的公开回答接口 `POST /v1/sessions/{会话}/requests/{请求}`，不按键。按请求编号答，canter 先到先得，换了框或已经答过不会落到别的框上（`alreadyResolved: true`）。主控看过 canter 服务端代码：这个接口不查是哪个客户端在答，corral 能答，canter 不用改；你实测时确认一次。

1. **corral**：
   - `status` 对 canter 兼容新增最前面那个框的请求编号，以及 pony 核对和显示要用的内容（批准框的工具名、改哪个文件、`availableDecisions`；计划框的计划正文）。字段名你定，写进 CONTRACT。
   - 加一个回答 canter 框的入口（命令和参数你定，写进 CONTRACT），pony 只通过它答：带请求编号，答选项、答一句话、允许这一次、批准计划。照 R57e 的路：会话从 `canter-session.json` 读并核对主会话，服务端从 `server.json` 找，只读这两个公开文件；`clientName: "corral"`；请求有时限，不卡住 pen。
   - 结果分清：答上了；已经被别人答过（`alreadyResolved`）；canter 明确拒绝（4xx，没改状态）；结果不明（开始写后断开、超时、5xx，不能重答）。退出码和 JSON 照 R57e 的区分方式。
   - 不是 canter、旧 pen、没有会话文件的，报不支持，不按键。
2. **pony 在 canter 上**：
   - 提问：照 R47b 的样子列题目和选项按钮，也能在线程里写一句话回答；用回答入口答，不按 Esc、不当文字送。答完把按钮去掉，写答了什么；`alreadyResolved` 或框已经不在了，写「已经在别处答过」之类，按钮去掉。canter 不收含换行的文字回答，线程里写了多行怎么处理你定，写进完成记录。
   - 批准框：在现有「拒绝」「打断」之外，按「用户定的」第 1 条给「允许这一次」（`decision: accept`），带 R47c 那样的 Slack 确认框；命令、文件看不全的不给。
   - 计划批准：给「批准，之后的操作仍逐个问」（`plan: {decision: implement, approvalPolicy: alwaysAsk}`）。
   - 一律不给：`acceptForSession`、`acceptWithExecpolicyAmendment` 这类持久放行，计划框的清空上下文。
   - 防误按照 R47c、R48g2：按之前核对实例、请求编号还是最前面那个、人刚在电脑上操作过不答；连点、重启不重答；结果不明照实说、请用户回电脑看。
   - 「拒绝」「打断」：照「用户定的」第 2 条。
   - 旧 corral 没有新字段、新入口时，照现在的「无法核对 canter 的框，没按；请回电脑处理」。
3. 文档：CONTRACT、DESIGN §7 和 §10、pony README 跟着写，写明对 paddock 的影响（应是兼容新增）。

## 怎么算做完

- 「要的结果」1–6 条都做到。
- 验证只做这些：
  - corral、pony 各自针对的测试（假 canter 服务端、假 corral、内存聊天适配器，隔离 HOME、CORRAL_HOME、PONY_HOME），先写测试确认现在失败：选项、文字、允许这一次、批准计划各答上一次；已在别处答过；canter 拒绝；结果不明；框换了；人刚操作过；连点；Claude Code、Kimi 照旧。
  - 真 canter 实测一次：按「用户定的」第 4 条。
  - `cargo test --all-targets --no-fail-fast`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。
  觉得不够，在回复里说，不要自己加。

## 真 canter 实测的边界

- 额度：4 轮短任务，不够先停下来问主控。
- 只在自己开的测试 agent `ranch/test-r58-<用途>` 上做：`--label role=test`、`--env CAIRN_DISABLE=1`，不带 `--yolo`（要看到批准框），不指定模型，用本分支编出的 corral；用完 `corral stop`。canter 用本机已在跑的服务端，不改 canter 配置。
- 让它做无害的事：命令、改文件只在临时目录里；计划只批准无害的计划。
- 卡在「是否信任这个目录」（60 秒还是 `starting`），停下来告诉主控，由用户去点，不要替它点。

## 不要做

- 不在 `crates/pony/src/chats/` 加 canter 特例；corral 公共代码只加通用挂点和命令入口，canter 协议细节只放 `canter.rs`（见「用户定的」第 5 条）。

- 不改 canter 仓库，不读 canter 状态目录里 `server.json` 以外的文件；要 canter 改的，停下来写清楚交主控。
- 不往 canter 的终端里按键答题。
- 不给持久放行、清空上下文这类选项，不给「用户定的」第 1 条以外的框「允许」。
- 不改 Claude Code、Kimi 及其他家的行为，不改 fence、dispatch，不升级依赖。
- 不改 PATH 上装好的程序，不打包、不部署、不装技能、不切共用 helper；不碰真实 Slack、`~/.pony`、pony 服务。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。
- 不按项目名或路径批量杀进程（`pkill -f canter`、`pkill -f corral`、`pkill -f pony` 这类）。停自己起的进程用记下的 PID。
- 不放宽现有测试的断言、不缩短超时、不删测试来过。
- 碰到要拿主意的事（包括实测发现 canter 不接受 corral 答、需要 canter 改），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r58` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、对 paddock 的影响、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-11-11，ranch/dev-r58。首次实现提交为 `47b2c32`；当时真 canter 卡在目录信任框，四轮实测未执行。用户处理信任后的实测和再次全量检查见下面的追加记录。

- **做了什么**：canter 的 `interaction.request` 提供主会话最前面请求的编号、类型、工具、文件、available_decisions 和计划正文；有子框时不给可回答请求。新增通用可选 `answer_api` 与 `corral answer` 命令，pen 核对实例、前框、人工输入并通过有时限线程调用，canter 协议和公开文件读取全在 canter.rs。明确区分成功、alreadyResolved、明确拒绝和结果不明，无按键回退和自动重试。pony 增加 canter 选项／单行文字回答、shell/write/edit 单次批准、implement + alwaysAsk 计划批准；沿用聊天控件和确认框，chats 未改。发送前持久化消费记号，回答后撤控件；核对换框、人工输入、实例、频道，连点／重启不重答。连续 blocked 的前框编号变化也会发下一框提醒。CONTRACT、DESIGN、pony README 和 AGENT_USAGE 已同步。
- **验证了什么**：先分别复现状态没有请求字段、CLI 没有 answer、尚不支持文字／批准／计划回答、pony 没有选项／文字入口／批准按钮，再实现到通过；后续针对测试还先复现了连续 blocked 漏下一框提醒、旧线程换绑后仍答原仓库，均已修复。corral 假服务端针对测试 5 项通过，含四种回答、alreadyResolved、4xx、断连／超时／5xx／无效回执、请求及实例不符、禁止沙箱批准、真人输入、响应期间 pen 可用且禁止升级。pony canter 针对测试最终 10 项通过，含文字原样／多行拒绝、允许与拒绝连点、发送前落盘、重启、换绑、已在别处答过、拒绝／未知结果、安全批准范围和缺字段降级；隔离假 corral 命令协议 2 项通过。全部合成测试只用隔离状态和假服务，不碰真实 Slack。
- **全量检查**：按任务各运行一次 `cargo test --all-targets --no-fail-fast`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。clippy、fmt 通过。全量测试首次为 557 passed、6 failed、19 ignored：1 个 canter 旧测试完整对象预期缺新增 request 字段，补全预期后单项通过；其余 5 个升级测试报新 pen 就绪超时，未改升级逻辑或超时，代表用例单跑通过，随后整个升级测试组 17 项通过，首次超时原因未确定。不把这次全量测试写成全绿，未重复全量；全量之后修正的线程换绑问题已包含在最终 pony 10 项针对测试中。`git diff --check` 通过。Cargo 全程使用 `.target/r58`，所有命令前台等完。
- **真 canter**：仅启动自己的一份 `ranch/test-r58-dialogs`，独立临时 CORRAL_HOME 和临时 cwd，`role=test`、`CAIRN_DISABLE=1`，本分支 corral，未带 yolo、未指定模型。本机 canter 为 0.1.0 / 031d834，沿用已运行服务端。第一轮 prompt 要求调用 ask_user 给 Red/Blue 两个选项；启动 70 秒后仍是 starting，读屏确认 Folder trust，last_human_input 为 null，尚无已完成的模型轮次。按任务停下，未按键、未记信任、未绕开信任；保留测试 agent 供主控安排用户处理，测试完成后仍须 stop。四轮上限未变，后续不能把本轮写成真机通过。
- **拿主意的地方**：线程含 CR/LF 时明确未送，请用户改一行，不压缩或改写原答案；已发送的单行原文保持不变，原提醒最多显示前 200 字并指向线程全文。命令／文件／计划完整、可读且不超过 1000 字才给批准；沿用 R47c 的可读性门槛。请求字段放在已有 interaction 内，以便公共状态传递不掺 canter 协议细节。只有用户定的安全选项，没有持久放行、清空上下文或继续计划。
- **没做的事**：真机选项／文字／命令允许／计划批准四轮，以及真实服务端是否接受 clientName=corral 尚待验证；真实 Slack 验收留给用户和主控部署后做。未改 canter、其他仓库、chats、fence、dispatch、依赖、真实 pony 状态或服务，未操作已有用户 agent，未安装、打包、部署、合并或推送。
- **对 paddock 的影响**：公开约定仅兼容新增 interaction.request、answer 命令和能力；现有调用不用动，前端若要回答在自己的任务接。旧 CLI 不认识命令，新 CLI 遇旧 pen 返回不支持；新 pony 遇旧字段或入口回电脑处理，无按键回退。不改 hook 或共用 helper。
- **踩过的坑**：通用 state_started 只在状态变化时更新，不能用于区分连续 blocked 的 canter 请求，现以请求编号辅助提醒去重。线程原有路由只核对原仓库仍在任一频道绑定，新回答入口另核对当前频道仍属原仓库。首次全量出现上述升级就绪超时，定向复跑未复现。持久化测试最初误以为磁盘 buttons 在顶层，实际包在 state 中，修正测试读法后验证发送前已落盘；这次测试夹具错误未当作功能 RED 证据。

首次记录的待处理事项：由用户处理测试 agent 的目录信任框后继续四轮实测；此项已在下面的追加记录中解决。

### 用户处理目录信任后继续实测

2026-11-11，用户明确告知目录信任已处理，并授权继续四轮真 canter 实测和完整再跑一次 `cargo test --all-targets --no-fail-fast`。

- **做了什么**：沿用原测试 agent `ranch/test-r58-dialogs`、原实例、独立临时 CORRAL_HOME 和临时 cwd，没有新开真实 agent；使用本分支编出的 corral，canter 0.1.0 / 031d834。没有指定模型或强度，公开会话快照显示服务端配置为 `deepseek/deepseek-flash` / high，权限 `alwaysAsk`，自带沙箱仍开着。用户保持 attach；回答均经 `corral answer`，未按键答题。

| 轮次 | 实测动作 | 观察到的结果 |
| --- | --- | --- |
| 1：选项 | 原启动 prompt 已令 ask_user 提问 `R58 color?`，给 Red / Blue；对当前请求用 `--option Blue` | 回执 `confirmed:true,already_resolved:false`；框消失，公开快照的 userInput.answer 为 option / Blue；模型回复 `R58 option: Blue`，会话 idle |
| 2：文字 | ask_user 提问 `R58 short answer?`，对当前请求用 `--text 'R58 自写一句话'` | 成功回执；快照原样保存 text / `R58 自写一句话`；模型回复 `R58 text: R58 自写一句话`，会话 idle |
| 3：命令允许 | 在测试临时工作目录运行 `printf R58_ALLOWED > r58-allowed.txt`；核对普通 approval、tool=shell、available_decisions 含 accept，再用 `--allow-once` | 成功回执；命令项目 completed、exitCode=0；文件内容为 `R58_ALLOWED`，模型回复 `R58 command allowed`；权限仍为 alwaysAsk |
| 4：计划批准 | 仅通过公开 PATCH 接口把这份测试会话的 planMode 设为 true；submit_plan 的无害计划只要求回复一句话、不执行命令或改文件；用 `--approve-plan` | 成功回执；planProposal.answer 精确为 `decision:implement,approvalPolicy:alwaysAsk`，项目 completed；模型回复 `R58 plan approved. No commands, no file changes, no sub-agents.`；计划模式退出，权限仍为 alwaysAsk |

- **验证和清理**：四个请求均由带 `clientName:"corral"` 的本分支回答入口成功解决，真实 canter 接受 corral 作答，不需要修改 canter。最终公开快照恰有 **4 个 turn**，全部 completed，没有待答请求。测试 agent 已 `corral stop`，返回 exit_code=0、stopped_by=SIGHUP，无 remaining_sessions；隔离 `corral ls` 为空，公开会话状态确认 closed。原始快照和测试日志仅留临时验证目录，不进仓库。
- **再次全量检查**：按用户本轮要求，在前台完整运行 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r58 cargo test --all-targets --no-fail-fast`，退出码 0；29 个测试目标汇总为 **564 passed、0 failed、19 ignored**。其中升级测试 17 项全部通过，本次没有再出现新 pen 就绪超时；首次超时原因仍未确定，前面的失败记录保留。完整日志在 `/tmp/r58-validation.MSlF7n/tests-final.log`。本轮只改完成记录，未重复运行上轮已通过的 clippy、fmt；提交前 `git diff --check` 通过。
- **拿主意和踩坑**：第二轮首次 send 被 human_active（8）拒绝，确认距 last_human_input 已超过 30 秒后才正常送出；未用 force，未重送结果不明的请求，被拒那次没有增加 turn。除这次已有保护生效外，四轮没有协议或实现问题。计划模式只改测试会话，不改服务端配置、模型或全局权限。
- **没做的事及影响**：本轮没有改实现，只补实测记录；未碰真实 Slack、用户 pony 或其他 agent，未部署、合并、推送。对 paddock 仍为兼容新增，现有调用不用动；真实 Slack 验收仍由主控部署后与用户一起做。

本任务要求的实现与验证已完成，没有待主控决定的实现问题；交主控审查。

### 采纳审查建议

2026-11-11，按主控采纳的审查意见 1–6 和用户随后选择 A 的第 7 条修改；审查原文只读主仓库的 `R58-审查.md`，未改主仓库。

1. **旧提问线程恢复普通聊天**：确认不是原来的 blocked 框、请求编号已换或实例重开时，清掉该线程提问上下文；这句和之后的话按普通消息发送或排队，不加答案前缀、不按 Esc。原框还在时保留多行和不完整题目的拦截。新增测试覆盖 idle、working、换号、重开，完整／截断题目，多行首句及 pony 重启后的第二句。先确认旧实现没有清理上下文而失败，再修到通过。
2. **pen 状态解析 panic**：回答入口照 `steer_status` 用 `catch_unwind`，失败回 `state_check_failed`，written/submitted/confirmed 均为 false，不调用 canter。新增测试通过隔离 pen 的公开协议注入损坏 cursor，确认 pen 和合成 agent 身份不变、服务端没有收到回答；清理损坏夹具后正常 stop。
3. **拒绝只撤原请求按钮**：核对拒绝只结束与原 request 相同的按钮，真正提交前仍作废同实例其他按钮。新增测试模拟旧线程路由后、提交前前框已换成 r2，先复现 r2 控件被误撤，再确认修复后 r2 仍能回答。收尾时补测这个竞态下原回复也必须按普通消息排队，确认旧路径漏排后修复；使用最新状态清掉上下文并排队，不把选项字母展开成另一份普通话。
4. **结果文案**：unknown、starting、exiting 写「无法核对 canter 的框，没答；请回电脑处理」；alreadyResolved 写「这个框已经处理过（别处答过或已取消），没再答」。新增测试覆盖三种状态的线程回复和按钮刷新，以及已处理回执。
5. **线程选择选项**：整句为显示的选项字母或完整标签时送 Option／`--option`；其他文字原样送 Text／`--text`，不裁空白、不改写。新增测试覆盖字母、标签、带附加文字、首尾空白及不存在的字母。
6. **计划可读性**：计划不再套 shell 的连续空白上限；保留完整非空、1000 字、Cf 和除换行／制表符外的 Cc 检查。新增测试覆盖 Markdown 空行缩进、1000／1001 字、格式及控制字符；命令规则保持原样。
7. **write/edit 展示内容**：corral 兼容新增 `content:{text,truncated}` 和 `edits:{text,truncated}`，前者是原文，后者是包含 oldText/newText 的 edits 数组 JSON 文本；各按紧凑 JSON 16 KiB 截断并保留字符边界。pony 提醒显示路径及内容，确认框沿用 R47c 的前 200 字及未显示全提示。路径加内容不超过 1000 字、完整可读才给允许；缺字段、截断、超限、不可读只给拒绝并说明。edit 额外检查解码后的字符，空 write 内容与空 newText（删除）可正常显示。corral、pony 各补一条测试，覆盖 write/edit、Unicode 截断、内容显示、允许／拒绝、1000 字边界及控制字符。apply_patch 未接。

- **设计与影响**：DESIGN、CONTRACT、pony README 已同步主控决定；公开格式仅兼容新增，paddock 现有调用不用动。旧 corral 缺改动内容时 pony 不提供文件允许按钮；聊天插件、Claude Code 和 Kimi 的流程没有加 canter 特例。
- **验证了什么**：七条建议新增 8 条针对测试（第 7 条 corral、pony 各一条），均先看到目标行为缺失再改到通过；第 1、3 条均保留明确失败复现。直接相关回归：pony core **179 passed**（含原 Claude Code、Kimi 流程）；最后补上换框竞态后，重跑 pony canter 子集 **16 passed**；pony lib **46 passed**；corral canter 集成 **22 passed、1 ignored**，steer 集成 **16 passed、1 ignored**，canter 单元 **11 passed**，这些回归均 0 failed。`cargo clippy --all-targets -- -D warnings` 只跑一次，通过；`cargo fmt` 只跑一次，完成格式化；`git diff --check` 通过。Cargo 全程用 `.target/r58`，所有命令前台跑完；未重跑全套。回归日志在 `/tmp/r58-review-validation.MEXCcr/`，不进仓库。
- **踩过的坑**：损坏 cursor 的测试最初从 CLI 发起，会先触发 CLI 自己的状态预检，没到 pen 的 answer；改为直接走公开 pen 协议才复现并验证本条要保护的入口。线程测试补上合成首轮结束后，才验证第二条消息发送。这些夹具问题不当成功能 RED 证据。
- **没做的事**：本轮未跑全套、未开真实 agent、未碰真实 Slack 或 pony 服务，未部署、合并、推送。没有待主控决定的实现问题。

### 复核后修改

2026-11-11，按主控采纳的复核意见第 1–3 条修改；「可以不改」三条保持原样。

- **路径与内容分开**：write/edit 路径含 CR、LF、Tab、U+0085、U+2028、U+2029 时，只给拒绝并说明路径必须单行；这类无效路径转义显示。路径独立一行，用行内代码放在内容代码块外；代码块只显示实际内容，真空内容写「写入内容（0 字）」，缺字段说明也放块外，不再和字面的「（空内容）」混同。新增测试先复现带换行的路径仍有允许按钮，再覆盖六种字符、write/edit、正常路径、空／字面内容和缺字段。
- **文件内容空白门槛**：write 内容及 edit 解码后的 newText 连续空行超过 2 行，或去掉每行行首缩进后仍有超过 4 个连续空白，只给拒绝并说明。空白行计入连续空行，LF、U+2028、U+2029 都按行边界处理；正常空格／Tab 缩进保留。新增测试先复现三行空行仍允许，再覆盖 2／3 行、4／5 个空白、长串空白、Unicode 行分隔和正常缩进；oldText 不加这条限制，计划与 shell 保持既有规则。
- **状态核对失败的说法**：state_check_failed 单独映射到「无法核对 canter 的框，没答；请回电脑处理」；CONTRACT 写明退出码 1，状态算不出、回答未发出。既有回执测试增加该错误码，先确认原来错误地显示结果不明，再验证修正提示、连点和重启都不重答。
- **验证了什么**：两条新增测试及扩展的回执用例均先确认目标缺陷，再修到通过。最终 `cargo test -p pony --test core canter_` **18 passed**、`approval_` **28 passed**、`needs_you_` **7 passed**，均 0 failed、0 ignored（筛选组之间有重叠）；覆盖 canter 的新行为及 Claude Code、Kimi 的直接相关批准／显示回归。`cargo clippy --all-targets -- -D warnings`、`cargo fmt` 各只跑一次，均退出 0；`git diff --check` 通过。全部 Cargo 命令用 `.target/r58`，命令均前台跑完，没有重跑全套。日志只留 `/tmp/r58-recheck-validation.Ini6S7/`，不进仓库。
- **拿主意与踩坑**：沿用已有确认框的前 200 字与未显示全提示，路径和内容使用同一份分隔后的正文。edit 的 600 个换行经 JSON 转义已先超过 1000 字门槛，测试夹具改用仍低于该门槛的 200 个换行，专门验证新增空白规则；write 仍测 600 个换行，未改上限或放宽断言。
- **文档、范围及影响**：DESIGN、CONTRACT、pony README 已同步。本轮只改 pony 内部核对、显示和提示，公开命令及字段不变，paddock 不用动。未改复核「可以不改」三条，未接 apply_patch，未跑全套，未开真实 agent，未碰真实 Slack 或 pony 服务，未部署、合并、推送。没有待主控决定的事。

### 第二轮复核后修改

2026-11-11，按主控采纳的第二轮复核「新的意见」第 1 条修改，内容总行数不另设上限。

- **做了什么**：write/edit 路径在单行、完整可读核对之外，另过 `readable_command`；连续空白超过 4 个只给拒绝并说明。write 内容及 edit 的 newText 每行缩进最多 32 个空格单位，Tab 按 4、其他空白字符按 1，空白行也检查；超过只给拒绝，并给出缩进超限的明确原因。正常缩进保留，oldText、计划与 shell 的规则不变。内容总行数不限，原有 1000 字、连续空行及行内空白规则保留。DESIGN 和 pony README 已同步。
- **针对测试**：新增路径长空白、超长缩进各一条测试，先分别复现 5 个路径空格和 33 个缩进空格仍给允许，再修到通过；均覆盖 write/edit。路径覆盖 4／5 个空格、300 个空格加路径后缀和 Unicode 空白；缩进覆盖 32／33 个空格、8／9 个 Tab、空格混 Tab、600 个空格及空白行，正常边界仍允许。另确认 300 行短内容仍允许，oldText 的长缩进不受新上限影响。
- **验证结果**：最终 `cargo test -p pony --test core canter_` **20 passed**，`cargo test -p pony --test core approval_` **28 passed**，均 0 failed、0 ignored（筛选组有重叠），覆盖直接相关的 canter 及 Claude Code、Kimi 批准回归。`cargo clippy --all-targets -- -D warnings`、`cargo fmt` 各一次，均退出 0；`git diff --check` 通过。Cargo 全程使用 `.target/r58`，所有命令前台跑完。日志仅留 `/tmp/r58-second-review.50Qo9x/`，不进仓库。
- **拿主意与踩坑**：按主控给定阈值实现；没有额外加总行数限制。没有新的踩坑。
- **没做的事及影响**：仅改 pony 内部批准核对和原因提示，公开命令／字段不变，paddock 不用动。第二轮的「可以不改」意见及此前保留项没有改；未接 apply_patch，未跑全套，未开真实 agent，未碰真实 Slack 或 pony 服务，未部署、合并、推送。没有待主控决定的事。

## 主控审查

2026-11-11，ranch/main。
- 看过 diff：corral 公共代码只加 `answer_api` 挂点、`answer` 命令和 pen 的回答线程（实例沿用 pen 通用核对），canter 协议全在 `canter.rs`；`chats/` 没动；「允许」在 corral、pony 两层都只放 shell/write/edit 的普通 approval 和 accept，计划只发 implement + alwaysAsk。真 canter 四轮通过，证实 canter 接受 corral 作答，不需要 canter 改。
- 交叉审查（ranch/review-r58，Claude Code opus xhigh，`docs/任务/R58-审查.md`）：首审可以合并、建议改 7 条，主控采纳 1–6，第 7 条（显示 write/edit 改动内容）用户选 A 一起做；第一轮复核挡出必须改 1 条（路径带换行可伪装），连同建议改 2、3 改好；第二轮复核可以合并，主控再采纳路径空白和缩进上限一条，主控自己看 diff 核对。apply_patch 改文件仍只给「拒绝」，以后和内容显示一起再看。
- 主控在最终 `963bdb9` 上跑 `cargo test --all-targets --no-fail-fast` 576 过、0 失败、19 忽略，clippy、fmt 通过。首次全量的升级就绪超时之后三次全量都没再出现。
- 对 paddock：兼容新增（`corral answer`、`capabilities.answer`、`interaction.request`），现有调用不用动。
- 结果：返工 1 次（交叉复核的必须改：路径带换行可伪装；其余几轮是采纳建议改）
