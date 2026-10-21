# 任务：R38g 让 pony 认出手机 Slack 上的 @

2026-10-21，ranch/main 交给 ranch/dev-r38g（Codex，gpt-6-astra / high）。
路由：常规 / 交叉审查不要 / 影响面：改行为（路由：常规（置信 0.95）、拿不准、拿不准；只改 Slack 适配器认 @ 的一处，主控判改行为）
类型：Bug 修复
依据：pony 上线实测时发现，手机 Slack 上新开对话 @pony，pony 不理。只改 Slack 适配器认 @ 的地方，不改共用核心，不改别的行为。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：「现在的这个AT的问题，你不打算使用hardcode吧？」
- 10-21：「这是插件部分的还是共有的？」（主控答：只在 Slack 适配器里改。）
- 10-21：「好的，修吧」
- 底线：「不要动ranch中corral的核心代码。」

## 现象

用户在手机 Slack 的频道顶层 @pony 说话，pony 不理，日志里也没有记录。桌面版把这两条显示成 `@B…（一串编号） 手机测试…`：手机插进正文的是 `<@B…>`（pony 这个应用的 bot ID），不是电脑上插的 `<@U…>`（机器人用户 ID）。适配器只认后者，就当成没 @，按设计不理。在 pony 认识的线程里回复不看 @，手机上是好的。

## 先读

- `AGENTS.md`「规矩」：测试不依赖真实 agent 和用户状态、验证、独立编译目录。
- `docs/调研/R38a-Slack机器人设计.md` 3.3「只订 `message.channels`…」那一条（主控已改：两种 ID 都认，连上时用 `auth.test` 取）。
- `crates/pony/src/chats/slack.rs`：取 `auth.test` 建 `Session` 的地方、`translate` 认 @ 的地方，以及补收时调 `translate` 的地方；`crates/pony/src/chats/slack/tests.rs` 里认 @ 的测试。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38g-pony-bot-mention`，分支 `r38g-pony-bot-mention`（已从 main 建好）。只动 `crates/pony/src/chats/slack.rs`、`crates/pony/src/chats/slack/tests.rs`、`crates/pony/README.md`，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r38g-pony-bot-mention`。

## 要做的

1. 连上时，从 `auth.test` 的返回里把 `bot_id` 也存下来。
2. 正文开头是 `<@机器人用户 ID>` 或 `<@bot ID>`，都算叫 pony，去掉开头的那个 @ 再往下走。在线收消息和补收两条路都一样。
3. 两个 ID 都是连上时现取的，代码、配置、测试里都不写真实 ID。
4. README 的 Slack 一节跟着改一句。对 paddock：无影响（`pony status` 不变）。

## 怎么算做完

- 「现在的这个AT的问题，你不打算使用hardcode吧？」：不写死任何 ID。
- 正文开头是 `<@bot ID>` 的话，和 `<@机器人用户 ID>` 一样被当成叫 pony；两种都不是的，照旧不算。
- 「这是插件部分的还是共有的？」：只改 Slack 适配器，`crates/pony/src/core/` 和 `chats/mod.rs` 没有改动；「不要动ranch中corral的核心代码。」：`crates/corral/` 没有改动。
- 验证只做这些：一条测试，正文以 `<@bot ID>` 开头时算叫 pony（修之前先跑，确认它因为这个问题失败）；`cargo test --all-targets` 一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改共用核心、`chats/mod.rs` 的接口、别的事件翻译和发消息的行为；不顺手改别的（包括日志措辞）。
- 不连 Slack、不发网络请求，不读真实的 `~/.pony`、`~/.corral`；`HOME`、`PONY_HOME`、`CORRAL_HOME` 都指向临时目录；绝不调真实的 corral，不跑真的 `launchctl`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不打包、不动 pony 的后台服务。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch`、`pkill -f pony` 这类）：主控、别的 agent 和用户正在跑的 pony 服务，命令行里都带着这些名字。停测试里起的进程用记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。
- 不合并到 main，不推送。只在 `r38g-pony-bot-mention` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：Slack 连接时从 `auth.test` 读取并保存 `bot_id`，与已有的 `user_id` 一起用于正文开头的 @ 匹配；在线接收、频道历史和线程补收共用此逻辑，只去掉开头匹配的一个 @。README 已补充说明。代码没有写死 ID，测试只使用 `UBOT`、`BBOT` 等虚构 ID。对 paddock 无影响，`pony status` 不变。
- 验证了什么：只新增 `bot_id_mention_is_recognized_and_removed` 一条测试。修复前运行 `cargo test -p pony bot_id_mention_is_recognized_and_removed`，在 `assert!(message.mentioned)` 失败，确认复现目标问题；修复后只运行一次 `cargo test --all-targets`，285 通过、0 失败、11 条原有测试默认忽略，新增测试通过。命令均在前台等到退出，使用任务指定的独立编译目录，HOME、PONY_HOME、CORRAL_HOME 均指向临时目录，Cargo 离线运行。
- 拿主意的地方：没有新增设计取舍；沿用适配器原有的必填认证字段读取方式、前导空白处理和事件过滤，只扩充匹配用的 ID。无需主控决定。
- 没做的事：没有修改共用核心、`chats/mod.rs`、`crates/corral/` 或其他行为；没有连接真实 Slack、读取真实 pony/corral 状态、调用真实 launchctl、操作用户 agent、部署、合并或推送。按任务限定，没有增加其他测试或运行 clippy、fmt 检查。默认忽略的测试需要外部旧版 corral 或真实 srt，本次未启用。
- 踩过的坑：没有。

## 主控审查

- 看了 diff：只动了 `chats/slack.rs`、`chats/slack/tests.rs`、README 和本文件；共用核心、`chats/mod.rs`、corral 没改。`bot_id` 和机器人用户 ID 一样在连上时从 `auth.test` 取，在线接收和补收都传进 `translate`；测试只用假 ID `BBOT`，没有真实 ID。
- 主控重跑全套 285 通过、0 失败、11 忽略；clippy、fmt 通过。
- 留下：`auth.test` 没返回 `bot_id` 时这次连接按 `slack_error` 失败（和取 `user_id` 一样）；bot token 的 `auth.test` 会返回它，上线实测确认。

结果：一次过
