# R48g：Kimi 的提问框、批准框、后台提问和打断（调研与方案）

2026-10-24，ranch/dev-r48g1。只出方案，不改代码。依据：任务书 `docs/任务/R48g1-Kimi提问和批准框调研与方案.md`；R48b 补测、R48d 审查第 4、5 条、R42 §2–§3 和用户的决定、R45–R47d 的任务文件和审查；pony、corral 现有代码；Kimi Code 2.3.1 源码（提交 `684bae9`，和 R48b 同一版，只读）；本轮真 Kimi 实测 10 轮。

标记：**【实测】** 本轮真 Kimi 跑出来的；**【源码】** 只看了 Kimi 或 ranch 源码；**【推测】** 由前两者推出来、没实测；**【未知】** 查不到。

## 结论

1. **提问框能用按键答完，答案在 wire 里有原文。**【实测】单选按数字、多选按数字勾选再按 Tab、「Other」按最后一个数字进入输入框打字再回车，最后在 Submit 页按 `1`。答完后主会话 wire 的 `interaction.resolved` 带着每道题的答案原文（以题目为键），corral 可以从这里确认「答上了、答的是什么」。
2. **Claude Code 那条路（按 Esc 关框、等空下来、把答案当普通消息送）在 Kimi 上要按两次 Esc。**【实测】按一次只关框，模型接着跑（R48b）；连按两次（一条 `keys esc esc`），第一下关框，第二下在约 50 ms 后让这一轮 `cancelled`，状态变成 idle、`interrupted:true`。之后照 pony 现在的前缀把答案送过去，模型认得（回「收到，你选了 B：蓝。」）。
3. **批准框：`1` 允许一次，`2` 本会话都允许，`3` 拒绝，`4` 拒绝并写理由，数字按下就生效；Esc 也是拒绝。**【实测】【源码】只拒绝（一次 Esc）时，模型拿到「被拒绝」后会自己换个办法接着干（实测 3 秒后又弹了一个框）；两次 Esc 才是「拒绝并停下这一轮」。pony 现在在 Kimi 批准框上给的「拒绝」按钮和「停」都只按一次 Esc，是同一个问题。
4. **后台提问：corral 现在读屏会报 `blocked`，但没有题目、`state_started` 为空。**【实测】所以 pony 能发「在等你」，看不到题目，也不给按钮；R48d 审查说的「多半看不到」要改成「看得到在等，看不到题目」。答完后状态从 idle 回到 working，是因为 Kimi 收到答案会**自己开一轮**（通知里带着答案）来处理，这轮结束就回 idle，不是卡住。要让 pony 看到题目，corral 要跨轮记住还没答的提问。
5. **打断：干活中按一次 Esc；停在提问框、批准框上按两次；空闲时不能按两次。**【实测】干活中一次 Esc，连跑着的 `sleep` 进程也一起停了。空闲时连按两次 Esc 会打开「Select messages to undo」（回退选择器），而 corral 还报 `idle`，这时再 `send` 一个回车就会选中一个回退点。
6. **推荐：**
   - 前台提问（模型停着等）：线程里写答案、选项按钮都**同 Claude Code**，只是 Esc 按两次；
   - 后台提问（模型没停）：不按 Esc（会停掉它别的活），等它空下来，**在框里按键答**；
   - 「拒绝」「停」：停在框上时按两次 Esc；
   - 「允许这一次」：只给 Bash 命令框，按 `1`。核对和确认都用 corral 从 wire 读出的结构化数据，不比屏幕：Kimi 的屏幕尾部靠不住（批准框弹出 10 秒后会多出一行提示）。
7. **corral 要补的都是 Kimi 的兼容新增**，只从 wire 读，不用换共用 helper：Bash 批准框的 `command`、后台提问的 `questions` 和 `state_started`、最近一次框的结果 `resolved`、非 Bash 批准框的一句话说明 `action`；可选再加一条读屏规则，把回退选择器报成 `blocked`。paddock 不用动。

## 依据与边界

- **环境**【实测】：本机 macOS，Kimi Code 2.3.1，界面显示 `K2.8 Preview thinking: max`，没指定模型；普通模式（没加 `--auto`），带 `KIMI_CODE_TRUST_WORKSPACE=1`、`CAIRN_DISABLE=1`。用 PATH 上的 corral（`25b4598`），隔离的 `CORRAL_HOME`。测试 agent 两个：`ranch/test-r48g1-q`（第 1–9 轮）、`ranch/test-r48g1-bg`（第 10 轮），都带 `role=test`，cwd 是一个临时 git 仓库，用完 `corral stop`，确认没有残留进程。`~/.kimi-code/config.toml`、`tui.toml` 测前、测后 sha256 相同；没读凭证、token、`device_id`、`server.token`。没碰 Slack、`~/.pony` 和用户的 agent。
- **原始输出**：放在 worktree 外的本会话 scratchpad：`/private/tmp/claude-501/-Users-firegnu-Developer-personal-projs-ranch-worktrees-r48g1/43385b74-b528-41fb-afb2-be195271079d/scratchpad/raw/`（下文写成 `raw/`，不进仓库）。报告里只摘片段，屏幕片段按屏幕行恢复、去掉边框线。
- **Kimi 源码**：下文写的源码位置都是 `684bae9` 里的路径，`apps/kimi-code/src/tui/` 简写为 `tui/`，`packages/agent-core-v2/src/` 简写为 `core/`。

**轮次账**（每轮一句话的短任务；按键不算轮，全部列出）：

| 轮 | 送的话（大意） | 之后按的键 | 结果 | 原始输出 |
|---|---|---|---|---|
| 1 | 一次问 3 道题：主食（单选）、配菜（多选）、饮料（单选） | `2`；`1` `3`；`tab`；`3`；`text:果汁 R48G1` `enter`；`1` | 答案原样进了 wire，模型复述正确 | `raw/r1-*` |
| 2 | 问 1 道题；「没回答就关掉的话，去 Bash 跑 sleep 20」 | `esc esc`（一条） | 关框，52 ms 后这一轮 cancelled，没调别的工具 | `raw/r2-*` |
| 3 | pony 现在的前缀加「B」 | — | 「收到，你选了 B：蓝。」 | `raw/r3-*` |
| 4 | Bash：`echo R48G1_A > a.txt && cat a.txt` | `1` | approved，文件写了，回 A_DONE | `raw/r4-*` |
| 5 | Bash 写 b.txt；被拒就改写 c.txt | 第一个框 `esc`；第二个框 `esc esc` | 拒后 3 秒弹出 c.txt 的框；两次 Esc 拒绝并停下 | `raw/r5-*` |
| 6 | background:true 问 1 道题，然后立刻结束这一轮 | 空闲时一条 `keys`：`3` `text:梨 R48G1` `enter` `1` | 状态读屏报 blocked、无题目；答完 Kimi 自开一轮，回「水果：梨 R48G1」 | `raw/r6-*` |
| 7 | Bash：`sleep 30 && echo SLEPT > s.txt` | `1`；跑起来 3 秒后 `esc` | cancelled，bash、sleep 进程都没了，s.txt 没写 | `raw/r7-*` |
| 8 | Write 写工作目录里的 w.txt | — | 没弹框，直接写了 | `raw/r8-*` |
| 9 | Write 写工作目录外（临时目录里）的文件 | `esc esc` | 弹「Write this file?」；拒绝并停下，文件没写 | `raw/r9-*` |
| 10 | （新 agent）background:true 问 1 道题，然后结束 | 先试 `send`；再 `esc esc` | send 被拒 `not_idle`；关框，Kimi 自开的通知轮 39 ms 后 cancelled | `raw/r10-*` |

另外，第 9 轮后空闲时只按键：`esc esc` 打开回退选择器（`raw/k1-*`），再按一次 `esc` 关掉（`raw/k2-*`）。共 10 轮，在 5–10 轮的额度内。

## 1. 提问框

### 1.1 画面和按键

**【实测】** 第 1 轮框刚弹出时（`raw/r1-screen-0.txt`）：

```text
question
 主食   (○) 配菜  (○) 饮料   Submit

? R48G1 主食选哪个？
→ [1] 面条
  [2] 米饭
  [3] Other

↑↓ select  1-3 / ↵ choose  ←/→/tab switch  esc cancel
```

**【实测】【源码】** 按键（`tui/components/dialogs/question-dialog.ts`）：

| 要做的 | 按什么 | 说明 |
|---|---|---|
| 单选 | 数字 `N` | 选第 N 项，**自动跳到后面第一道没答的题**；后面都答了就跳到 Submit 页（185–190、364–369 行） |
| 多选 | 数字 `N` 勾上或取消，再按 `tab`（或 `→`） | 勾选不跳页（192–194、312–319 行） |
| 自己写（Other） | 数字 `选项数+1` → 打字 → `enter` | 进入单行输入框，回车保存；单选题保存后自动跳（301–310、336–362 行）。输入只有一行，回车就是保存 |
| 换题 | `tab` / `→` 下一页，`←` 上一页 | 最后一页是 Submit |
| 提交 | Submit 页按 `1`（`2` 是 Cancel，等于关框） | 回车按当前高亮，进 Submit 页时默认高亮 Submit（253–261、276 行） |
| 关框 | `esc`（`ctrl-c`、`ctrl-d` 同） | 答案为空（128–136 行） |

- 一次 1–4 道题、每题 2–4 个选项（`core/agent/tools/ask-user-question/ask-user-question.ts` 21–23、67–71 行），加上 Other 最多 5 项，数字 1–5 够用。
- 每道题都有 Submit 页，单题也要最后按一次 `1`。没答完的题也能提交，只交答了的（417–434 行）。
- 第 1 轮实际按法：`2`（主食 米饭，自动跳到配菜）→ `1` `3`（勾青菜、鸡腿）→ `tab` → `3`（饮料的 Other）→ 打「果汁 R48G1」→ `enter`（保存，跳到 Submit）→ `1`。提交前的确认页（`raw/r1-screen-5.txt`）：

```text
question   (✓) 主食  (✓) 配菜  (✓) 饮料   Submit
Review your answer before submit
Q  R48G1 主食选哪个？    →  米饭
Q  R48G1 配菜要哪些？    →  青菜, 鸡腿
Q  R48G1 饮料选哪个？    →  果汁 R48G1
Ready to submit your answers?
→ [1] Submit
  [2] Cancel
↑↓ select  1/2 choose  ↵ confirm  ←/→/tab switch  esc cancel
```

- 第 6 轮把「选 Other → 打字 → 回车 → 提交」放进一条 `corral keys` 送（每段间隔 50 ms），也一次成功。
- **【实测】框开着时画面不动**：第 1 轮框弹出后 13 秒内 `last_output` 一直是 `1792843946.748`（`raw/r1-status-blocked-later.json`）。

### 1.2 答完以后看到什么

**【实测】** 主会话 wire（`raw/r1-wire.txt`）：

```jsonl
{"type":"interaction.request","agentId":"main","id":"question_11ca…","kind":"question","request":{"turnId":0,"questions":[…]},"time":1792843946744}
{"type":"interaction.resolved","agentId":"main","id":"question_11ca…","response":{"answers":{"R48G1 主食选哪个？":"米饭","R48G1 配菜要哪些？":"青菜, 鸡腿","R48G1 饮料选哪个？":"果汁 R48G1"},"method":"enter"},"time":1792843988212}
{"type":"turn.ended","agentId":"main","turnId":0,"reason":"completed",…,"time":1792843992086}
```

- 答案以**题目原文**为键；多选用 `, ` 连起来；Other 是打的原话；没答的题不出现（`tui/reverse-rpc/question/handler.ts` 43 行起）。模型收到的工具结果是同样的 JSON（`raw/r1-wire-toolresult.txt`）。
- 关框（Esc）时 `response` 是 `null`，工具结果是 `{"answers":{},"note":"User dismissed the question without answering."}`（`raw/r2-wire.txt`，和 R48b 一致）。
- 钩子：`PostToolUse` 的 `tool_output` 里也有答案（R48b 附录），但钩子分不出主会话和子 agent（R48b §7）。corral 现在只记事件名 `Kimi:PostToolUse`（`raw/r1-events.txt`）。
- corral 状态：提交后变 `working`，回复写完变 `idle`，`corral reply` 是「你选择了主食米饭，配菜青菜和鸡腿，饮料是自定义的「果汁 R48G1」。」（`raw/r1-reply.json`）。
- **corral 怎么确认「答上了、答的是什么」**：用主会话 wire 里和请求同一个 `id` 的 `interaction.resolved.response.answers`，这是唯一既分得清主会话、又带结构化答案的地方。corral 现在读到它只把状态改回 working，不对外给答案（见 §5.3 c）。

### 1.3 Claude Code 那条路在 Kimi 上行不行

- **按一次 Esc 不行。**【实测（R48b）】【源码】Esc 只是关框，工具返回「用户没回答」，模型接着跑下一步。所以 pony 按完等不到 idle，要等它把这一轮按自己的猜测做完，排着的答案才送得进去。
- **连按两次 Esc 可以。**【实测】第 2 轮（`raw/r2-wire.txt`、`raw/r2-status-after.json`）：

```jsonl
{"type":"interaction.resolved",…,"response":null,"time":1792844048801}
{"type":"context.append_loop_event",…"tool.result",…"User dismissed the question without answering.",…"time":1792844048802}
{"type":"turn.ended","agentId":"main","turnId":1,"reason":"cancelled",…,"time":1792844048853}
```

  第一下关框，第二下在 52 ms 后停掉这一轮；模型没来得及调「sleep 20」。corral：`state:"idle"`、`interrupted:true`、`last_event:"Interrupt"`。
- **之后送答案，模型认得。**【实测】第 3 轮照 pony 现在的前缀（R45/R46）送：

```text
（这是对刚才提问的回答；选项：A 红 B 蓝；关闭提问框不表示放弃作答）
B
```

  回复「收到，你选了 B：蓝。」，没有接着去跑原来说好的 sleep（`raw/r3-reply.json`）。
- **第二下 Esc 为什么能停下**【源码】：Kimi 的输入框收到 Esc 时，只要这一轮还在跑就取消这一轮（`tui/controllers/editor-keyboard.ts` 259–263 行，`streamingPhase !== 'idle'` → `cancelCurrentStream`）。前台提问开着时这一轮一定在跑（模型在等这个工具），所以第二下一定落在「跑着」上。**空闲时就不一样了**：见 §4。

## 2. 批准框

### 2.1 画面和按键

**【实测】** 第 4 轮 Bash 批准框（`raw/r4-screen-0.txt`）：

```text
▶ Run this command?
cwd: /private/tmp/r48g1.D4m0pi/kw
$ echo R48G1_A > a.txt && cat a.txt

▶ 1. Approve once
  2. Approve for this session
  3. Reject
  4. Reject with feedback

↑/↓ select · 1/2/3/4 choose · ↵ confirm
```

第 9 轮往工作目录外写文件的框（`raw/r9-screen-0.txt`）：

```text
▶ Write this file?
/tmp/r48g1.D4m0pi/outside.txt
1  R48G1_O
▶ 1. Approve once  2. Approve for this session  3. Reject  4. Reject with feedback
↑/↓ select · 1/2/3/4 choose · ↵ confirm · ctrl+e preview
```

**【源码】** 按键（`tui/components/dialogs/approval-panel.ts` 261–318 行，选项在 `tui/reverse-rpc/approval/adapter.ts` 7–12 行）：

| 按什么 | 结果 |
|---|---|
| `1` | Approve once（`decision:"approved"`），**按下就生效，不用回车** |
| `2` | Approve for this session（`approved`，`scope:"session"`，以后同类不再问） |
| `3` | Reject（`rejected`） |
| `4` | Reject with feedback：打开输入框写理由 |
| `↑` `↓` + `enter` | 选当前高亮项，默认高亮 1 |
| `esc`（`ctrl-c`、`ctrl-d` 同） | `rejected` |

### 2.2 按了以后

- **允许一次**【实测】第 4 轮按 `1`：wire `{"type":"interaction.resolved",…,"response":{"decision":"approved"}}`，命令执行，a.txt 内容 `R48G1_A`，这一轮 completed，回复 A_DONE（`raw/r4-wire-resolved.txt`）。
- **只拒绝（一次 Esc 或 `3`），模型会接着干。**【实测】第 5 轮第一个框按一次 `esc`（`raw/r5-wire.txt`、`raw/r5-poll1.txt`）：`decision:"rejected"`，工具结果是 `Tool "Bash" was not run because the user rejected the approval request.`（`isError:true`）；状态 working 约 3 秒后，又弹出一个新框（`echo R48G1_C > c.txt`，按提示「被拒就改写 c.txt」）。【源码】拒绝只是让这次工具调用返回一个错误结果，不停这一轮（`core/agent/toolApproval/toolApprovalService.ts` 226–229 行；`denyToolExecution` 只返回 `{output, isError:true}`，`core/agent/toolExecutor/beforeToolExecuteEvent.ts` 19–21 行）。这和 Claude Code 不一样：Claude Code 拒绝后这一轮就停下等你。
- **两次 Esc = 拒绝并停下。**【实测】第 5 轮第二个框、第 9 轮 Write 框都是：`rejected` 后约 50 ms 这一轮 `cancelled`，状态 idle、`interrupted:true`，b.txt、c.txt、outside.txt 都没写（`raw/r5-poll2.txt`、`raw/r9-poll.txt`、`raw/r9-wire-resolved.txt`）。

### 2.3 corral 现在给什么、wire 里有什么

- **【实测】** corral：`state:"blocked"`、`last_tool:"Bash"`（Write 框是 `"Write"`）、`state_started` = 框弹出的时间（毫秒精度，和 wire 的 `interaction.request.time` 相同），`command:null`——`command` 只给 Claude Code（`crates/corral/src/cli.rs` 188 行）。
- **【实测】** wire 的 `interaction.request.request`（`raw/r4-wire-request.txt`、`raw/r9-wire-request.txt`）：
  - Bash：`toolName:"Bash"`、`action:"Running: echo R48G1_A > a.txt && cat a.txt"`、`display:{"kind":"command","command":"echo R48G1_A > a.txt && cat a.txt","cwd":…}`。`display.command` 是原文；`action` 太长时会被 Kimi 截成「…」（R48b 样本：`Running: /bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 1…`）。
  - Write：`toolName:"Write"`、`action:"Writing /tmp/r48g1.D4m0pi/outside.txt"`、`display:{"kind":"file_io","operation":"write","path":…,"content":"R48G1_O"}`。
- **【实测】默认模式下写工作目录里的文件不弹框**（第 8 轮），弹框的主要是 Bash 和工作目录外的读写。

### 2.4 屏幕尾部在 Kimi 上靠不住

- **【实测】** 第 4 轮框弹出约 9.5 秒后，`last_output` 从 `…53.577` 变成 `…63.065`，之后不再变（`raw/r4-last-output-poll.txt`）。屏幕流末尾变成了工具那一行补上的 `Press Ctrl+B to run in background`，批准框已经不在 `corral read` 的末尾了（`raw/r4-screen-1.txt`）。【源码】Bash 工具行在开始 10 秒后加这句提示（`tui/components/messages/tool-call.ts` 70–71 行，`DETACH_HINT_DELAY_MS = 10_000`），等批准的时候也算在里面。
- **【实测】** 第 9 轮的 Write 框在 `corral read --bytes 4000` 里也不在末尾：框画完以后，后面又是一段对话重画。
- 所以 R47c 那套「屏幕尾部和发提醒时一字不差」「`last_output` 没变」放到 Kimi 上，10 秒以后点的按钮几乎都会被挡掉；读屏认框也容易认错。Kimi 应该改用 wire 里的结构化数据（§5.2）。

### 2.5 哪些批准算危险（照 R42 §2.6）

- **一律不给「允许」**：
  - `2. Approve for this session`：改变以后的行为，和 Claude Code 的「永远允许」一类；
  - `4. Reject with feedback`：要打字，不需要；
  - 非 Bash 的批准（写、改工作目录外的文件，联网，开子 agent，MCP，计划模式等）：第一版只给「拒绝」。原因是没有像命令那样可以原样核对、原样给用户看的一段内容，文件内容可能很长；
  - 命令看不全：corral 的 `command` 截断了，或者过不了 R47c 的检查（太长、不可见格式字符或控制字符、连续空白超过 4 个）；
  - Kimi 自己会标红的危险命令：递归或强制删除、`sudo`、`curl|sh`、`dd of=`、`mkfs`、写裸设备、`chmod 777`、fork bomb（`tui/reverse-rpc/approval/adapter.ts` 213–222 行 `DANGER_PATTERNS`）。电脑前的人会看到标红，手机上看不到，不远程批准；
  - 同时开着别的框：后台提问在前面时，批准框排在后面（§3.1），这时按 `1` 会落到提问上。
- **给**：Bash 命令框的 `1. Approve once`，带 R47c 那样的 Slack 确认框。

## 3. 后台提问

### 3.1 怎么出现

- **【源码】** 模型调 `AskUserQuestion` 时带 `background:true`（要有 TaskList、TaskOutput、TaskStop 三个工具，默认有）：工具立刻返回一个 task_id，提问框照样弹出来、一直开着，模型接着干或者结束这一轮；用户答了或关了，Kimi 把结果当作一条通知交给模型（`core/agent/tools/ask-user-question/askUserQuestionTool.ts` 104–158 行，`question-background-task.ts`，`core/agent/task/taskService.ts` 1129 行起 `loop.notify`）。
- **【实测】** 第 6 轮 wire（`raw/r6-wire-1.txt`）：`interaction.request`（`turnId:5`）→ 1 ms 后 `tool.result`（`task_id: question-z6yz8oqo / status: running / next_step: Continue your work…`）→ 2 秒后这一轮 `completed`，回复 BG_ASKED。之后提问框一直开着，键盘归它（后面按的数字都进了框）。
- **【源码】两个框同时要开时排队**（`tui/reverse-rpc/modal-coordinator.ts` 68–89 行）：先开的在前面；不同类的后来者排在后面，等前一个关了再出来；同类的后来者（两个提问或两个批准）顶替前一个显示。所以后台提问开着时模型再要批准，批准框排在提问后面，屏幕上还是提问框。

### 3.2 corral 现在为什么看不到题目

- **【源码】** `crates/corral/src/agents/kimi.rs` 只按主会话的当前轮读 wire：`interaction.request` → blocked、带题目；紧接着的 `tool.result`（290 行，不看是哪一轮）→ working、没有题目；`turn.ended` → idle。
- **【实测】** 读屏规则（`kimi.rs` 103–107 行：最下面 4 行同时有 `select`、`choose`、`←/→/tab switch`、`esc cancel`）认出了框，把状态改成 `blocked`。但 `state_started` 只在对外状态和读 wire 得到的状态相同时才给（`cli.rs` 179 行），读屏改出来的 blocked 就是 `null`；题目也已经被 `tool.result` 清掉了。第 6、10 轮框开着时都是这样（`raw/r6-poll1.txt`、`raw/r10-status-blocked.json`）：

```json
{"state":"blocked","state_started":null,"last_event":"Stop","last_tool":"AskUserQuestion","q":false}
```

- **pony 现在会怎样**【源码】：两轮都是 blocked，pony 发「在等你」提醒（`state_started` 为空按 0 去重）；`displayed_questions` 拿不到题目，就附 `corral read` 的屏幕末尾 20 行。可第 6 轮那时的末尾是对话内容（`● BG_ASKED`），不是提问框（`raw/r6-screen-0.txt`）。Kimi 加 `AskUserQuestion` 不给按钮（`crates/pony/src/core/engine/buttons.rs` 112–114 行）。所以 pony 是**看得到在等你，看不到题目**。
- **【实测】框开着时别的话送不进去**：`corral send` 返回 `not_idle`（`raw/r10-send-while-bg.json`）。这是好事，否则文字和回车会打进提问框，数字会选中选项。但也意味着：用户不答也不关，pony 排着的话就一直送不进去。
- 读屏规则认不认得出取决于框在屏幕最下面 4 行。两次实测都认出来了，没试别的窗口大小（【未知】）。

### 3.3 状态为什么会从 idle 回到 working

**【实测】** 第 6 轮答完（`raw/r6-wire-2.txt`、`raw/r6-poll2.txt`）：

```jsonl
{"type":"interaction.resolved",…,"response":{"answers":{"R48G1 水果选哪个？":"梨 R48G1"},"method":"enter"},"time":1792844367309}
{"type":"turn.prompt","agentId":"main","input":[{"type":"text","text":"<notification … type=\"task.completed\" …>\nTitle: Background question answered\n…<answer>\n{\"answers\":{\"R48G1 水果选哪个？\":\"梨 R48G1\"}}\n</answer>\n</notification>"}],"origin":{"kind":"task",…},"turnId":6,"time":1792844367327}
{"type":"turn.ended","agentId":"main","turnId":6,"reason":"completed",…,"time":1792844371551}
```

- 两个原因叠在一起：
  1. corral 读到 `interaction.resolved` 时，id 还等于记下的那个提问（`kimi.rs` 314 行，轮结束时没清），就把状态改成 working；
  2. 18 ms 后 Kimi **自己开了新的一轮**（`origin.kind:"task"`），把答案交给模型，模型回「水果：梨 R48G1」，4.2 秒后这轮结束，corral 回到 idle。
- 所以这个 working 是真的在干活，结束了就回 idle，本轮样本里没有卡住。关框也一样，会开一轮「Background question dismissed」（第 10 轮）。
- 这一轮不是用户输入开的：corral 的钩子只认 `origin_kind=user` 的 TurnStarted（`kimi.rs` 50 行），`turn_started` 没变（`raw/r6-poll2.txt` 里一直是 `1792844290.261`）。pony 判断新的一轮靠 `turn_started`，这轮的回复不会转回 Slack。

### 3.4 要看到它需要什么

corral 读 wire 时，要**跨轮记住还没答的主会话提问**：按 `id` 记 `interaction.request(kind=question)`，见到同 id 的 `interaction.resolved` 才清掉，不因为 `tool.result`、`turn.ended`、新一轮而丢。这道题在最前面、状态是 blocked 时，对外给 `questions`（标明后台）、`state_started`（框弹出的时间）。

另外，wire 的读取游标现在按 `turn_started` 重置，只读这一轮之后的事件（`kimi.rs` 186–222、255 行）。后台提问可能是更早的轮问的，所以游标重置或从头重放时也要能找回还开着的提问。细节见 §6。

## 4. 打断

| 场景 | 按什么 | 结果 | 依据 |
|---|---|---|---|
| 干活中（含跑着的 Bash） | 一次 `esc` | 这一轮 cancelled；工具结果 `Interrupted by user`；bash 22833、sleep 22834 两个进程都没了，s.txt 没写 | 【实测】第 7 轮 `raw/r7-ps-before.txt`、`raw/r7-ps-after.txt`、`raw/r7-wire.txt`；R48b |
| 前台提问框 | `esc esc` | 关框后 52 ms cancelled | 【实测】第 2 轮 |
| 前台提问框 | 一次 `esc` | 只关框，模型接着跑 | 【实测】R48b；【源码】 |
| 批准框 | `esc esc` | 拒绝后约 50 ms cancelled | 【实测】第 5、9 轮 |
| 批准框 | 一次 `esc` | 只拒绝，模型换办法接着干 | 【实测】第 5 轮 |
| 后台提问开着，它空闲 | `esc esc` | 第一下关框，Kimi 立刻开了一轮「已关掉」通知，第二下让这轮 39 ms 后 cancelled | 【实测】第 10 轮 `raw/r10-wire.txt` |
| 后台提问开着，它在干活 | `esc esc` | 第一下进提问框（关框），第二下停掉在跑的这一轮 | 【推测】【源码】键盘归最前面的框；没测 |
| 空闲、没有框 | 不按 | 一次 `esc` 只记一下；600 ms 内第二次 `esc` **打开回退选择器** | 【实测】`raw/k1-*`；【源码】`editor-keyboard.ts` 264–271 行、`constant/kimi-tui.ts` 24 行 |

**怎么确认停了**【实测】：每次停下后，corral 都是 `state:"idle"`、`interrupted:true`、`last_event:"Interrupt"`（第 2、5、7、9、10 轮的 poll 文件），和 pony 现在的「停」确认条件（idle 且 interrupted，`engine.rs` 977 行）对得上。

**空闲时按两次 Esc 的后果**【实测】（`raw/k1-idle-escesc-screen.txt`、`raw/k1-idle-escesc-status.json`）：

```text
Select messages to undo  ↑↓ navigate · Enter select · Esc cancel
  用 Bash 工具执行：echo R48G1_B > b.txt …
  …
❯ 用 Write 工具（不要用 Bash）创建文件 /tmp/r48g1.D4m0pi/outside.txt …
```

此时 corral 报 `state:"idle"`。再 `corral send` 的话，文字之后的回车会选中一个回退点。再按一次 `esc` 就关掉了（`raw/k2-close-undo-screen.txt`）。所以 pony 只能在确认停在框上时按两次；干活中按一次。

## 5. 推荐方案

### 5.1 pony 在 Kimi 上各走什么路

| 动作 | Claude Code 现在 | Kimi 推荐 |
|---|---|---|
| 前台提问：线程里写答案 | Esc 关框（这一轮也停了）→ 等 idle → 加前缀送（R45、R46） | **`esc esc`** → 等 idle 且 `interrupted` → 加前缀送。前缀、排队、核对都同 Claude Code |
| 前台提问：选项按钮 | 都选好后拼成文字，走上一条（R47b） | 同 Claude Code |
| 后台提问：线程里写答案、选项按钮 | （Claude Code 没有） | 不按 Esc。**等它空下来，在框里按键答**：单选按数字；多选按数字再 `tab`；线程里写的话填进第 1 题的 Other（打字再 `enter`）；最后在 Submit 页按 `1`。按完看 corral 给的答案对不对 |
| 批准框「拒绝」 | 一次 Esc | **`esc esc`**（拒绝并停下这一轮，和 Claude Code 一样等你说下一步） |
| 批准框「允许这一次」 | 读屏认框，按选项号 | 只给 Bash 命令框，按 **`1`**。命令、核对、确认都用 corral 的结构化字段，不读屏 |
| 「停」「打断」 | 一次 Esc（amp 两次） | 干活中一次 Esc；停在框上（提问、批准、后台提问）两次；空闲不按（同现在） |

**前台提问为什么不在框里按键答**：Kimi 的框能按键答（§1.1），模型还能在同一轮里拿到结构化答案。但推荐还是走 Claude Code 那条路，理由是：

- 只按 Esc，Esc 在 Kimi 所有框上都是拒绝或关闭，绝不会批准任何东西；数字键要是落到批准框上，`1`、`2` 就是批准；
- 线程里随便写的话（多道题、多行）原样送过去，不用硬塞进单行的 Other；
- 是 pony 现成的路，改动最小，答完的回复照常转回线程；
- 实测走通了（第 2、3 轮）。

代价是这一轮被打断、再开一轮：模型先看到「用户没回答就关了」「被打断」，再看到带前缀的回答。第 3 轮模型直接认了答案。

**后台提问为什么不一样**：

- 按 Esc 关框再送文字，第二下 Esc 会停掉它正在干的别的活；
- Kimi 本来就设计成后台提问的答案以通知送到；
- 只在它空下来时按，就不会和它新弹的框抢：空闲时没有在跑的轮，不会冒出新框。框排队的规则是提问在前、批准在后（§3.1），按完 Submit 就不再按别的键，排在后面的框不会被按到；
- 第 6 轮一条 `keys` 送完整个序列，一次成功。

### 5.2 防误按（照 R42 §2 逐条）

**2.1 发提醒时记什么**：同 Claude Code（`instance`、`kind`、`state_started`、`last_tool`、`last_output`、`last_human_input`），再加 Kimi 的 `questions`（含是否后台）、`command`。不把屏幕尾部当核对依据（§2.4）。给按钮的条件：

- 前台提问的选项按钮：同 R47b（有 `questions`、没截断、有 `state_started`）；
- 后台提问的选项按钮：同上，而且 corral 标明它是后台提问；
- 「拒绝」：停在框上就给（agy 以外，同现在）；
- 「允许这一次」：`kind:"kimi"`、blocked、`last_tool:"Bash"`、有 `command` 且没截断、过 R47c 的字符检查、不在 §2.5 的危险命令里、没有别的框开着。

**2.2 点之前核对**：

1. 谁点的、点的哪个：同 R47a。
2. 同一个 agent：同一个实例，没暂停。
3. 同一次等待：还是 blocked，`state_started` 一样（Kimi 是框弹出的毫秒时间，前台后台都有）、`last_tool` 一样；允许还要 `command` 逐字一样，提问还要 `questions` 一样。
4. 电脑上没人动过：`last_human_input` 不晚于 `state_started`。`corral keys` 不算人工输入，本轮按了几十下键，`last_human_input` 一直是 `null`（【实测】各 status 文件）。这一条在 Kimi 上更要紧：Kimi 的提问框记着每道题选了什么、停在哪一页，有人在电脑上动过，pony 按同样的键就会答错。
5. 6. 画面没动过、重读屏幕一字不差：**Kimi 不用**（§2.4），改用第 3 条里的结构化字段。这些字段来自 wire，比读屏准。
7. 后台提问另加：它空闲；corral 说最前面的就是这道题，没有别的框开着。不空闲就排着，在线程里说「它还在干活，空下来就替你选」。

**2.3 点之后**：先记成已用、存盘，再按，只按一次。确认办法：

- 前台提问、「拒绝」、「停」：同 R45，15 秒内看到 idle 且 `interrupted` 就算成功；
- 后台提问：corral 新给的 `resolved`（§5.3 c）里这道题的答案和 pony 要填的一样，就改消息「已答：…」；不一样照实说「框里交上去的是 X，和你选的不一样」（交了就撤不回）；10 秒没结果照实说、请回电脑看；
- 「允许这一次」：`resolved.decision` 是 `approved` 才写「已按：允许这一次」；是 `rejected` 或 10 秒没结果，照实说。

**2.4 用户已在电脑上答过**：同 R47a，等待结束（离开 blocked、`state_started` 变了、实例变了）或有人工输入，就去掉按钮。Kimi 还能说得更具体：`resolved` 里出现这次框的结果，就写「已在电脑上处理：允许 / 拒绝 / 答了 …」。

**2.5 过期、重复点、连点**：同 R47a（一条提醒共用一个记号，重启不重按，等待结束去按钮）。后台提问可能开很久，只要还是同一个框，按钮就一直有效。剩下的空隙：

- 前台提问、拒绝只按 Esc，不怕空隙；
- 后台提问按数字时，电脑前有人同时按，答案会错。交上去以后能从 `resolved` 看出来，照实说；
- 「允许」按 `1` 的那几十毫秒里，框被人答掉又弹出新框：新框要模型再跑一步才出得来，几十毫秒内来不及（【推测】）。Esc 在所有框上都只会拒绝或关闭。

### 5.3 corral 的 Kimi 适配器要补什么

都只从主会话 wire 读，在查询的那个 corral 里算，**不改钩子，不用换共用 helper**；全是兼容新增。

a. **`command` 给 Kimi**：停在 Bash 批准框（`interaction.request` 的 `kind:"approval"`、`display.kind:"command"`）时，给 `{"text":…,"truncated":…}`，`text` 是 `display.command` 原文（不用会被截断的 `action`），16 KiB 规则同 R47c。现在只给 Claude Code。

b. **后台提问**：跨轮记住还没答的提问（§3.4）。这道题在最前面、状态 blocked 时，给 `questions`（新增 `"background":true`）、`state_started`（框弹出的时间）、`last_tool:"AskUserQuestion"`。答了或关了就清掉。现在给 `questions:null`、`state_started:null`。

c. **最近一次框的结果，新增只读 `resolved`**：pony 用它确认按键的结果，也用它认出「已在电脑上处理」。

   ```json
   {"resolved":{"kind":"question","state_started":1792843946.744,"at":1792843988.212,"answers":{"R48G1 主食选哪个？":"米饭"},"truncated":false}}
   {"resolved":{"kind":"approval","state_started":1792844153.57,"at":1792844204.879,"decision":"approved"}}
   ```

   - `state_started` 对上那次框；关框时 `answers` 为 `null`；
   - 只取主会话；新的用户输入或下一个框弹出时换掉；
   - 答案沿用 R43 的 16 KiB 截断，截断了就不能拿来核对。

d. **`action`**：停在任何 Kimi 批准框上时，给 Kimi 自己那句说明，例如 `{"text":"Writing /tmp/…/outside.txt","truncated":false}`，只用来在 Slack 上显示。非 Bash 的批准框读屏又靠不住，没有它，手机上就看不出要批准的是什么。

e. **可选**：读屏规则加一条「回退选择器」（`Select messages to undo` 加 `Esc cancel`）→ blocked。现在它开着时 corral 报 idle（§4），`send` 的回车会选中回退点。推荐的做法里 pony 不会把它打开，这条是兜底，人在电脑上误开时也有用。

**兼容**：

- `command` 多一家；`questions` 多一种情形，多一个可选的 `background`；`state_started` 在一种原来为 `null` 的情形下有值；新增可选的 `resolved`、`action`。旧调用方（pony、paddock）都是忽略不认识的字段（pony 的 `Status` 没有 `deny_unknown_fields`）；
- 新 pony 遇到旧 corral：没有 `command` 就不给允许，没有 `background` 和 `resolved` 就不在框里按键答后台提问、不给允许，退回「回电脑处理」；
- 新旧 corral 交替读同一个 agent：照 R48d 给 Kimi 的 wire 游标升版本，旧游标从头重放；旧版读到新游标忽略多出的字段，行为和现在一样。

### 5.4 Slack 上的样子

**前台提问**：同 Claude Code（R44 列题目、R47b 选项按钮、R45 线程回答）。只是把 Kimi 现在末尾那句「请回电脑回答；这里的回复会等它处理完再送。」换成 Claude Code 那句「在这个线程写答案（例如『1 选 B；2 选 A、C』），会直接送过去。」。

**后台提问**（和 Claude Code 不一样）：

```
⚠ ranch/dev-x 在问你（后台提问，1 道题）：它没停下来等你，答案会填进它的提问框
1. 水果（单选）：R48G1 水果选哪个？
   A 苹果
   B 香蕉
   1. 水果   [A 苹果] [B 香蕉]
在这个线程写答案也行，会填进第 1 题的「其他」。
这个提问开着时它收不到新消息；不想答，回「停」关掉它。
```

- 它在干活时点了按钮：线程里说 `它还在干活，空下来就替你选`；
- 答上了：改消息 `已答：1. 苹果（10:42）`；
- 交上去的不一样：`框里交上去的是「香蕉」，和你选的不一样；请回电脑看看`；
- 在电脑上答了：`已在电脑上答了：苹果`。

**批准框**（和 Claude Code 不一样：不贴屏幕末尾，用 corral 给的命令原文）：

```
⚠ ranch/dev-x 要运行命令（Bash）：
~~~
echo R48G1_A > a.txt && cat a.txt
~~~
[允许这一次] [拒绝]
要允许：点「允许这一次」并确认；要拒绝：点「拒绝」或回「停」，它会停下这一轮，等你接着说。
```

- 「允许这一次」的确认框同 R47c：标题 `允许 ranch/dev-x 这一次？`，正文 `Bash` 加命令前 200 字，按钮「允许」「算了」；
- 按后：`已按：允许这一次（10:42）`；`已按：拒绝，它停下了（10:42）`；
- 不给允许时（非 Bash、危险、看不全）：

```
⚠ ranch/dev-x 要批准：Writing /tmp/…/outside.txt（Write）
[拒绝]
这种框不能在这里允许；要允许：回电脑处理。要拒绝：点「拒绝」或回「停」。
```

**「停」的回话、`@pony ?`**：同 Claude Code。

### 5.5 两条已知问题

- **后台提问漏掉**：corral 补 §5.3 b，pony 照上面的样子列题目、给按钮。
- **提问线程里「停」停不下来**：停在框上时按两次 Esc。
- 本轮另发现一条同类的：Kimi 批准框上现在的「拒绝」按钮和「停」也只按一次 Esc（`engine.rs` 937 行只有 amp 按两次），模型会换个办法接着干，pony 15 秒后报「还在等你」或「还在干活」。也改成两次。

## 6. 实现要改哪些地方（给 R48g2 派活用）

**corral（`crates/corral/`）**，粗估 200–300 行加测试：

- `src/agents/kimi.rs` 的 `wire_events`：记下批准框的 `display.command`、`action`；按 id 跨轮记住还没答的提问；记最近一次 `interaction.resolved` 的结果。`interaction.resolved` 只清对应的那个框；轮结束不清后台提问。游标（`WireCursor`）加字段、升版本。游标重置时（`turn_started` 变了、文件变了）要能找回更早的轮里还开着的提问：可以把这份提问单独带进新游标，没有游标时从头扫一遍 wire 里的 `interaction.request` / `interaction.resolved`。
- `src/cli.rs` 状态合并（179–193 行）：`command` 放开给 Kimi；读屏改出来的 blocked 遇到最前面是后台提问时，给 `questions`、`state_started`；新增 `resolved`、`action`；`wait` 返回的状态对象跟着带。
- 可选：`kimi.rs` 的 `screen_rules` 加回退选择器。
- 测试（`crates/corral/tests/kimi.rs`，合成 wire，隔离 HOME / CORRAL_HOME / KIMI_CODE_HOME，用 `/bin/cat` 冒充 kimi）：前台提问答了、关了；后台提问跨过 `turn.ended` 和新一轮还开着、晚些答了、关了；批准允许、拒绝；两个框同时开（谁在前）；子 agent 的事件不算；`command` 截断；旧游标升级和新旧交替重放。
- 文档：`docs/CONTRACT.md`（R48d 那节和 R47c `command` 那节）、`docs/DESIGN.md` §7 Kimi。

**pony（`crates/pony/`）**，粗估 300–500 行加测试：

- `src/core/engine.rs`：
  - `is_question`（2205 行）放开 Kimi 的前台提问；`route_message` 里提问核对的 `kind` 检查（651 行）放开 Kimi；
  - `needs_you_screen` 的 Kimi 末尾那句（2288 行）换成 Claude Code 那句；
  - `start_interrupt` 按键（937 行）：Kimi 停在框上时按两次；
  - 后台提问：新的提醒样子、排到空下来再按、按完看 `resolved`。
- `src/core/engine/buttons.rs`：去掉 Kimi 提问不给按钮的排除（112–114 行），前台交给 `question_offer`；Kimi 批准框的「拒绝」按两次。
- `src/core/engine/questions.rs`：后台提问用按键答，要拼按键序列（单选、多选加 `tab`、Other 打字加 `enter`、最后 `1`）；线程里的话去掉换行、去掉控制字符，空的不按。
- `src/core/engine/approvals.rs`：加 Kimi 的规则：不读屏；命令取 `status.command`；Kimi 的危险命令表；`stale` 不比 `last_output` 和屏幕；按后看 `resolved.decision`。
- `src/corral.rs`：`Status` 加 `resolved`、`action`，`Questions` 加 `background`；`Corral` 接口加一个只能送数字 1–5、`tab`、`enter`、单行文字的「答提问」方法（`approve` 仍然只送单个数字）。
- 测试（内存聊天适配器、假 corral、隔离 `PONY_HOME`）：
  - 前台提问：线程回答 → `esc esc` → idle 且 interrupted → 加前缀送；选项按钮；
  - 后台提问：在干活时排着，空下来按出准确的按键序列（单选、多选、Other）；`resolved` 对上、对不上、没来；有人工输入、有别的框时不按；
  - 允许：按 `1`；命令逐字核对；危险命令、非 Bash、`2` 永远不给；`resolved` 是 approved、rejected、没来；
  - 「拒绝」「停」：Kimi 在框上按两次、干活中按一次、空闲不按；
  - 旧 corral 没有新字段时退回「回电脑处理」；
  - Claude Code、amp、agy 等别家的行为不变（回归）。
- 文档：`docs/DESIGN.md` §10、R38a 对应说明、pony README。

**真 Kimi 实测（R48g2，部署前在测试 agent 上做，约 6 轮）**：前台提问线程回答、前台多题按钮、后台提问空闲时按钮答、批准允许、批准拒绝、提问框上「停」。本轮没测到的「后台提问开着时它在干活」也补一次。

**对 paddock 的影响**：corral `status` 的都是兼容新增（`command` 多一家、`questions` 多一种情形和 `background`、`state_started` 多一种情形有值、新增 `resolved`、`action`）。paddock 现在只显示状态，不用动；以后要在 paddock 里显示后台提问或批准的命令，在 paddock 自己的任务里接。pony 的改动是 ranch 内部的，和 paddock 无关。

## 未知

- 后台提问开着而它在干活时按 Esc：没测，只有源码推断（§4）。
- 两个同类框同时开（例如前台、后台两个提问）：源码是后来的顶替前一个显示，前一个的回答怎么处理没看清，也没测。推荐只在只开着一个框时给按钮、按键。
- 计划模式的批准（`plan_review`，Reject / Revise）、开子 agent、联网、MCP 的批准框长什么样：没测，第一版只给拒绝。
- 读屏规则在别的窗口大小、题目很长时还认不认得出后台提问框：只在同一个窗口大小下测了两次。
- 「Reject with feedback」打字：没测，方案里不用。
- 以上都是 Kimi Code 2.3.1 的样子，升级后按键和框可能变；R48g2 实测时先核对版本。

## 要用户定的

1. **前台提问的答法**：推荐「两次 Esc 关框并停下，再送文字」（同 Claude Code）；备选「在框里按键答」（模型同一轮拿到结构化答案，但要按数字键，多题的自由回答不好塞）。
2. **后台提问**：推荐「等它空下来，在框里按键答」；备选是只列题目、不在 Slack 里答，回电脑答。
3. **「拒绝」按两次 Esc（拒绝并停下）**：推荐，和 Claude Code 拒绝后等你说下一步一样；备选是只拒绝，模型自己换办法接着干。
4. **corral 改动**：§5.3 a–d 推荐都做（都是兼容新增，只换 corral）；e（回退选择器报 blocked）可选。
5. **非 Bash 的批准第一版只给「拒绝」**：推荐。
