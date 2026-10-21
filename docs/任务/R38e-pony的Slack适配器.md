# 任务：R38e 做 pony 的 Slack 适配器

2026-10-21，ranch/main 交给 ranch/dev-r38e（Codex，gpt-6-astra / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.98）、要、碰要害；凭证和发到 Slack 的内容算安全和隐私，连接、回执、重连和限频算并发；按第 1 条读 `critical-other`，审查读 `review-claude`：Claude Code opus[1m] xhigh）
类型：功能变更
依据：R38 的实现第四件。照 R38d 定下的接口填 `chats/slack.rs`，让 pony 真正能连 Slack：Socket Mode 收事件、Web API 发消息和查频道、读历史。顺带修共用核心里 R38d2 审查留下的两处（27、28）。本轮不连真实的 Slack、不建 Slack 应用（R38f 用户在场时做），不做 `pony service`。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：「按你的建议，写 R38e 任务书吧。」（主控建议：R38d2 审查留下的 27、28 不单开任务，由 R38e 顺带改，范围只限这两处。）
- 10-21 用户定：Slack 自己用 ureq + tungstenite 接，不用 slack-morphism（设计稿「用户定的」第 4 条）。
- R38 验收第 6 条：「Slack 的凭证只放在本机，不进任何仓库。」
- 底线：「不要动ranch中corral的核心代码。」

## 先读

- `AGENTS.md`「规矩」：语言与依赖、测试不依赖真实 agent 和用户状态、验证、独立编译目录。
- `crates/pony/README.md`，特别是「适配器接口（给 R38e）」；`crates/pony/src/chats/mod.rs`、`chats/slack.rs`（现在是占位）。
- `docs/调研/R38a-Slack机器人设计.md`：3.3（Slack 怎么接、权限、发消息、收消息、manifest），3.9 第 8 条（`<!channel>` 等），第 5 节（自己接要做的），第 6 节（凭证）。
- `docs/任务/R38d-审查.md` 第 29 条，`docs/任务/R38d2-审查.md` 第 27、28、29 条。
- `docs/CONTRACT.md`「`pony status`（R38）」：`connection`、`reason`、`workspace`、`bot`、`other_connections`、`channels[]` 的取值。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38e-pony-slack`，分支 `r38e-pony-slack`（已从 main 建好）。只动 `crates/pony/`、`Cargo.lock`，以及本任务文件末尾。
- 共用核心（`crates/pony/src/core/`）只改 27、28 涉及的那两处和它们的测试；`chats/mod.rs` 的接口不改，确实要改先问主控。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r38e-pony-slack`。

## 要做的

1. **Slack 适配器**（`chats/slack.rs`），照接口说明和设计稿 3.3：
   - 连接：每次连接／重连前经 `credentials` 重新读凭证；`apps.connections.open` 取地址（app token），经代理（配置了 `proxy` 时）连 wss；收 `hello` 报 `connected` 和 `other_connections`；`auth.test` 取工作区名、机器人名和机器人的用户 ID；每个事件信封先 `emit` 成功再回执；`disconnect`（`warning`、`refresh_requested`）重新取地址再连，`link_disabled` 报 `socket_mode_off`；网络错 1、2、4…60 秒退避；30 秒 ping，90 秒没收到就当断了。`connected` 只在新建连接时报一次，连上很快又断的，重连同样退避（R38d 审查 29）。
   - 收消息：只要没有子类型的普通消息、`file_share`、`message_deleted`；去掉 `bot_id` 的和其余子类型；还原 `&lt;` `&gt;` `&amp;`、`<url|字>`，认出并去掉开头的 `<@机器人>`；有 `thread_ts` 且不等于自己的 `ts` 才是线程回复。
   - 发消息：`chat.postMessage` 带 `thread_ts`，正文放 markdown 块，另给一行 `text`；关掉链接预览；`notify` 的开头放 mrkdwn section 写 `<@owner> …`；正文里的 `<!channel>`、`<!here>`、`<!everyone>`、`<@U…>`、`<!subteam^…>` 默认转义成不会真的 @ 到人（R38f 实测时再看要不要放开）；每频道约每秒 1 条；块太长或不对的错误报 `resize`。
   - 频道信息：`conversations.info` 取名字、是否私有、是否归档、机器人在不在里面（`is_member`，没有就以发消息时的 `not_in_channel` 为准），映射到 `not_member`、`not_found`、`archived`。
   - 历史和核对：`history` 用 `conversations.history` 加 `conversations.replies`，用父消息的 `latest_reply` 只读有新回复的线程，父消息往回找 30 天，读完所有分页；`exists` 照接口说明，只在确定删了时返回 false。
   - 错误：Slack 的 `ok: false` 转成公开错误码；429 照 `Retry-After`。
2. **共用核心两处**（`docs/任务/R38d2-审查.md`）：
   - 27：照审查的改法，两轮相隔明显超过正常间隔时，把这段空档从补收和核对的期限里扣掉，期限只算 pony 醒着的时间；补审查写的场景测试（不发断线事件，隔 1 小时再跑一轮）。
   - 28：照审查的改法，清 `seen` 时，还有待补收记录的频道，晚于记录下界的条目留着；改那行注释和 README 的说法；补审查写的场景测试。
3. **依赖**：加 `tungstenite`（用户已定）和 ureq（工作区已在用 `=3.4.2`，`rustls`）；TLS 用 rustls，不用 native-tls。别的新库先问主控。
4. README 里 Slack 这一家和 27、28 的说法跟着改；写清对 paddock 的影响（`pony status` 的 `chats[]` 开始有真实的连接状态、工作区名、频道名，格式不变）。

## 怎么算做完

- 用户原话：「Slack 的凭证只放在本机，不进任何仓库。」：凭证只从 `PONY_HOME` 的 `secrets/slack.toml` 读；token、wss 地址（带 ticket）、HTTP 头和请求体不出现在日志、`pony status`、`status.json`、`state.json` 和报错里；测试只用 `xoxb-test` 这种假 token。
- 在本机起的假 Slack（假 HTTP 和 WebSocket 服务器）上跑通：连上报 `connected`、工作区名、机器人名、连接数；收事件先落盘再回执；`disconnect` 和断网后重连并退避；`link_disabled`、认证失败、429 各自的状态；收到的普通消息、线程回复、带附件的、删除事件翻成核心格式；发消息的请求体（markdown 块、`thread_ts`、`notify` 时 @ owner、意外的 @ 被转义、关链接预览）；频道信息的映射；`history` 只读有新回复的线程、读完分页；`exists`。
- 审查 27、28 的场景各有测试，修之前先跑，确认因为这个问题失败。
- R38c、R38d、R38d2 已有的测试照常通过；核心源码里不出现 `slack` 的测试照常通过。
- 「不要动ranch中corral的核心代码。」：`crates/corral/` 没有改动。
- 验证只做这些：上面各条有测试；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。觉得不够，在回复里说，不要自己加。

## 不要做

- **不连真实的 Slack**：不往 `slack.com` 或任何外部地址发请求；测试只连本机起的假服务器。没有真 token，也不要去找。
- **假服务器的地址只给测试用**：正式的 `pony` 程序不能被环境变量或配置改成把 token 发到别的主机。要怎么做才能既测得到又不留这个口子，拿不准就问主控。
- 不改 `crates/pony/` 以外的文件（除 `Cargo.lock` 和本任务文件末尾）；不改 CONTRACT、DESIGN、设计稿；不改 `chats/mod.rs` 的接口（要改先问）；核心只改 27、28 两处。
- 不建、不读真实的 `~/.pony`；测试和你自己试的时候绝不调真实的 corral，不读写 `~/.corral`；`HOME`、`PONY_HOME`、`CORRAL_HOME` 都指向临时目录。
- 不做 `pony service`，不写 `~/Library/LaunchAgents`（R38f）。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch`、`pkill -f pony` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停测试里起的进程和服务器用记下的 PID 或句柄。
- 限频等待（429 的 `Retry-After`、每频道每秒 1 条）如果会让核心长时间停住，停下来问主控怎么处理，不要自己改接口。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r38e-pony-slack` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话；另写一段「给 R38f 的上线说明」：建应用、生成两个 token、写 `~/.pony` 时要注意什么，实测时要看哪几处（包括意外 @ 的转义要不要放开、要不要代理）。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
