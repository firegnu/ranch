# pony

ranch 的独立聊天机器人包，不依赖 corral、dispatch 或 fence 的 Rust 包。
公开输出见 `docs/CONTRACT.md` 的 `pony status`，设计见 `docs/DESIGN.md` §10。

R38d 在 R38c 的命令外壳上接入共用核心，R38e 接入 Slack。`pony run` 前台运行；
配置、凭证、状态、日志都在 `PONY_HOME`（默认 `~/.pony`）。没有配置时
`status` 报 `missing`，`run` 记一行日志后退出 0。

已配好 pony 的机器上，再绑一个仓库只需：

1. 建私有频道。
2. `/invite @pony`。
3. 配置里的 owner 发 `@pony 绑 <仓库名>`；pony 在线程里回「已绑 <仓库>」及主控状态，主控还没开也能绑。

只在未绑频道里认「绑」加一个仓库名；其他 @ 会提示用法，已绑频道里这句话照常送给主控。
公开频道或查不到是否私有均拒绝；名字不合规则、仓库已绑别的频道、配置有错或刚被改过也不写。
重复绑定提示原频道名，查不到才用 ID；写盘失败只报告错误码。
`core/config.rs` 用 `toml_edit` 向现有 `channels` 追加，支持行内数组和数组表，保留原注释和顺序；
末尾没有换行时允许补一个换行，磁盘冲突检查仍比较原始内容。
其他无法原样保留的 TOML 单独说明格式问题并请用户手动添加，原文件不动。
配置是符号链接、硬链接或对当前用户只读时拒绝自动绑定，不替换链接或顺着链接写目标。
候选内容经过原有配置校验后，写入同目录临时文件、保留原权限并同步，改名前再次核对
磁盘原文、链接和写权限。重读配置或校验凭证失败时禁用绑定，提示先修配置，重读成功再恢复。
所有绑定拒绝记一条 `bind_failed <code>`，错误码经 `safe_code` 过滤，不记配置、路径或原始错误内容。
写完交给现有每 2 秒重读生效，不直接改内存路由；这一家的重连和离线补收照旧。
配置及 `pony status` 格式不变，旧版 pony 能读新增绑定，paddock 不用动。

- `corral.rs` 只调用公开命令。按配置、PATH、`~/.local/bin/corral` 找程序，
  启动检查 `--version` 的 contract 1；只用 ls、status、reply、send。
  发送参数是 `send NAME -- 正文`，不加 force，也不读 corral 的状态文件。
- `core/engine.rs` 统一处理 owner、频道绑定、路由、线程、队列、每轮回复、
  主控新回复和 Needs you 提醒、`?`、离线补收；每 2 秒轮询，空闲须连续
  两轮稳定。未接入仓库不调用 status，也不输出其存在与状态。
- `core/state.rs` 把线程、待回复、队列原文、消息去重、补收游标和接收箱
  写入 0600 的 `state.json`：同目录临时文件同步后改名，再同步目录。
  只有拿运行锁的进程创建 Store。适配器工作线程调用 Events::emit，成功
  后才回执；落盘不依赖核心是否正在等 corral 或发消息。别人的消息在
  落盘前换成不带正文、用户 ID 的 Ignored 事件。
  state 和 status 内容没变时不改写文件；在线时间每分钟保存一次，给无游标
  频道的补收留下最多一分钟重叠。消息 ID 通常留 24 小时；频道还有待补收记录时，
  晚于其下界的 ID 一直保留到补收完成，避免长时间离线后把已处理消息误报成未送。
- 发送前持久化 Sending。重启遇到它时不重送，在线程回
  「上次发送结果不明，没重送，请确认后重发」，并记录 send_uncertain。
  这是主控决定的崩溃恢复边界；其余队列按顺序恢复。
- 每次连上读取已记录游标之后的频道消息，包含新旧线程里的回复，按时间顺序去重、
  路由、排队；补收正文带发送时间和离线说明。超过 12 小时提示重发，
  首次连上不补收；之后没有游标的频道以上次在线时间为界。收件箱已有的
  在线消息不走补收路径。每次连上核对排队原消息是否还在，已删的撤回；
  核对报错的只暂缓该条并重试。线程 30 天未用后清除 agent 对应及线程游标，
  过期线程 ID 再留 90 天，以便用户再回复时得到明确提示。
- 补收失败时按频道保存原下界、首次尝试时间、错误码和退避计时，重启也保留。
  按 2、4、8… 秒重读，间隔最多 5 分钟；每次连上立即再试一次。
  读成前冻结该频道游标及本平台的在线时间；读成并处理完后才清掉待补收项。
  本次读取失败后才判断是否已超过 12 小时；到期时清掉这段待补收记录，
  在频道说明错误码并请用户重发。读成时照消息年龄处理，不因待补收记录到期而丢掉消息。
  exists 失败按 2、4、8… 秒退避，间隔最多 5 分钟；每次连上重置退避和 10 分钟
  期限，立即核对一次。超过 10 分钟还核对不了，在线程回「没法核对这句还在不在，
  先不送；要送请重发」并出队。计时写入状态文件，重启后的连上事件会重新计时。
  两轮相隔超过 60 秒时，将整个空档从补收和核对期限中扣除；即使醒来时还没收到
  断线事件，也保留待重试项。消息本身的 12 小时年龄限制不变。
- `core/split.rs` 保留原文字节，按空行、行尾、代码行尾、字符边界分段，
  尽量不拆表格；代码围栏和段号另存。投递因格式或长度被拒时，仅对
  失败段对半再切，最小到 1,000 字；重切段增加自己的 `(i/N)`，已经
  成功的段不重发。回复／提醒的完成和去重标记在发送前保存；最终失败
  在线程和日志报告，不改 status，连接状态只由适配器报告。
- corral 路由错误回原线程后出队；盯 agent 或补收频道出错只跳过该项并记
  错误码，失败补收按上面的规则重试。`?` 将状态报错的 agent 列为「读不出」，
  其余照列。发送确认失败只拦住对应队列，其他频道继续。旧回复时间早于本轮
  开始时回「这一轮结束了，没有新回复」。带附件只送文字，并在线程说明。
- `run` 的 POSIX 记录锁、配置重读、凭证权限检查和日志轮转沿用 R38c。
  不在持锁进程调用 `lock::holder` 或 `status::read`。日志只记事件、
  路由元数据和错误码，不记消息正文、回复正文或凭证。配置删掉正常退出。
  Slack 的「忽略了一条别人的消息」只记绑定频道里非 owner 真人发的新消息；
  自己的回声、其他机器人、编辑等不处理的事件不记日志。

## 适配器接口（给 R38e）

`Chat` 静态登记在 `CHATS`。已有的 kind、凭证键、配置解析不变；新增
`connect(Connect) -> Box<dyn Connection>`。每个连接对象负责自己的网络
工作线程，close 和 Drop 时停止并等待它们退出。同步的发送和历史接口
可以在内部使用独立工作线程和限频队列。

`Connect` 提供本家的配置、proxy、凭证读取回调和 Events：

- 每次连接／重连前调用 credentials，重新检查文件权限并读取凭证。
  网络与认证失败用 Connection 事件报告并由适配器退避重试；启动连接
  本身不应等待网络成功。
- 先 emit Connection（connecting / connected / failed，以及公开的原因、
  workspace、bot、其他连接数），再交付该连接收到的消息。不要传 token、
  ticket URL、原始 HTTP 错误或原始 payload 作为公开说明。
- Message 提供 channel、id、thread、sender、mentioned、解码后的 Markdown
  text、attachments、Unix 秒 at。只交付新消息，不交付机器人消息和编辑。
  Deleted 提供原消息的 channel、id；emit 成功才 ACK。核心去重与排队。
  核心在路由确认后说「附件没送」，只有附件时说「附件没送，只收文字」。
- Connection::name 是用户提示中的平台名称；limits 给字数上限和线程能力。
  Slack 填 Slack、10,000、有线程；无线程平台要求每句 @ 并点名 agent。
- send(Outgoing) 接收频道、可选线程、Markdown、notify，返回实际消息 ID。
  notify=true 让适配器提醒 owner；平台转义、屏蔽正文中的意外 @、通知
  文案、关闭链接预览、限频与网络重试由适配器处理。SendError 的 resize
  只用于确定未发送且可以缩小重试的长度／格式错误；code 必须是公开错误码。
- channel(id) 返回名字、是否私有、problem（无问题为 None；否则 not_member、
  not_found、archived）。
- history(channel, &HistoryAfter) 读完所有分页，返回这个频道里比游标新的
  Message，**包括新旧线程的回复**。HistoryAfter.id 是可选的不透明消息 ID，
  at 是 Unix 秒下界（不含）；无 ID 时按 at 读取。首次无历史起点不调用，
  后续无频道游标时用上次在线时间。核心不逐线程调用 history；Slack 侧负责
  找到有新回复的父消息，例如利用 latest_reply，避免逐一读取所有已知线程，
  父消息向前找 30 天（和线程过期一致），覆盖这个范围内旧父消息下的新回复。
  核心再检查 owner、时间、绑定及消息 ID，
  合并排序后路由，跳过收件箱里的在线消息。读取失败按 2、4、8… 秒退避，最多
  5 分钟，计时跨重启保留；新的连上事件立即重试。本次读取失败后才判断待补收
  记录是否已超过 12 小时，读成时仍按每条消息的 12 小时窗口处理。
- exists(channel, thread, id) 返回原消息是否仍存在；顶层消息 thread=None，
  回复则传父消息 ID。仅确定删除才返回 false；网络、无权限或无法核实时
  返回错误。每次连接后核心检查 Queued 原消息，false 按撤回处理；错误只
  暂缓这条，按 2、4、8… 秒退避（最多 5 分钟）。每次连上重置退避和 10 分钟
  期限，先核对一次；超过 10 分钟仍核对不了才提示后出队，不阻塞其他 agent。
- Connection 操作返回的错误一律非致命，包括 channel／history／exists 的 io、
  state_invalid、secrets_too_open 等代码；核心按错误来源区分，不因同名代码退出。
  核心自身的状态文件读写、凭证检查错误仍按原规则处理。公开错误不得含凭证。

## Slack（R38e）

- 用 ureq 3.4.2 和 tungstenite 0.30.0。HTTP 与 WebSocket 的传输共用 ureq 的
  rustls 验证及 HTTP(S) CONNECT 代理支持；不启用 native-tls，不支持 SOCKS。
  `proxy` 必须写在配置里，不读取环境代理或系统代理设置。
- 正式 Web API 地址固定为 `https://slack.com/api/`，禁止重定向；Socket 地址
  只接受 Slack 返回的 `wss://*.slack.com`。没有环境变量或配置可以替换 API 地址；
  本机 HTTP/WS 测试入口只编入单元测试。凭证仍只经核心的 credentials 回调从
  `PONY_HOME/secrets/slack.toml` 读取，每次重连重查权限、重读两个 token。
- 独立 Socket 线程取地址、认证、连接、收事件并回执；`hello` 才报 connected，
  每条连接只报一次。需要交给核心的事件先 emit 落盘，成功才 ACK；失败关闭连接，
  不回执。自己的回声、其他机器人、编辑等不处理的事件，以及未绑定频道里的
  非 owner 消息，确认无需落盘后直接 ACK，不 emit，也不记日志。
  30 秒 ping、90 秒没收到数据就重连。重连按 1、2、4…60 秒退避，连接稳定保持
  90 秒后才重置；快速连接又断开不会重置。close/Drop 唤醒退避并等待工作线程退出。
  连接状态落盘失败也照常退避重试，不让 Socket 线程悄悄退出；标准错误只记固定代码，
  写标准错误失败不影响收发和重试。
- 普通消息、附件消息、删除事件翻译成共用事件；线程回复勾了「同时发到频道」的
  `thread_broadcast` 按普通线程回复处理，补收时按 ID 去重。忽略机器人和编辑等
  其他子类型；链接展开带来的 attachments 不算附件，真正的附件仍然提示未送。
  在线接收和补收都识别并去掉正文开头的机器人用户 ID 或 bot ID 的 @，
  两个 ID 每次连接时从 `auth.test` 获取，不写死。
  「分享消息」带 `is_share: true` 的 attachment 即使带原消息链接也算附件，
  不读取其中的转发内容，沿用核心的「附件没送」提示；只贴链接的预览仍不算附件。
  发消息用 markdown 块，另给一行通知文字；notify 的第一块用 mrkdwn @ owner，
  正文中的 `&`、`<`、`>` 转义，避免回复内容意外 @ 其他人或全频道；关闭链接预览。
  长度／块格式错误（含 `msg_blocks_too_long`）才要求核心缩小重试。发消息只有确定
  未发送的解析／建连／CONNECT 失败才隔 1、2、4 秒重试，等待能被 close 唤醒；
  解析、连接分别设 3、5 秒超时，单次请求最多 10 秒。已开始发送或读取响应时的
  I/O 错误、响应阶段超时不重发，避免重复消息。
- **主控决定：限频期间核心暂停，只是延迟不丢。** 同频道发送至少间隔一秒；
  HTTP 429 按 `Retry-After` 等完再重发同一请求，不当作最终发送失败交给核心。
  等待期间 Socket 线程照常收事件、落盘、ACK、收发心跳；核心送话、查回复、状态
  快照更新会延后。若后来出现独立的网络错误，仍按原有失败规则处理。
  标准错误每次写一行 `rate_limited wait_seconds=N`，不带 token、ticket、头或正文；
  R38f 的 launchd 服务应收集标准错误。量大时再另做异步投递，不改本轮接口。
- 频道信息映射为 not_member、not_found、archived；没有 is_member 时保留发送报的
  not_in_channel。**私有频道显示 `not_found` 多半是还没邀请机器人**，先核对频道 ID
  并 `/invite @pony`；Slack 无法让适配器区分不存在和不可见，映射保持不变，R38f
  实测实际错误。历史读完全部游标分页，父消息往回找 30 天，只读 latest_reply
  晚于下界的线程，按消息时间合并去重。exists 完整查询精确时间范围；无权限、
  网络错、分页不完整都报错，不误判删除。

对 paddock：`pony status` 的 `chats[]` 开始报告真实连接状态、工作区名、机器人名、
连接数和频道名，JSON 格式不变；corral 的公开约定不变，paddock 不用动。
R38f 起 `service` 在 macOS 按自有 plist 是否存在报 `installed`／`not_installed`，
其他平台为 `unsupported`，格式不变，paddock 不用动。`installed` 不等于已经
加载或正在运行；是否有 pony 持锁仍看 `running`。status 不调用 launchctl。

测试只用内存适配器、内存 corral、Rust 假命令及本机 HTTP/WebSocket 假服务器。
CLI 测试配置本机拒绝 CONNECT 的假代理，正式二进制也不会访问外网；HOME、
PONY_HOME、CORRAL_HOME 全部隔离，不依赖用户状态和真实 agent。

## 常驻服务（R38f）

`pony service install|uninstall [--dry-run] [--yes]` 只支持 macOS 用户级 launchd。
不带 `--yes` 或带 `--dry-run` 都只输出计划，不创建目录、不写文件、不调用
launchctl、不等交互确认；两个参数一起给时仍以 `--dry-run` 为准。
实际执行前拒绝 root，并用 `getpwuid_r(geteuid())` 核对 HOME 是当前用户的
账户主目录（按目录身份比较，允许指向同一目录的链接）；不符合时报
`unsafe_context`，不写文件、不调用 launchctl。预览仍允许临时 HOME。

- install 原子写 `~/Library/LaunchAgents/local.ranch.pony.plist`，执行
  `/bin/launchctl bootstrap gui/<uid> <plist>`。plist 的程序是
  `~/.local/bin/pony run` 的绝对路径，登录时启动，非零退出后重起。
  `ThrottleInterval` 限制两次启动至少相隔 10 秒：运行很久才崩时可以马上重起，
  一起来就退出（如锁冲突）才需要等到这个间隔届满。路径中的 XML 特殊字符会转义。
- 只认带独立一行 `<!-- PONY_SERVICE = 1 -->` 标记的普通文件。
  已有外来文件、plist 是链接，或 `Library`／`LaunchAgents` 是链接时，报
  `foreign`，不调用 launchctl，也不写删它；HOME 本身可以是链接。
  新文件用同目录临时文件同步后硬链接，不覆盖期间出现的文件；更新自有文件
  用同步后 rename。写删前重查内容，bootout 后再查一次。
- 重装相同内容（`same`）不改写 plist、不主动重启；只有这个状态下，bootstrap
  报错而 `/bin/launchctl print gui/<uid>/local.ranch.pony` 成功才按「已加载」处理。
  内容不同（`replace`）先准备日志，再 bootout，确认卸下后再原子写入、bootstrap，让新定义
  真正生效；预览也会列出这一步。卸载失败时旧文件不变，可以重试。
- 新建（`create`）时 bootstrap 失败而 print 成功，报 `already_loaded`：已有
  别处加载的同名定义，不自动卸它；复查后回收本次新建的 plist，避免重试时
  错当作 `same`。先手动 bootout 或注销再装。其他加载失败报 `launchctl_failed`，
  保留 plist；若此前已卸下旧服务，错误说明会明确指出，并提示排除原因后重试。
- uninstall 先 `/bin/launchctl bootout gui/<uid>/local.ranch.pony`，确认卸下后
  删自有 plist；已卸载（ESRCH）也可删。返回 0 或 36（EINPROGRESS）时，立即查 print，
  若还在则按一秒间隔重查，直到返回 113（找不到服务），最多等 30 秒；超时报 `stop_pending`，
  留着原文件、提示稍后重跑。`replace` 共用这个等待过程。其他 bootout 错误
  保留文件并报错；卸下后若删写文件失败，说明服务已卸下以及需要重试的命令。
  plist 已不存在时直接成功。卸载保留 `~/.pony` 的配置、凭证、状态和日志，
  也不删共享的 Library／LaunchAgents 目录。
- 服务固定使用上述 HOME 下的默认路径，不把调用者的 `PONY_HOME`、代理或
  token 写入 plist。`PONY_HOME` 仍可用于前台 run/status 的隔离数据；要查看
  默认服务的数据，调用 status 时不要设另一个 `PONY_HOME`。
- install 在 bootout／bootstrap 前准备 `~/.pony` 和 `~/.pony/logs`（0700），
  及 `launchd.log`（0600）。目录和日志必须属于当前用户且不是链接，目录权限不符仍报错。
  已有日志不截断；当前用户自己的普通日志文件若权限比 0600 宽（如 launchd 重建的 0644），
  通过已打开的文件收紧到 0600，不放宽权限。准备失败时不停止旧服务。
  正式程序只执行固定的 `/bin/launchctl`，
  不能通过 PATH、配置或环境变量换成别的程序；测试替身仅由内部单元测试传入。

所有结果是一行 JSON。成功退出 0；操作失败退出 1，输出
`{"ok":false,"error":"foreign","message":"…"}` 这类固定错误码和说明；
参数错误为 `usage`、退出 2。其他错误码为 `unsupported`（非 macOS）、
`invalid_data`（HOME 或路径不可用）、`changed`（文件中途变化）、
`unsafe_context`（root、HOME 不符或无法核对账户主目录）、
`unsafe_path`（日志目录或文件权限不合要求）、`already_loaded`（新建时同名服务已加载）、
`stop_pending`（仍在停止）、`io`、`launchctl_failed`。launchctl 退出码按数字显示。
不透传 launchctl 的输出，也不读取配置或凭证。

成功 JSON 的字段（预览和实际操作相同）：

| 字段 | 含义 |
| --- | --- |
| `ok` | `true`。 |
| `action` | `install` 或 `uninstall`。 |
| `dry_run` | 本次是否只预览，包括没带 `--yes` 的情况。 |
| `applied` | 已执行为 `true`（包括重复装卸）；预览为 `false`。 |
| `path` | plist 的绝对路径。 |
| `status` | 执行前的计划：`create`、`same`、`replace`、`remove`、`absent`。 |
| `content` | install 将写入的完整 plist 字符串；uninstall 为 `null`。 |
| `directories` | install 需要的目录：`path`、`mode`、`operation: "create_if_missing"`；已有目录不改权限，新建权限还受 umask 限制。uninstall 为空数组。 |
| `log` | 日志的 `path`、`mode`、`operation: "create_if_missing"`；`restrict_if_wider: true` 表示已有自有普通文件权限过宽时收紧到 0600。uninstall 为 `null`。 |
| `commands` | 按顺序列出可能执行的命令：`argv` 是参数数组（含程序绝对路径），`when` 为 `apply`、`bootstrap_failed` 或 `bootout_in_progress`；后者用于 bootout 返回 0 或 36 后确认卸下，有 `interval_seconds: 1`、`timeout_seconds: 30`。缺失 plist 的卸载为空数组。 |

## 部署与退回

以下步骤留到合并后、用户在场时执行。开发和自动测试不安装真实服务、不连 Slack。

1. 按设计稿 `docs/调研/R38a-Slack机器人设计.md` §3.3 的 manifest 建 Slack
   应用，启用 Socket Mode，只订 `message.channels`、`message.groups`，不订
   `app_mention`。bot 权限是 `chat:write`、`channels:history`、`groups:history`、
   `channels:read`、`groups:read`。安装到工作区得到 bot token；在 Basic Information
   新建带 `connections:write` 的 app token。
2. 两枚凭证只在本机编辑 `~/.pony/secrets/slack.toml` 的 `app_token`、`bot_token`，
   不粘进命令参数、任务、仓库或日志。`~/.pony`、`secrets/` 由当前用户所有且
   为 0700，凭证是 0600 的普通文件，不用符号链接。按设计稿 §3.2 写
   `~/.pony/config.toml`：owner 成员 ID、频道 ID 与仓库绑定；私有频道要
   `/invite @pony`。先试直连，连不上再填 HTTP(S) `proxy`，不支持 SOCKS。
   不依赖 shell 的代理环境变量或 macOS 系统代理，launchd 的环境也不同于终端。
3. 按 ranch 的全局部署流程生成新的不可变版本目录。用户在普通终端先用
   `<新版本目录>/bin/ranch deploy <版本> --dry-run` 预览，再去掉 `--dry-run`
   确认部署；它一起切换 `~/.local/bin/{corral,ranch,fence,pony}`，并按需重装
   corral、corral-dispatch 技能及已装的全局插件。服务只经 pony 的链接启动，
   不用重新生成指向某个版本目录的 plist；corral 运行中的 agent 不在这里升级。
4. 在当前用户的图形登录会话中预览、确认内容后安装，再看状态：

   ```sh
   pony service install --dry-run
   pony service install --yes
   pony status
   ```

   macOS 可能通知「已添加后台项目」。`installed` 但 `running: false` 时，
   看配置、`last_exit` 和日志；只安装文件不能保证配置正确或 Slack 已连上。
5. 后续用 `ranch deploy <版本>` 部署。它会把 pony 这一步列入计划，确认后
   自动执行：服务已装且由 launchd 运行时，plist 为 `replace` 就调用目标版本的
   `pony service install --yes`；定义相同则比对正在运行的真实程序和目标的
   sha256，内容不同才 kickstart。程序相同不重启；服务未装或非 macOS 只切链接；
   服务装着但没在跑时说明 `last_exit`，手动运行时提醒自行重启，两种都不重启。
   重启后最多等 30 秒，核对 running、新 pid 和目标程序；Slack 连接状态照实
   显示，不要求已 connected。pony 操作失败时先说明当前状态并给日志位置，不自动撤回。
   若替换服务定义失败，旧定义可能已卸下，按提示先用目标版本的
   `pony service install --yes` 恢复。重跑同一目标的 deploy 时，若失败记录表明
   pony 原本在跑，也会重新列出服务安装、等待和核对，不把它当成原本就停着的服务。

   没配置时 pony 退出 0，launchd 不再重起。运行中删掉配置也一样；放回配置后
   要执行 `launchctl kickstart -k "gui/$(id -u)/local.ranch.pony"` 或重新登录，
   单独放回文件不会触发启动，deploy 也不会启动原本没在跑的服务。
   这里的「删配置」只指 `config.toml`。要删除整个 `~/.pony`、`logs/` 或
   `~/.local/bin/pony` 链接，先 uninstall；否则 launchd 打不开日志路径或程序，
   pony 没机会正常退出，会因启动失败反复重试。
6. 退回时，用户在场执行 deploy 打印的退回命令（用仍保留的 deploy 程序绝对
   路径运行 `ranch deploy <旧版本>`，链接和技能一起退回；记录在
   `~/.local/share/ranch/deploys.jsonl`），pony 的定义变化、重启和核对也由 deploy
   按第 5 步处理；失败时按输出查看 status 和日志。
   不要覆盖、删除仍被进程或 hook 引用的不可变版本目录。
   若退回的 pony 版本还不支持 service 命令，先用当前版本卸服务再切链接。
   只想停止常驻时，使用：

   ```sh
   pony service uninstall --dry-run
   pony service uninstall --yes
   pony status
   ```

日志有两处：`~/.pony/logs/pony.log` 记连接变化、路由元数据和公开错误码，
满 1 MiB 轮转、留 3 份；`~/.pony/logs/launchd.log` 同时接标准输出和标准错误，
包括启动失败、panic、连接状态写盘失败及 `rate_limited wait_seconds=N`。
launchd.log 当前不自动轮转；需要时先卸服务、归档日志，再重装。
看 `config_missing`、凭证权限错误、连接错误、限频等待；不要把消息正文、
token、Authorization 头、带 ticket 的 wss 地址写到日志或故障报告。

### 上线实测清单

- [ ] `pony status` 的连接状态、工作区／机器人／频道名和其他连接数正确。
  私有频道 `not_found` 先核对频道 ID 和邀请，分别观察未邀请、已邀请的实际错误。
- [ ] 顶层 @、已知线程回复、每轮回复和 Needs you 提醒都回到预期频道／线程；
  owner 每轮收到通知。核对手机是否响，以及「电脑不活跃时才推到手机」的设置。
  当前 notify 仅在 section 块 @ owner，fallback text 没 mention；不响再另改。
- [ ] 普通附件及「分享消息」均明确提示「附件没送」，只贴链接的预览不提示；
  确认 `is_share` 在第一次收到的 message 事件里，而非后补的 `message_changed`
  （pony 忽略后者）；若只在后者出现，当前修复在线消息的行为仍需另改。
  线程回复勾「同时发到频道」仍送同一个 agent，断线补收同一句只送一次。
- [ ] 合盖／断线再连后按原线程补收；删掉排队原消息能撤回。私有频道的
  `conversations.replies` 权限与分页可用；让线程回复排队、断线再连，核对
  带 `oldest`、`latest` 的 exists 查询没有把仍存在的回复误判为撤回。
- [ ] 真实 wss/rustls 连接挂几分钟，跨过多次 250 毫秒读超时仍可收发，
  尽可能观察一次 `refresh_requested` 后恢复；本机假 ws 不覆盖真实 TLS 路径。
- [ ] Markdown、代码、表格、长回复完整；`&lt;`、`&gt;`、`&amp;` 显示正确。
  超长且含大量 `<>&` 的代码触发缩小重试后仍完整；记录实际长度错误码。
- [ ] `<!channel>` 和不带尖括号的 `@here`、`@channel`、`@everyone`、`@名字`
  不意外通知别人。默认不放开正文中的意外 @，是否放开交用户决定。
- [ ] 记录一次补收 30 天父消息的 history／replies 请求数量、是否遇到 429、
  `Retry-After` 等待带来的核心延迟。当前决定仍是限频期间核心暂停，收事件、
  落盘、ACK 和心跳独立继续；量大时再决定是否另做异步投递。
- [ ] 需要代理时填写配置后可重连；重登可起服务，切新版链接加 kickstart 可换版，
  删除配置正常退出后不会循环重起，恢复配置加 kickstart 可恢复。
- [ ] 服务正在运行时，`pony service uninstall --yes` 一次成功，不留下待卸的 plist。
- [ ] 在系统设置「登录项」中关掉 pony 后，install 的报错能看懂。
- [ ] `replace` 后用 `launchctl print` 核对 path 和 arguments，确认加载的是新定义。
