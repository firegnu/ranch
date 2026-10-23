# 任务：pony 后续想法（R38 上线后的讨论记录和先后）

2026-10-21，ranch/main 起草；10-22 按先后重排（用户：「关于 R38任务书中的列的东西，你能否整理一下这个任务书，按照你的理解来排序，先开发什么再开发什么。」）；10-23 按「做完了、还没做、只是想法」重新分开（用户：「将已经做过的和还没做的（想法也包含）要分开，哪些做完了，哪些还没做，哪些现在只是想法」）。
类型：其他
依据：R38 第一期上线后的后续清单。本文件只记录和分类，不设计、不实现；哪一件要做，开工前单独写任务书，给用户看过再派。条目编号沿用 10-22 的排序，方便对照旧记录。

## 用户原话

- 10-21：「现在的安装测试，是ranch的安装直接就可以安装了吗？」
- 10-21：「现在加一个bot我感觉有点复杂」
- 10-21：「当时咱们的决策是一个channel一个主控，如果要把主控接到一个chananl中会有什么问题？」
- 10-21：「也就是说，现在主控的名字匹配是写死的main？」
- 10-21：「现在我这种的有成熟的产品或者开源的东西吗？」
- 10-21：「先不出设计，先把咱们讨论的我需要记录下来作为一个draft task。」
- 10-21：「现在比较一下你刚才发的同类产品，还有没有可以做的。也补充到这个文档里面」
- 10-21：「另外还有一个问题，加进去：1. 接入其他的社交软件 2. 现在加入一个bot很麻烦，你能说清步骤吗？以你为例子。」
- 10-22：「关于 R38任务书中的列的东西，你能否整理一下这个任务书，按照你的理解来排序，先开发什么再开发什么。」
- 10-23：「我的意思是，即使R38和R39还有待做的任务或者想法，但是你要收拾一下这两个任务文件，将已经做过的和还没做的（想法也包含）要分开，哪些做完了，哪些还没做，哪些现在只是想法。」

## 做完了

| 条目 | 做了什么 | 哪几件活 | 上线 |
|---|---|---|---|
| 再绑一个仓库太麻烦（原 2 节） | 私有频道里说 `@pony 绑 <仓库>` 就写进配置；「已绑」那条线程里不带 @ 也送给主控 | R40a、R40a2 | 10-22 |
| 通知 paddock 接 P5-147（原 6.1） | paddock Settings 的 Chat 页（paddock 提交 `0df5288`） | paddock 自己做 | — |
| 2 HANDOFF 瘦身（原 6.10） | 约 3.1 万字减到约 2 万字；10-22 起 HANDOFF 只记四类事实 | 主控做 | 10-22 |
| 3 一条命令部署（原 1 节、6.3、6.9） | `ranch deploy <版本>`：先列计划，确认后切链接、按需重装技能和插件、自动重启 pony，打印退回办法 | R41、R41b、R41c | 10-22 |
| 5 在手机上回答确认框（原 6.2、7.1） | Needs you 提醒带题目；提问框在 Slack 里给选项按钮（单选、多选），打字回答时附选项对照；权限框给「拒绝」；Claude Code 的 Bash 权限框、计划批准给「允许这一次」（带确认框、点之前核对、只按一次，确认框显示 corral 记下的原样命令） | R42 设计、R42t 实测、R43、R44、R45、R46、R47a、R47b、R47c、R47d | 10-22–10-23，真机都测过 |
| 6 从手机打断当前这一轮（原 7.3） | 线程里整句「停」「打断」按 Esc；`@pony ?` 和 pony 送话后的线程里给「打断」按钮 | R45、R47a | 10-22–10-23 |
| 7 `@pony ?` 多给点信息（原 6.5、7.5） | 列频道仓库全部 agent 的状态和持续时间，本仓库的 agent 加标题、最后的工具、最后一句回复 | R44 | 10-22 |

## 还没做（定了要做，或者范围清楚、等触发条件）

- **9 第二期：在聊天里开、停 agent**（DESIGN §10 已定的第二期，原 4 节、7.8；用户 10-20「同意，分两期」）：开的时候选家、模型、强度，不给跳过权限确认。范围大，先出设计。**触发条件**：用户点名。
- **11 新机器一条命令装好**（`R40b-pony-setup新机器一条命令装好.md`，草稿未提交；原 7.9 配对码认领 owner）：**触发条件**：要给另一台机器或另一个 Slack 工作区装 pony。
- **17 在频道顶层回答提问时给提示**（10-23 实测 R46 时发现）：用户连着五次在频道底部输入框直接回答提问提醒，没进提醒的线程；不带 @ 的那几条 pony 照规矩不理、也不回话，用户不知道哪里不对。R47b 上线后，有选项按钮的提问直接点提醒上的按钮就行，不管线程；这一条只管还得打字回答的时候：题目或选项太长没给按钮、想答选项以外的话、不是 Claude Code 的 agent 在问。想法：有提问没答时，频道顶层不带 @ 的话，pony 回一句「要答 <agent> 的题，请点那条提醒的『回复』，在线程里写」，不按键、不送话。不做「顶层的话直接当回答」：顶层带 @ 的话本来是给主控的，猜错会按 Esc 关掉提问框。**触发条件**：选项按钮用了一阵之后，打字回答仍常落到顶层。用户 10-23：「好的，记进 R39」；按钮能在顶层点、这条只管打字回答，用户 10-23 同意这样写。低档小活。
- **13 `~/.pony/logs/launchd.log` 不自动轮转**（原 6.7）：现在只有几行，长期要清。顺手做。
- **14 审查留下的「可以不改」「建议改」**（原 6.8）：集中看一次，挑几条修。清单见下面「附：审查留下的小尾巴」。顺手或攒一起做。
- **15 偶发失败的 corral 测试**（原 6.11）：`raw_v2_events_keep_original_input_reply_and_retry_grace_with_v1` 失败过一次，原因没查；再出现就查。
- **16 没实测的项目**（原 5 节、6.6）：排队和撤回（让 agent 前台跑 `ping -c 100 127.0.0.1` 忙起来再发话）、合盖或断网后的补收、长回复分段、`@here` 不误伤、勾「同时发送到频道」的回复。用户 10-22：「测试先放后面，现在的够我用了，之后碰到问题再修」。不专门安排，用的时候碰到问题再开修的活。（Needs you 提醒、按钮、提问选项 10-22–10-23 已真机测过。）
- **上线后留意**（R47c、R47d 审查提的，不是待办）：批准后 pony 会不会常报「没看到反应」（10-23 第一次实测报的是「已按」）；长路径的持久项折行、命令缩进超过 4 个空格时不给「允许」，属保守方向；计划正文太长时计划批准按钮很少出现。

## 只是想法（没定做不做，或者要先出设计、用户先定规则）

- **4 提醒别太吵**（原 3 节「ranch 主控接 Slack」、6.4、7.4）：现在主控每一轮回复都 @ 用户、手机都响。用户 10-21 说过「先这么定，如果以后嫌吵，咱们再改」。真把 `ranch/main` 这类每几分钟一轮的主控绑进来之前，要先定规则，比如：只在 Needs you 和主控回复里带问题时 @；或者单个 agent、单个频道可以静音。规则由用户定，实现不大。**触发条件**：用户嫌吵，或者要绑 ranch/main。用户 10-22：「第四条往后放，暂时我不觉得吵」。（注：#ranch 已经绑了 ranch，10-23 实测时主控每轮都发，提醒容易被顶上去。）
- **8 收到的附件存到本机**（原 7.2）：存下来、把路径写进给 agent 的话。10-23 主控估过：和 R47b 差不多大；Slack 要加 `files:read` 权限、重装应用；先出设计定存哪（套墙的 agent 默认读不到 `~/.pony`）、大小上限、多久清、收哪些类型。用户 10-23：「这个先不做了」。
- **10 一个频道绑几个仓库、主控名可配**（原 3 节）：现在一频道一仓库、主控固定叫 `<仓库>/main`。要改设计和配置；没有具体需要之前不动。
- **12 接第二家聊天软件**（原 8 节）：结构已经留好，用户点名哪一家再做；工作量约等于 R38e 加 R38f 的上线实测。候选见附「接入其他聊天软件」。
- **主控不建议现在做的**：
  - 斜杠命令和弹窗（原 7.6）：要改 Slack 应用清单，现在靠点名、线程和按钮够用。
  - 一轮中间的进度（原 7.7）：会吵，先要定显示哪些。
  - 让别人发话（原 7.10）：要先改「只认 owner」的设计。
  - 文件发回 Slack（原 7.11）：涉及隐私，要先出设计。
  - 归档不动的频道（原 7.12）、多台机器、语音、手表上批准（原 7.13）。

## 有意不做的（记下，不当待办）

- 从 Slack 开的会话默认跳过权限确认。
- 「永远允许」「这次会话都允许」「之后自动接受修改」「跳过权限」一类按钮（用户 10-22）。
- 本机 HTTP 自动化接口：pony 的公开约定只有命令和 `pony status`。
- 绑定某一家 agent 的做法：pony 走 corral，不挑家。

## 不归 pony、等别人定的老事项

- R17c（5090 上验 Linux sandbox）暂缓；paddock 的 P5-141（主控卡片 sandbox 勾）、R35 设置页连线面板由 paddock 接。

## 附：讨论记录

### 安装和升级（原 1 节）

- ranch 部署只放好 `pony` 程序、切 `~/.local/bin/pony` 链接；第一次在一台机器上用，还要用户在场建 Slack 应用、写 `~/.pony`、`pony service install --yes`（本机 10-21 已做完）。
- 每次升级切链接之后还要跑 `launchctl kickstart -k "gui/$(id -u)/local.ranch.pony"`，后台的 pony 才换到新程序；plist 定义变了要 `pony service install --yes`。

### 把一个仓库的主控接进 Slack（原 2 节，10-22 按 R40a、R40a2 更新）

本机已有 pony 应用和服务，Slack 应用、凭证、服务都不用动：

1. Slack 里建一个私有频道，比如 `#ranch`。
2. 在这个频道里发 `/invite @pony`。
3. 发 `@pony 绑 ranch`。pony 在线程里回「已绑 ranch」和主控状态；之后在这条线程里不带 @ 回话，或者在频道顶层 @pony 说话，都送给 `ranch/main`。公开频道会被拒绝。

接 ranch/main 之前先看第二档第 4 条：它很多轮是派出去的 agent 做完时的提醒触发的，每轮都 @ 用户，手机会经常响。

换一台机器或别的工作区，从头装要十来步，见 `crates/pony/README.md`「部署与退回」第 1–4 步（建应用、拿两枚 token、写 `~/.pony/secrets/slack.toml`、复制成员 ID 写 `owner`、装服务）；R40b 草稿想把它缩成 `pony setup` 一条命令。

### 几个主控接进同一个频道 / 主控名字（原 3 节）

- 现在：一个频道只绑一个仓库，一个仓库只绑一个频道（提醒发到哪才唯一）。
- 主控认的是 `<仓库>/main`，`main` 这个短名在共用核心里写死两处：频道顶层不点名时送给 `<仓库>/main`，名字以 `/main` 结尾的 agent 每一轮新回复都提醒。这是 corral、派活、paddock 共用的约定；用户 10-21 听完说「我知道了」。
- 几个仓库绑进同一个频道会碰到的：不点名时送给谁说不清（要么每句写全名，要么定一个默认仓库）；几个主控的回复和 Needs you 挤在一个频道，吵，也没法只静音某个仓库；验收 4「跨仓库 @ 被拒」要改；要改设计和配置。「一个仓库只绑一个频道」可以保留。
- 想法：配置里给频道加一项「主控是谁」，不写默认 `main`。

### 现成的同类产品（原 4 节，10-21 搜索结果，未逐个试用）

- 官方：Claude Code Remote Control（手机 Claude App 接着用本机会话，只管 Claude Code）；Claude in Slack / Claude Tag（在 Slack 里开的是云端会话）；Claude Code Channels（Telegram、Discord，暂不支持 Slack）。
- 第三方手机 App：Happy（只管 Claude Code，完成或出错推送）、Omnara（Claude Code 和 Codex，手机上能批准权限请求）。
- 开源 Slack 桥：SlackAgentBridge（macOS、launchd，每个会话一个私有频道，默认跳过权限确认）、herdr-slackbot（做完或卡住时私信，按钮批准权限请求）、OpenACP（Telegram、Discord、Slack，带批准、拒绝按钮），另有 remote-agentic-coding-system、mpociot/claude-code-slack-bot、Crewly。
- pony 有的：建在 corral 上，不挑 agent；按仓库分频道；主控每轮提醒；忙时排队、删消息撤回、离线补收；不从 Slack 开 agent，不跳过权限确认；凭证只在本机；和 paddock 共用 corral。
- 别人有、pony 还没做的：手机上批准权限请求（第三档第 5 条）、从手机开停 agent（第四档第 9 条），以及原 7 节各条（已排进上面各档）。
- 出处：
  - [herdr-slackbot](https://github.com/daCapricorn/herdr-slackbot)
  - [SlackAgentBridge](https://github.com/SergioTCG/SlackAgentBridge)
  - [DEV：用 Telegram、Discord、Slack 控制 Claude Code](https://dev.to/tigergethigher/how-to-control-claude-code-from-telegram-discord-or-slack-self-hosted-open-source-1jk8)
  - [mpociot/claude-code-slack-bot](https://github.com/mpociot/claude-code-slack-bot)
  - [Crewly](https://crewlyai.com/en/blog/claude-code-slack-bot)
  - [remote-agentic-coding-system](https://github.com/coleam00/remote-agentic-coding-system)
  - [MindStudio：Claude Code Channels 对比](https://www.mindstudio.ai/blog/claude-code-channels-vs-openclaw-mobile-agent-control)
  - [Anthropic：Use Claude in Slack](https://support.claude.com/en/articles/12461605-use-claude-in-slack?lang=us)
  - [TechCrunch：Claude Code in Slack](https://techcrunch.com/?p=3073830)
  - [Omnara vs Happy vs SeaWork](https://seawork.ai/en/blogs/omnara-vs-happy-vs-seawork/)
  - [Omnara（App Store）](https://apps.apple.com/app/id6748426727)
  - [TechRadar：Remote Control](https://www.techradar.com/pro/anthropic-reveals-remote-control-a-mobile-version-of-claude-code-to-keep-you-productive-on-the-move)
  - [从手机新开 Claude Code 会话](https://pasqualepillitteri.it/en/news/12249/claude-code-start-session-from-phone)
  - [Happy（Korben）](https://korben.info/en/happy-claude-code-mobile-assistant.html)
  - [Agent Client Protocol 客户端列表（OpenACP 一条）](https://agentclientprotocol.com/get-started/clients)

### 接入其他聊天软件（原 8 节）

- 结构上已经留好：每家一个适配器文件 `chats/<名字>.rs`，在登记表 `CHATS` 加一行；用户在配置里加这家的段，凭证放 `secrets/<名字>.toml`。共用核心不改，有测试守着核心源码里不出现任何一家的名字。
- 每家要做的和 Slack 一样：连接和重连、把消息翻成共用格式（@ 的写法、转义、线程、附件、删除）、发消息和分段上限、限频重试、怎么让手机响、频道信息给 `pony status`。
- 选哪家看三点：能不能只往外连（不要公网地址）、有没有线程、机器人怎么建。主控印象，没逐个核对文档：
  - 飞书：长连接收事件（往外连 WebSocket），有话题；设计时拿它检验过接口，是最顺的第二家，国内直连。
  - 钉钉：Stream 模式（也是往外连）；线程、@ 写法没查。
  - Telegram：BotFather 给一个 token，最简单；长轮询往外连。群里要开话题才有线程，私聊没有线程，核心要退成「每句写明 agent」。
  - Discord：Gateway WebSocket 往外连，有线程；读消息正文要开 Message Content 权限。
  - 企业微信、微信：没查。
- 要先定的：先接哪一家；同一个仓库能不能同时绑在两家、提醒发一家还是两家；凭证规矩照 Slack，只放本机。

### 附：审查留下的小尾巴（原 6.8）

- R38c 审查「可以不改」：7（权限必须正好 0700/0600，更严的 0400 也报错）、9（两处出错路径不写 `status.json`）、10（写到一半被杀留下的 `run/.status.*.tmp`）、11（`repo` 没排除 corral 保留名）。
- R38c 复核「可以不改」1：配置里像 token 的裸键名会被写进 `config_message`。
- R38d2 审查 29（频繁重连时核对不了的那句一直挂着）。
- R38e 审查 8、10、11、12。
- R38f 审查「可以不改」1–3：中途被杀留下 `.pony-*.tmp`；手删 plist 后服务仍在跑；没设 `ProcessType`。
- R40a 审查「可以不改」4（多行数组里新加项的排版）；R40a2 审查「可以不改」2（在过期线程里绑定）。
- R44、R45 没采纳的改进：R44 `Questions` 字段约束；R45 pony 重启后回答前缀、排队文案不准确。R41c 服务重装被 Ctrl-C 打断后不自动装回，见 HANDOFF。
- R47c 复核「可以不改」4（Bash 有原始命令比对后，整窗计数可以只留给计划框）、5（Needs you 正文的屏幕末尾仍吞空格，可以加一行原始命令）。
- R47d 审查「建议改」1（补可选提示行在命令里、问题行后等位置的反向用例）、「可以不改」4（DESIGN、README 里提示行的位置写得不够准）。

