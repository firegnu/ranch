# 任务：R38h pony 日志不再把自己的回声和不处理的事件记成「别人的消息」

2026-10-21，ranch/main 交给 ranch/dev-r38h（Codex，gpt-6-astra / high）。
路由：常规 / 交叉审查不要 / 影响面：改行为（路由：常规（置信 0.43）、拿不准、拿不准；只影响日志，但改动挨着「先落盘再回执」那条路，主控判改行为）
类型：Bug 修复
依据：pony 上线实测时发现，日志里大量「忽略了一条别人的消息」其实是 pony 自己发出、又被 Slack 推回的消息，主控据此看错过一次。本轮只改日志记什么，不改收发行为。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：「日志那个是什么问题？」（主控解释后）「那就改了吧」
- 底线：「不要动ranch中corral的核心代码。」

## 现象

`~/.pony/logs/pony.log` 里每次 pony 在频道发一条消息，几秒后就多一行「忽略了一条别人的消息」。工作区里只有用户一人，这些都是 pony 自己的消息被 Slack 推回来。现在的路径：Slack 适配器 `translate` 不认的事件（自己或别的机器人发的、编辑、加入频道等其余子类型）一律 `emit(Event::Ignored)`；`core/state.rs` 把非 owner 发的消息也换成 `Event::Ignored`；`core/engine.rs` 收到 `Ignored` 就记「忽略了一条别人的消息」。两类混在一起了。

## 先读

- `AGENTS.md`「规矩」：测试不依赖真实 agent 和用户状态、验证、独立编译目录。
- `crates/pony/README.md` 开头几条（核心怎么处理事件、日志记什么）和 Slack 一节（先 emit 落盘、成功才回执）。
- `crates/pony/src/chats/slack.rs` 的 `receive`（emit 和回执）、`translate`；`crates/pony/src/chats/mod.rs` 的 `Event`、`Events`；`crates/pony/src/core/state.rs` 的 `events`；`crates/pony/src/core/engine.rs` 里记「忽略了一条别人的消息」的两处。
- 设计稿 `docs/调研/R38a-Slack机器人设计.md` 3.3 第 3 条、3.5 开头（别人的消息不回、不送，日志只记「忽略了一条别人的消息」）。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38h-pony-ignore-log`，分支 `r38h-pony-ignore-log`（已从 main 建好）。只动 `crates/pony/`，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r38h-pony-ignore-log`。

## 要做的

1. 「忽略了一条别人的消息」只在真有别人（不是 owner 的真人用户）在绑了仓库的频道里发消息时记。
2. pony 自己发出又被推回的消息、别的机器人的消息、编辑和其余不处理的事件，不再记成这句：不记，或者记成别的一句，你定，写进完成记录。
3. 收发行为不变：别人的消息照旧不回、不送、不落正文；每个事件照旧先落盘（或确认不用落盘）再回执，不能出现不回执、或回执早于该落的盘。
4. 优先只改 Slack 适配器；要改共用核心或 `chats/mod.rs` 的接口，先停下问主控。共用核心里不能出现任何平台名。
5. README 涉及日志的说法跟着改。对 paddock：无影响（`pony status` 不变）。

## 怎么算做完

- 「那就改了吧」：pony 自己的回声和不处理的事件不再记成「忽略了一条别人的消息」；别人发的消息仍记这句。
- 「不要动ranch中corral的核心代码。」：`crates/corral/` 没有改动。
- 验证只做这些：一条测试，pony 自己的消息被推回时日志里没有「忽略了一条别人的消息」，别人发的消息仍有（修之前先跑，确认它因为这个问题失败）；`cargo test --all-targets` 一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改收发、路由、补收、回执和落盘的行为；不顺手改别的。
- 不连 Slack、不发网络请求，不读真实的 `~/.pony`、`~/.corral`；`HOME`、`PONY_HOME`、`CORRAL_HOME` 都指向临时目录；绝不调真实的 corral，不跑真的 `launchctl`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不打包、不动 pony 的后台服务。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch`、`pkill -f pony` 这类）：主控、别的 agent 和用户正在跑的 pony 服务，命令行里都带着这些名字。停测试里起的进程用记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。
- 不合并到 main，不推送。只在 `r38h-pony-ignore-log` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
