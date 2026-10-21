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

## 讨论到的事

### 1. 安装和升级

- 现在 ranch 部署只放好 `pony` 程序、切 `~/.local/bin/pony` 链接；第一次在一台机器上用，还要用户在场建 Slack 应用、写 `~/.pony`、`pony service install --yes`（本机 10-21 已做完）。
- 以后每次升级，切四个链接之后还要多跑一行 `launchctl kickstart -k "gui/$(id -u)/local.ranch.pony"`，后台的 pony 才换到新程序；plist 定义变了要 `pony service install --yes`。
- 想法：把这一步并进 ranch 的部署步骤，部署时顺手重启 pony，用户不用另记。

### 2. 「加一个 bot 有点复杂」

用户指的是哪一种还没问清，三种麻烦程度不同：

- 再绑一个仓库：建私有频道、`/invite @pony`、复制频道 ID、`config.toml` 加一行（不用重启）。想法：配置里允许写频道名，pony 用已有的 `channels:read`、`groups:read` 查出 ID。
- 在别的机器或工作区再建一个 Slack 应用：Slack 的规矩，绕不开；想法是把清单写顺，或者加一个 `pony` 命令帮着建目录、查配置和权限。
- 接另一个聊天软件（飞书等）：写新适配器，照 R38e 开发、交叉审查，再照 R38f 在场上线。

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

## 重开前要用户定的

1. 先做哪几条：手机上批准权限请求、第二期开停 agent、部署时自动重启 pony、配置写频道名、一个频道绑多个仓库或主控名可配、ranch 主控接 Slack。
2. 「加一个 bot 复杂」指的是哪一种。
3. 要做的条目里，哪些要先出设计稿给用户定。
