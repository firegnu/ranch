# 任务：pony 后续想法（R38 上线后的讨论记录）

**草稿**：只记下 10-21 上线后和用户讨论到的事，还没定做哪几件、怎么做。用户 10-21：「先不出设计，先把咱们讨论的我需要记录下来作为一个draft task。」重开前要定：做哪几条、先后顺序、哪些要先出设计。

2026-10-21，ranch/main 起草，待用户看过。
类型：其他
依据：R38c–R38h 已上线（`versions/2513b3a`），本机 `#paddock` 绑 paddock 仓库，验收 1–4、6 实测通过。下面是上线实测和讨论中冒出来的问题和想法，本文件只记录，不设计、不实现。

## 用户原话

- 10-21：「现在的安装测试，是ranch的安装直接就可以安装了吗？」
- 10-21：「现在加一个bot我感觉有点复杂」
- 10-21：「当时咱们的决策是一个channel一个主控，如果要把主控接到一个chananl中会有什么问题？」
- 10-21：「也就是说，现在主控的名字匹配是写死的main？」
- 10-21：「现在我这种的有成熟的产品或者开源的东西吗？」
- 10-21：「先不出设计，先把咱们讨论的我需要记录下来作为一个draft task。」
- 10-21：「现在比较一下你刚才发的同类产品，还有没有可以做的。也补充到这个文档里面」
- 10-21：「另外还有一个问题，加进去：1. 接入其他的社交软件 2. 现在加入一个bot很麻烦，你能说清步骤吗？以你为例子。」

## 讨论到的事

### 1. 安装和升级

- 现在 ranch 部署只放好 `pony` 程序、切 `~/.local/bin/pony` 链接；第一次在一台机器上用，还要用户在场建 Slack 应用、写 `~/.pony`、`pony service install --yes`（本机 10-21 已做完）。
- 以后每次升级，切四个链接之后还要多跑一行 `launchctl kickstart -k "gui/$(id -u)/local.ranch.pony"`，后台的 pony 才换到新程序；plist 定义变了要 `pony service install --yes`。
- 想法：把这一步并进 ranch 的部署步骤，部署时顺手重启 pony，用户不用另记。

### 2. 「加一个 bot 有点复杂」

用户指的是哪一种还没问清，三种麻烦程度不同：

- 再绑一个仓库：建私有频道、`/invite @pony`、复制频道 ID、`config.toml` 加一行（不用重启）。想法：配置里允许写频道名，pony 用已有的 `channels:read`、`groups:read` 查出 ID。
- 在别的机器或工作区再建一个 Slack 应用：Slack 的规矩，绕不开；想法是把清单写顺，或者加一个 `pony` 命令帮着建目录、查配置和权限。
- 接另一个聊天软件（飞书等）：见第 8 节。

#### 步骤：把 ranch/main 接进 Slack（用户 10-21：「现在加入一个bot很麻烦，你能说清步骤吗？以你为例子。」）

本机已有 pony 应用和服务，只是再绑一个仓库，Slack 应用、凭证、服务都不用动：

1. Slack 里建一个私有频道，比如 `#ranch`。
2. 在这个频道里发 `/invite @pony`。
3. 拿频道 ID：点频道顶上的名字，「关于」页最下面有频道 ID，点复制。
4. 改 `~/.pony/config.toml`，在 `[slack]` 的 `channels` 里加一行 `{ id = "<刚复制的频道 ID>", repo = "ranch" }`。存盘就行：pony 每轮看配置有没有变，变了自己重读、重连 Slack，不用 kickstart。写错了它继续用旧配置，`pony status` 报 `config: "invalid"` 和原因。
5. 核对：`pony status` 里多出 ranch 这一项，`name` 是 `ranch`、`problem` 是 `null`（`not_member` 就是第 2 步没做）；在频道里发 `@pony ?`，应该列出 `main — 空闲` 或 `干活中`。
6. 在频道顶层直接说话，就送给 `ranch/main`；回复贴在那条消息的线程里，每一轮都 @ 你。

ranch/main 这边什么都不用改：pony 认的是名字 `ranch/main`，在 corral 里跑着就行。接之前要先想好第 3 节说的提醒频率：ranch/main 很多轮是派出去的 agent 做完时的 `--after` 提醒触发的，每轮都 @ 你，手机会经常响。

换一台机器或别的工作区，从头装要十来步，都在 `crates/pony/README.md`「部署与退回」第 1–4 步：

1. 在 Slack 网页上用清单（设计稿 3.3）新建应用，开 Socket Mode；
2. 装到工作区，拿 bot token；
3. 在 Basic Information 新建带 `connections:write` 的 app token；
4. 在本机写 `~/.pony/secrets/slack.toml`（两枚凭证），目录 0700、文件 0600；
5. 在 Slack 个人资料里复制自己的成员 ID；
6. 写 `~/.pony/config.toml`：`owner`，再照上面 1–4 步建频道、邀请、复制 ID、加绑定；
7. 连不上 Slack 再填 `proxy`；
8. ranch 部署切 `~/.local/bin/pony` 链接；
9. `pony service install --dry-run`，再 `--yes`；
10. `pony status` 核对。

麻烦主要在三处：要到 Slack 界面里复制 ID（频道 ID、成员 ID）；要手改 TOML；第一次建应用的几页设置。能减步骤的想法：配置里写频道名（上面一条）；在频道里对 pony 说「绑 ranch」由它自己写配置（新想法，要改设计：现在配置只由用户手写）；配对码认领 owner（7.9）；一个 `pony` 命令帮着建目录、查配置和权限（上面一条）。

### 3. 几个主控接进同一个频道 / 主控名字

- 现在：一个频道只绑一个仓库，一个仓库只绑一个频道（提醒发到哪才唯一）。
- 主控认的是 `<仓库>/main`，`main` 这个短名在共用核心里写死两处：频道顶层不点名时送给 `<仓库>/main`（`core/engine.rs` 拼名字处），名字以 `/main` 结尾的 agent 每一轮新回复都提醒（同文件判断新回复处）。这是 corral、派活、paddock 共用的约定；用户 10-21 听完说「我知道了」。
- 几个仓库绑进同一个频道会碰到的：
  - 不点名时送给谁说不清：要么每句写全名，要么定一个默认仓库。
  - 几个主控每轮回复和所有 Needs you 挤在一个频道，按线程回复仍送得对，但吵，也没法只静音某个仓库。
  - 验收 4「跨仓库 @ 被拒」要改成「这个频道绑了的仓库都能点名」。
  - 要改设计和配置（允许一个频道绑多个仓库）；「一个仓库只绑一个频道」可以保留，提醒去向仍唯一。
- 把 ranch 主控（ranch/main）接进 Slack 的特别问题：它很多轮是开发 agent 的 `--after` 提醒触发的，每轮结束都会 @ 用户，手机会响得很频繁；要接先定哪些回复值得推。
- 想法：配置里给频道加一项「主控是谁」，不写默认 `main`。

### 4. 现成的同类产品（10-21 搜索结果，未逐个试用）

- 官方：
  - Claude Code Remote Control（2026-02 起，手机 Claude App 接着用本机会话，08 月起可从手机新开会话；只管 Claude Code，终端关了就不行）。
  - Claude in Slack / Claude Tag（在 Slack 里开的是云端会话，不是本机 agent）。
  - Claude Code Channels（Telegram、Discord，暂不支持 Slack）。
- 第三方手机 App：Happy（只管 Claude Code，任务完成或出错推送）、Omnara（Claude Code 和 Codex，手机上能批准权限请求）。
- 开源 Slack 桥：
  - SlackAgentBridge（macOS、launchd，每个会话一个私有频道；会从 Slack 新开 agent，默认跳过权限确认）。
  - herdr-slackbot（Herdr 插件，Windows；agent 做完或卡住时私信，按钮批准权限请求，手机发话）。
  - OpenACP（基于 Agent Client Protocol，Telegram、Discord、Slack，带批准、拒绝按钮）。
  - 另有 remote-agentic-coding-system、mpociot/claude-code-slack-bot、Crewly。
- pony 和它们比：
  - pony 有的：建在 corral 上，不挑 agent（Claude Code、Codex、pi、omp），不用换启动方式；按仓库分频道；主控每轮提醒；忙时排队、删消息撤回、离线补收；不从 Slack 开 agent，不跳过权限确认；凭证只在本机；和 paddock 共用 corral。
  - 别人有、pony 第一期没做的：
    - 手机上批准权限请求：Needs you 现在只提醒，线程里回的话要等电脑上点完确认框才送（设计稿「用户定的」第 3 条，第一期有意不做）。主控认为这是最值得做的一条。
    - 从手机新开、停 agent：已定的第二期（DESIGN §10：「第二期另开任务：在聊天里开、停 agent」）。
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

### 5. 上线后还没实测的（见 HANDOFF 第 16 条和 `crates/pony/README.md` 上线实测清单）

- 排队和撤回（上次 Claude Code 把 `sleep` 放到后台、agent 没忙起来；下次让它前台跑 `ping -c 100 127.0.0.1`）。
- Needs you 提醒（等真碰上）。
- 验收 5：要第二个 Slack 账号，用户暂时略过。
- 合盖或断网后的补收、长回复显示、`@here` 等不误伤、勾「同时发送到频道」的回复。

### 6. 主控列的可以继续做、优化的（10-21，用户：「你觉得还有哪些地方可以继续做的或者优化的。也列一个列表给我」「加进 R39 草稿吧」）

按对用户的用处排，前面的更值得做。主控建议先做 6.1 和 6.9；6.2 价值最大，但要先出设计。

pony 本身：

1. 通知 paddock：pony 已上线、`pony status` 有真实数据，P5-147（验收 7：Settings 显示连没连上、绑了哪些频道）可以接。只是给 `paddock/main` 发一句话，不写代码。
2. 手机上批准权限请求（同上面第 4 节）。
3. 升级时自动重启 pony（同上面第 1 节）。
4. 提醒吵闹程度可调：现在主控每轮回复都 @ 用户、手机都响。设计稿「用户定的」第 1 条用户原话「先这么定，如果以后嫌吵，咱们再改」；用一阵嫌吵再改，比如只在 Needs you 时 @。
5. `@pony ?` 多给点信息：现在只有「main — 空闲」，可以加 agent 在做什么（标题或最后一句回复）。
6. 把没测的测完（同上面第 5 节），不用写代码。
7. `~/.pony/logs/launchd.log` 不自动轮转（README 写着「需要时先卸服务、归档日志，再重装」）；现在只有几行，长期要么定期清，要么让 pony 自己管。

审查留下、当时定「可以不改」的小尾巴：

8. 集中评估一次，挑几条修：
   - R38c 审查「可以不改」：7（权限必须正好 0700/0600，更严的 0400 也报错）、9（两处出错路径不写 `status.json`）、10（写到一半被杀留下的 `run/.status.*.tmp`）、11（`repo` 没排除 corral 保留名）。
   - R38c 复核「可以不改」1：配置里像 token 的裸键名会被写进 `config_message`。
   - R38d2 审查 29（频繁重连时核对不了的那句一直挂着）。
   - R38e 审查 8、10、11、12。
   - R38f 审查「可以不改」1–3：中途被杀留下 `.pony-*.tmp`；手删 plist 后服务仍在跑；没设 `ProcessType`。

ranch 整体：

9. 部署命令 `ranch deploy <版本>`：现在每次部署由主控写一串 `ln -sfn …` 交用户跑，有时还要重装技能、切共用 helper、kickstart pony，全靠 HANDOFF 里的经验。做成命令，先列要做什么、用户确认后一次做完、记下退回办法。
10. HANDOFF 瘦身：「安装」一节列着每一版的部署记录，留现装和上一版就够，更早的在 git 历史里。
11. 偶发失败的 corral 测试 `raw_v2_events_keep_original_input_reply_and_retry_grace_with_v1`：R38d2 那次失败过一次（`send … --timeout 2` 返回 `not_delivered`），之后没再出现，原因没查。

等别人或用户定的老事项（HANDOFF 里挂着）：

12. R17c（5090 上验 Linux sandbox）暂缓；paddock 的 P5-141（主控卡片 sandbox 勾）、R35 设置页连线面板由 paddock 接。

### 7. 对照同类产品还能做的（10-21，用户：「现在比较一下你刚才发的同类产品，还有没有可以做的。也补充到这个文档里面」）

只看了 README 和介绍页，没有试用。第 4 节已记的两条（手机上批准权限请求、第二期开停 agent）不重复。主控建议的先后：7.1（并进 6.2 一起设计）、7.2、7.3、7.4；其余等用一阵再看。

能直接用上的：

1. 按钮回答，发前重看屏幕（herdr-slackbot；SlackAgentBridge 类似）：权限请求、AskUserQuestion（单选、多选、自由填）、计划批准都给按钮；点了以后先重看一次屏幕，用户已在电脑上答过就不发。是 6.2 的具体做法参照；「发前重看屏幕」和主控送话前的核对（R37）是同一个思路。
2. 收到的附件存到本机、把路径写进给 agent 的话（SlackAgentBridge）。pony 现在只送文字，回「附件没送」。要定：存哪、大小上限、什么时候清。
3. 从手机打断当前这一轮，会话留着（SlackAgentBridge `/sab-stop`）。不同于第二期的「停 agent」，要单列；做法要用 corral 已有的命令，不改 corral 核心。
4. 单个 agent 静音（herdr-slackbot 的 🔕）。是 6.4「提醒吵闹程度可调」的一种做法。

可以考虑的：

5. 状态面板（两家都有）：Slack 的 App Home 页列出各 agent 和状态、带按钮；SlackAgentBridge 的状态还显示「在干活」和已用时间。和 6.5「`@pony ?` 多给点信息」重叠，可以一起定。
6. 斜杠命令和弹窗（两家都有）：弹窗里选 agent、看前面的对话、再发话；命令不带参数时弹出选择。pony 现在靠点名和线程。要在 Slack 应用里加命令和交互，改清单、重装应用。
7. 一轮中间的进度（OpenACP 实时显示工具调用和结果；只看到目录页的转述，它的 GitHub 页抓取时 404，没核实）。pony 只送每轮最后的回复。会吵，要先定显示哪些。
8. 从聊天里改模型、强度（SlackAgentBridge `/sab-model`、`/sab-effort`）；从手机新开时选模型、强度、权限模式，不给跳过权限（herdr-slackbot `/herdr new`）。并进第二期开停 agent 一起考虑。
9. 配对码认领（herdr-slackbot：电脑上显示 6 位码，在 Slack 里输入认领 owner）。pony 现在要在配置里手写 owner 的用户 ID；和第 2 节「加 bot 复杂」有关，能让第一次配置简单些。
10. 让别人发话（SlackAgentBridge：协作者的话带上名字送给 agent，但不能执行命令、不能批准）。pony 现在只认 owner，别人的话一律不理（验收 5）；要改得先改设计。
11. 文件发回 Slack（SlackAgentBridge：一次性上传许可，最多 10 个、100 MiB，只限工作区里的文件）。涉及隐私，要先出设计。

较远、主控不建议现在做的：

12. 归档不动的频道（SlackAgentBridge `/sab-cleanup`）：它每个会话一个频道才需要；pony 一个仓库一个频道，用处小。
13. 多台机器（SlackAgentBridge 预览中）、语音（Happy、Omnara；Slack 手机自带语音输入可能就够）、手表上批准（Omnara）。

pony 有意不做的（记下，不当待办）：

- 从 Slack 开的会话默认跳过权限确认（SlackAgentBridge 默认 `--dangerously-skip-permissions`、`--yolo`）。
- 本机 HTTP 自动化接口（SlackAgentBridge 的 127.0.0.1:8877）：pony 的公开约定只有命令和 `pony status`。
- 绑定某一家 agent 的做法：pony 走 corral，不挑家。

出处（另见第 4 节）：

- [SlackAgentBridge README](https://github.com/SergioTCG/SlackAgentBridge)
- [herdr-slackbot README](https://github.com/daCapricorn/herdr-slackbot)
- [Agent Client Protocol 客户端列表（OpenACP 一条）](https://agentclientprotocol.com/get-started/clients)
- [Omnara vs Happy vs SeaWork](https://seawork.ai/en/blogs/omnara-vs-happy-vs-seawork/)

### 8. 接入其他社交软件（用户 10-21：「接入其他的社交软件」）

- 结构上已经留好（设计稿第 2 节、DESIGN §10）：每家一个适配器文件 `chats/<名字>.rs`，在登记表 `CHATS` 加一行；用户在配置里加这家的段（如 `[feishu]`），凭证放 `secrets/<名字>.toml`。共用核心不改，有测试守着核心源码里不出现任何一家的名字。
- 每家要做的和 Slack 一样：连接和重连、把收到的消息翻成共用格式（@ 的写法、转义、线程、附件、删除）、发消息和分段上限、限频重试、「要提醒」时怎么让手机响、频道信息给 `pony status`。工作量大约是 R38e（Slack 适配器，碰要害，带交叉审查）加 R38f 那样的在场上线实测。
- 选哪家主要看三点：能不能只往外连（不要公网地址）、有没有线程、机器人怎么建。下面是主控印象，没逐个核对文档：
  - 飞书：有「长连接接收事件」，往外连 WebSocket，有话题；设计时拿它检验过接口，是最顺的第二家。国内网络直连，大概不用代理。
  - 钉钉：有 Stream 模式（也是往外连）；线程、@ 写法没查。
  - Telegram：BotFather 建机器人只给一个 token，最简单；长轮询收消息，也是往外连。群里线程要开话题（forum），私聊没有线程，核心要退成「每句写明 agent」（设计稿里留了这一项，第一期没用到）。
  - Discord：Gateway WebSocket 往外连，有线程；读消息正文要在开发者后台打开 Message Content 权限。
  - 企业微信、微信：没查。
- 要先定的：
  - 先接哪一家，用户点名。
  - 同一个仓库能不能同时绑在两家（比如 paddock 在 Slack 和飞书都有频道）：现在只规定了「同一家里」一个仓库只绑一个频道，跨家时提醒发一家还是两家都发，没定。
  - 别的平台的凭证规矩照 Slack：只放本机，不进仓库。


1. 先做哪几条：手机上批准权限请求、第二期开停 agent、部署时自动重启 pony、配置写频道名、一个频道绑多个仓库或主控名可配、ranch 主控接 Slack，接其他聊天软件（第 8 节，先接哪家），以及第 6、7 节各条。
2. 「加一个 bot 复杂」指的是哪一种。
3. 要做的条目里，哪些要先出设计稿给用户定。
