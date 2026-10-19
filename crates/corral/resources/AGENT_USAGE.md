# corral 使用说明（给 agent 看）

corral 让你开启别的交互式编程 agent（Claude Code、Codex），给它送话、等它答完、取回复。每个 agent 有一个名字（如 `demo/ask-3`），用名字操作它。人随时可以在自己的终端里 `corral attach <名字>` 进去看、插话。

所有命令输出一行 JSON，看 `ok` 和退出码。完整契约见 corral 仓库的 docs/CONTRACT.md。**不要读 corral 状态目录里的任何文件。**

**你自己也必须是用 corral 启动的。** 环境变量 `CORRAL_NAME` 就是你自己的名字；它为空时不要委派，告诉人先用 `corral start` 启动你、再 `corral attach` 进来。

## 临时委派：开一个 agent，问一个问题

```sh
corral start demo/ask --unique --cwd <仓库目录> --env CAIRN_DISABLE=1 --prompt "<你的问题>" -- claude
corral wait <名字>
corral reply <名字>
```

- `start` 输出里的 `name` 就是之后要用的名字（`--unique` 会补后缀，如 `demo/ask-3`）。
- 第一句话**必须**用 `--prompt` 带上；新开的 agent 不能用 `send` 送第一句。amp 例外，见下。
- 换成 Codex：`-- codex --yolo --disable memories`（在 Codex 沙箱里 corral 会直接拒绝，见下；`--disable memories` 关掉这个会话的 Codex 记忆，它是跨项目的，会把别的项目、旧约定带进回答，人明确要它带记忆时才去掉）。换成 pi：`-- pi`。换成 omp：`-- omp --approval-mode yolo`。
- 换成 amp：`-- amp`，不能带 `--prompt`（会退回 1）：start 完先 `corral wait <名字> --timeout 60` 等到 `idle`，再 `corral send <名字> "<你的问题>"` 送第一句，然后 wait。amp 靠装在它配置目录里的 corral 插件报状态；一直是 `starting`，可能是插件没装、没加载成功，或者卡在对话框上，告诉人用 `corral integration status` 查（没装就 `corral integration install --agent amp`，装完要重开这个 amp），不要自己装。
- 换成 agy：`-- agy --dangerously-skip-permissions`，同样**必须**带 `--prompt`（agy 收到第一句话之前什么都不报，不带就一直是 `starting`）。agy 也靠装在它配置目录里的 corral 插件报状态；带了 `--prompt` 还一直是 `starting`，可能是插件没装、被停用，或者卡在「是否信任这个目录」的对话框上，告诉人用 `corral integration status` 查（没装就 `corral integration install --agent agy`，停用了就 `agy plugin enable corral`，之后要重开这个 agy；对话框要人 `corral attach <名字>` 去点），不要自己装。
- `--env CAIRN_DISABLE=1` 让临时委派的 agent 不用 cairn（工作接续记忆）：它开在同一个目录，会把它的停点存到你这条工作线上。没装 cairn 时这个变量不起作用，照写即可，不要删。人要开一个以后自己长期对话的 agent，直接 `corral start <名字> --cwd <目录> -- claude`，不带这个变量。
- `reply` 的 `text` 是回复原文，多段文字和代码块原样保留。最近一轮被打断时退回 1 `no_reply`，带 `interrupted: true`，不返回上一轮的回复。

追问、用完：

```sh
corral send <名字> "<追问>"
corral wait <名字>
corral reply <名字>
corral stop <名字>
```

对方不会自己退出，你也**不要自己关**：人可能还要接着让它做。把结果告诉人时说明它还开着、名字是什么；人说关掉时才 `stop`。`corral ls` 看还开着哪些。

**一个 agent 只走一条通道**：用 corral 开的 agent 仍然是一个普通的 agent 会话，别的渠道（会话之间的消息、子 agent 工具）也能碰到它，但 corral 看不见那些输入。送话、等待、停止都只用 corral，不要混用；否则状态和「最近一次输入的来源」会失真，`wait` 和 `reply` 会对不上。

## 长任务：交出去不等，做完被提醒

要跑很久，或者人说「做完告诉我」，就不要在前台 `wait`：

```sh
corral start demo/ask --unique --cwd <仓库目录> --env CAIRN_DISABLE=1 --prompt "<你的问题>" -- codex --yolo --disable memories
corral send "$CORRAL_NAME" "<提醒的话>" --after <名字> --timeout 3600
```

- 交给对方的话末尾加上「命令都在前台跑完，全部做完后，回复最后一行写 DONE」。
- 提醒的话写成「<名字> 这一轮结束了，去看它的状态和回复」。`send --after` 立即返回；告诉人已经交出去，然后结束这一轮。
- 对方这一轮结束（或卡在对话框、退出、等满 `--timeout`）时，这句话会像人打字一样送进你的输入框；你正在干活、人刚在你的窗口里操作过时，它会等一会儿再送。
- 已识别为被打断的这一轮不触发提醒，原提醒继续等下一轮正常结束；等满 `--timeout` 仍照旧送，所以收到提醒不代表做完。
- 收到后先 `corral status <名字>`：`interrupted: true` 表示被打断，告诉人，不当成做完；否则再看 `corral reply <名字>`。reply 报 `no_reply` 且带 `interrupted: true` 时也按打断处理。回复最后一行是 DONE，取结果告诉人，不要 stop；对方又在干活，是被叫早了，再挂一次 `send --after`；没有 DONE 但对方已经停了，告诉人去 attach 看。
- corral 只知道「这一轮结束」，不知道「做完」，所以一定要靠 DONE 判断。
- 让已经开着的对方接着做长活：用 `send` 把新任务交过去（末尾同样带 DONE 约定），再挂 `send --after`，不要重新 start。
- 新提醒返回 `request_id`。用 `corral after <接收者名字> --request-id <ID>` 查 waiting/sending/sent/confirmed/not_delivered/unknown/expired；pending 不是送达。unknown 表示不能证实原请求，不能重发或换 ID 再送。

## 等待时要知道的

- `wait` 返回的 `result`：`idle` 这一轮结束；`interrupted` 这一轮被打断，不能当成答完；`blocked` 弹了权限框或提问框，要人处理（告诉人去 `corral attach <名字>`）；`stopped-quiet` 只有加了 `--quiet 秒` 才会出现，表示 agent 在 working 状态下长时间没动静，可能被打断，但不能据此确定。
- `status.interrupted` 是布尔值：已识别最近这一轮被打断为 `true`，状态仍是 `idle`；新输入开始下一轮就回到 `false`。没有可靠中断事件的 agent 不能据此排除打断，仍要核实回复。
- 已实测可识别：Codex、amp、加载新采集器的 pi（旧 pi 需要重开；amp 要连续两次 Esc）。Claude Code 从对话记录末尾识别晚于最近一次输入的明确打断标记；调用方和提醒进程需用新版，且该 CORRAL_HOME 的 `.runtime/helper` 要经新版 `corral upgrade` 等方式切到能记录路径的版本；单纯切换 CLI 或重开不够，记录读不到或格式不认识时仍认不出。omp 在一轮最终 `agent_end` 的最后一条 assistant 消息同时带 `stopReason: "aborted"`、`errorMessage: "Interrupted by user"`、`errorId` 的 UserInterrupt 位（67108864）时识别为打断；调用方和提醒进程用新版即可，采集器不用换、omp 不用重开。只取消命令后模型继续正常结束不算打断，其他中止和 `willContinue` 照旧。agy 未报中断钩子的路径仍有识别限制，不能仅凭 `interrupted: false` 或旧回复判断完成。
- `idle` 只表示这一轮结束，agent 之后可能自己再开一轮。拿到 `reply` 后如果 `last_input_source` 是 `agent`，说明这一轮是它自己开的。
- `wait` 默认最多等 600 秒，可用 `--timeout` 改；超时退出码 4。你的 shell 工具如果有超时（常见是两分钟），就用 `--timeout 90` 反复调用，直到返回结果。状态一直是 `starting`，说明它启动时卡在对话框里，让人接入去看。

## send 被拒绝时

| 退出码 | 原因 | 怎么办 |
|---|---|---|
| 7 `not_idle` | agent 还在忙或还没启动好 | 先 `wait` 再 `send` |
| 8 `human_active` | 人刚在接入窗口里操作过 | 过一会儿再试；确定要打断人才加 `--force` |
| 3 `not_delivered` | 送了但 agent 没收到（屏幕上可能是菜单或对话框） | 不要盲目重试回车；`corral status` 看状态，必要时让人接入去看 |
| 10 `paused` | agent 被人暂停了 | 不要自己 `resume`，告诉人；人恢复它之后再送 |

送成功时如果 `merged_with_draft` 是 `true`，说明输入框里原来有人留下没提交的文字，你的话接在它后面一起提交了；agent 收到了，**不要重送**，在意的话让人接入去看。

## 不要按名字批量杀进程

corral 启动的每个 agent，**它的栏位进程和 agent 进程的命令行里都带着名字和工作目录**。`pkill -f <项目名>`、`pgrep -f <路径> | xargs kill` 这类命令会一次打中所有沾边的 agent：开你的那个（它的名字往往最短最通用，最容易被匹配上）、和你并行干活的、以及你自己。栏位收到 SIGTERM 就会连同 agent 一起退出。

要停自己起的服务：起的时候记下 PID（`cmd & echo $!`），停的时候 kill 那个 PID；或者固定端口，用 `lsof -ti:<端口> | xargs kill`。绝不按项目名或路径匹配。

## 其他命令

```sh
corral status <名字>
corral keys <名字> esc
corral ls
corral where <名字>
corral attach --wait <名字>
corral pause <名字>
corral resume <名字>
```

- `keys` 送原始按键（`enter` `esc` `up` `down` `ctrl-c`，或 `text:<文字>`），不看状态，后果自负；`esc` 可以打断正在干活的 agent。按完键 agent 要过一会儿才反应，紧接着 `send` 可能因为状态还没变被退回 7，先 `status` 或 `wait` 再送。`keys` 不算人在打字，不会触发 8。
- `read` 给的是**累积的输出**（最近若干字节的滚动缓冲），不是当前画面：启动横幅、早已过去的提示都还在里面，拿整段做子串匹配会被历史内容骗到。要判断「现在屏幕上是什么」只看尾部（最后几百字符），并且只作排查，不要靠它做决定。
- `pause` 把 agent 连同它的子孙进程一起冻住（`SIGSTOP`），不占 CPU、不联网；`resume` 从原地接着跑（`SIGCONT`）。`status`、`ls` 里 `paused` 为 `true` 表示暂停中，原来的 `state` 不变。暂停中 `send`、`keys` 退回 10，接入窗口里打的字被丢掉，`wait` 和 `send --after` 一直等到它恢复（或超时），`stop` 先恢复再停。暂停和恢复由人决定：别人的 agent 不要替人 `pause` 或 `resume`。运行中的旧版 corral 管着的 agent 返回 9 `unsupported`，要先 `corral upgrade`。
- 退出码 2：不存在或已退出；6：你在 Codex 沙箱里，corral 用不了，需要调用方不带沙箱启动；9：版本不兼容，报告给用户，不自行重启。

## 通用升级

只有获得升级授权后，才运行 `corral upgrade --all` 或 `corral upgrade <名字> --exe <新版本绝对路径>`。`status` 公开实际 exe、custody、capabilities 和 upgrade 的 epoch/attempt/target。accepted、pending、hold、unknown 都不是 complete；批量逐项核对 pen 与提醒，不因 CLI 已换版本而推定会话都已升级。丢回执先查询，不自动重发。

Hold 用 `corral recover <名字> --exe <兼容版本绝对路径>` 恢复，在同一 epoch 递增 attempt。旧 pen 无能力返回 needs_restart，但不会自动停止、重启或 resume。旧固定 hook 和无记录 after 的首次过渡未解决；升级不承诺任意崩溃无损。详细边界见包内 UPGRADING.md。
