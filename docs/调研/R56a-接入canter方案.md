# R56a：corral 接 canter 的方案

2026-11-03，ranch/dev-r56a。这一轮只出设计，不写代码。

依据：
- `docs/任务/R56-接入canter.md`：用户要的 10 条、这一版不做的、规矩。
- `docs/任务/R56a-接入canter方案.md`：任务文件，含 canter 主控 11-03 补的要点。
- DESIGN §6–§10、CONTRACT。
- corral、pony、派活表和技能的代码，行号以本分支起点 `b88fbd2` 为准。
- canter 的公开接口，只读这三处：
  - `canter --help`（本机 `0.1.0 · 16fb98c`）；
  - canter `docs/DESIGN.md`；
  - `crates/canter-protocol/README.md`。
- 一次隔离环境下的实测，见「实测记录」。

paddock 的代码只读过两处，用来确认它现在怎么显示 `hooks` 和 `unsupported`，没有改。

引用记号：
- 【canter DESIGN n】指 `~/Developer/personal_projs/canter/docs/DESIGN.md` 第 n 行。
- 【协议 README n】指 `~/Developer/personal_projs/canter/crates/canter-protocol/README.md` 第 n 行。
- 【实测 n】指下面「实测记录」第 n 条。
- corral 代码写 `文件:行`。

## 结论先说

1. **通用挂点**：`Adapter` 加一个可选方法，声明「这一家带一个盯状态的线程」；有它的家，pen 在主循环里起这个线程、在换 pen 和退出前收回。
   - 线程把 canter 的事件翻译成现有的 v1 事件，追加到这个 agent 的 `events` 文件里，和钩子写的一模一样。
   - 状态、`interrupted`、回复、送话确认、`wait`、题目、命令全都走现有的状态机和读取路径。
   - 别家不起线程，行为逐字不变。
2. **开和会话跟随**：
   - corral 在 canter 的参数前加 `--session-file <agent 目录>/canter-session.json --close-on-exit`；首句照默认放在 `-- <首句>`；不加 `--tag`。
   - 线程读会话文件，知道当前是哪个会话；再读 `<stateDir>/server.json` 找到 socket。
   - 用 `GET /v1/info` 核对：`protocolVersion` 为 1，`features` 里有 `managedClients`。核对过就用 `observe=true` 订阅事件。
   - 会话文件里的 ID 变了就换订阅。还没有会话文件时报 `starting`。
3. **状态**：空闲与否只看 `session.status`。
   - 回到 idle 时，看最后一轮的结果：做完写 `Stop`（带回复），被打断写 `Interrupt`，出错写 `StopFailure`。
   - `request.opened` 写成 `PermissionRequest`，带 `command` 或 `questions`。
   - canter 另外报 `interaction:{kind,count,running}`，格式和 Kimi 的一样。
4. **停**：
   - 靠 `--close-on-exit`。停止步骤是 HUP 等 3 秒、TERM 等 3 秒，最后 KILL。
   - agent 退出后，pen 等这次跟过的会话都变成 `closed`，最多 12 秒（盖住 canter 10 秒的租约）。
   - 没等到的会话写进 `exit.json` 和 `corral stop` 输出的新字段 `remaining_sessions`。
   - 不另调同步 DELETE。
5. **升级**：
   - 换 pen 前先收回线程，因为 fork 时进程里只能有一个线程。
   - 线程的游标存进 pen 快照（新字段，SCHEMA 不变），新 pen 用 `after=游标` 接着订阅，不丢事件，也不重报旧回复。
   - canter 服务端自己换版本时会报 `serverShutdown`，这不当停止：重读 `server.json`，重连后接着订阅。
6. **不支持的明确报**：
   - 暂停、继续：CLI 和 pen 两头都按种类拒绝，报新错误码 `unsupported_kind`（退出码 1）。它和「旧 pen 要升级」的 `unsupported`（退出码 9）分开。
   - `--sandbox`，不管是显式给的还是从父实例继承的：在 start 时、调 fence 之前拒绝，同一个错误码。
   - `status`、`ls`、`integration status` 里，canter 那一项加 `unsupported:["pause","sandbox"]`，paddock 可以据此提前不显示这些操作。
7. **`integration status`**：canter 一行写 `hooks:"launch"`，paddock 现在就会显示成 Built in；技能位置用 `codex`（`~/.agents`）。不加新的接法名。
8. **pony**：
   - 「拒绝」「打断」都给 canter 按一次 Esc。
   - canter 框开着时每 2 秒左右重画一次【实测 11】，pony 现在「两次画面一样才按」的核对对它永远不成立。所以 canter 改用 Kimi 那套结构化核对（`interaction.count == 1` 等），不比较屏幕。
   - 标题：去掉开头的 `✓`、`✋` 和转圈字符后，剩下 `canter · <目录>` 的，当成只有程序名，不当摘要显示。
9. **派活**：表里加 `[agents.canter]`（`canter --yolo`）和 `[cells.canter]`，只在点名时派。两份技能照 Kimi 的写法各加一段。
10. **对 paddock**：全是兼容新增，没有不兼容的改动（第 11 节列全）。
11. **实现拆两件**：corral 本体一件（碰要害，要交叉审查）；pony、派活表和技能一件，文件和前一件不重叠，可以并行。
12. **要用户定的**有两条：
    - corral 要不要替用户记 canter 的目录信任。建议不记。
    - 会话没确认关掉时 `corral stop` 怎么报。建议照 Kimi 的 `remaining_pids`：`ok:true`，加 `remaining_sessions`。
13. **要 canter 补文档或说明的**有 3 条，都不挡实现（第 12 节）：
    - `/v1/info` 的版本字段实际叫 `serverVersion`，不叫 `version`；
    - 子 agent 请求的事件形状只写在 canter DESIGN；
    - `session.status error` 什么时候出现。

## 实测记录

### 环境

- canter `0.1.0 · 16fb98c`。`HOME`、`CANTER_HOME` 都放在 `/tmp/r56a-probe`，用完删掉。
- `config.json` 设成 0600，指向本机的假 Chat Completions 服务。这个服务是放在 scratchpad 里的一次性 Node 脚本，不在仓库里：按脚本回 SSE，能挂起不回，能回 HTTP 400。配置里 `prompt_suggestions:false`。
- 终端画面用 corral 开的 `ranch/test-r56a-probe`。启动时带了这些参数：
  - `--label role=test`
  - `--env CAIRN_DISABLE=1`
  - 隔离的 `--env HOME=…`、`--env CANTER_HOME=…`
- 装着的 corral 是 `f4a9a29`，它不认识 canter，按未知程序处理（停止时 HUP 等 3 秒，再 TERM 等 3 秒）。
- 事件用 `curl --unix-socket` 订阅。
- 自己起的服务端和假模型用完都按 PID 停掉。没有连用户的 `~/.canter`，也没有连用户正在跑的服务端。

### 看到的

1. `server.json` 是 `{"pid","version","protocolVersion","socketPath","instance"}`。`socketPath` 是解析过的绝对路径（`/private/tmp/...`）。
2. `GET /v1/info` 回 `{"features":[…],"serverVersion":"0.1.0","protocolVersion":1,"sessions":[{id,cwd,name,status,updatedAt,lastEventSeq}]}`。
   - 版本字段叫 `serverVersion`，没有 `version`；`server.json` 里才叫 `version`。
   - `features` 实际有：`managedClients`、`subagentSessions`、`subagentTools`、`subagentStop`、`memory`、`runningTurns`、`sessionFork`、`conditionalName`、`sessionTrash`、`folderTrust`、`nonInteractiveInput`、`review`、`addDirectories`、`tags`、`stats`、`goals`、`lsp`。
3. 会话文件：
   - 内容是 `{"cwd":"/private/tmp/r56a-probe/work","sessionId":"s-…","stateDir":"/tmp/r56a-probe/state"}`。`stateDir` 原样写，没有解析；`cwd` 是解析过的。
   - 信任框开着时不写这个文件。
   - `/new` 以后改写成新 ID。
   - 服务端 replace 后终端重连，用同一个 ID 重写一次（mtime 变了）。
4. SSE：
   - 请求：`GET /v1/events?session=<id>&observe=true&after=N`。
   - 每帧三行：`id: N`、`event: message`、`data: {seq,sessionId,timestamp（毫秒）,type,…}`。另有 `: heartbeat` 注释行。
   - 订阅开着的会话时，会话关了或服务端退出，流就结束（EOF）。
   - 订阅一个已经关掉的存盘会话时，补发完不会结束，一直挂着。
5. 一轮正常的事件顺序：
   1. `turn.started`：带 `turn.input` 的文字、`clientName:"canter-tui ttysNNN"`、`inputId`。
   2. `session.status working`。
   3. `item.completed`（userMessage）。
   4. `item.started`、`item.delta`、`item.completed`（agentMessage）。
   5. `telemetry.*`。
   6. `turn.completed`，`turn.status` 为 `completed`。
   7. `session.status idle`。
6. 模型回 HTTP 400 时：
   1. `error`，带 `willRetry:false`。
   2. `turn.completed`，status 为 `failed`，带 `error.message`。
   3. `session.status idle`。会话回到 idle，不是 `error`。
7. 批准框：
   - 弹框时发 `request.opened`，内容是 `request:{id:"approval-…",turnId,createdAt,kind:{type:"approval",itemId,tool:"shell",arguments:{command},availableDecisions,proposedExecpolicyAmendment,sessionPrefixes}}`。
   - 然后 `session.status waiting_approval`。
   - `echo` 这类只读命令在白名单里，不弹框【canter DESIGN 118】；`mkdir … && touch …` 才弹。
8. 提问框：
   - 模型调 `ask_user` 工具时发 `request.opened`，`kind:{type:"userInput",itemId,question,options:[{label,description}]}`。一次只有一题、单选，没有 header，也没有多选。
   - 然后 `waiting_approval`。
   - 选了一项以后：`request.resolved`，`resolution:{type:"userInput",answer:{type:"option",label:"Red"}}`，再回到 working。
9. 按 Esc：
   - 框上按一次：
     1. `request.resolved`：`resolution:{type:"cancelled"}`，`clientName:null`。
     2. `item.completed`：failed。
     3. `turn.completed`：interrupted。
     4. idle。
   - 干活时按一次：`turn.completed`（interrupted），然后 idle。
   - 被打断的那一轮里可能已经有一条完成了的 agentMessage（实测是 `"partial answer "`）。它不能当回复。
10. 终端标题：
    - 干活时 `⠙ canter · work`，第一个字符是转圈动画。
    - 等批准或等回答时 `✋ canter · work`。
    - 一轮做完 `✓ canter · work`。
    - 被打断后、服务端 replace 后是 `canter · work`。
    - 信任框时标题为空。
11. 框开着时，canter 每 2 秒左右重画一次（有个等待计时，比如 `1m 00s`），所以 `corral read --bytes 4000` 读到的内容每 2 秒就变。
12. `/new` 以后：
    - 旧会话还开着（idle），没有任何事件。
    - 新会话先来 `session.updated`（tags 也带过来了），再来 `session.status idle`。
13. `canter serve --replace`：
    - 旧服务端把开着的会话都报 `session.status closed`、`closeReason:"serverShutdown"`，流随之结束。
    - 新服务端的 pid、instance 都变了，`socketPath` 不变。
    - 终端重连后只恢复它当前的会话：发一条新的 `session.status idle`，seq 接着往上涨；会话文件重写一次。
    - 终端之前 `/new` 换走的那个会话保持 closed。
14. `corral stop`（发 HUP）：终端 0.1 秒内退出，退出码 0；服务端关掉会话，`closeReason:"clientRequest"`。
15. `kill -9` 终端：正好 10 秒后会话被关，`closeReason:"clientRequest"`。
16. 没有服务端时，终端自己起 `canter serve`：
    - 服务端的父进程是终端，但它自成进程组、自成会话（`ps` 的 STAT 是 `Ss`，PGID 等于自己的 PID）。
    - 它继承第一个终端的环境。
    - 终端退出后服务端还留着。
17. `canter … -- "-dash prompt"` 能把以 `-` 开头的首句送进去。
18. `POST /v1/folderTrust {"cwd"}` 回 `{"path","trusted":true}`；`GET /v1/folderTrust?cwd=` 也能查。
19. 其他接口：
    - 快照 `GET /v1/sessions/{id}` 回 `{"session":{id,settings,status,closeReason?,createdAt,updatedAt,turns,pendingRequests,queuedInputs,tokenUsage,error,lastEventSeq},"eventSeq":N}`。`agentRequests`、`backgroundCommands` 为空时不出现。
    - `POST /v1/sessions` 必须带 `"model":""`，不带就报 `invalid_request`。
    - 同步的 `DELETE /v1/sessions/{id}` 回 `{"eventSeq":N}`。

### 没实测的

- 子 agent 的 `agent.request.*`
- `sandboxApproval`、`planApproval`
- 目标续跑、后台命令跑完叫醒模型
- `/resume`、分叉、交接
- 提问框上按 Esc
- `canter upgrade`，以及服务端 replace 时正好有轮在跑
- `session.status error`
- 多行文字送进去以后 `turn.input` 是什么样

## 1. 通用挂点

### 接口

`Adapter`（`agents/mod.rs:47`）加一个默认返回空的方法：

```rust
/// A thread that follows the agent's state, started and stopped by the pen (R56).
fn watcher(&self) -> Option<&'static dyn Watcher> { None }
```

`Watcher` 是 corral 公共代码里的小接口。pen 只跟它打三种交道：

- **起**：pen 交给线程这些东西：
  - agent 状态目录、实例号、corral 记的 cwd、实例启动时间；
  - 上次的游标（不透明的 JSON）；
  - 一个停止标记；
  - 一块和 pen 共享的小结构，线程在里面放「游标、连没连上、出错原因、收尾结果」。
- **停**：pen 置停止标记，最多等 3 秒收回线程，拿回游标。
- **收尾**：agent 退出后，pen 问线程收尾做完没有，拿到没确认关掉的会话。

这一家在线程里具体做什么，pen 不管。canter 的线程和翻译都放在 `agents/canter.rs`，订阅也在这个文件里，这符合用户定的「放在 canter 自己的接入文件，公共代码只加一个通用挂点」。

### 线程的产出怎么进现有路径

推荐：线程把翻译出来的 **v1 事件追加到 `<dir>/events`**，写法和钩子一样：
- 一行一次 `write_all`，文件用 `O_APPEND`、权限 0600（`hooks.rs:61-69`）；
- 每行带 `v:1`、`inst`、`t`、`session_id`、`cwd`、`has_transcript:true`；
- 另带 `canter_seq`，方便查问题。状态机不认识这个字段，会忽略它。

读取方完全复用现有的：
- `events::read`、`update`（`events.rs:32-117`、`223`）；
- 送话确认 `confirmation`（`cli.rs:330`）；
- `reply`（`cli.rs:744`）、`wait`；
- cursor 缓存。

agy、amp 也是直接出 v1 事件、状态机不改（DESIGN §7 第 109 行）。

备选和取舍：

- **像 Kimi 那样由读取方现算**：线程只把原始帧落盘，或者干脆不落盘，由读取方自己连 canter，`status` 时现算。
  - 好处：改翻译只要换 CLI，不用换 pen。
  - 代价：
    - paddock 每秒一次 `status`，每次都要读文件或连 socket；
    - 要新的 cursor 版本和 `FILES` 项；
    - 送话确认要另写一套。
  - 不推荐。翻译错了用 `corral upgrade` 换 pen 就能修，corral 本来就有这条路。
- **写 v2 原始事件**（`events/raw.rs`）：v2 的事件名是 pi、omp 的（`agent_end`、`message_end`……）。canter 要在 `raw.rs` 里再加一整套分支，等于把「读取方现算」搬进事件文件，并不比 v1 省事。不推荐。

### 线程的生命周期

- **起**：在 `Pen::run()` 开头起（`pen.rs:778`）。三种情况都会走到这里，所以只需要起一处：
  - 新开；
  - `__pen-resume`（`upgrade.rs:577`，最后调 `pen.run()`）；
  - 备用 pen 接手：它 exec 的是 `__pen-resume`（`upgrade.rs:477-515`）。

  observer 模式也起，因为它同样要报状态。
- **只给 canter-aware 的 pen 起**：Pen 加一个新字段（`#[serde(default)]`），存这一家的游标。新开 canter 时是 `Some`。旧 corral 开的 canter 换到新 pen 时，快照里没有这个字段，就是 `None`，不起线程。这种 agent 启动时没带 `--session-file`，跟不了，状态报 `unknown`，见第 3 节。
- **停**：
  - 线程读 socket 时设 1 秒超时，每秒看一次停止标记。canter 的心跳 15 秒才一次【协议 README 386】，不能靠它来叫醒线程。
  - 线程只在两帧之间停，所以不会写出半行。
- **换 pen 前必须先停**：
  - handoff 要 fork（`upgrade.rs:355`）。多线程的进程 fork 以后，子进程里只剩调 fork 的那个线程；别的线程手上的锁（包括 malloc 的锁）永远不会放开，备用进程再分配内存就可能卡死。
  - 所以主循环在取出 `pending_upgrade`（`pen.rs:789`）以后、调 `handoff` 之前，先停线程，把游标写进 Pen 的新字段，再交接。
  - 3 秒内停不下：这次升级按失败处理（`upgrade.result:"failed"`，`last_error` 写原因），线程照旧跑。
  - handoff 失败、回到旧 pen 时（`upgrade.rs:373-389`），用同一份游标重新起线程。
- **出错**：连不上、`info` 核对不过、会话文件读不出，都写进 `pen.log`（同一个原因只写一次），隔 1 秒重试；不写事件，不动状态。断开超过 20 秒（比 15 秒的心跳长），CLI 把这个 agent 报成 `unknown`（第 3 节）。
- **线程 panic**：pen 不跟着退出（`JoinHandle` 拿到 Err），写 `pen.log`，按「断开」处理，不自动重起。下次 `corral upgrade` 会重起线程。
- **pen 退出**：agent 退出后，线程转去做收尾确认（第 5 节）。确认完或到时间，pen 收回线程，再写 `exit.json`。
- **别家**：`watcher()` 默认返回 `None`，pen 不起线程，新字段一直为空，快照和事件都不变。

## 2. 开和会话跟随（第 1 条）

### 启动参数

```text
canter --session-file <agent 目录>/canter-session.json --close-on-exit <用户的参数> [-- <首句>]
```

- `hook_args` 放在程序名后面（`agents/mod.rs:263-280`）。
- 会话文件放在 agent 状态目录里，和 `events` 在一起，`corral ls` 清理时一起删掉。它要加进 `state::FILES`（`state.rs:19`）；`agents/mod.rs` 的测试 `kinds_are_unique_and_files_written_are_internal` 本来就要求这一点。
- 用户自己的参数里已经有 `--session-file`：报 `usage`，因为 corral 要靠自己那一份跟会话。已经有 `--close-on-exit`：不再重复加。
- 首句用默认的 `prompt_args`，也就是 `-- <首句>`；以 `-` 开头也没问题【实测 17】。
- `--tag`：不加。
  - 会话归属靠会话文件，用不上标签。
  - 标签会存进用户的 canter 会话记录。
  - 「列出别的客户端开的会话」这一版不做（R56「这一版不做」）。
- `--cwd`、`--env`、`--label` 都照现有规矩。
- 环境变量：
  - canter 建会话时，客户端把自己的环境交给服务端，模型跑的命令用的是这份环境【协议 README 211】【canter DESIGN 115】。所以在 canter 里跑的命令能看到 corral 设的 `CORRAL_NAME`（corral 技能靠它认出自己）。这一点是按文档写的，没实测，联调时核对一次。
  - 终端自己起的服务端会继承第一个终端的环境【实测 16】，但命令不用服务端的环境，所以不影响。
- 支持的启动方式只有交互式的 `canter [选项] [首句]`、`--resume`、`-c`。`serve`、`exec` 等子命令没测，也不专门处理。

### 找服务端

1. 读会话文件，取 `sessionId`、`stateDir`。多出来的字段不管；读不出来或者只读到半个文件，就等下一次再读。
2. 读 `<stateDir>/server.json`，取 `socketPath`。这是 canter 写明的发现约定【canter DESIGN 543】。`stateDir` 就是终端用的 `CANTER_HOME`，corral 不自己去猜 `~/.canter`。canter 状态目录里只读 `server.json` 这一个文件。
3. `GET /v1/info`，要求 `protocolVersion == 1`、`features` 里有 `managedClients`。不满足就在 `pen.log` 里写「canter too old」或「protocol N」，不订阅，状态最后报 `unknown`。版本字段实际叫 `serverVersion`【实测 2】，只写日志，不拿来判断。
4. `GET /v1/events?session=<id>&observe=true&after=<游标>`，一律带 `observe=true`【协议 README 384-386】。这样 corral 不算 canter 的客户端，不影响「最后一个客户端走后 10 秒收尾」的规则。

### 会话换了怎么办

- 每 0.5 秒看一次会话文件（设备号、inode、mtime、内容）。ID 变了就换订阅。
- 跟上一个新会话时，怎么开头：
  - 会话的 `createdAt`（取快照）晚于实例启动：这是第一次建的会话，或者 `/new`、分叉、交接出来的新会话。从 `after=0` 补发：先写一条 `SessionStart`，再照第 3 节翻译全部事件。
  - 早于实例启动：这是 `--resume`、`-c`、`/resume` 恢复的旧会话。
    1. 先取快照，用快照的 `lastEventSeq` 做起点。快照和事件序号是对得上的【协议 README 122】。
    2. 写 `SessionStart`。
    3. 照快照现状补一条：有没解决的框就补 `PermissionRequest`；`status` 是 working 就补一条不带工具名的 `PreToolUse`（只把状态改成 working，不算一次输入）。

    旧的回复不重报，之后的事件照常处理。
- 同一个 ID 的文件被重写（服务端 replace 以后）：不换会话，只照第 6 节重连。
- 旧会话：`/new` 以后旧会话还开着【实测 12】。corral 不再跟它的状态，但记住它的 ID，停的时候要确认它也关了（第 5 节）。
- 会话文件还没出现：
  - 一条事件都没有，`events::read` 给出 `fresh`（`events.rs:21`），也就是 `starting`。
  - 信任框和刚启动分不出来，都报 `starting`（canter 主控的要点）。
  - 带首句启动的，首句的 `turn.started` 到之前也是 `starting`，这是现有规则（`cli.rs:160-168`）。
- `SessionStart` 的 `cwd` 写 corral 记的 agent 目录：
  - canter 的会话归属由会话文件说了算，不用 `events.rs:162` 那种「同目录」的猜法。
  - 会话文件里的 `cwd` 是解析过的路径，可能和 corral 记的写法不一样【实测 3】。

## 3. 状态、打断、回复、提问、命令（第 2、3 条）

### 规则

- 空闲与否只看 `session.status`【协议 README 401】。
  - `turn.completed` 只记下这一轮是怎么结束的；真正写结束事件，要等到 `session.status idle`。
  - 原因：一轮结束不等于空闲。目标续跑、后台命令跑完叫醒模型，都会马上开下一轮。
- 跳过的帧：
  - `seq` 为 0 的帧【协议 README 373 起】；
  - `item.delta`、`telemetry.*`、`warning`。
- 去重：游标记着每个会话处理到的 seq，续订时用 `after=seq`，只处理 `seq` 大于游标的帧。这样补发和直播有重叠也没关系。
- 时间：v1 事件的 `t` 取帧的 `timestamp / 1000`（同一台机器的钟），这样 `turn_started`、`state_started` 都和实际一致。

### 对照表

| canter | 线程写的 v1 事件 | corral 报的 |
|---|---|---|
| 开始跟一个会话 | `SessionStart` | 主会话换成它，idle |
| `turn.started` | `UserPromptSubmit`，`prompt` 是 `turn.input` 里的文字连起来；没有文字（目标续跑、后台叫醒）就是空串 | working，`interrupted:false`，记下输入摘要，供送话确认和 `last_input_source` 用 |
| `item.started`（工具） | `PreToolUse`，`tool_name` 用 canter 的工具名 | working，`last_tool` |
| `request.opened`，`approval` | `PermissionRequest`，`tool_name` 取 `kind.tool`。工具是 `shell` 时带 `command`：取 `arguments.command`，16 KiB 截断，和 `hooks.rs:74` 一样 | blocked，`command` |
| `request.opened`，`userInput` | `PermissionRequest`，`tool_name:"ask_user"`，`questions:{items:[{header:"",question,multi_select:false,options}],truncated}`，用 `hooks.rs:94` 同一套白名单和 16 KiB 上限 | blocked，`questions` |
| `request.opened`，`sandboxApproval` 或 `planApproval`【协议 README 3、534】 | `PermissionRequest`，`tool_name` 就用请求种类的名字 | blocked，不给 `command`、`questions` |
| `agent.request.opened`（子 agent 的框，【canter DESIGN 29】） | 主会话自己没有框时写一条 `PermissionRequest`，不带内容 | blocked（canter 主控说两处都算 blocked）；`interaction.kind:null` |
| `request.resolved`、`agent.request.resolved` | 还有别的框：对最早那个框再写一条 `PermissionRequest`。没有了：写 `PostToolUse` | 回到 working |
| `session.status` 是 working，而 corral 这边是 idle | 补一条不带工具名的 `PreToolUse` | working，不算输入 |
| `session.status idle`，最后一轮是 completed | `Stop`，`last_assistant_message` 取上次 idle 以来最后一条完成的 agentMessage；没有就不带 | idle，`reply` |
| 同上，最后一轮是 interrupted | `Interrupt` | idle，`interrupted:true`；`reply` 报 `no_reply`（CONTRACT R22） |
| 同上，最后一轮是 failed【实测 6】 | `StopFailure` | idle，`reply` 不变，和 Claude Code 出错时一样 |
| `session.status idle`，上次 idle 以后没开过轮（比如服务端 replace 后恢复会话那一条） | 不写 | — |
| `session.status error` | `StopFailure` | idle，等人处理（第 12 节第 3 条） |
| `session.status closed`，`serverShutdown` | 不写，去重连（第 6 节） | 状态不变 |
| `session.status closed`，其他原因 | `SessionEnd` | exiting |
| `input.queued`【协议 README 148】 | 不写。轮到它时会有带 `inputId` 的 `turn.started` | — |
| `backgroundCommand.*`【协议 README 27】 | 不写。后台命令跑完叫醒模型时，会有新的 `turn.started` | 后台命令在跑时照样报 idle，和 Claude Code 的后台任务一样 |

### 共享代码要放宽的两处

- **`questions` 和 `command`**：
  - 现在的限制：
    - `events.rs:38-47` 只在 `tool_name` 是 `AskUserQuestion` 或 `Bash` 时，才留下 `questions` 或 `command`。
    - `cli.rs:186-197` 只在 `last_tool` 对得上时才给出这两个字段；`command` 还要求 `kind == "claude"`。
    - canter 的工具名是 `ask_user` 和 `shell`，过不了这两道门。
  - 推荐：
    - 状态机改成「`PermissionRequest` 带了这个字段就留」。现有钩子只在那两个工具上才带这两个字段（`hooks.rs:50-60`），所以别家的结果逐字不变，cursor 也不用升版。
    - CLI 对 canter 的门槛放宽成「blocked，而且快照里有」。
  - 不推荐：canter 的事件冒充 `AskUserQuestion`、`Bash`。这样共享代码一行都不用改，但 `last_tool` 会对外报一个假的工具名。
- **`interaction`**：线程在它写的每条事件里都带上 `interaction:{kind,count,running}`。状态机见到这个字段就原样存下（只多一行，别家的事件没有这个字段，结果不变），`status` 对 canter 把它报出来。含义照 CONTRACT 对 Kimi 写的那段（R48g2，CONTRACT 第 152 行）：
  - `kind`：主会话最早那个没解决的框，`userInput` 报 `question`；`approval`、`sandboxApproval`、`planApproval` 报 `approval`。只有子 agent 的框时为 `null`。
  - `count`：主会话和子 agent 没解决的框加起来的总数（`pendingRequests` 加 `agentRequests`）。
  - `running`：有一轮开着，也就是 `turn.started` 之后、`turn.completed` 之前。
  - 不给 `resolved`、`action`：这一版 pony 不做允许，也不远程回答。

### 什么时候报 `unknown`

- 新 pen 回给 CLI 的 status 里，多带线程的情况：线程在不在、最后一次连上是什么时候、出错原因。这只是 pen 和 CLI 之间的内部协议，不是公开约定。
- CLI 对 canter 的判断：
  - pen 的回复里没有这些信息：要么是旧 pen，要么是旧 corral 开的 canter。报 `unknown`。
  - 断开超过 20 秒：报 `unknown`。
  - 其余照状态机。
- 原因只写进 `pen.log`，不加公开字段。用户 10-14 不要 `state_source` 这类字段（DESIGN §7 第 111 行）。

## 4. 送话、按键、attach、read（第 4 条）

全都照旧经终端。corral 不调 canter 的 `POST …/input`、`…/interrupt`、`…/requests`。几个要注意的地方：

- `send` 的空闲检查（`cli.rs:300`：不是 idle 就退回 7 `not_idle`）依据的是线程翻译出来的状态。连不上时状态是 `unknown`，`send` 照现有规则退回 7。这时没法确认话送没送到，退回是对的。
- 送话确认靠 `turn.started` 里的输入文字算摘要（`cli.rs:330-363`）。canter 会不会把粘贴进去的多行文字原样放进 `turn.input`，没实测。实现时要测多行；对不上时，现有的「并进草稿」判断（`merged_with_draft`）能兜一部分，兜不住就要在翻译里规范化。
- 人在终端里趁它干活时打字：canter 先排队（`input.queued`），轮到时发 `turn.started`。corral 把它算一次新输入，`last_input_source` 按现有规则判成 human（`cli.rs:214-227`）。
- Esc：canter 按一次就打断【实测 9】，`keys esc` 照用。
- `read`、`attach` 不变。不需要 `screen_rules`：框由事件报出来。

## 5. 停（第 5 条）

推荐只用 `--close-on-exit`，加上确认；不另调 DELETE。

- **`quit_steps`**：`[{"signal":"HUP","wait":3},{"signal":"TERM","wait":3}]`，最后照例 KILL（`agents/mod.rs:53`）。
  - 实测发 HUP 后 0.1 秒就退了【实测 14】。
  - 终端正常退出（包括收到 HUP、TERM）会马上通知服务端收尾【协议 README 80-95】，这也是 canter 主控的要点。
  - 终端被 KILL 时，服务端 10 秒后自己收尾【实测 15】。
- **`stop_descendants` 保持 false**：终端自己起的服务端是它的子进程，但自成会话【实测 16】，按进程组发信号碰不到它。要是跟踪子孙，就会把用户所有会话共用的服务端一起杀掉。
- **确认**：agent 退出后、pen 写 `exit.json` 之前，线程把这个实例跟过的每一个会话都确认到 `closed`。
  - 当前会话：看订阅里来的 `session.status closed`。
  - `/new` 换走的旧会话：取快照看 `status`。
  - 全都关了就马上结束；最多等 12 秒，盖住 10 秒的租约。
  - 不管 `closeReason` 是什么，关了就算。
- **没确认关掉的**：
  - 会话 ID 写进 `exit.json` 的 `remaining_sessions`。
  - `corral stop` 的输出带同名字段。照 R48e 的 `remaining_pids`：`ok:true`、退出码 0，没有残留就不出现这个字段。
  - `pen.log` 写原因：还开着（附它的状态），或者服务端连不上。
  - 要不要改成算失败，见「要用户定的」第 2 条。
- **为什么不调 DELETE**：
  - 受管模式已经覆盖了两种情况：终端正常退出，和终端死掉。
  - corral 分不出一个会话此刻还归不归这个终端，canter 不公开归属（第 12 节第 4 条）。盲调可能把用户在别处刚恢复的同一个会话关掉。
  - DELETE 唯一的好处，是终端被 KILL 时能快 10 秒。
- **服务端连不上**（`server.json` 没了，或 socket 连不上）：没法确认，全部写进 `remaining_sessions`，`pen.log` 写「server unreachable」。
- **`corral stop --timeout`**：最坏要 3 + 3 + 12 秒，约 18 秒，再加上现有的余量。CONTRACT 里照 Kimi 那样写一句「调用方的 `--timeout` 要留够」。
- **canter 已知没修的两点**（canter 主控转述 canter HANDOFF）：
  - 服务端排空期间，终端显式退出不会触发收尾，由停机路径关会话。这时会话照样会变成 closed，corral 照样能确认。
  - 租约过期后终端会一直显示 `Reconnecting`。corral 不靠画面判断，不受影响。

## 6. 升级（第 6 条）

### `corral upgrade` 换 pen

1. `prepare_upgrade`（`upgrade.rs:311`）：对 canter 的 agent，目标程序的 `__pen-probe` 要报它能跟 canter（probe 回复里加 `watchers:["canter"]`）。报不出来就拒绝，报 `incompatible`、退出码 9，免得换成一个不会跟的 pen，状态从此卡住。
2. 主循环取出 `pending_upgrade` 以后、调 `handoff` 之前，停线程（见第 1 节），把游标写进 Pen 的新字段：
   - 字段加 `#[serde(default)]`，SCHEMA 仍是 1，R50 起加字段都是这么做的；
   - 旧快照里没有这个字段，按「不起线程」处理。
3. 新 pen 走 `__pen-resume` → `run()`，用游标起线程，以 `after=游标` 续订：
   - 中间漏掉的帧由 canter 补发【协议 README 380】，不丢；
   - 只处理 seq 大于游标的帧，不会重报旧回复；
   - 游标里记着当前会话已经开过头，所以不重写 `SessionStart`。
4. 失败退回（handoff 里 exec 失败，或者备用 pen 接手）：都从同一份游标起线程，同上。
5. 线程停着的那几秒，事件不丢，只是晚到。CLI 读的是 events 文件，这期间照常报上一个状态。

### canter 服务端自己换版本（`serve --replace`、`canter upgrade`）

- 看到 `closed` 加 `serverShutdown`，或者流断了（EOF）：不写事件，不当停止。
  1. 每秒重读一次 `<stateDir>/server.json`；
  2. 新 socket 连得上、`info` 核对过以后，用 `after=游标` 续订同一个会话。新服务端会恢复终端的当前会话，seq 接着涨【实测 13】。
- 恢复时会再来一条 `session.status idle`【实测 13】。游标里这一轮早已结束，就不重写 `Stop`（对照表里「上次 idle 以后没开过轮」那一行）。
- 恢复以后终端要是换了会话（会话文件里的 ID 变了），照第 2 节换订阅。
- replace 时正好有一轮在跑：这一轮会断掉【canter DESIGN 115】，但 canter 那时发什么事件没实测。翻译照第 3 节的表做，不另外去猜，联调时看一次。
- 终端 `/new` 换走的旧会话不会被恢复【实测 13】。停的时候它已经是 closed，不会进 `remaining_sessions`。
- 20 秒还连不上：报 `unknown`（第 3 节），线程接着每秒重试。

## 7. 不支持的明确报（第 7 条）

| 操作 | 在哪拦 | 报什么 |
|---|---|---|
| `corral pause` 或 `resume` 一个 canter | CLI 发请求前按 `meta.json` 里的种类先拦；pen 收到 `pause`、`resume` 时再拦一次（`pen.rs:587` 起的 `request`） | `ok:false`、`error:"unsupported_kind"`、message 为 `canter does not support pause`（或 resume）、带 `name`、退出码 1 |
| `corral start --sandbox <档案> -- canter …` | CLI 的 start 里、`sandbox::check` 之前（`cli.rs:658-661`） | 同一个错误码。message 说清楚：canter 的命令在共用的服务端里跑，不在墙里。不起 pen |
| 从父实例继承的 `child-sandbox` 档案 | 同上，`child_sandbox::inherited()` 拿到档案以后 | 同上 |

- **为什么两头都拦**：
  - 新 CLI 碰上旧 pen（部署前开的 canter，旧 pen 把它当未知程序，pause 真的会冻）：靠 CLI 拦。
  - 旧 CLI 碰上新 pen：靠 pen 拦。旧 CLI 收到 pen 的未知错误会原样转出（`cli.rs:697-703`，只把 `bad_op` 改成 `unsupported`），所以新旧 CLI 的输出一样。
- **为什么一定要拦暂停**：除了用户定的这一版不做，还因为 freeze 会冻住 agent 的子孙连同它们的会话（`freeze.rs`）。终端自己起的服务端就是它的子孙【实测 16】，一冻就会把用户所有 canter 会话一起卡住。
- **为什么 sandbox 要 corral 自己拦**：
  - fence 现在没有 canter 的种类数据，会报 `sandbox_failed`（fence 里的错误是 `unsupported_kind`）。
  - 但用户可以自己加 `~/.fence/agents/canter.toml`。那样 fence 只把终端关进墙，命令照样在墙外的服务端里跑，等于悄悄退成无墙。
- **为什么不复用 `unsupported`**：
  - paddock 把 `unsupported`（退出码 9）显示成「needs corral upgrade」（paddock `app/src/pause.rs:106`），意思是「旧 pen 不会，升级就好」。
  - canter 不支持暂停是「这一家没有这个能力」，升级也不会好。所以用新码。
  - 名字和 fence 的 `unsupported_kind` 是一个意思（这个种类没有这项能力），但放在顶层，不会和 `sandbox_failed.fence` 里的混在一起。
- **事先就能知道**：`status`、`ls` 里每个 canter agent，以及 `integration status` 的 canter 那一行，都加 `unsupported:["pause","sandbox"]`。别家不出现这个字段，输出逐字不变。
- **canter 自带的沙箱**：JSON 里不报。
  - `sandbox` 字段只指 fence，canter 的照报 `null`。
  - canter 的沙箱是服务端的全局配置（canter 主控的要点；会话设置里的 `sandboxEnabled`【协议 README 281 起】），开没开由 canter 自己显示。
  - 只在技能和 CONTRACT 的说明里分开写一句：「`--sandbox` 是 corral 的 fence，canter 不支持；canter 自己的沙箱照它的配置工作，`--yolo` 只是不弹批准框，命令仍在 canter 的沙箱里跑」。

## 8. `integration status`（第 8 条）

```json
{"kind":"canter","path":"/Users/u/.cargo/bin/canter","version":"canter 0.1.0 · 16fb98c","hooks":"launch",
 "skills":[{"target":"codex","path":"/Users/u/.agents/skills/corral/SKILL.md","status":"same"}],
 "unsupported":["pause","sandbox"]}
```

- **程序**：照现有的找法（`integration.rs:61`）。
- **版本**：取 `canter --version` 的第一行（`integration.rs:72`），实际就是 `canter 0.1.0 · 16fb98c`。
- **接法**：`hooks:"launch"`。
  - 这个值的意思本来就是「corral 启动它时接上，不用另外装」：paddock `app/src/integrations.rs:22` 的注释这么写，第 365 行把它显示成 Built in。canter 正是这样：两个启动参数加一个订阅，不装东西，也不写 canter 的配置。
  - 换成新值（比如 `"watch"`），paddock 现在会原样显示成 Other（同一个文件第 666 行的测试），还得改 paddock 才好看，又不带来任何新操作。
  - 不加说明状态从哪来的字段（同第 3 节）。
- **技能**：`skill_targets` 为 `["codex"]`，也就是 `~/.agents/skills`；项目里的 `.agents/skills` 由 canter 自己读（R56 第 8 条）。和 Codex、pi、Kimi 共用一份，不另装。
- 没有 `install`，因为不是 `global`。
- 登记顺序放在最后（`ADAPTERS` 末尾，`agents/mod.rs:37`）。CONTRACT 里写的登记顺序跟着改。

## 9. pony（第 9 条）

### 现在的做法

R47a、R48g2 的代码在 `crates/pony/src/core/engine.rs` 和 `engine/buttons.rs`：

- 干活时「打断」：按一次 Esc（`engine.rs:972-975`；只有 amp 和 Kimi 的框按两次）。canter 照这样就行。
- 框上「拒绝」：除了 Kimi，都是发提醒时读一次屏幕，按的时候再读一次，两次一样才按（`buttons.rs:123-129`、`268-290`）。canter 框开着时每 2 秒重画一次【实测 11】，两次永远不一样，这个按钮永远按不下去。

### 推荐

1. **canter 走 Kimi 那套结构化核对**：
   - 发按钮和按下去时都要满足：
     - `state:"blocked"`；
     - `interaction.count == 1`，且 `interaction.kind` 不为空；
     - `state_started` 没变；
     - 人没在电脑上动过（`last_human_input` 晚于 `state_started` 就不按）。
   - 不读屏幕，也不比屏幕。
   - 实现上把 `kimi_dialog()`（`crates/pony/src/corral.rs:63`）这类判断扩成「Kimi 或 canter」。
   - 但「按两次」只留给 Kimi（`engine.rs:974`）。canter 按一次：框上按一次 Esc 就取消请求并打断这一轮【实测 9】。
2. **核对打断结果**：canter 被打断一定报 `interrupted:true`【实测 9】，所以照 Kimi 设 `require_interrupted`（`engine.rs:943`）。按下去以后，要等到它 idle 并且 interrupted，才算拒绝或打断成功。
3. **子 agent 的框**（`interaction.kind` 为 `null`，或 `count` 大于 1）：不按，提示回电脑处理，和 Kimi 一样（CONTRACT 第 153 行）。
4. **认标题**（`summary_title`，`engine.rs:2292`）：
   - 开头要去掉的符号里加上 `✓`、`✋`。转圈用的点字（U+2800 段）已经在去掉之列。
   - 去完以后，以 `canter ·` 开头的（不分大小写）当成只有程序名，不显示。这个标题说明不了在干什么，目录名在 corral 名字里也已经有了。
5. **不做**：允许、远程回答、在 Slack 里显示 canter 的题目（R56「这一版不做」）。
   - `is_question`、`displayed_questions`（`engine.rs:2276-2290`）仍然只认 claude 和 Kimi。
   - canter 的「需要你」提醒照旧读屏幕末尾，读到的是某一刻的画面，可以接受。

## 10. 派活表和技能（第 10 条）

### `dispatch.toml`

照 `[agents.kimi]`（第 36–41 行）和 `[cells.kimi]`（第 114–116 行）：

```toml
[agents.canter]
efforts = []
models = []
argv = ["canter", "--yolo"]
labels = []

[cells.canter]
description = "只在用户或任务文件点名 canter 时使用"
agents = ["canter"]
```

- 不加 `env`：canter 没有「这次启动不弹信任框」的开关，`--yolo` 不跳过信任框（R56 第 10 条）。这一点和 Kimi 的 `KIMI_CODE_TRUST_WORKSPACE` 不一样。
- 不分档、不选模型：用 canter 服务端配置里的模型，不加 `-m`。
- 交叉审查：点名 canter 做碰要害的活时，照 Kimi 读 `review-codex`。

### corral-dispatch 技能

文件是 `crates/dispatch/resources/corral-dispatch/SKILL.md`：

- 第 194、195、196、207、246、293 行：家名单「pi、omp、amp、agy、Kimi」都加上 canter。规矩照 Kimi：点名才派，不分档，读同名格子 `canter`，默认标签为空，交叉审查读 `review-codex`。
- 第 256 行的表加一行：`| canter | canter 格；首句带 --prompt，用 canter 服务端配置的模型 |`。
- 第 258–260 行（信任框）加一句：
  > canter 的 `--yolo` 不跳过信任框，也没有免掉它的开关；一直 `starting` 就照「卡在信任框」处理，请用户 attach 点一次。canter 按仓库根记信任，worktree 算进主仓库【canter DESIGN 37】，同一个仓库点过一次，之后的 worktree 不再弹。
- 新加一条：
  > canter 不能套墙。带了 `--sandbox`，或主控设了 child-sandbox 时，`corral start` 会报 `unsupported_kind`。不要去掉墙重试，停下来告诉用户。

### corral 技能

- `crates/corral/resources/SKILL.md` 第 24 行（Kimi 那段）后面加 canter 一段：
  > 要开 canter，把最后换成 `-- canter`，照常带 `--prompt`；免确认派活用 `-- canter --yolo`（Allow all，命令仍在 canter 自带的沙箱里跑）。canter 不支持 `pause`、`resume` 和 `--sandbox`，会报 `unsupported_kind`。卡在信任框时一直是 `starting`。
- 同一文件第 32 行「已实测可识别」那句加上 canter：按一次 Esc，corral 从 canter 的事件认出被打断。
- `crates/corral/resources/AGENT_USAGE.md` 第 98 行（暂停）加一句：canter 不支持暂停，报 1 `unsupported_kind`。

## 11. 给 paddock 的公开约定

下面全是兼容新增，没有不兼容的改动。

1. **开**：`corral start <名字> … -- canter …` 能开 canter，`kind` 为 `"canter"`。首句、`--cwd`、`--env`、`--label` 照旧。
2. **`status`、`ls`**：
   - canter 的 `state` 可能是 `starting`、`working`、`idle`、`blocked`、`exiting`、`unknown`。报 `unknown` 有三种情况：
     - pen 太旧；
     - 部署前的旧 corral 开的 canter；
     - 连不上 canter 服务端超过 20 秒。
   - `interrupted`、`questions`、`command` 照 CONTRACT。
   - 新增 `interaction:{kind,count,running}`：只有 canter 和 Kimi 有，含义同 CONTRACT R48g2（第 152 行）。
   - 新增 `unsupported:["pause","sandbox"]`：只有 canter 有，别家没有这个字段。
   - `sandbox` 对 canter 恒为 `null`，这个字段只指 fence。
3. **`pause`、`resume`**：对 canter 报 `ok:false`、`error:"unsupported_kind"`、带 `name`、退出码 1。
   - 要和 `unsupported`（退出码 9，要 `corral upgrade`）区分开。
   - 建议 paddock 事先按 `unsupported` 不给 canter 显示暂停，「全部暂停」也跳过它。万一调到了，显示成「canter doesn't support pause」之类。
4. **`start --sandbox`**（或主控设了 child-sandbox）开 canter：报 `ok:false`、`error:"unsupported_kind"`、退出码 1，不起 pen。建议新建 canter 时不给选墙，依据是 `integration status` 里的 `unsupported`。
5. **`stop`**：
   - 可能多出 `remaining_sessions:["s-…"]`，只有 canter 会有。规则照 `remaining_pids`：仍然 `ok:true`、退出码 0，没有残留就不出现（这一条看「要用户定的」第 2 条怎么定）。
   - canter 停一次最多约 18 秒，`--timeout` 要留够。
6. **`integration status`**：多一行 canter（第 8 节），`hooks:"launch"`，带 `unsupported`。前端照列表画，不要写死名单（CONTRACT 现有的要求）。
7. **其余命令**：`reply`、`wait`、`send`、`keys`、`read`、`attach` 不变。
8. **新旧版本交替**：
   - 旧 CLI 不认识 canter，照旧报 `unknown`。暂停被新 pen 拒绝时，旧 CLI 原样转出 `unsupported_kind`。
   - 部署前开的 canter 在部署后仍然报 `unknown`，能停；要看状态就重开。
9. **paddock 那边的第 4 条（按能力显示）**用 `unsupported` 做；第 1、2、5 条照上面的字段做。

## 12. 要 canter 改的或要用户定的（对 canter 接口的疑问）

1. **`GET /v1/info` 的字段没写进文档**
   - 现状：协议 README 只写了 `InfoResponse` 和 `features`，没列字段。canter DESIGN 只说核对「协议和服务端版本」【canter DESIGN 543】。canter 主控转述时说字段叫 `version`，实测叫 `serverVersion`【实测 2】，`server.json` 里才叫 `version`。
   - 影响：corral 只判断 `protocolVersion` 和 `features`，版本只写日志，不挡实现。
   - 建议：canter 在 README 里把 `InfoResponse` 的字段列出来，以 `serverVersion` 为准。
2. **子 agent 的请求**
   - 现状：`agent.request.opened`、`agent.request.resolved` 和快照里的 `agentRequests`，只写在 canter DESIGN（P2-178）【canter DESIGN 29】，协议 README 里没有，也没实测。
   - 缺什么：README 级别的字段形状；在终端里对子 agent 的框按 Esc 会怎样。
   - corral 怎么处理：只数个数、算 blocked，不给内容；有子 agent 的框时，pony 不按。
   - 建议：canter 在 README 补上；联调时走一次子 agent 的批准。
3. **`session.status error` 什么时候出现**
   - 现状：canter DESIGN 列了六种状态【canter DESIGN 74】。实测模型出错时，那一轮是 `failed`，会话回到 `idle`【实测 6】，没见过 `error`。
   - 缺什么：`error` 的含义，以及之后还能不能接着用这个会话。
   - 建议：corral 先按 `StopFailure`（idle）报，等 canter 补一句说明再定。
4. **会话归属不公开**
   - 现状：受管模式的归属只放在服务端内存里【canter DESIGN 554】。
   - 影响：corral 因此不调同步 DELETE 兜底（第 5 节）。
   - 建议：现在不需要 canter 改。以后要「停不掉时强关」再提。
5. **框开着时每 2 秒重画**【实测 11】
   - 不要 canter 改，pony 改用结构化核对（第 9 节）。写在这里，是因为任何靠「两次画面一样」来核对的做法都会碰到它。
6. **信任框时没有任何可读的状态**
   - 现状：会话文件要等会话建成才写，所以信任框和刚启动分不出来。canter 主控已经说明两种都报 `starting`。
   - 不要求改，派活技能照「卡在信任框」处理。

## 13. 测试怎么做

1. **翻译（纯函数）**：输入一串 canter 帧，输出一串 v1 事件。
   - 测试数据照【实测 5–9、12、13】的帧手写，不含私人数据。
   - 要覆盖的用例：
     - 正常的一轮；
     - 出错的一轮；
     - 批准后允许；批准时按 Esc；
     - 提问后回答；
     - 干活时按 Esc；
     - 目标续跑（两轮之间没有 idle）；
     - seq 为 0 的帧、重复的帧；
     - `serverShutdown`、`clientRequest` 两种关闭。
   - 再把输出喂给 `events::read`，核对 `status`、`reply`、`wait` 的结果。
2. **线程和 pen（集成测试，不需要装 canter）**：
   - 测试里用 Rust 起一个假的 canter 服务端：临时目录里的 Unix socket，写 `server.json`，回 `/v1/info`、快照和 SSE。
   - 假的 `canter` 程序是一个 shell 脚本，照 `tests/kimi.rs:27` 写名为 `kimi` 的脚本那样做，只写会话文件然后等着。
   - 要覆盖：
     - 会话文件晚出现（`starting`）；
     - `/new` 换 ID；
     - 服务端换 socket 后重连；
     - `corral stop` 确认关闭，以及 `remaining_sessions`；
     - `corral upgrade` 前后不丢、不重；
     - 线程停不下时升级失败；
     - pause、sandbox 报 `unsupported_kind`；
     - 旧 pen 的快照（没有新字段）接着跑，报 `unknown`。
   - 和现有测试一样隔离 `HOME`、`CORRAL_HOME`。
3. **真 canter（可选，默认 `#[ignore]`）**：
   - PATH 上没有 `canter` 就跳过，并打印原因。
   - 隔离 `HOME`、`CANTER_HOME`：每个测试用自己的目录，因为一个目录只能跑一个服务端。
   - `config.json` 设 0600，指向测试里起的假 Chat Completions 服务。这个服务用 Rust 写，按脚本回 SSE，格式见 canter-model 的 README。配置里 `prompt_suggestions:false`。
   - 先用 `POST /v1/folderTrust` 记下信任，只对隔离目录生效。
   - 服务端按 PID 停。
   - 不调真实模型，不碰 `~/.canter`。按「测试不依赖真实 agent」的规矩，这个测试不进默认的 `cargo test`。
4. **pony**：用现有的假 corral（`Corral` trait）喂 canter 的 status，核对按钮出不出、按几次 Esc、标题怎么显示。
5. **联调**（用户在场，canter 的 P2-193）：
   - R56 写定的主路径：开、看状态、等批准、回复、送话、打断、停、换 pen。
   - 另外再加：
     - 多行送话；
     - canter `serve --replace`；
     - 子 agent 的批准；
     - canter 跑的命令里 `CORRAL_NAME` 对不对；
     - replace 时正好有一轮在跑。

## 14. 实现怎么拆

### 主控先做

把第 11 节写进 CONTRACT，把挂点和 canter 的接法写进 DESIGN §7，再交给 paddock/main。

### R56b：corral 本体

碰要害，要交叉审查。

改的文件：
- 新文件 `crates/corral/src/agents/canter.rs`，放：
  - 适配器：`kind`、`hook_args`、`quit_steps`、`skill_targets`、`watcher`；
  - 找服务端；
  - 走 Unix socket 的 HTTP/1.1 GET，要能处理 SSE 和 JSON，chunked 和带 `Content-Length` 的都要支持；
  - 翻译、游标、收尾确认。
- `agents/mod.rs`：加 `watcher()` 默认方法、`Watcher` 接口，登记 canter。
- `pen.rs`：
  - 起停线程、Pen 的新字段；
  - status 回复里带线程的情况；
  - agent 退出后的收尾确认，`exit.json` 里的 `remaining_sessions`；
  - pause、resume 按种类拒绝。
- `pen/upgrade.rs`：
  - handoff 前停线程，失败时重起；
  - probe 报 `watchers`；
  - 目标程序跟不了 canter 时拒绝。
- `events.rs`：`questions`、`command` 改成按字段留；存 `interaction`。
- `cli.rs`：
  - `status`：`questions`、`command` 的门槛，`interaction`，`unknown`，`unsupported`；
  - `start`：拦 sandbox，`--session-file` 冲突报 `usage`；
  - pause、resume 先按种类拒绝；
  - `stop` 转出 `remaining_sessions`；
  - `ls` 带 `unsupported`。
- `state.rs`（`FILES`）、`integration.rs`（`unsupported`）。
- 新测试 `tests/canter.rs`。

依赖：
- 推荐不加。手写最小的 HTTP 客户端，只需要三种 GET 和一种 SSE。
- pony、dispatch 已经锁定了 `ureq =3.4.2`，但要接 Unix socket 得自己写 transport，省不了多少。

验证：
- `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`；
- 第 13 节的 1、2，3 在本机手动跑一次。

### R56c：pony、派活表、技能

中档。和 R56b 文件不重叠，可以并行。pony 的远程按键核对，建议也交叉审查。

改的文件：
- `crates/pony/src/corral.rs`：canter 也算结构化框。
- `crates/pony/src/core/engine.rs`：`stop_reason`、`require_interrupted`、`summary_title`。
- `crates/pony/src/core/engine/buttons.rs`：canter 不读屏幕。
- `crates/dispatch/resources/dispatch.toml`。
- `crates/dispatch/resources/corral-dispatch/SKILL.md`。
- `crates/corral/resources/SKILL.md`、`AGENT_USAGE.md`。

pony 照第 11 节的约定写，用假 corral 测；真跑要等 R56b 部署以后。

验证：pony、dispatch 的测试，加上上面三项检查。

### 联调

R56b、R56c 合并、部署以后，用户在场，和 canter/main、paddock/main 一起走主路径（R56 拆法第 4 步）。

## 要用户定的

### 1. corral 要不要替用户记 canter 的目录信任

canter 主控说这一点必须由用户定。

- **A（推荐）不记**。派到一个新仓库，第一次会卡在信任框上。照现有的「卡在信任框」处理，请用户 attach 点一次。canter 按仓库根记信任，worktree 算进主仓库【canter DESIGN 37】，所以一个仓库只要点一次。
- **B**：派 canter 之前，由 corral 或派活技能调 `POST /v1/folderTrust {cwd}` 记下信任。
  - 好处：不会再卡住。
  - 代价：这是 corral 去改用户 canter 里的长期记录，信任一旦记下，用户自己开的 canter 也会受影响；而且要连用户的 canter 服务端去写东西。
- **C**：只有 `corral start` 明确带一个参数（比如 `--trust`）时才记。派活技能默认不带。

### 2. canter 的会话没确认关掉时，`corral stop` 怎么报

- **A（推荐）照 Kimi 的 `remaining_pids`**：`ok:true`、退出码 0，多带 `remaining_sessions`。
  - 理由：终端这时已经停了，再调一次 stop 也没用。调用方（paddock、主控）看到这个字段，就提示「canter 上还有会话没关」。
  - 实际出现的情况只有两种：服务端连不上，或者关会话超过 12 秒。
- **B 报失败**：`ok:false`、新的错误码、退出码非 0。
  - 好处：更显眼。
  - 代价：agent 其实已经没了，`corral ls` 里看不到它，调用方没法重试；也和 Kimi 的规矩不一致。

## 用户 11-03 定的

- 第 1 条：A，不记信任（用户选「不记，卡住时我去点」）。
- 第 2 条：A，`ok:true` 加 `remaining_sessions`（用户选「算成功并列出没关的会话」）。
- 另一条（主控审查时补问）：paddock 需求单第 5 条要 Integrations 页的 canter 一行「状态来源说明照 corral 报的内容显示」，和用户 10-14「不加 `state_source`」冲突。用户选「不加字段，显示 Built in」：canter 一行照第 8 节报 `hooks:"launch"`，不加状态来源字段。
