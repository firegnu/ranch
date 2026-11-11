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
