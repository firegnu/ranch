# R48b：Kimi Code 真账号补测

2026-10-24，ranch/dev-r48b；只调研，不实现或安装集成。

## 结论先说

1. **【查证】原生配置钩子确实能用，但需要 Kimi 专用翻译，不能直接套 agy 或 amp。** 20 种事件均登记了探针；实际收到的 stdin、毫秒时间、进程链见附录。`exec <探针>` 下父进程是 Kimi、祖父是 pen。当前未知适配器只继承四个 `CORRAL_*` 变量，正式接入还需补身份门控。
2. **【查证】TERM、HUP 结束 Kimi 都不等于结束其工具子树。** 真正启动 Bash → sh → sleep 后，TERM/HUP 样本的 Kimi 分别在约 86/82 ms 内退出，三个子进程各在 5 秒后仍存活；两次均未收到 SessionEnd。正常 /exit 则收到 SessionEnd。已按记下的 PID 清理。不能只给主 PID 换一种信号就宣称“停止已支持”。详见 §4。
3. **【查证】新窗口可以直接从编辑器送首句；以 `-` 开头也可以。** `corral send NAME --force -- '-…'` 一次成功，`SessionStart` 随第一句才来。`--session <不存在的 ID>` 没有预建会话；恢复已有 ID 则在输入前触发 source=resume。详见 §5。
4. **【查证】默认模式的批准框、提问框和两种 Esc 已抓到。** 提问框取消产生成功的 PostToolUse（答案为空），模型继续；运行中 Esc 才产生 Interrupt。工具 exit 7 产生 PostToolUseFailure，之后主轮仍正常 Stop。不能把这些事件都压成“完成”。
5. **【查证】`--auto` 加 `KIMI_CODE_TRUST_WORKSPACE=1` 已无框完成小文件写入和回读。** 对提问请求，模型没有实际调 AskUserQuestion，自己作出选择；源码另有 auto 下拒绝该工具的策略，不能把本轮口头解释说成“实收了拒绝结果”。详见 §6。
6. **【未知】本轮没有遇到 Kimi 自动改变 config 的内容。** 探针块保留是实测事实，“自动改写后仍保留”不是本轮已通过的实验；不为制造样本而改用户模型或偏好。详见 §3。

所有结论限于本机 macOS、Kimi Code **2.3.1**、本账号当前默认配置。本轮欢迎页为 **K2.8 Preview / max**，SessionStart 的模型 alias 为 `kimi-code/kimi-for-coding`；未切模型。R48 的 K3/high 是上一轮事实，不是本轮默认值。

## 依据、输入账与边界

**【查证】** 先读了 CLAUDE、HANDOFF、R48 全报告与完成记录/审查、DESIGN §7（及项目范围/安装规矩）。本机用 PATH 上的 corral 开测试进程，每个均带 `role=test`、`CAIRN_DISABLE=1`、探针门控变量，工作目录是 `/tmp/r48b-probe.gF8csL/work`（Kimi 规范化为 `/private/tmp/...`）。同时最多一个测试 agent，未操作用户已有 agent。

**【查证】输入账：共 10 次文本提交，其中 8 次模型用户输入，另有 /compact、/exit 各一次；加上批准 Enter 3 次和 Esc 2 次，连界面按键也保守计入为 15 次操作，没有超过 15 轮。** 子 agent 的一次模型执行及压缩的模型工作也纳入观察，没有另外派任务。先后测试名字为 `ranch/test-r48b-default`、`ranch/test-r48b-session`、`ranch/test-r48b-auto`、`ranch/test-r48b-hup`。

| 文本提交 | 场景 | 结果与凭据 |
|---|---|---|
| 1 | 默认模式、前导 `-` 首句 | R48B_READY，主 turn 0 completed，首句 hook 延迟实验 |
| 2 | Bash 写文件/批准 | marker.txt 实际写入，批准/工具 hooks 齐全 |
| 3 | AskUserQuestion | 真实框；框上 Esc 后继续，运行中 Esc 后 cancelled |
| 4 | Bash exit 7 | 批准后 PostToolUseFailure；主轮正常 completed |
| 5 | Bash/sh/sleep + TERM | 真子进程运行；主 PID 退出、子树残留 |
| — | 不存在 ID 启动、再恢复已有 ID | 不增加用户输入；前者退出，后者输入前 SessionStart(resume) |
| 6 | auto 写文件/请求提问 | Write+Read 真完成，无框；模型未调用问题工具 |
| 7 | 内置子 agent | CHILD_OK / PARENT_OK，子 Stop 混入同 session hooks |
| 8 | /compact | PreCompact/PostCompact 与完成画面 |
| 9 | /exit | SessionEnd(reason=exit)，进程正常关闭 |
| 10 | 新 auto 会话 Bash/sh/sleep + HUP | 信任/批准均无框；子树与 hook 结果见 §4 |

**【查证】源码依据固定为 R48 使用的同一提交** `684bae9ea6a437f3bae98bba1dcb797a917d500b`，下载官方源码归档作本地只读核对；没有把最新 main 当作本机实现：

- S1：[hooks 事件全集](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/features/externalHooks/internal/types.ts)、[配置 schema](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/features/externalHooks/configSection.ts)、[运行器](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/features/externalHooks/internal/runHook.ts)。
- S2：[agent 事件](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/features/externalHooks/agent/agentExternalHooksService.ts)、[session 事件](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/features/externalHooks/session/sessionExternalHooksService.ts)。
- S3：[TUI 启动与停止](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/apps/kimi-code/src/tui/kimi-tui.ts)、[配置保留格式写回](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/config/tomlWriteback.ts)。
- S4：[auto 的提问拒绝策略](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/agent/permissionPolicy/policies/auto-mode-ask-user-question-deny.ts)。
- H：本机 `/opt/homebrew/bin/herdr` 的 `strings`，Kimi manifest `2026.06.10.1`；只读取程序字符串，没有调用 integration install/uninstall 或写 herdr 配置。

## 1. 钩子实收

**【查证】探针的做法：** 修改前备份 config；末尾追加 `# >>> corral r48b probe` / `# <<< corral r48b probe` 包裹的 20 个 `[[hooks]]`，每条 `event`、`command = "exec /tmp/r48b-probe.gF8csL/probe"`、`timeout = 10`。临时探针为 Rust，先检查 `R48B_PROBE_DIR` 精确等于测试目录，不符立即退出、不读 stdin、不输出；符合才把 stdin 原文和元数据写入临时目录。成功退出且 stdout/stderr 空白，不返回批准/拒绝指令。未读 credentials、token、device_id 的内容。

**【查证】环境与身份：** 实收环境变量名只有 `CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME`，没有 `CORRAL_KIND`、`CORRAL_CWD`、`CORRAL_PEN_PID`、`CORRAL_HELPER`；这是现装 corral 尚未适配 Kimi 的结果。每条探针的直接父进程均可在附录核对，不输出环境值。默认会话的链为 `probe → kimi(79730) → corral pen(78820) → launchd`；恢复会话的链改为 `probe → kimi(98404) → pen(97615) → launchd`。`exec` 去掉了中间 shell；不能假设任意 command 写法都有相同链。

**【查证】HUP 新会话的链为 `probe → kimi/kimi-code(20964) → pen(20291) → launchd`。** 程序的 comm 在本轮观察中可显示 kimi 或 kimi-code，不能只靠进程短名作归属判断。

**【查证】同步与异步不能混为一谈：** 第一条 UserPromptSubmit 人为睡 1500 ms：起点 `1792813935709`，返回 `1792813937219`，TurnStarted 到 `1792813937234` 才触发，证明此钩子会挡住提交。普通探针包含数次 ps，单次耗时见附录；这些不是零成本回调。**【查证，仅源码 S2】** SessionStart/End、PreToolUse、UserPromptSubmit、Stop、SubagentStart、PreCompact 路径等待钩子；TurnStarted、PermissionRequest/Result、PostToolUse/Failure、Interrupt、StopFailure、Heartbeat、TaskStarted、Notification、SubagentStop、PostCompact 多数 fire-and-forget。不能用落盘顺序给所有异步事件推导强顺序，也不能声称已逐个测过阻塞时长。

**【查证】核心事件顺序与完整字段：** 见附录原文；以下概括不替代原文。

- 第一轮：SessionStart(source=startup) → UserPromptSubmit(prompt 为 text 数组、is_steer=false) → TurnStarted(turn_id=0、origin_kind=user、prompt 为字符串) → Stop(stop_hook_active=false)。第一轮主 wire `turn.ended(completed)` 时间为 `1792813942258`，Stop 探针开始于 `1792813942246`：Stop 是结束前检查，不能当最终提交点。
- 写文件：PreToolUse(Bash) → PermissionRequest → 人工 Enter 批准一次 → PermissionResult(decision=approved) → PostToolUse(tool_output=R48B_FILE) → Stop；磁盘 marker.txt 确实是 R48B_FILE。
- 提问：PreToolUse(AskUserQuestion，含完整 questions) → 真实 question 面板。没有 PermissionRequest。框上 Esc 后 PostToolUse 输出 `answers:{}` 与 dismissed 说明；主 wire 的 interaction.resolved 为 null，之后继续下一个模型步骤。运行中再 Esc 才有 Interrupt(turn_id=2,reason=cancelled)，主 wire 同轮 cancelled。
- 故意执行 `/bin/sh -c "exit 7"`：批准后 PostToolUseFailure，error 含 code=internal、退出码 7 的 message、retryable=false；随后模型报告退出码并正常 Stop。**【未知】** 没有制造模型/API 整轮失败，StopFailure 未实际触发；不能用工具失败代替它。

**【查证】状态归属不能只看 session_id 或父进程。** PreToolUse、Stop、TurnStarted 等 payload 不统一带 agent_id；本轮子 agent 的 Stop 确实在同一 Kimi 进程、同一 session 内出现（§7）。**【推测】** 需与主 wire 的 `agentId=main` 和主轮身份核对，子活动不能抢主会话或提前置 idle。

## 2. 真实画面与标题

**【查证】默认模式 Bash 批准框，以下是 corral read 捕获内容按屏幕行恢复的底部原文（省略边框线）：**

```text
▶ Run this command?
cwd: /private/tmp/r48b-probe.gF8csL/work
$ printf R48B_FILE > marker.txt && cat marker.txt

▶ 1. Approve once
  2. Approve for this session
  3. Reject
  4. Reject with feedback

↑/↓ select · 1/2/3/4 choose · ↵ confirm
```

**【查证】提问框：**

```text
question
R48B    Submit

? R48B 选哪个
→ [1] A
      测试 A
  [2] B
      测试 B
  [3] Other

↑↓ select  1-3 / ↵ choose  ←/→/tab switch  esc cancel
```

**【查证】标题：** 初始无会话时 corral status.title 为空；第一轮以首句作标题，随后自动变成 `R48B 首句测试`。批准框、提问框、工作中、结束后的同一标题不区分等待。没有 amp 的等待前缀。**【未知】** 没有额外造 Write/Edit/计划批准框，没有把 Bash 样本外推为全部面板。

**【查证，H】herdr 当前批准规则取整屏近期内容，组合 confirm、choose 与批准语境（run this command / write this file / apply these edits 等）；提问规则组合 select、esc cancel、question 行和问号题目行，并要求 choose/toggle/save 之一。** 本轮两个画面符合这些特征；strings 会把 Unicode 符号拆成多行，因此这里只对照词与语义，不把 strings 输出误当完整配置文件。herdr 还用月亮/转圈认 working。**【推测】** corral 可复用面板特征；工作与结束应以 hooks/wire 为准，不用转圈或标题猜 interrupted。R7 现有规则只把 working/idle 修正为 blocked，不能拿它补新窗口的 starting 就绪，也不能用过往输出环里的一段旧面板判当前 blocked。

## 3. 标记块与配置自动改写

**【查证】** 初始 config 为 2446 字节，加探针后 4361 字节。每次比对都先确认整段探针字节完全相同，再剔除这一段与备份比较；截至各次实测，块未挪动、未改格式，块外内容相同。探针一直可用。

**【未知】** 本轮未捕获“非探针配置内容由 Kimi 自己改写”的样本，因此不能证明模型目录刷新等写回一定保留首尾注释。没有为了强制写回而切换模型、修改努力档位或其他偏好。**【查证，仅源码 S3】** 配置写回有按 domain 保留其余文本/注释的实现。**【推测】** 对别的 domain 的更新有望保留 hooks 区域；hooks domain 自身重序列化或其他客户端写回仍需安装器防守，不能凭源码替代本轮真跑。

**【查证】清理结果：** 所有测试 Kimi 退出后，再次确认整段探针字节一致，精确删除当初追加的块（包括追加的空行），没有用备份覆盖文件。去块后的内容与备份逐字节相同，仍为 2446 字节；本轮剩余配置差异为零，没有可列出的自动改动字段。按任务要求 grep 确认无 r48b。用户会话记录等 Kimi 正常写入保留，凭证类文件未读取或恢复。

## 4. TERM / HUP 停止

**【查证】TERM 样本在默认模式，工具批准后已看到 PID 文件和实际进程树：**

```text
Kimi 79730 (PGID 79730)
  /bin/bash 92899 (PGID 92899)
    /bin/sh 92900 (PGID 92899)
      /bin/sleep 92902 (PGID 92899)
```

在 Unix 毫秒 `1792814045937` 开始，对 **79730 一个 PID** 发 TERM，20 ms 间隔轮询 kill -0（另有进程查询开销）：Kimi 第一次确认消失为 86 ms；5 秒后 92899、92900、92902 均仍存活且为运行/睡眠态，Bash 已归 PPID 1。没有 SessionEnd；恢复同会话后 Kimi 补了“上次工具未记录结果，不要假定成功”的工具结果。之后只给这三个记下的 PID 发 TERM 清理，确认全部消失。corral stop 该名字报 not_found，因为 pen 已随 Kimi 退出。

**【查证】HUP 样本是新启动的 auto 会话：** 未弹信任或批准框，实际进程为 Kimi 20964 (PGID 20964) → Bash 22917 → sh 22923 → sleep 22924（后三者 PGID 22917）；hup-shell.pid 确认为 22923。Unix 毫秒 `1792814256591` 开始只给 20964 发 HUP，首次确认 Kimi 消失为 82 ms；5 秒后后三者仍为活的睡眠态进程，Bash PPID 已变 1，没有 SessionEnd。再次按三个已记录 PID 发 TERM 清理。未记录本机信号退出码，不能把源码里的 129/143 当成本轮直接测到的退出码。

**【查证】对照：** 恢复会话正常 /exit 有 SessionEnd(reason=exit)。两次信号样本都确实执行了 sleep，区别于 R48 T4 未证实子进程已启动的样本。信号与模式的组合仅各一次（TERM 默认、HUP auto），不是同模式多轮对照统计。

**【未知】** 本轮测的是单 PID 信号，不是向所有子进程组广播；不能把结果说成 corral 未来适配器的完整 stop 验收。没有测 MCP、Windows/Linux、后台任务树，也没有把小样本耗时写成最坏上限。TERM 没收到 SessionEnd 的具体内部原因未定位；S3 有正常清理路径和紧急退出兜底，本轮事实优先于“源码说应当清理”的推断。

## 5. 首句和就绪

**【查证】首次启动：** 不带 prompt、使用信任环境变量，界面出现 `No session yet — one will be created on your first message.`、空编辑器和模型状态行后，钩子目录仍空，title 也空。随后调用：

```sh
corral send ranch/test-r48b-default --force -- '-R48B 首句测试，只回复 R48B_READY，不调用工具。'
```

一次提交成功，UserPromptSubmit 的全文与原句一致，回复 R48B_READY。`confirmed:false` 只表示现装 corral 没有 Kimi 确认钩子，不能把 send 返回当作模型已经收到了；本轮用探针与 wire 确认实际接收。**【查证】** R48 的前导 `-` 失败是未用选项终止符，不能据此限制 Kimi 首句。也不必重演 keys 文本与 Enter 的时序问题。

**【查证】--session：** 以 `session_r48b_not_created` 启动独立测试 agent 后进程自行退出，之后 read/status/stop 都 not_found，没有 SessionStart；corral 清掉了已退出实例，未保留到错误画面，不虚构实收 stderr。**【查证，仅源码 S3】** 该路径只 listSessions、找已有会话；找不到抛 not found，不创建。恢复本轮真实 ID 时，在任何新输入之前就有 source=resume 的 SessionStart，编辑器显示历史和 Never Ask。

**【推测】可靠启动应分两个阶段：** 先等待真实可输入编辑器（同时排除信任、登录、模型选择、加载画面），再送首句；收到 UserPromptSubmit/TurnStarted 才确认本轮开始。新 session 不能“等 SessionStart 再送首句”，那会互等。不要靠固定 1 秒，也不要把 send 的 false confirmation 当成功确认。**【未知】** 本轮只是一次新会话首句成功与一次恢复，不是加载抖动下的可靠性统计；尚无已实测的公开机器可读 ready 信号。

## 6. 派活启动方式

**【查证】** `kimi --auto --session <本轮已有ID>`，加 `KIMI_CODE_TRUST_WORKSPACE=1`，没有信任框、批准框、问题框。实际调用 Write(auto.txt, AUTO_OK)、Read(auto.txt)，文件为 7 字节，返回 `1\tAUTO_OK`；有对应 Pre/PostToolUse、最终 Stop 与主 wire completed。没有 PermissionRequest。这是一个小文件任务的通过样本，不是全部风险动作免框的保证。

**【查证】auto 下提问的本轮事实：** 输入明确要求尝试 AskUserQuestion，但模型解释 auto 禁问并直接选择 red；hooks 和主 wire 没有该工具调用。**【查证，仅源码 S4】** auto 策略对 AskUserQuestion 返回 deny，而非等待人工；模型还收到不调用该工具的提示。**【未知】** 本轮未得到实际拒绝 payload，不能声称“工具调用后收到拒绝”已经真跑。若以后 pony 必须展示题目，要在能提问的模式下测题目桥接，不能一面强制 Never Ask，一面承诺所有问题都会弹框。

## 7. 子 agent、压缩与事件覆盖补充

**【查证】子 agent 真跑：** 第 7 条输入要求只开一次内置 Agent、沿用默认模型。真实调用 Agent（参数没有 model），界面显示 Coder / K2.8 Preview / max，0 tools，返回 CHILD_OK；主 agent 随后回复 PARENT_OK。有 `agents/agent-0/wire.jsonl`，其 turn.ended(agentId=agent-0, turnId=0, completed) 为 `1792814119947`；主 wire 的 turn 6 completed 为 `1792814123381`。

**【查证】探针顺序：** 主 TurnStarted(turn_id=6,origin_kind=user) → PreToolUse(Agent) → 子 TurnStarted(turn_id=0,origin_kind=system_trigger,origin_name=subagent) → SubagentStart(agent_name=coder) → **Stop** → SubagentStop(response=CHILD_OK) → PostToolUse(Agent) → **Stop**。子与主的 Stop 都只有同一 session_id、session_title、stop_hook_active=false，父进程也都是 98404，没有 agent_id、turn_id 可用。子 TurnStarted 的探针甚至先于 SubagentStart 落盘。**【推测】** 简单计数 SubagentStart/Stop 并忽略中间 Stop 也不足以应付并发/乱序；主 wire 是阶段 1 必需的完成证据，至少这个样本已推翻“只认主进程父链就能区分子 agent”。

**【查证】压缩与正常退出：** /compact 实收 PreCompact(trigger=manual,token_count=25684)，随后 PostCompact(trigger=manual,estimated_token_count=24568)，画面显示 Compaction complete (25684 → 24568 tokens)。/exit 实收 SessionEnd(reason=exit)，测试实例自行退出，后续 corral stop 报 not_found。这证明正常关闭路径能发 SessionEnd，但不能据此保证信号路径也会发。

**【查证】20 种事件全部挂载，实际收到 16 种、55 次。** 下面的零次只表示没触发，不表示 Kimi 不支持：

| 事件 | 次数 | 本轮依据或未触发原因 |
|---|---:|---|
| SessionStart / SessionEnd / SessionHeartbeat | 3 / 1 / 3 | 新会话两次、恢复一次；正常 exit 一次；运行期间自然心跳 |
| UserPromptSubmit / TurnStarted | 8 / 9 | 8 次模型用户输入；额外一次子 agent turn |
| PreToolUse / PostToolUse / PostToolUseFailure | 8 / 5 / 1 | Bash、AskUserQuestion、Write、Read、Agent；exit 7 失败；两个 sleep 结束前父进程被信号终止 |
| PermissionRequest / PermissionResult | 3 / 3 | 默认模式三个 Bash 批准，均选一次批准 |
| Stop / Interrupt / StopFailure | 6 / 1 / 0 | 含子 agent Stop；运行中 Esc 一次；未制造模型/API 整轮失败 |
| SubagentStart / SubagentStop | 1 / 1 | 默认模型的 coder 子 agent 完成 |
| PreCompact / PostCompact | 1 / 1 | 手动 /compact 完成 |
| UserPromptQueued | 0 | 本轮按场景完成后再提交，没有在工作中排队 |
| TaskStarted / Notification | 0 / 0 | 子 agent 为前台等待完成，未造后台任务通知；两次 sleep 也指定前台 |

**【未知】其余边界：** 没试批准框上的 Esc（已测提问框上的 Esc 与运行中 Esc），没试后台提问、后台子任务、自动压缩、Stop hook 要求续跑以及同步 hook 超时/失败。未为扩大覆盖追加输入，也未把缺失事件伪造成探针实收。

## 对阶段 1 的建议

以下全为 **【推测：建议】**，不是已实现或已批准的设计；本轮不改 DESIGN。

- **安装：** 优先研究原生 `[[hooks]]` 加 corral 标记块和独立 helper，免去新增插件注册表。正式安装器必须把配置视为用户共享文件：保留其余字节、检查重复/损坏标记、拒绝冲突、在写回前重读比较、原子写入；卸载只删能确认归属的条目。本轮没有自动改写样本，不能只凭两个注释还在就宣布安装方案完备。若 hooks 被其他客户端重排，应报冲突而非整份恢复备份。
- **单独写 Kimi 翻译：** hooks 本质可复用共用 helper 和 v1 状态，但字段名、prompt 数组、Stop 的时机、子任务身份及回复来源与现有家不同。命令用 exec，核对 CORRAL_KIND、CORRAL_INSTANCE、CORRAL_EVENTS、CORRAL_CWD、CORRAL_PEN_PID 和实际父链，缺失即退出。Kimi 的 observer 应空输出成功返回；不要照搬 agy 的 `{}` 输出习惯或返回权限指令。
- **挂的事件与状态：** SessionStart 建立/切回会话；UserPromptSubmit 记录原始输入并置 working，TurnStarted 提供轮次和 origin（过滤子任务 origin，主 wire 再核对）；PreToolUse 记录工具，PermissionRequest → blocked，PermissionResult → working；AskUserQuestion 的 PreToolUse 配合主 wire question interaction → blocked，取消/回答后的 PostToolUse 与 resolved → working；PostToolUseFailure 仅记录工具失败并维持 working；Interrupt + 主轮 cancelled → idle/interrupted；StopFailure + 主轮 failed →结束失败；Stop 只作“检查完成”的触发器，必须见主 wire completed 才 idle/给回复；SessionEnd 是辅助关闭信息，进程退出仍由 pen 兜底。SubagentStart/Stop、Pre/PostCompact 可用于观测，不应独立置主轮 idle。Heartbeat 不更新轮次；TaskStarted/Notification 不等于新用户输入，暂不据它们切主状态。
- **回复：** 从本 session 的 `agents/main/wire.jsonl` 取对应主轮的 text content.part，过滤 think、工具参数/结果、旧轮、子 wire；处理多段和文件尾半行。completed 才发布正常回复，cancelled 不把旧回复冒充本轮。Stop stdin 本身没有回复正文。路径应从可信会话身份及 Kimi 的数据根定位，不接受任意钩子路径直接读取。
- **读屏：** 默认批准框组合 approve/reject、choose、confirm 和实际操作面板；提问框组合 question、题目、select、esc cancel、choose 等。选当前终端屏幕适合的底部范围，适配窄屏/折行并排除正文引用；不要直接照搬 herdr 的 whole_recent 到输出环。标题不用作 blocked；读屏不猜 interrupted，也不替代主轮完成。
- **停止：** 先优雅 TERM，但要在父进程退出前记录并管理实际子树/独立进程组，再按超时清理残留，最后才强制结束。不能只向 Kimi 自己的 PGID 发信号，因为 Bash 使用独立 PGID。本轮单 PID TERM 已证实不足；新增的子树处理要在正式开发任务明确兼容与 PID 复用边界，不能在本次文档里宣称已解决。
- **首句：** 给新会话一个明确的“编辑器可输入但尚未创建 session”启动阶段；确认可输入后通过 PTY 送文本与提交，CLI 文本用 `--` 处理前导连字符；收到主用户输入事件才确认。`--session` 只用于恢复已知会话，不用于预建。加载/信任/登录未结束时不能提前宣告 idle。
- **派活模式：** 需要无人值守的小任务可选 `--auto` + 信任环境变量，但它主动消除提问；要求 pony 收问题的流程需另选保留提问的模式并验收。不要把本轮简单文件任务当成分工表编码能力评估或 sandbox 验收。

## 对 R48 的逐条补全或修正

| R48 位置 | 本轮结论 |
|---|---|
| §2 原生 hooks 真账号输入、父链、环境未知 | **【查证】** 本轮实际挂全事件，附录保存实收全文；exec 探针父 Kimi、祖父 pen，只有四个 CORRAL 环境名。不是仅 doctor 接受配置。 |
| §2 优先论证插件注册方案 | **【推测】** 原生标记块已工作，可作为更小候选；自动改写后保留尚未实测，不能无条件定案。 |
| §2 主/子身份可能混淆 | **【查证】** 真子 agent 的 Stop 与主 Stop 无可区分身份；只核对父链不够，主 wire 校验不可省。 |
| §3 批准框未真跑、T2 未执行工具 | **【查证】** 实际批准 Bash 并写入 marker.txt，PermissionRequest/Result 与画面对应。工具 exit 7 与失败 hook 也已测到。 |
| §3 Esc、Stop payload/最终完成 | **【查证】** question Esc 不终止主轮；运行中 Esc 有 Interrupt；Stop 先于主 wire completed，且子 Stop 会混入。 |
| §4 前导 `-` 首句不可靠 | **【查证】** corral send 以 `--` 分隔选项，一次提交前导 `-` 文本成功；不再把 CLI 解析错误算成 Kimi 限制。 |
| §4 `--session <id>` 预建未知 | **【查证】** 不存在 ID 的测试实例退出；源码明确只恢复，已有 ID 则输入前有 SessionStart。 |
| §4 TERM/HUP 子树清理未知/建议优先研究 TERM | **【查证】** 两种信号单 PID 都留下真正运行的 Bash/sh/sleep，均未见 SessionEnd；停止方案必须另处理子树。 |
| §4 --auto 和信任仅源码/T0 | **【查证】** 真实账号免框 Write+Read 完成；提问仍只有“本轮未调”与源码 deny 两层证据，没有实际拒绝 payload。 |
| §1 config 改动字段未知 | **【未知】** 本轮块外无变化，没有复现 R48 的自动内容改写，不能倒推上一轮改了哪些字段。 |
| §7 默认模型 K3/high | **【查证】** 本轮默认 K2.8 Preview/max、alias kimi-code/kimi-for-coding；没有主动切换。阶段 1 不应硬编码 R48 当时的默认值。 |
| “要用户定的”接入范围/账号边界 | **【查证】** 本任务已授权复用账号、自动刷新与临时探针、最多 15 轮。按主控给的阶段 0→1→2→3→可选4研究，不再把已定事项列为待批准；正式集成安装仍不在本轮范围内。 |

**【查证】对 paddock 的影响：** 本轮只有研究文档，paddock 不用动。正式阶段 1 若新增启动就绪语义或共享配置块安装约定，须在该任务先改 DESIGN，并单独写清 paddock 是否需要展示/适配；本轮没有改任何公开协议。

## 附录 A：每次探针实收原文

**【查证】** 共 55 次。以下按探针启动的 Unix 毫秒排序；start_ms/end_ms 是探针记录时刻，不能当作 Kimi 内部发事件时刻。元数据的进程行依次为 PID、PPID、comm，用户 HOME 路径统一写成 `$HOME`，环境只保留变量名。stdin 为探针收到的完整 JSON，不截字段、不截工具输出；本轮内容均来自上述合成测试，没有凭证字段。JSON 内的 `tool_output` 本身可能已由 Kimi 截断，这是上游给出的内容，不是报告再截。

<details>
<summary>展开 55 次事件、进程链与 stdin 全文</summary>


### E01

```text
start_ms=1792813935658
pid=80708
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
80708 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813935664
```

```json
{"hook_event_name":"SessionStart","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","source":"startup","model":"kimi-code/kimi-for-coding","profile":"agent"}
```

### E02

```text
start_ms=1792813935709
pid=80733
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
80733 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
injected_delay_ms=1500
end_ms=1792813937219
```

```json
{"hook_event_name":"UserPromptSubmit","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"-R48B 首句测试，只回复 R48B_READY，不调用工具。","prompt":[{"type":"text","text":"-R48B 首句测试，只回复 R48B_READY，不调用工具。"}],"is_steer":false}
```

### E03

```text
start_ms=1792813937234
pid=81022
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
81022 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813937245
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"-R48B 首句测试，只回复 R48B_READY，不调用工具。","turn_id":0,"origin_kind":"user","prompt":"-R48B 首句测试，只回复 R48B_READY，不调用工具。"}
```

### E04

```text
start_ms=1792813942246
pid=81570
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
81570 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813942253
```

```json
{"hook_event_name":"Stop","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"-R48B 首句测试，只回复 R48B_READY，不调用工具。","stop_hook_active":false}
```

### E05

```text
start_ms=1792813952561
pid=82470
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
82470 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813952567
```

```json
{"hook_event_name":"UserPromptSubmit","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","prompt":[{"type":"text","text":"测试实际工具执行：必须调用 Bash，运行 printf R48B_FILE > marker.txt，然后 cat marker.txt。不要只口头回答，不调用其他工具，不读本目录以外文件。"}],"is_steer":false}
```

### E06

```text
start_ms=1792813952575
pid=82481
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
82481 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813952581
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","turn_id":1,"origin_kind":"user","prompt":"测试实际工具执行：必须调用 Bash，运行 printf R48B_FILE > marker.txt，然后 cat marker.txt。不要只口头回答，不调用其他工具，不读本目录以外文件。"}
```

### E07

```text
start_ms=1792813959588
pid=83583
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
83583 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813959596
```

```json
{"hook_event_name":"PreToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Bash","tool_input":{"command":"printf R48B_FILE > marker.txt && cat marker.txt"},"tool_call_id":"tool_Dtwtyh79jxFhNyK9JfwSdFYA"}
```

### E08

```text
start_ms=1792813959606
pid=83593
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
83593 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813959614
```

```json
{"hook_event_name":"PermissionRequest","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","id":"approval_649120bc-88a9-4f06-8b26-d78c4556db36","agent_id":"main","turn_id":1,"tool_call_id":"tool_Dtwtyh79jxFhNyK9JfwSdFYA","tool_name":"Bash","action":"Running: printf R48B_FILE > marker.txt && cat marker.txt","display":{"kind":"command","command":"printf R48B_FILE > marker.txt && cat marker.txt","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"},"tool_input":{"command":"printf R48B_FILE > marker.txt && cat marker.txt"}}
```

### E09

```text
start_ms=1792813971164
pid=84927
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
84927 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813971172
```

```json
{"hook_event_name":"PermissionResult","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","id":"approval_649120bc-88a9-4f06-8b26-d78c4556db36","agent_id":"main","turn_id":1,"tool_call_id":"tool_Dtwtyh79jxFhNyK9JfwSdFYA","tool_name":"Bash","action":"Running: printf R48B_FILE > marker.txt && cat marker.txt","display":{"kind":"command","command":"printf R48B_FILE > marker.txt && cat marker.txt","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"},"tool_input":{"command":"printf R48B_FILE > marker.txt && cat marker.txt"},"decision":"approved"}
```

### E10

```text
start_ms=1792813971170
pid=84930
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
84930 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813971177
```

```json
{"hook_event_name":"PostToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Bash","tool_input":{"command":"printf R48B_FILE > marker.txt && cat marker.txt"},"tool_call_id":"tool_Dtwtyh79jxFhNyK9JfwSdFYA","tool_output":"R48B_FILE"}
```

### E11

```text
start_ms=1792813975134
pid=85226
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
85226 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813975143
```

```json
{"hook_event_name":"Stop","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","stop_hook_active":false}
```

### E12

```text
start_ms=1792813980888
pid=85875
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
85875 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813980893
```

```json
{"hook_event_name":"UserPromptSubmit","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","prompt":[{"type":"text","text":"请立即真实调用 AskUserQuestion：题目 R48B 选哪个，两个选项 A 和 B，各说明为测试 A 和测试 B。必须弹工具提问框，不要用正文提问。若我取消问题，先用 Bash 执行 sleep 60 再结束。"}],"is_steer":false}
```

### E13

```text
start_ms=1792813980899
pid=85886
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
85886 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813980905
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","turn_id":2,"origin_kind":"user","prompt":"请立即真实调用 AskUserQuestion：题目 R48B 选哪个，两个选项 A 和 B，各说明为测试 A 和测试 B。必须弹工具提问框，不要用正文提问。若我取消问题，先用 Bash 执行 sleep 60 再结束。"}
```

### E14

```text
start_ms=1792813990316
pid=87079
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
87079 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813990324
```

```json
{"hook_event_name":"PreToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"R48B 选哪个","header":"R48B","options":[{"label":"A","description":"测试 A"},{"label":"B","description":"测试 B"}]}]},"tool_call_id":"tool_iA3vlnicj3RqCq6U1HFI52gn"}
```

### E15

```text
start_ms=1792813994289
pid=87705
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
87705 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813994295
```

```json
{"hook_event_name":"PostToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"R48B 选哪个","header":"R48B","options":[{"label":"A","description":"测试 A"},{"label":"B","description":"测试 B"}]}]},"tool_call_id":"tool_iA3vlnicj3RqCq6U1HFI52gn","tool_output":"{\"answers\":{},\"note\":\"User dismissed the question without answering.\"}"}
```

### E16

```text
start_ms=1792813995632
pid=87768
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
87768 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813995639
```

```json
{"hook_event_name":"SessionHeartbeat","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","uptime_ms":60002}
```

### E17

```text
start_ms=1792813998541
pid=88127
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
88127 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792813998547
```

```json
{"hook_event_name":"Interrupt","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","turn_id":2,"reason":"cancelled"}
```

### E18

```text
start_ms=1792814003857
pid=88680
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
88680 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814003863
```

```json
{"hook_event_name":"UserPromptSubmit","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","prompt":[{"type":"text","text":"请真实调用 Bash 执行 /bin/sh -c \"exit 7\"。这是工具失败探针，不需要修复或重试，工具返回后只报告实际退出码。"}],"is_steer":false}
```

### E19

```text
start_ms=1792814003870
pid=88690
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
88690 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814003879
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","turn_id":3,"origin_kind":"user","prompt":"请真实调用 Bash 执行 /bin/sh -c \"exit 7\"。这是工具失败探针，不需要修复或重试，工具返回后只报告实际退出码。"}
```

### E20

```text
start_ms=1792814012374
pid=89548
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
89548 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814012382
```

```json
{"hook_event_name":"PreToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Bash","tool_input":{"command":"/bin/sh -c \"exit 7\""},"tool_call_id":"tool_WzYp8AhEuELTjsujohgU2DAu"}
```

### E21

```text
start_ms=1792814012390
pid=89554
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
89554 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814012397
```

```json
{"hook_event_name":"PermissionRequest","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","id":"approval_f5386ae0-c356-4081-8553-0c9c998abc0a","agent_id":"main","turn_id":3,"tool_call_id":"tool_WzYp8AhEuELTjsujohgU2DAu","tool_name":"Bash","action":"Running: /bin/sh -c \"exit 7\"","display":{"kind":"command","command":"/bin/sh -c \"exit 7\"","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"},"tool_input":{"command":"/bin/sh -c \"exit 7\""}}
```

### E22

```text
start_ms=1792814014851
pid=89761
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
89761 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814014860
```

```json
{"hook_event_name":"PermissionResult","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","id":"approval_f5386ae0-c356-4081-8553-0c9c998abc0a","agent_id":"main","turn_id":3,"tool_call_id":"tool_WzYp8AhEuELTjsujohgU2DAu","tool_name":"Bash","action":"Running: /bin/sh -c \"exit 7\"","display":{"kind":"command","command":"/bin/sh -c \"exit 7\"","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"},"tool_input":{"command":"/bin/sh -c \"exit 7\""},"decision":"approved"}
```

### E23

```text
start_ms=1792814014858
pid=89792
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
89792 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814014864
```

```json
{"hook_event_name":"PostToolUseFailure","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Bash","tool_input":{"command":"/bin/sh -c \"exit 7\""},"tool_call_id":"tool_WzYp8AhEuELTjsujohgU2DAu","error":{"code":"internal","message":"Process exited with code 7\nCommand failed with exit code: 7.","retryable":false}}
```

### E24

```text
start_ms=1792814017863
pid=90172
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
90172 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814017872
```

```json
{"hook_event_name":"Stop","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","stop_hook_active":false}
```

### E25

```text
start_ms=1792814021157
pid=90941
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
90941 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814021164
```

```json
{"hook_event_name":"UserPromptSubmit","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","prompt":[{"type":"text","text":"停止测试：必须真实调用 Bash，前台执行 /bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 120'，不要后台化，不调用其他工具，等待命令完成。"}],"is_steer":false}
```

### E26

```text
start_ms=1792814021170
pid=90946
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
90946 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814021180
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","turn_id":4,"origin_kind":"user","prompt":"停止测试：必须真实调用 Bash，前台执行 /bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 120'，不要后台化，不调用其他工具，等待命令完成。"}
```

### E27

```text
start_ms=1792814031149
pid=91925
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
91925 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814031159
```

```json
{"hook_event_name":"PreToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Bash","tool_input":{"command":"/bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 120'","timeout":180},"tool_call_id":"tool_O8T6wah5yAr5G6IsO7srmMOQ"}
```

### E28

```text
start_ms=1792814031166
pid=91930
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
91930 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814031171
```

```json
{"hook_event_name":"PermissionRequest","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","id":"approval_0204823b-b115-4425-925d-6976fe7db25a","agent_id":"main","turn_id":4,"tool_call_id":"tool_O8T6wah5yAr5G6IsO7srmMOQ","tool_name":"Bash","action":"Running: /bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 1…","display":{"kind":"command","command":"/bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 120'","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"},"tool_input":{"command":"/bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 120'","timeout":180}}
```

### E29

```text
start_ms=1792814039539
pid=92898
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
92898 79730 /tmp/r48b-probe.gF8csL/probe
79730 78820 kimi
78820     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814039546
```

```json
{"hook_event_name":"PermissionResult","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","id":"approval_0204823b-b115-4425-925d-6976fe7db25a","agent_id":"main","turn_id":4,"tool_call_id":"tool_O8T6wah5yAr5G6IsO7srmMOQ","tool_name":"Bash","action":"Running: /bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 1…","display":{"kind":"command","command":"/bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 120'","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"},"tool_input":{"command":"/bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 120'","timeout":180},"decision":"approved"}
```

### E30

```text
start_ms=1792814068757
pid=98837
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
98837 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814068764
```

```json
{"hook_event_name":"SessionStart","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","source":"resume","session_title":"R48B 首句测试","model":"kimi-code/kimi-for-coding","profile":"agent"}
```

### E31

```text
start_ms=1792814074147
pid=99433
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
99433 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814074153
```

```json
{"hook_event_name":"UserPromptSubmit","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","prompt":[{"type":"text","text":"自动模式验收：只在当前目录真实创建 auto.txt，内容为 AUTO_OK，然后读取确认。接着必须尝试调用 AskUserQuestion 问颜色，选项 red/blue；若工具说自动模式不允许问，自己选 red 并结束，不切换模式、不请求人工操作。"}],"is_steer":false}
```

### E32

```text
start_ms=1792814074163
pid=99438
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
99438 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814074173
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","turn_id":5,"origin_kind":"user","prompt":"自动模式验收：只在当前目录真实创建 auto.txt，内容为 AUTO_OK，然后读取确认。接着必须尝试调用 AskUserQuestion 问颜色，选项 red/blue；若工具说自动模式不允许问，自己选 red 并结束，不切换模式、不请求人工操作。"}
```

### E33

```text
start_ms=1792814088414
pid=2149
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 2149 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814088423
```

```json
{"hook_event_name":"PreToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Write","tool_input":{"path":"auto.txt","content":"AUTO_OK"},"tool_call_id":"tool_kR7iDpQUCZ6d0j7C6LlyyU41"}
```

### E34

```text
start_ms=1792814088433
pid=2158
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 2158 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814088439
```

```json
{"hook_event_name":"PostToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Write","tool_input":{"path":"auto.txt","content":"AUTO_OK"},"tool_call_id":"tool_kR7iDpQUCZ6d0j7C6LlyyU41","tool_output":"Wrote 7 bytes to auto.txt"}
```

### E35

```text
start_ms=1792814092129
pid=2679
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 2679 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814092135
```

```json
{"hook_event_name":"PreToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Read","tool_input":{"path":"auto.txt"},"tool_call_id":"tool_fGwBRngb7UpEwOpVtspxoSvz"}
```

### E36

```text
start_ms=1792814092144
pid=2684
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 2684 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814092149
```

```json
{"hook_event_name":"PostToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Read","tool_input":{"path":"auto.txt"},"tool_call_id":"tool_fGwBRngb7UpEwOpVtspxoSvz","tool_output":"1\tAUTO_OK"}
```

### E37

```text
start_ms=1792814098723
pid=4303
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 4303 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814098733
```

```json
{"hook_event_name":"Stop","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","stop_hook_active":false}
```

### E38

```text
start_ms=1792814105578
pid=4951
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 4951 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814105583
```

```json
{"hook_event_name":"UserPromptSubmit","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","prompt":[{"type":"text","text":"子 agent 钩子测试：只用一次内置 Agent 工具，沿用当前默认模型，不指定或切换模型。给它任务：只回复 CHILD_OK，不用工具，不读文件，不再开子 agent。等待它结束后主 agent 只回复 PARENT_OK。"}],"is_steer":false}
```

### E39

```text
start_ms=1792814105591
pid=4957
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 4957 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814105601
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","turn_id":6,"origin_kind":"user","prompt":"子 agent 钩子测试：只用一次内置 Agent 工具，沿用当前默认模型，不指定或切换模型。给它任务：只回复 CHILD_OK，不用工具，不读文件，不再开子 agent。等待它结束后主 agent 只回复 PARENT_OK。"}
```

### E40

```text
start_ms=1792814115806
pid=6234
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 6234 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814115818
```

```json
{"hook_event_name":"PreToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Agent","tool_input":{"description":"子 agent 钩子测试","prompt":"这是一个测试任务。请不要调用任何工具、不要读取任何文件、不要再开任何子 agent。直接以纯文本回复且只回复：CHILD_OK。除此之外不要输出任何其他内容。"},"tool_call_id":"tool_PxMWqSlbzkKs5TM4hdUCTJNs"}
```

### E41

```text
start_ms=1792814115849
pid=6239
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 6239 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814115860
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","turn_id":0,"origin_kind":"system_trigger","origin_name":"subagent","prompt":"这是一个测试任务。请不要调用任何工具、不要读取任何文件、不要再开任何子 agent。直接以纯文本回复且只回复：CHILD_OK。除此之外不要输出任何其他内容。"}
```

### E42

```text
start_ms=1792814115849
pid=6242
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 6242 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814115860
```

```json
{"hook_event_name":"SubagentStart","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","agent_name":"coder","prompt":"这是一个测试任务。请不要调用任何工具、不要读取任何文件、不要再开任何子 agent。直接以纯文本回复且只回复：CHILD_OK。除此之外不要输出任何其他内容。","session_title":"R48B 首句测试"}
```

### E43

```text
start_ms=1792814119937
pid=6709
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 6709 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814119943
```

```json
{"hook_event_name":"Stop","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","stop_hook_active":false}
```

### E44

```text
start_ms=1792814119952
pid=6724
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 6724 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814119957
```

```json
{"hook_event_name":"SubagentStop","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","agent_name":"coder","response":"CHILD_OK","session_title":"R48B 首句测试"}
```

### E45

```text
start_ms=1792814119953
pid=6725
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 6725 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814119958
```

```json
{"hook_event_name":"PostToolUse","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","tool_name":"Agent","tool_input":{"description":"子 agent 钩子测试","prompt":"这是一个测试任务。请不要调用任何工具、不要读取任何文件、不要再开任何子 agent。直接以纯文本回复且只回复：CHILD_OK。除此之外不要输出任何其他内容。"},"tool_call_id":"tool_PxMWqSlbzkKs5TM4hdUCTJNs","tool_output":"agent_id: agent-0\nactual_subagent_type: coder\nstatus: completed\nstop_reason: completed\n\n[summary]\nCHILD_OK\n\nresume_hint: Continue with Agent(resume=\"agent-0\", prompt=\"...\"). Use agent_id only; do not set subagent_type. The subagent retains its prior context; redo any unfinished tool call if its result was lost."}
```

### E46

```text
start_ms=1792814123369
pid=7226
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 7226 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814123380
```

```json
{"hook_event_name":"Stop","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","stop_hook_active":false}
```

### E47

```text
start_ms=1792814128729
pid=8597
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
 8597 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814128735
```

```json
{"hook_event_name":"SessionHeartbeat","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","uptime_ms":60001}
```

### E48

```text
start_ms=1792814178922
pid=14315
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
14315 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814178928
```

```json
{"hook_event_name":"PreCompact","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","trigger":"manual","token_count":25684}
```

### E49

```text
start_ms=1792814188732
pid=15540
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
15540 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814188740
```

```json
{"hook_event_name":"SessionHeartbeat","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","uptime_ms":120001}
```

### E50

```text
start_ms=1792814210398
pid=18266
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
18266 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814210403
```

```json
{"hook_event_name":"PostCompact","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"R48B 首句测试","trigger":"manual","estimated_token_count":24568}
```

### E51

```text
start_ms=1792814225485
pid=19780
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
19780 98404 /tmp/r48b-probe.gF8csL/probe
98404 97615 kimi-code
97615     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814225491
```

```json
{"hook_event_name":"SessionEnd","session_id":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","reason":"exit","session_title":"R48B 首句测试"}
```

### E52

```text
start_ms=1792814236117
pid=21894
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
21894 20964 /tmp/r48b-probe.gF8csL/probe
20964 20291 kimi-code
20291     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814236123
```

```json
{"hook_event_name":"SessionStart","session_id":"session_33e14dec-6f4e-41d6-965c-c74619c6f7e7","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","source":"startup","model":"kimi-code/kimi-for-coding","profile":"agent"}
```

### E53

```text
start_ms=1792814236175
pid=21905
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
21905 20964 /tmp/r48b-probe.gF8csL/probe
20964 20291 kimi-code
20291     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814236180
```

```json
{"hook_event_name":"UserPromptSubmit","session_id":"session_33e14dec-6f4e-41d6-965c-c74619c6f7e7","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"只执行一次 Bash：前台运行 /bin/sh -c 'echo $$ > hup-shell.pid; /bin/sleep 120'。不要后台化，不执行其他命令，等待完成。这是正常停止清理测试。","prompt":[{"type":"text","text":"只执行一次 Bash：前台运行 /bin/sh -c 'echo $$ > hup-shell.pid; /bin/sleep 120'。不要后台化，不执行其他命令，等待完成。这是正常停止清理测试。"}],"is_steer":false}
```

### E54

```text
start_ms=1792814236193
pid=21914
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
21914 20964 /tmp/r48b-probe.gF8csL/probe
20964 20291 kimi-code
20291     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814236203
```

```json
{"hook_event_name":"TurnStarted","session_id":"session_33e14dec-6f4e-41d6-965c-c74619c6f7e7","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"只执行一次 Bash：前台运行 /bin/sh -c 'echo $$ > hup-shell.pid; /bin/sleep 120'。不要后台化，不执行其他命令，等待完成。这是正常停止清理测试。","turn_id":0,"origin_kind":"user","prompt":"只执行一次 Bash：前台运行 /bin/sh -c 'echo $$ > hup-shell.pid; /bin/sleep 120'。不要后台化，不执行其他命令，等待完成。这是正常停止清理测试。"}
```

### E55

```text
start_ms=1792814243653
pid=22907
env_names=CORRAL_EVENTS,CORRAL_HOME,CORRAL_INSTANCE,CORRAL_NAME
22907 20964 /tmp/r48b-probe.gF8csL/probe
20964 20291 kimi-code
20291     1 $HOME/.local/share/ranch/versions/dd4d661/bin/corral
    1     0 /sbin/launchd
end_ms=1792814243660
```

```json
{"hook_event_name":"PreToolUse","session_id":"session_33e14dec-6f4e-41d6-965c-c74619c6f7e7","cwd":"/private/tmp/r48b-probe.gF8csL/work","client_type":"kimi_code_cli","session_title":"只执行一次 Bash：前台运行 /bin/sh -c 'echo $$ > hup-shell.pid; /bin/sleep 120'。不要后台化，不执行其他命令，等待完成。这是正常停止清理测试。","tool_name":"Bash","tool_input":{"command":"/bin/sh -c 'echo $$ > hup-shell.pid; /bin/sleep 120'","timeout":180},"tool_call_id":"tool_u5ZJ5XJrhx5h8kryOrIB766C"}
```

</details>

## 附录 B：主/子 wire 的交互与结束证据

**【查证】** 仅摘录本轮实际 interaction.request/resolved、turn.ended；不复制系统提示、请求上下文或用户配置。最后一条 agentId=agent-0 来自子 wire，其余来自 main。

<details>
<summary>展开交互与结束记录</summary>

```jsonl
{"type":"turn.ended","agentId":"main","turnId":0,"reason":"completed","durationMs":5034,"traceId":"91f9d466bd4bfd1259f941499930659e","time":1792813942258}
{"type":"interaction.request","agentId":"main","id":"approval_649120bc-88a9-4f06-8b26-d78c4556db36","kind":"approval","toolCallId":"tool_Dtwtyh79jxFhNyK9JfwSdFYA","request":{"id":"approval_649120bc-88a9-4f06-8b26-d78c4556db36","sessionId":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","agentId":"main","turnId":1,"toolCallId":"tool_Dtwtyh79jxFhNyK9JfwSdFYA","toolName":"Bash","action":"Running: printf R48B_FILE > marker.txt && cat marker.txt","display":{"kind":"command","command":"printf R48B_FILE > marker.txt && cat marker.txt","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"}},"time":1792813959601}
{"type":"interaction.resolved","agentId":"main","id":"approval_649120bc-88a9-4f06-8b26-d78c4556db36","response":{"decision":"approved"},"time":1792813971159}
{"type":"turn.ended","agentId":"main","turnId":1,"reason":"completed","durationMs":22575,"traceId":"8d3c97ff4fa3fe1f9fbe7982547ddede","time":1792813975144}
{"type":"interaction.request","agentId":"main","id":"question_d24bd93f-6634-4aaa-81c9-07186c720dbf","kind":"question","toolCallId":"tool_iA3vlnicj3RqCq6U1HFI52gn","request":{"turnId":2,"toolCallId":"tool_iA3vlnicj3RqCq6U1HFI52gn","questions":[{"question":"R48B 选哪个","header":"R48B","options":[{"label":"A","description":"测试 A"},{"label":"B","description":"测试 B"}]}]},"time":1792813990328}
{"type":"interaction.resolved","agentId":"main","id":"question_d24bd93f-6634-4aaa-81c9-07186c720dbf","response":null,"time":1792813994284}
{"type":"turn.ended","agentId":"main","turnId":2,"reason":"cancelled","durationMs":17641,"traceId":"f896ca52c6164204e74c2cf078bd4a4c","time":1792813998535}
{"type":"interaction.request","agentId":"main","id":"approval_f5386ae0-c356-4081-8553-0c9c998abc0a","kind":"approval","toolCallId":"tool_WzYp8AhEuELTjsujohgU2DAu","request":{"id":"approval_f5386ae0-c356-4081-8553-0c9c998abc0a","sessionId":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","agentId":"main","turnId":3,"toolCallId":"tool_WzYp8AhEuELTjsujohgU2DAu","toolName":"Bash","action":"Running: /bin/sh -c \"exit 7\"","display":{"kind":"command","command":"/bin/sh -c \"exit 7\"","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"}},"time":1792814012384}
{"type":"interaction.resolved","agentId":"main","id":"approval_f5386ae0-c356-4081-8553-0c9c998abc0a","response":{"decision":"approved"},"time":1792814014847}
{"type":"turn.ended","agentId":"main","turnId":3,"reason":"completed","durationMs":14007,"traceId":"484bbbd5231ee667b26d018ac603837c","time":1792814017872}
{"type":"interaction.request","agentId":"main","id":"approval_0204823b-b115-4425-925d-6976fe7db25a","kind":"approval","toolCallId":"tool_O8T6wah5yAr5G6IsO7srmMOQ","request":{"id":"approval_0204823b-b115-4425-925d-6976fe7db25a","sessionId":"session_dc0f3e16-56c6-45dd-a951-ee96710eb376","agentId":"main","turnId":4,"toolCallId":"tool_O8T6wah5yAr5G6IsO7srmMOQ","toolName":"Bash","action":"Running: /bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 1…","display":{"kind":"command","command":"/bin/sh -c 'echo $$ > term-shell.pid; /bin/sleep 120'","cwd":"/private/tmp/r48b-probe.gF8csL/work","language":"bash"}},"time":1792814031160}
{"type":"interaction.resolved","agentId":"main","id":"approval_0204823b-b115-4425-925d-6976fe7db25a","response":{"decision":"approved"},"time":1792814039534}
{"type":"turn.ended","agentId":"main","turnId":5,"reason":"completed","durationMs":24579,"traceId":"884e1e7fae4ce763658a06dba080904b","time":1792814098734}
{"type":"turn.ended","agentId":"main","turnId":6,"reason":"completed","durationMs":17796,"traceId":"b91218376966538ab0c7174f5451401c","time":1792814123381}
{"type":"turn.ended","agentId":"agent-0","turnId":0,"reason":"completed","durationMs":4105,"traceId":"94afe8380aa17fdf15d4eb2b50b3d582","time":1792814119947}
```

</details>
