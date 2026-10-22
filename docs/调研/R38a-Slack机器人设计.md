# R38a：Slack 机器人设计（pony）

2026-10-21，ranch/dev-r38a。只出设计，不写代码、不连 Slack、不注册应用。依据：`docs/任务/R38-从Slack等聊天软件at着agent开发.md`（用户已定的、交给 ranch 定的、验收 1–7）、`docs/任务/R38a-Slack机器人设计.md`、DESIGN §1–4、§7、§8，CONTRACT，corral 的 `cli.rs`（send、wait、reply 的实现和参数解析）和 `agents/mod.rs`（适配器登记），本机只读的 `corral ls`、`corral status`，paddock 收件箱判新回复和等你的代码（`app/src/agents.rs`、`attention.rs`）。Slack 文档和 Rust 库的出处在文末，都是 2026-10-21 查的。

用户已定的各条照用，没有改。名字定为 **pony**（Pony Express，送信的马；用户 10-21 定）：包 `crates/pony`、命令 `pony`、目录 `~/.pony`。

## 要用户定的（按重要程度）

1. **后台怎么常驻**
   - A（推荐）：macOS 的用户级 launchd 服务。`pony service install` 写 `~/Library/LaunchAgents/local.ranch.pony.plist` 并加载；登录就起，崩了 10 秒后重起，没配置时正常退出、不再重起。这是写全局位置，照 DESIGN §7 全局安装的规矩：用户在场、先 `--dry-run` 看、确认才写，只动自己这一个文件，能卸干净。部署新版后 `launchctl kickstart -k` 换到新程序。第一期只在这台 Mac 上开，够用；Linux 以后要用时另做 systemd 用户服务。
   - B：`pony start` 自己转到后台、`pony stop` 停。不写系统位置，两个平台一样；但重启电脑、注销后不会自己起来，崩了也不会重起，要人发现了再起（paddock 能显示没在跑）。
   - 不推荐：用 `corral start` 把它当成一个 agent 跑。它会出现在 `corral ls` 和 paddock 的 agent 列表里，状态一直是 `unknown`。
2. **Slack 用哪个 Rust 库**（详见第 5 节）
   - A（推荐）：自己接。HTTP 用 ranch 已在用的 ureq 3.4.2，WebSocket 用 tungstenite 0.30（同步）。新增 10 个包，不引入异步运行时；能走代理；日志里写什么由自己定。要多写约 450 行代码（另有测试）。
   - B：slack-morphism 2.31.0。它够格：活跃、下载多，类型全，重连现成。但要新增 101 个包（tokio、hyper、tower、futures、icu、jiff、tracing 等），pony 得写成异步程序，和 ranch 其余的同步代码不一样。它的 Socket Mode 直接连 Slack，源码里没有代理设置。本机系统设置里开着 HTTP 代理 127.0.0.1:1082，命令行程序默认不走它；如果这台 Mac 不经代理连不上 Slack，B 就得改库或绕开。
3. **agent 正在干活时，又来了一句话**（详见 3.6）
   - A（推荐）：排队。线程里回 `→ X（正在干活，这句排在第 1 个，这一轮结束就送）`。之后按顺序送，一轮送一句；真送出去时在线程里再说一声 `已送给 X（排了 12 分钟）`。把还在排队的那条 Slack 消息删掉，就是撤回。pony 重启后队列还在。
   - B：不送。线程里回 `X 正在干活，这句没送`；这一轮结束时照常贴回复，要说的话再发一次。最简单，也不会有过时的话晚送进去；但在手机上得等着回头再发。
4. **Needs you 时，手机上能做到哪一步**（这是和已定的对不上的地方，见下一节）
   - A（推荐）：第一期照已定的，只用 send、wait、reply、status、ls 这五条命令。提醒写清楚它在等什么（权限框或提问），带上 `status` 里的最后一个工具和终端标题；对话框要回电脑上处理（attach 或 paddock）。在提醒线程里回的话先排着，对话框处理完、它空下来再送（问题 3 选 B 时改成不送，等它空下来再发）。
   - B：提醒里再贴屏幕末尾几十行（用 `corral read`），手机上能看到对话框写了什么，但仍然点不了。这超出了已定的五条命令；屏幕上可能有路径、代码，同样只发绑了频道的仓库。
   - C：再加 `corral keys`，在线程里回 `1`、`esc` 这类按键来答对话框。手机上就能放行，但这等于远程按键，误按会批准危险命令，不建议第一期做。
5. **paddock 的 P5-147 要不要提前开**：`pony status` 的命令和 JSON（第 4 节）能先定下来。R38b 把它写进 CONTRACT 之后，paddock 就能照着做 Settings；在没配置的机器上它显示「未配置」，部署后才有真实数据。P5-147 原写着「等 ranch 那件做完、部署后再开」。推荐 R38b 合并后就开，以后只做兼容新增。

上面五条以外的都给了推荐；主控审查时有异议再提。

## 用户定的（10-21）

1. **主控新回复和提醒**：每一轮都发到 Slack。提醒（主控新回复、Needs you）和每一轮回复的第一段，照原设计 @ 用户。pony 不判断用户在不在电脑前；手机吵不吵，靠 Slack 自己的通知设置（频道静音、只收 @、电脑上不活跃时才推到手机等）。用户原话：「先这么定，如果以后嫌吵，咱们再改。」R38f 上线实测时，核对 Slack 有没有「电脑上不活跃时才推到手机」这类设置。
2. **后台怎么常驻**（问题 1）：A，launchd 用户服务。
3. **Needs you 时手机上能做到哪一步**（问题 4）：10-21 选 A，只发提醒、回答排着。10-22 经 R42 实测和 R43、R44，R45 改成只按 Esc：「停」打断或拒绝，Claude Code 提问提醒线程一回答就关框、等空闲后送；不批准任何框，仍无按钮。详见 §3.5、§3.8。
4. **Slack 用哪个 Rust 库**（问题 2）：A，自己用 ureq + tungstenite 接。
5. **agent 忙时又来了话**（问题 3）：A，排队；删掉还在排队的那条 Slack 消息就是撤回。
6. **名字**：照推荐，就叫 pony。
7. **P5-147 什么时候开**（问题 5）：照推荐，R38b 把 `pony status` 写进 CONTRACT 之后，paddock 的 P5-147 就可以开工。
8. **离线时的消息补收**（10-21 用户在设计合并后提出：「pony 不补收离线时的消息：Mac 睡着或 pony 没在跑时，你在手机上发的话会直接丢掉」；主控给了三个办法，用户：「按你的推荐，选第 2 种，加说明，12 小时上限」）：补收并照常送，见 3.11。改了 3.5「不加前缀」那一条：只有补收的话，送给 agent 时开头加一句说明。

下面正文已照这些决定改成定稿说法，没选的选项不再展开。

## 和已定的各条对不上的地方

- **Needs you 的线程里回复，送不进正在等对话框的 agent**。agent 是 `blocked` 时 `corral send` 一律退回 7 `not_idle`（`cli.rs` 的 `send_once`）。10-21 定先排着、回电脑处理；R45 起，Claude Code 提问提醒的回答先核对同一框，按 Esc 后等空闲再送，其他框仍排队，可另回「停」拒绝。主控新回复的提醒线程不受影响，那时 agent 是 `idle`。
- 其余照用，设计里各处的落点：
  - 插件式：第 2 节。
  - 和 agent 只走 corral 公开命令：pony 用 `ls`、`status`、`reply`、`send`、`read`、`keys`（只按 Esc）。不用 `wait`，理由见 3.7。
  - 第一期的范围：3.5–3.10 做送话、贴回、提醒、看状态；不开、不停 agent。
  - 一个频道对应一个仓库、`→ <全名>`、带斜杠的全名没有这个 agent 就报错：3.5。
  - 没绑频道的仓库一个字都不发：3.5、3.8、3.10。另外 pony 对这些仓库的 agent 连 `status` 都不调。
  - 回复原样整段发：3.9。分段只在段尾加 `(1/3)` 这样的记号，代码块断开处补上收尾和开头的 ```，正文一个字不改、不删。
  - 只认一个用户：3.5。
  - 只在这台 Mac 上开：没有配置文件就什么都不连，别的机器默认关着；连上时看 Slack 报的连接数，有别的连接就在 status 里提示（3.3）。
  - 凭证只放本机：第 6 节。
  - 给 paddock 一条只读命令：第 4 节。

## 1. 总体

pony 是 ranch 里单独的一块，照 DESIGN §8 fence 的做法：

- 包 `crates/pony`，命令 `pony`，打进版本目录的 `bin/pony`，部署时切 `~/.local/bin/pony`。
- 和 corral 只通过命令来往：调 `corral ls`、`status`、`reply`、`send`、`keys`（只按 Esc），读它们的 JSON；`read` 只取屏幕末尾文字。Cargo 里不依赖 `corral-core`、`ranch-dispatch`、`fence`，不读 `~/.corral` 里的任何东西。
- 找 corral：配置里的 `corral`（绝对路径），没有就先找 PATH，再找 `~/.local/bin/corral`（launchd 起的程序 PATH 只有系统目录，会落到后一个）。起来时调一次 `corral --version`，看 `contract` 是不是 `"1"`。
- 以后整块拆出去：把 `crates/pony` 挪到自己的仓库，自己打包、部署。corral 一侧本来就没有任何关于它的代码，不用改。
- **pony 不改 corral 的代码**。只通过现有公开命令来往：`ls`、`status`、`reply`、`send`、R44 的 `read`、R45 的 `keys`（只按 Esc，amp 两个 Esc）。题目和框上打断识别由用户单独同意的 R43 提供；pony 不读 corral 内部文件。

```
    Slack（Socket Mode，wss 往外连）            Slack Web API
          │ 事件                                     ▲ 发消息
          ▼                                          │
 ┌────────────── pony run（常驻，单实例）──────────────────────────┐
 │ 适配器 chats/slack.rs ──事件──► 共用核心（一个事件循环）──发送──► 发送线程（每频道 1 条/秒） │
 │                                   │ 路由·线程表·排队·盯 agent·提醒·分段·状态       │
 │                                   ▼                                             │
 │                     corral 那层：每 2 秒 ls、status，一轮结束读 reply；send         │
 └───────────────────────────────────┼─────────────────────────────────────────────┘
                                     ▼ 只调 corral 的公开命令
                                 corral → 各 agent
 ~/.pony/：config.toml · secrets/slack.toml · state.json · run/ · logs/
 paddock ──► pony status（只读本机文件，不联网）
```

## 2. 插件式结构

照 DESIGN §7 corral 接 agent 的做法：每家一个文件，登记在一张表里，共用的代码只查表。

```
crates/pony/src/
  main.rs            命令行：run、status、service、--version
  core/              共用核心，不写任何一家的名字
    config.rs  secrets.rs  route.rs  threads.rs  relay.rs  watch.rs  split.rs  state.rs  status.rs
  corral.rs          调 corral 命令、解析 JSON
  chats/
    mod.rs           Chat 接口和登记表 CHATS
    slack.rs         Slack
```

### 适配器接口

| 项 | 管什么 | Slack 怎么填 | 飞书（只用来检验接口够不够，没细核） |
| --- | --- | --- | --- |
| `kind` | 配置里的段名、凭证文件名、`pony status` 里的 `kind` | `slack` | `feishu` |
| 凭证的键 | `secrets/<kind>.toml` 里必须有哪些键 | `app_token`、`bot_token` | 应用 ID、密钥 |
| 读这家的配置 | 校验用户 ID、频道 ID 的写法，解析这家专有的项；共用的 `owner`、`channels` 由核心统一要求 | 见 3.2 | 同 |
| 连接 | 起连接、收事件、回执、断了重连，报连上、断开和原因 | Socket Mode | 官方有「长连接接收事件」，也是往外连 WebSocket、3 秒内处理、同一应用多个连接时随机一个收到 |
| 翻译收到的消息 | 翻成共用格式：频道、消息 id、所在线程、发的人、有没有 @ 机器人、去掉 @ 和转义后的正文、有没有附件；另有「某条消息被删了」 | `message` 事件、`<@U…>`、`&lt;` | 收消息事件、它的 @ 写法和话题 |
| 上限 | 一条消息最多多少字、有没有线程 | 10,000 字、有线程 | 按它的文档 |
| 发消息 | 把 Markdown 发到频道或某个线程，返回消息 id；核心标了「要提醒」的，要让用户的手机响；这家的限频、重试、格式转换都在这里 | `chat.postMessage` 加 markdown 块；要提醒时开头 @ owner | 卡片里的 markdown |
| 频道信息 | 名字、是否私有、机器人在不在里面，给 `pony status` | `conversations.info` | 按它的接口 |

- 共用格式里，正文和要发的字都是 Markdown，「@ 的写法」「转义」「消息格式」都在适配器里换。
- 没有线程的平台：核心退成「每句都要 @、写明 agent」，回复以 `<全名>:` 开头直接发在频道里。第一期用不到，只在接口里留着这一项。
- 登记表：`static CHATS: &[&dyn Chat] = &[&slack::Slack];`。核心按登记顺序找配置里的段、起连接、出 status。

### 共用核心管什么

- 配置：读、校验、随改随生效（3.2）。
- 凭证：按适配器声明的键读 `secrets/<kind>.toml`，查权限，只把值交给那一家的适配器（第 6 节）。
- 权限：只认配置里的 `owner`，别人的消息不回、不记内容。
- 路由：@ 后第一个词对应哪个 agent，跨仓库、没绑频道的仓库，`?` 看状态（3.5）。
- 线程表：线程和 agent 的对应，存盘（3.11）。
- 送话和排队：调 `corral send`，照结果在线程里回话（3.6）。
- 盯 agent：每 2 秒 `ls`、`status`，一轮结束读 `reply`，贴回线程或发提醒，Needs you，去重（3.7、3.8）。
- 分段：按适配器给的上限切 Markdown（3.9）。
- 状态、日志、给 paddock 的 `pony status`（第 4 节）。

### 接第二家（比如飞书）

加 `chats/feishu.rs`，在 `CHATS` 加一行。用户那边在配置里加 `[feishu]` 段，放 `secrets/feishu.toml`。共用核心不改。

为了守住这一条：

- 核心测试用一个只在测试里登记的内存假适配器，验收 1–5 的逻辑都在这一层测。
- 加一条测试：共用核心的源码里不出现任何已登记适配器的 `kind`。

## 3. 交给 ranch 定的各项

### 3.1 命令

| 命令 | 做什么 |
| --- | --- |
| `pony run` | 前台跑机器人。launchd 用它，调试时也可以在终端里直接跑。同一个 `PONY_HOME` 只能有一份：拿不到锁就退出 1，launchd 10 秒后再试，等前一份退了自然接上。没有配置文件就记一行日志、退出 0。 |
| `pony status` | 给 paddock 的只读命令（第 4 节）。 |
| `pony service install\|uninstall [--dry-run] [--yes]` | 装、卸 launchd 服务。不带 `--yes` 只列要写什么、要跑哪条 `launchctl`。非 macOS 报 `unsupported`。 |
| `pony --version` | `{"ok":true,"version":"0.1.0","contract":"1"}`，和 corral 一样。 |

- 所有命令输出一行 JSON；参数错退出 2、`usage`。
- 测试和隔离用环境变量 `PONY_HOME`（默认 `~/.pony`），像 fence 的 `FENCE_HOME`。

### 3.2 配置文件

`~/.pony/config.toml`，初次配置由用户写，不含凭证。R40a 起，在已配置的机器上再绑仓库只需建私有频道、`/invite @pony`、由 owner 发 `@pony 绑 <仓库名>`；pony 向这一家的 `channels` 加绑定，保留注释、顺序和原权限，校验并原子写入，交给现有重读生效。公开频道、查不到是否私有、名字不合法、重复绑定、配置有错或被改过均拒绝；不做解绑、改绑，详见 DESIGN §10。

```toml
# 没有这个文件，pony 什么都不连（别的机器默认关）
# corral = "/Users/u/.local/bin/corral"   # 可选，默认 PATH，再 ~/.local/bin/corral
# proxy = "http://127.0.0.1:1082"         # 可选，连聊天软件走的代理

[slack]
owner = "U0123456789"            # 只听这个用户的：Slack 里个人资料 → ⋯ → 复制成员 ID
channels = [
  { id = "C0AAAAAAAAA", repo = "paddock" },
  { id = "C0BBBBBBBBB", repo = "ranch" },
]
```

- 校验，错了就整份不用，报出文件、位置和原因：
  - 顶层只认 `corral`、`proxy` 和已登记适配器的段。
  - 每段必须有 `owner`、`channels`。
  - `repo` 按 corral 名字一段的规矩：ASCII 字母或数字开头，只含字母、数字和 `._-`。
  - 同一家里一个频道只绑一个仓库，一个仓库也只绑一个频道，提醒发到哪才唯一。
- **用频道 ID，不用频道名**：频道改名不影响，也不用为了按名字查频道多要权限。名字由 pony 连上后查出来，给 status 显示。
- **随改随生效**（和 `ranch dispatch` 每次读表一样）：
  - 运行中每轮检查文件有没有变，变了就重读。
  - 新内容有错就继续用旧的，status 报 `config: "invalid"` 和原因。
  - 改了 `proxy` 或这一家的段就重连。
- **代理要写明**：launchd 起的程序不继承 shell 的环境变量，ureq、tungstenite 也不读 macOS 系统设置里的代理。本机系统设置开着 HTTP、HTTPS 代理 127.0.0.1:1082（`scutil --proxy`）。部署时先试直连，连不上再填 `proxy`。

### 3.3 Slack 怎么接、要哪些权限

**连法**：Socket Mode，pony 往外连 wss，本机不需要公网地址（已定）。

1. 用 app 级 token（`xapp-`，权限 `connections:write`）调 `apps.connections.open`，token 放在 `Authorization` 头里，拿到临时的 wss 地址。
2. 连上后先收到 `hello`，里面有 `num_connections`。大于 1 就说明还有别的连接（另一台机器，或一份没退干净的 pony）。Slack 会把每条事件交给任意一条连接，所以 status 报 `other_connections`，paddock 显示成警告。
3. 每个事件信封都要按 `envelope_id` 回执，否则 Slack 会重发。Events API 要求 3 秒内答复，重发最多 3 次。pony 先把这条消息记进状态文件再回执（几毫秒），重启也不丢；按消息的 `ts` 去重。别人的消息不记，直接回执、丢掉。
4. 断线：
   - 收到 `disconnect`（`warning`、`refresh_requested`，Slack 每隔几小时就会这样换一次连接）就重新取地址再连。
   - `link_disabled` 说明应用设置里关了 Socket Mode，报 `socket_mode_off`。
   - 网络错就按 1、2、4…60 秒退避重连。
   - 每 30 秒发一次 ping，90 秒什么都没收到就当断了。Mac 睡眠醒来走同一条路。
5. 每次重连都重读凭证文件：改了 token 或权限，不用重启 pony。

**权限**（bot token `xoxb-`）：

| 权限 | 用来 |
| --- | --- |
| `chat:write` | 发消息、回线程 |
| `channels:history`、`groups:history` | 收公开、私有频道的 `message.channels`、`message.groups` 事件。线程里不 @ 的回复只能这样收到 |
| `channels:read`、`groups:read` | `conversations.info`：频道名、是否私有、是否已归档，给 status。机器人在不在频道里看返回的 `is_member`（文档的例子里没有这个字段，实现时核对；没有就以发消息时报的 `not_in_channel` 为准） |

- 只订 `message.channels`、`message.groups`，不订 `app_mention`：线程里的回复反正要靠 message 事件收，再订 `app_mention` 同一句 @ 会来两次。@ 机器人由适配器在正文里认 `<@机器人的用户 ID>`（U 开头）或 `<@机器人的 bot ID>`（B 开头；手机 Slack 插的是后者，用户 10-21 实测），两个 ID 都在连上时用 `auth.test` 取（它不要权限、两个都返回），不写死。
- 不要：`app_mentions:read`、`chat:write.public`（只在被邀请的频道里说话）、`files:*`、`reactions:*`、`users:read`、`im:*`。
- **建议用私有频道**：仓库里的代码、路径会发进去，私有频道只有邀请的人看得到（已定：工作区管理员仍可能看到）。机器人要 `/invite @pony` 进频道才收得到消息。
- 发消息：
  - `chat.postMessage` 带 `thread_ts`，正文放 markdown 块（`type: markdown`，认标准 Markdown，含代码块和表格），另给一行 `text` 当通知里显示的那句。
  - 关掉链接预览（`unfurl_links`、`unfurl_media` 为 false），免得 Slack 的服务器去抓回复里私有仓库的链接。
  - 要提醒的消息（Needs you、主控新回复、每一轮回复的第一段），开头放一个 mrkdwn 的 section 块，写 `<@owner> ranch/main 新回复` 这样的一行。Slack 默认只在被 @、私信时推送，频道里机器人的普通消息手机不响。@ 的只有用户本人。`→` 那一行、排队说明、后面几段不 @。pony 不判断用户在不在电脑前，每一轮都照这样发；手机吵不吵靠 Slack 自己的通知设置（用户 10-21）。
  - 限频大致每频道每秒 1 条：发送线程按频道排着发，遇到 429 照 `Retry-After` 等。
- 收消息（适配器里做）：
  - 要的只有四种：没有子类型的普通消息、带附件的 `file_share`、`message_deleted`，以及线程回复时勾了「同时发送到频道」的 `thread_broadcast`（当普通线程回复处理；补收时它在频道历史和线程回复里各出现一次，按消息 ID 去重。用户 10-21 定，R38e 审查第 3 条）。带 `bot_id` 的、其余子类型（`message_changed` 等）都不要。
  - `message_deleted`（它带 `hidden`）翻成「`deleted_ts` 那条被删了」，用来撤回排队的话。
  - 正文还原 `&lt;` `&gt;` `&amp;`；`<url|字>` 还原成 url；去掉开头的 `<@机器人>`。
  - 带附件的只送文字，并在线程里说「附件没送」。
- 判断线程：消息有 `thread_ts` 且不等于自己的 `ts` 就是线程里的回复。
- 应用用 manifest 建（部署时用户在场做，见第 8 节）。manifest 不能生成 app 级 token，要在应用设置的 Basic Information 里另外生成：

```yaml
display_information:
  name: pony
features:
  bot_user:
    display_name: pony
    always_online: false
oauth_config:
  scopes:
    bot: [chat:write, channels:history, groups:history, channels:read, groups:read]
settings:
  event_subscriptions:
    bot_events: [message.channels, message.groups]
  interactivity:
    is_enabled: false
  org_deploy_enabled: false
  socket_mode_enabled: true
  token_rotation_enabled: false
```

### 3.4 后台怎么常驻（用户 10-21 定：launchd 用户服务）

- `pony service install` 写的 plist：
  - `ProgramArguments` 是 `~/.local/bin/pony run`：指向链接，部署切了链接、`launchctl kickstart -k gui/<uid>/local.ranch.pony` 就换到新版本，旧版本目录不被引用。
  - `RunAtLoad` 为 true；`KeepAlive` 里设 `SuccessfulExit` 为 false：退出码非 0 才重起，launchd 默认最快 10 秒一次。
  - 标准输出、错误输出写到 `~/.pony/logs/launchd.log`，只用来接 panic。
  - 写法照 corral integration：原子写，那里已有不是 pony 写的文件就报 `foreign`、不动。
- 加载用 `launchctl bootstrap gui/<uid> <plist>`，卸载 `bootout` 后删文件。macOS 可能弹「已添加后台项目」的通知。
- 单实例：`~/.pony/run/lock`，`pony run` 起来先拿锁。`pony status` 只查锁、不拿锁（`fcntl` 的 `F_GETLK` 能查出谁拿着锁），免得和正在起来的 `run` 抢。
- 日志 `~/.pony/logs/pony.log`，满 1 MiB 轮转，留 3 份。记连接变化、路由到哪个 agent、频道 ID、消息 `ts`、corral 和 Slack 的错误码。**不记**消息正文、回复正文、任何 token、`Authorization` 头、wss 地址（地址里带一次性的 ticket）。
- 不采用 pony 自己转后台（`pony start`/`stop`）。

### 3.5 一句话进来之后

只处理 `owner` 发的、绑了仓库的频道里的消息。别人的一律不回、不送，日志只记「忽略了一条别人的消息」。

| 情况 | 怎么做 |
| --- | --- |
| 频道顶层、没 @ 机器人 | 不理，用户可能只是在频道里记东西 |
| 频道顶层 @ 机器人 | 开一段新对话，下面按第一个词路由；在这条消息的线程里回 |
| 已知线程里的回复（带不带 @ 都行） | 送给这个线程对应的 agent，不再看第一个词；整句只有 `?` 的除外，那是看状态 |
| 不认识的线程里 @ 机器人 | 当成新对话，这个线程从此对应路由到的 agent |
| 不认识的线程里没 @ | 不理 |
| 没绑仓库的频道里 @ 机器人 | owner 发「绑」加一个仓库名时，确认是私有频道并写入绑定，在线程回「已绑 <仓库>」及主控状态；其他话回「这个频道没绑仓库。要绑，发 `@pony 绑 <仓库名>`」，不列仓库 |

**路由**：取 @ 之后的第一个词。

R45：整句「停」或「打断」是打断指令，已知线程停对应 agent，明确跨仓库送话建立的线程也放行；频道顶层停主控，后面只跟一个本频道仓库的 agent 名字时停那个 agent。顶层跨仓库回「只能停本频道仓库的 agent，没按」，像 agent 名但不存在时回「没有 X 这个 agent，没按」，不转给主控。「停一下，先别提交」「停 一下」仍是普通话。按前查状态：只在 working/blocked 时按；暂停、空闲、启动中、读不出、实例已换、消息发出后另开一轮、30 秒内有人工输入或离线补收都不按，并在线程说明。agy 不按，amp 一次命令发两个 Esc，其余一次 Esc；按前持久化，每秒确认、最多 15 秒、不重试，重启只继续确认。确认空闲后说「停下了」或「框已拒绝」，只与同线程 pony 原轮的打断提醒合成一句，其他线程仍各自收到结果。超时照实说，排队的话继续等空闲。

- 整句只有 `?` 或 `？`：看状态（3.10），不送给任何 agent。corral 的名字只能以 ASCII 字母或数字开头，`?` 不会和 agent 撞名。
- 第一个词带斜杠（`ranch/main`）：
  - 斜杠前的仓库没绑频道：只回「这个仓库没接入 Slack」，不送、不转，也不说有没有这个 agent。
  - 绑了，但 `corral ls` 里没有这个 agent：回「没有 ranch/foo 这个 agent，没送」，不转给主控。
  - 有：送给它，正文是第一个词之后的部分。
- 第一个词符合 corral 名字一段的写法，而且这个仓库有 `<仓库>/<这个词>`：送给它，正文是后面的部分。中文开头的话不可能是 agent 的短名，直接走下一条。
- 其余：整句送给 `<仓库>/main`。主控没在跑就回「paddock/main 没在跑，没送」。
- 送的正文为空：回一段用法（怎么点名、`?` 看状态）。
- 机器人先在线程里回一行 `→ <全名>`，再去送；送的结果跟在后面（3.6）。线程开头记的那个 agent 后来重开过（实例变了），这一行写成 `→ paddock/main（重开过）`。
- 送的就是用户打的字：`corral send <全名> -- <正文>`。`--` 让以 `-` 开头的话也原样送过去（corral 的参数解析认 `--`，之后不再当选项）。不加 `--force`。例外：离线补收的话（3.11）、提问框回答（3.8）开头加一句说明。

### 3.6 agent 正在干活时（用户 10-21 定：排队）

送之前先 `corral status`。另外，pony 送出的话还没等到这一轮结束时，这个 agent 算「在等回复」，再来的话也排队，不会在它开始这一轮之前连送两句。

| 状态 | 怎么做 | 线程里说 |
| --- | --- | --- |
| `idle` | 立刻送 | `→ X` |
| `working`、`starting` | 排队 | `→ X（正在干活，这句排在第 n 个，这一轮结束就送）` |
| `blocked` | 排队 | `→ X（在等你处理对话框，处理完再送）` |
| 暂停中 | 排队，不替人 resume | `→ X（暂停中，恢复后再送）` |
| `unknown`（没钩子的程序） | 照送（corral 允许），等不到回复 | `→ X（它报不出状态，送了，但等不到回复）` |
| 不在了 | 不送 | `X 已经不在了，没送` |

`corral send` 的结果：

| 结果 | 怎么做 |
| --- | --- |
| 成功 | 没有另外的话；`merged_with_draft` 为真时补一句「输入框里原有没提交的字，和你的话一起送了」 |
| 7 `not_idle` | 放回队首，等它空下来 |
| 8 `human_active` | 人刚在它窗口里操作过：30 秒后再试，不加 `--force`（和 paddock 的 Loop 一样） |
| 10 `paused` | 照「暂停中」排着 |
| 3 `not_delivered` | 不重送，回「送了但 X 没收到（可能停在菜单或对话框上），没重送」 |
| 2 | 照「不在了」 |
| 其他 | 回 corral 的 `error` 和 `message` |

- 排队的话按顺序送，一轮一句。真送出去时在原线程说「已送给 X（排了 12 分钟）」，这一轮的回复贴回那个线程。
- 撤回：删掉还在排队的那条 Slack 消息，就从队里拿掉，线程里说一句「撤回了」。改消息不算，排着的照原文送，要改就删了重发。
- agent 退出了：排着的都不送，在各自线程里说。
- 不采用「忙时不送、让用户再发」。

### 3.7 一轮结束，贴回线程

- 不用 `wait`：一个盯着所有 agent 的循环同时管「贴回线程」和「提醒」两件事。用 `wait` 的话，每段在等的对话要一直开着一个进程，提醒还得另做。判「这一轮结束」的办法和 `wait` 一致：`idle` 稳定一会儿才算（`wait` 要 0.5 秒，这里是相邻两轮、约 2 秒都一样）。
- 循环每 2 秒：
  - `corral ls`，只留仓库绑了频道的 agent；没绑的不再往下读。
  - 对留下的每个调 `corral status`。
  - 一轮结束：相邻两轮都是 `idle`、`[state, last_event, last_event_at]` 不变，且 `turn_started` 和上次处理过的不同。这时调 `corral reply`。
- 这一轮是 pony 送的话开的（这个 agent 在等回复），就贴回那个线程：
  - 有回复：照 3.9 分段贴。
  - `no_reply` 带 `interrupted: true`：贴「X 这一轮被打断了，没有回复」。
  - agent 退出了：贴「X 退出了，没等到回复」。
  - 然后送队里的下一句。
- 不是 pony 开的：照 3.8 判断要不要提醒。
- 一轮里人也在电脑上插了话：pony 分不出来，这一轮的回复照样贴回线程。

### 3.8 主控新回复和 Needs you

**主控**：名字是 `<仓库>/main` 的 agent，照已定的「默认送给这个仓库的主控（`<仓库>/main`）」。paddock 收件箱认的是 `role=controller` 标签，本机的主控两样都对得上。

**主控新回复**：

- 判断：一轮结束（同 3.7），`reply` 的 `at` 比记下的新，而且这一轮不是 pony 开的。
- 发在绑的频道顶层：第一行 `ranch/main 新回复`（Slack 里前面 @ 用户，见 3.3），接着是回复正文；太长时第一段在这条里，其余在它的线程里（3.9）。
- 这条提醒的线程对应这个主控，在里面回就是送给它。
- 不是主控的 agent 的新回复不提醒（paddock 收件箱也是这样），除非这一轮是 pony 开的。
- 人在电脑上和主控对话时，每一轮也照样提醒。pony 不判断用户在不在电脑前，手机吵不吵靠 Slack 自己的通知设置。用户 10-21：「先这么定，如果以后嫌吵，咱们再改。」

**Needs you**：

- 判断：相邻两轮 `status` 都是 `blocked`。绑了频道的仓库里所有 agent 都算，不只主控。
- 只发在该 agent 所在仓库绑定的频道顶层，不发进对话线程；Slack 里前面 @ 用户。R44 更新：Claude Code 提问框有 `status.questions` 时，按顺序列出完整题目、选项及说明，例如：

  ```
  ⚠ paddock/dev-x 在问你（2 道题）
  1. 颜色（单选）：选哪种颜色？
     A 红色 — 暖色
     B 蓝色 — 冷色
  2. 水果（可多选）：选哪些水果？
     A 苹果
     B 香蕉
  在这个线程写答案（例如『1 选 B；2 选 A、C』），会直接送过去。
  ```

- `truncated:true` 加「题目不完整，请回电脑看」。其他框、其他家或没有题目时，在下面原提醒的末尾答法之前附上 `corral read NAME --bytes 4000` 的末尾 20 行，连续空行并成一行，用 `~~~` 代码块包住、不遮盖。读不到照旧发原提醒：

  `⚠ paddock/dev-x 在等你（权限框或提问）· 最后的工具 Bash · 标题「◐ P5-160 …」。要拒绝：在这个线程回『停』；要允许：回电脑处理。在这里回别的话，等它处理完再送。`

- 这条的线程同样对应这个 agent。
- R45：提问提醒记下实例、state_started 和已处理标记；线程里的回答（除「停」「?」）先核对还是同实例、blocked、AskUserQuestion、同 state_started，且最近 30 秒无人输入，才按一次 Esc。等连续两次空闲后沿原队列送，前面说明「这是对刚才提问的回答」，只有 Esc 命令成功才补「关闭提问框不表示放弃作答」。核对不过或离线补收不按，只排队并解释；15 秒没空闲也继续排着。自动关框这一路提示「关掉提问框后就送」「提问框已关，回答这就送过去」，不说「框已拒绝」。同一提醒连答不重按，回答和「停」先后到共用确认；同线程还有回答排着时保留提问标记，全部送出后清掉，后续按普通话处理。agy 末尾仍是「要回电脑上处理；在这里回的话，等它处理完再送。」。

**不重复**：

- 每个 agent 记下：
  - 实例；
  - 处理过的 `turn_started`；
  - 发过的 `reply.at`；
  - 提醒过的那次等待：用（实例，`state_started`）标记。
- 离开 `blocked` 再进来，是新的一次。
- 这些存在 `state.json`，重启后照旧。
- agent 重开（实例变了）就从头记；agent 不在 `ls` 里了就删掉。
- 第一次看到的 agent（pony 刚起，或新开的 agent）：现有的回复只当起点，不发。
- pony 停着时结束的轮：重启后和记下的 `at` 比，每个 agent 最多补发最近一条（corral 只留最近一条回复），开头写「（pony 不在线时）」。

### 3.9 长回复分段

不截、不删、不改字。由核心做，上限由适配器给（Slack 取 10,000 字：markdown 块一次最多 12,000 字，留些余量给代码块补的 ``` 和段号）。

1. 按行走，记着是不是在 ``` 或 ~~~ 代码块里，以及代码块的语言。
2. 一段攒到放不下下一行就断。断点按顺序选：
   - 代码块外的空行；
   - 代码块外的行尾；
   - 代码块里的行尾：本段末尾补上 ```，下一段开头用同样的语言重开；
   - 一行本身就超长（比如压成一行的 JSON）：按字数硬断。
3. 表格尽量整块放进一段，放不下才在行间断开。
4. 多于一段时，每段末尾加一行 `(i/N)`。
5. 发到哪：第一段发到原本要发的地方（线程里，或频道顶层的提醒），其余按顺序发在同一个线程里。
6. 空回复发「（空回复）」。
7. 发送失败：
   - Slack 说块太长或块不对（`msg_blocks_too_long` 等）：把这一段对半再切，最小到 1,000 字。
   - 还不行：在线程里发「第 i/N 段发不出去：<错误码>」，也写进日志和 status，不悄悄丢掉。
8. 实现时要实测：回复里出现 `<!channel>`、`<!here>`、`<@U…>` 时，markdown 块里会不会真的 @ 到人。会的话由适配器转义。

### 3.10 看状态

`@pony ?`，频道里、线程里都行。只列绑了频道的仓库，这个频道的仓库排在最前：

```
paddock（#paddock）
• main — 干活中，12 分钟 · 「P5-160 设置页连线」 · 在用 Bash
• dev-p5-160 — 等你（对话框），3 分钟 · 在用 Bash
• dev-p5-161 — 空闲，40 分钟 · 上一句「改好了，测试全过。」
ranch（#ranch）
• main — 空闲，120 分钟
• dev-r38a-1 — 暂停中，60 分钟
```

- 状态的说法：
  - `working`：干活中，带这一轮多久了（`turn_started`）。
  - `idle`：空闲多久（`reply.at`，没有回复用 `state_started`）；本频道被打断的加「上一轮被打断」，不带旧回复。
  - `blocked`：等你（对话框），带等待多久（`state_started`，没有则用 pony 本次运行第一次看到这次 blocked 的时间）。
  - 暂停：暂停中，带暂停多久（`paused_at`）。
  - `starting`：启动中。
  - `unknown`：读不出。
- 排着话的 agent 后面加「排着 2 句」。
- R44：本频道仓库再带标题（去首尾空白和开头转圈符号、最多 40 字，空或只是程序名不显示）、在用的工具；空闲的多调一次 `reply`，显示第一行非空文字、最多 60 字。其他仓库只给状态和时长，不给标题、工具、回复，也不为摘要额外查回复；空闲时间用已知的 `reply.at`，没有则用 `state_started`。时间字段缺失不猜。

### 3.11 重启后

`~/.pony/state.json`（0600）。只有拿着锁的那一份 pony 写，同目录临时文件写好再改名替换。

- **线程和 agent 的对应还在**：按（适配器，频道，线程 id）记 agent 全名、当时的实例、建立和最后使用的时间。按全名送；实例变了在 `→` 行注明「重开过」。30 天没用的删掉，再在那个线程里回，机器人说「这个线程太久没用，对应关系已清掉，请在频道里重新 @ 我」，不猜。
- **在等回复的对话还在**：重启时那一轮已经结束了，就把回复贴回线程。
- **排队的话还在**：话的原文存在 `state.json` 里，只在本机。
- **去重记号还在**：见 3.8。
- **最近处理过的消息 `ts` 还在**：重启后 Slack 重发同一条也不会送两次。
- **pony 不在线时 Slack 里发的话要补收**（用户 10-21 定）：Slack 只在几分钟里重发几次，过了就不再推送，但消息还在频道里。
  - 每次连上（启动、重连）后，按记下的最后处理过的消息 `ts`，读每个绑了的频道的历史（`conversations.history`），以及线程表里离线期间可能有回复的线程（`conversations.replies`），只取 `owner` 发的、比记下的 `ts` 新的。用的是已有的 `channels:history`、`groups:history`，不加权限；自建应用不受 2025 年对非 Marketplace 应用的限频（每分钟 1 次、15 条），按正常限频走。
  - 取到的按发出的先后，照 3.5 的规则路由、照 3.6 排队，按 `ts` 去重，不会和 Slack 的重发撞成两次。
  - 送给 agent 时开头加一句：`（这是你 09:12 在 Slack 发的，pony 当时离线，现在才送到）` 加换行，再接原文；线程里的 `→` 行也注明「离线期间发的」。
  - 发出超过 12 小时的不自动送：在它的线程里回「这句是 12 小时以前发的，没送；要送请重发」。
  - 第一次起来（没有记下的 `ts`）不补收，免得把频道里的旧话都送进去。

## 4. 给 paddock 的只读命令：`pony status`

可以直接放进 CONTRACT。

- 只读本机文件（配置、`run/status.json`、锁），不联网、不调 corral、不碰 agent，几毫秒。可以每几秒调一次。
- 成功退出 0，一行 JSON。没配置、没在跑、连不上都是 `ok: true` 里的数据，不是错误；只有参数错是 `usage`、退出 2。
- 输出里没有 token、用户 ID 和消息内容。

```json
{"ok":true,"contract":"1","version":"0.1.0",
 "config":"ok","config_message":null,
 "service":"installed",
 "running":true,"pid":41234,"started":1792550000.5,"last_exit":null,
 "chats":[
  {"kind":"slack","connection":"connected","since":1792550003.1,"reason":null,"message":null,
   "workspace":"firegnu","bot":"pony","other_connections":0,
   "channels":[
    {"id":"C0AAAAAAAAA","name":"paddock","private":true,"repo":"paddock","problem":null},
    {"id":"C0BBBBBBBBB","name":null,"private":null,"repo":"ranch","problem":"not_member"}]}]}
```

| 字段 | 含义 |
| --- | --- |
| `config` | `ok`、`missing`（没有配置文件：这台机器没开）、`invalid`。`invalid` 时 `config_message` 写文件、位置和原因，凭证文件的错不带原文（第 6 节）。在跑的 pony 遇到改坏的配置会继续用旧的，这里照样报 `invalid`。 |
| `service` | `installed`、`not_installed`、`unsupported`（非 macOS）。 |
| `running` | 有 `pony run` 拿着锁。为真时有 `pid`、`started`（Unix 秒）。 |
| `last_exit` | 没在跑时上一次为什么退出：`{"at":…,"code":"secrets_too_open","message":"…"}`；不知道时为 `null`。 |
| `chats` | 配置里每家一项，按登记顺序。没配置时是空数组。 |
| `chats[].kind` | 适配器，现在只有 `slack`。前端照列表画，不写死。 |
| `connection` | `connected`、`connecting`（第一次连或正在重连）、`failed`（连不上，等着重试）、`off`（pony 没在跑）。 |
| `since` | 进入当前 `connection` 的时间。 |
| `reason`、`message` | `failed` 时的原因代码和说明。代码有：`secrets_missing`、`secrets_too_open`、`secrets_invalid`、`auth_failed`、`socket_mode_off`、`network`、`rate_limited`、`slack_error`；以后可能加，不认识的照 `message` 显示。 |
| `workspace`、`bot` | 连上后由 `auth.test` 得到的工作区名、机器人名，没连上过时为 `null`。 |
| `other_connections` | Slack 报的除自己以外的连接数，大于 0 说明可能有另一台机器也开着。 |
| `channels` | 配置里绑的频道，按配置顺序。`id`、`repo` 总有。`name`、`private` 连上并查过之后才有，否则为 `null`。 |
| `channels[].problem` | `null` 或 `not_member`（机器人没被邀请进去）、`not_found`、`archived`；以后可能加。 |

paddock 怎么显示（P5-147 定样子，这里只说数据怎么对应）：

- 找命令：PATH 上找不到 `pony`、`~/.local/bin/pony` 也没有，就是「未安装」，和找 corral 的办法一样。
- `config: missing`：「未配置（只在要用的那台机器上配）」。
- 没在跑：显示 `last_exit.message`；`service: not_installed` 时提示怎么装。
- 连没连上：`connection` 加 `reason`、`message`；`other_connections > 0` 时显示警告。
- 绑了哪些频道：`channels` 每项 `#name`（没有就显示 `id`）→ `repo`，有 `problem` 的标出来。
- 遇到没写的字段要忽略，不认识的取值要能显示，和 CONTRACT 开头的规矩一样。

## 5. Slack 的 Rust 库

Slack 官方没有 Rust SDK。候选：

| | slack-morphism 2.31.0（`hyper` 特性） | 自己接：ureq 3.4.2 + tungstenite 0.30.0 |
| --- | --- | --- |
| 最近发版 | 2026-10-10；一年内 16 个版本 | ureq 是 ranch 已在用的版本；tungstenite 0.30.0 发于 2026-07-11 |
| 下载 | 共 570 万，近 90 天 182 万 | tungstenite 共 3.5 亿，近 90 天 1.03 亿 |
| 维护 | GitHub 232 星、2 个没关的 issue；作者一人提交 749 次，其余多是自动升级机器人 | snapview，GitHub 2,399 星 |
| 新增依赖（macOS、正常依赖，和 ranch 现在的比） | 101 个全新的包：tokio、hyper、h2、hyper-rustls、tokio-tungstenite、tower、futures、url/idna/icu、jiff、serde_with/darling、tracing、signal-hook、ctrlc 等；另有 sha2 0.11 等已有包的新大版本 | 10 个全新的包：tungstenite、sha1、rand、rand_core、chacha20、data-encoding、thiserror、thiserror-impl、hybrid-array、const-oid；另有 digest 0.11 一族与 ranch 现有的 0.10 并存 |
| 代理 | Socket Mode 用 `connect_async(url)` 直连，源码里没有代理设置 | ureq 有 `Proxy`（HTTP CONNECT、SOCKS）；WebSocket 先自己连代理做 CONNECT，再把连接交给 `client_tls_with_config` |
| 写法 | 异步，要起 tokio | 同步线程，和 ranch 其余代码一样 |
| 日志 | 连不上时在 error 级日志里写 wss 地址（带 ticket） | 自己定，不写 |

其他候选都不够格：

- slack-api：最后发版 2020-06-29。
- slack-rust：0.0.1-alpha，2021-12-31。
- slack-rs：0.1.70，共 603 次下载。

**结论**：

- 用户 10-21 定：自己接（ureq + tungstenite），不采用 slack-morphism。slack-morphism 照 AGENTS.md 够格，不用它的理由：
  - pony 只用 5 个接口：`apps.connections.open`、`auth.test`、`conversations.info`、`chat.postMessage`，加 Socket Mode。为它引入一百多个包和一个异步运行时不划算。
  - 本机可能要走代理，slack-morphism 的 Socket Mode 不支持。
- 自己接要做的：
  - Socket Mode 客户端约 300 行：取地址、经代理连 wss、收 `hello`、`events_api`、`disconnect`，回执，ping，退避重连。
  - 4 个 Web API 调用约 150 行：用 `serde_json::Value` 只取要的字段；`ok: false` 转成错误码；429 照 `Retry-After` 等。
  - 测试：本机起假的 HTTP 和 WebSocket 服务器（tungstenite 自带服务端），用环境变量把地址指过去，测回执、重连、限频、分段发送。
  - 合起来是一件中等大小的活（第 8 节的 R38e）。

## 6. 凭证

- **放哪**：`~/.pony/secrets/slack.toml`，一家一个文件，键由适配器声明：

  ```toml
  app_token = "xapp-…"   # Socket Mode 用
  bot_token = "xoxb-…"   # 调 Web API 用
  ```

- **权限**：`~/.pony` 和 `secrets/` 是 0700，文件 0600，属于当前用户，是普通文件。不是就不连，报 `secrets_too_open`，像 ssh 对私钥那样：
  - 拒绝符号链接，免得凭证实际放在某个 dotfiles 仓库里；
  - 每次连之前都查一遍。
- **不进仓库**：
  - 凭证只在 `~/.pony`；仓库里只出现这个路径和假 token。
  - 测试用隔离的 `PONY_HOME` 和假 token。假 token 写成 `xoxb-test` 这种不像真 token 的样子，免得推送前跑的 gitleaks 当成泄露。
  - 推送前照 AGENTS.md 跑 gitleaks、trufflehog，它们的默认规则认得 Slack 的 token。
- **不进给 paddock 的输出**：`pony status` 不读凭证内容，只报 `secrets_missing`、`secrets_too_open`、`secrets_invalid`。凭证文件解析出错时，只报行号，不带出错那一行的原文（TOML 解析器的报错默认会带）。
- **不进日志**：
  - 不记 HTTP 头、请求体；
  - 不记 `apps.connections.open` 返回的 wss 地址（带 ticket）；
  - Slack 的错误只记 `error` 码。
- **谁还读得到**：
  - 本机同一用户的程序都能读这个文件，包括没套墙的 agent。这和 `~/.ssh`、`~/.fence/keys` 一样。
  - 套了 fence `dev` 墙的 agent 读不了 HOME，所以读不到。
- **被偷了能做什么**：
  - 拿到 bot token：能以机器人身份在它所在的频道发消息、读历史。
  - 拿到 app token：能开 Socket Mode 连接分走一部分事件，也就是读到用户发的话。
  - 都**不能**冒充用户给 agent 下命令：pony 只认 Slack 报的发话人是 `owner`，那是用户本人的账号。
  - 发现泄露：在 Slack 应用设置里重新生成 token，换掉文件，pony 下次重连自动用新的。
- **不选钥匙串**：macOS 钥匙串按程序给访问权。pony 每次部署都换一个版本目录里的新程序，升级后多半要重新弹框授权；弹框时人不在电脑前，pony 就卡在那里。文件加权限检查，和 fence 存 key 的做法一致。

## 7. 对 `docs/DESIGN.md` 的改动建议（不在这次改）

- §1 表格加一行：`pony（聊天软件机器人，命令 pony，每台机器一份、后台常驻）`｜新做（R38）｜第一期，见 §10。
- §2 全局位置加 `~/.pony/`、`~/.local/bin/pony`、`~/Library/LaunchAgents/local.ranch.pony.plist`；前端同样只通过命令（`pony status`）用它。
- §3 安装：
  - 版本目录加 `bin/pony`。
  - 部署多切一个链接 `~/.local/bin/pony`。
  - 装、换常驻服务（`pony service install`、`launchctl kickstart -k`）是用户在场时的部署操作。
  - 没有 `~/.pony/config.toml` 的机器什么都不连。
- §5 加一条：用户 10-20、10-21 把范围从「到 dispatch 为止」扩到聊天软件机器人（R38，放进 ranch、插件式、分两期）。
- 新加 §10「聊天软件机器人 pony（R38）」，要点：
  - 来历和用户原话；
  - 独立的包和命令，只调 corral 公开命令，Cargo 不依赖 corral，能整块拆出去（同 §8）；
  - 插件式：适配器接口各项、登记表 `CHATS`、共用核心不写任何一家的名字，接第二家只加一个文件和一行；
  - 第一期的规矩（频道等于仓库、没绑不发、只认 owner、只开一台）；
  - 路由、排队、提醒和去重、分段、重启后还在什么；
  - 常驻方式和状态、凭证、日志的位置；
  - 给 paddock 的 `pony status`；
  - 第二期（开停 agent）另开。
- 顺带：`AGENTS.md`、`CLAUDE.md` 开头「现在有 corral；以后还有…」可以加上 pony。

## 8. 实现怎么拆

| 件 | 做什么 | 依赖 | 对 paddock |
| --- | --- | --- | --- |
| R38b 定文档 | 照第 7 节改 DESIGN，CONTRACT 加 `pony status`（照第 4 节）。只改文档 | 用户已定（10-21） | 合并后 paddock 就可以照 CONTRACT 开 P5-147（用户 10-21 定） |
| R38c 包和外壳 | 建 `crates/pony`；`--version`、`status`、`run` 外壳；配置和凭证的读取、校验、权限检查；单实例锁、状态文件、日志轮转；打包工具把 `bin/pony` 放进版本目录。在隔离的 `PONY_HOME` 里测 status 的各种情况（没配置、配置错、凭证太宽、在跑、没在跑） | R38b | 部署后 `pony status` 是真命令，没配置时报 `missing` |
| R38d 共用核心 | 路由、线程表、送话和排队、盯 agent 和提醒、去重、分段、`?`、重启恢复。用内存假适配器和假 corral 测验收 1–5 的逻辑：没绑仓库的 agent 连 `status` 都不读、别人的消息不理、跨仓库被拒、分段不丢字。加「核心源码不出现适配器名」的测试 | R38c（接口在 R38c 定好） | 无 |
| R38e Slack 适配器 | Socket Mode（代理、回执、重连、`hello` 里的连接数）、4 个 Web API、事件翻译、markdown 块、限频。用本机假 HTTP 和 WebSocket 服务器测 | R38c；和 R38d 可以同时做 | 无 |
| R38f 常驻和上线 | `pony service install\|uninstall`；部署说明。用户在场时：用 manifest 建 Slack 应用、生成两个 token、写 `~/.pony`、建私有频道并邀请机器人、装服务。实测验收 1–6，实测 markdown 块里 `<!channel>` 和长回复的样子，实测要不要代理；核对 Slack 有没有「电脑上不活跃时才推到手机」这类通知设置（用户 10-21） | R38d、R38e | paddock 拿到真实数据，验收 7 |

- 每件照 AGENTS.md 跑 `cargo test --all-targets`、`clippy -D warnings`、`fmt --check`。
- 测试不连 Slack、不碰真实 agent：corral 那层用内存假实现，或用工作区编出的 corral 和 `/bin/cat` 这类合成程序，HOME、`CORRAL_HOME`、`PONY_HOME` 都隔离。

## 9. 对 paddock 的影响

- 只是兼容新增：新命令 `pony status`。corral 的命令、JSON、`~/.corral` 都不变，paddock 现有功能不用动。
- pony 送进去的话在 corral 里是 `last_input_source: "send"`，和 paddock 自己送的一样。主控的回复会同时出现在 paddock 收件箱和 Slack 里。
- paddock 的 Loop 和 pony 都可能给主控送话：两边都遵守 `not_idle`、`human_active`，不会硬送。

## 10. 第一期不做的

- 在聊天里开、停 agent（第二期）。
- 私聊机器人、斜杠命令、按钮。
- 附件、图片。
- 内容过滤（已定）。
- 多个用户（已定）。
- 多台机器（已定）。
- 改消息后跟着改。
- 在 Slack 里批准权限框（R45 只用 Esc 拒绝及关闭提问框后送文字回答）。
- 判断用户在不在电脑前、据此少发提醒（用户 10-21：以后嫌吵再改）。

## 出处（都在 2026-10-21 查）

Slack：

- Socket Mode（app 级 token、10 条连接、事件交给任意一条、回执、`disconnect` 的几种原因、每几小时刷新、`hello` 的字段、不能上架 Marketplace）：https://docs.slack.dev/apis/events-api/using-socket-mode
- `apps.connections.open`（app 级 token 放 `Authorization` 头）：https://docs.slack.dev/reference/methods/apps.connections.open
- `connections:write`：https://docs.slack.dev/reference/scopes/connections.write
- Events API（3 秒内答复、最多重发 3 次、`event_id` 唯一）：https://docs.slack.dev/apis/events-api/
- `message.channels`（要 `channels:history`）：https://docs.slack.dev/reference/events/message.channels
- message 事件的子类型、`hidden`、`message.groups` 要 `groups:history`：https://docs.slack.dev/reference/events/message
- `message_deleted`（`deleted_ts`）：https://docs.slack.dev/reference/events/message/message_deleted
- `app_mention` 和 `app_mentions:read`：https://docs.slack.dev/reference/events/app_mention ，https://docs.slack.dev/reference/scopes/app_mentions.read
- 用 `thread_ts` 和 `ts` 认线程：https://docs.slack.dev/messaging/retrieving-messages
- `chat.postMessage`（`chat:write`、`text` 建议不超过 4,000 字、超过 40,000 字截断、`markdown_text` 12,000 字、`thread_ts`、每频道每秒 1 条、关链接预览）：https://docs.slack.dev/reference/methods/chat.postMessage
- markdown 块（一次 12,000 字，认代码块、表格等）：https://docs.slack.dev/reference/block-kit/blocks/markdown-block
- `conversations.info`（`channels:read`、`groups:read`）：https://docs.slack.dev/reference/methods/conversations.info
- `auth.test`（不要权限，返回 `user_id`、`bot_id`）：https://docs.slack.dev/reference/methods/auth.test
- 限频（429、`Retry-After`）：https://docs.slack.dev/apis/web-api/rate-limits
- 应用 manifest：https://docs.slack.dev/reference/app-manifest
- 通知默认只推私信、@ 和关键词，可以按频道改成所有新消息：https://slack.com/help/articles/201355156-Configure-your-Slack-notifications

Rust 库：

- crates.io 接口：
  - https://crates.io/api/v1/crates/slack-morphism （版本、日期、下载量）
  - https://crates.io/api/v1/crates/slack-morphism/2.31.0/dependencies （依赖、特性）
  - https://crates.io/api/v1/crates/tungstenite
  - https://crates.io/api/v1/crates/slack-api
  - https://crates.io/api/v1/crates/slack-rust
  - https://crates.io/api/v1/crates/slack-rs
- GitHub：
  - https://github.com/abdolence/slack-morphism-rust
  - https://github.com/snapview/tungstenite-rs
  - 星数、issue 数、贡献者来自 `api.github.com/repos/<仓库>`。
- slack-morphism 的说明：https://slack-rust.abdolence.dev/
- slack-morphism 的 Socket Mode 直连、错误日志带地址：https://github.com/abdolence/slack-morphism-rust/blob/v2.31.0/src/hyper_tokio/socket_mode/tungstenite_wss_client.rs
- 依赖个数：在草稿目录各建一个只有依赖的空工程（不碰仓库），对 `aarch64-apple-darwin` 跑 `cargo tree -e normal`，和 ranch 工作区现有的依赖比。
- ureq 的代理：https://docs.rs/ureq/3.4.2/ureq/struct.Proxy.html
- tungstenite 在已连好的流上做 TLS 和握手：https://docs.rs/tungstenite/0.30.0/tungstenite/fn.client_tls_with_config.html

飞书（只用来检验接口）：

- 长连接接收事件（不需公网地址、3 秒内处理、集群模式随机一个连接收到、每个应用最多 50 条连接）：https://open.larkoffice.com/document/server-side-sdk/java-sdk-guide/handle-events
