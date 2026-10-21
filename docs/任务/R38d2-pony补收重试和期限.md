# 任务：R38d2 修 pony 补收失败后的重试间隔和期限

2026-10-21，ranch/main 交给 ranch/dev-r38d2（Codex，gpt-6-astra / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.52）、要、碰要害；重试和期限算并发和核心规则；按第 1 条读 `critical-other`，审查读 `review-claude`：Claude Code opus[1m] xhigh）
类型：Bug 修复
依据：R38d 交叉审查复核第 2 轮留下的建议改 24、25（`docs/任务/R38d-审查.md`），用户 10-21 定在 R38e 之前修。只改共用核心里补收和核对的重试、期限；不动别的行为，不连 Slack。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：「先写 R38d2 任务书吧.」（主控建议在 R38e 之前开一个小任务修 24、25，用户选了这个。）
- 底线：「不要动ranch中corral的核心代码。」

## 先读

- `AGENTS.md`「规矩」：测试不依赖真实 agent 和用户状态、验证、独立编译目录。
- `docs/任务/R38d-审查.md`：「复核意见（第 2 轮）」的 24、25、26 三条（问题、复现场景、改法），以及前面「主控对复核意见（第 1 轮）的决定」的 18、20 条（现在的规则从哪来）。
- `crates/pony/README.md`；`crates/pony/src/core/engine.rs` 的 `backfill` 和核对排队消息（`exists`）那两段；`crates/pony/tests/core.rs` 里 18、20 条对应的测试。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38d2-pony-retry`，分支 `r38d2-pony-retry`（已从 main 建好）。只动 `crates/pony/`，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r38d2-pony-retry`。

## 要做的

1. **补收失败后的重读要拉开间隔**（审查 24）：现在某个频道补收失败后每 2 秒重读一次。改成和 `exists` 一样退避：按 2、4、8… 秒拉开，最长 5 分钟，计时存在 `state.json`、跨重启保留；收到新的连上事件时立刻重试一次。
2. **期限不能让「断线／睡眠」吃掉最后一次尝试**（审查 25）：
   - 补收的 12 小时：每次连上之后，至少先再读一次；读成了就照常处理（12 小时以内的照送，更早的照现在的规则回「12 小时以前发的」），读失败了才判断是否到期、到期才说「请重发」。
   - 核对排队消息的 10 分钟：每次连上时重新计时，连上之后至少核对一次；仍核对不了，超过 10 分钟才照现在的规则说明并出队。
3. README 和「给 R38e 的接口说明」里涉及这两处的说法跟着改。对 paddock：无影响（`pony status` 不变）。

## 怎么算做完

- 审查 24 的场景：一个频道的 `history` 一直出错，60 秒内调用次数按退避递减，而不是每 2 秒一次；新的连上事件来了立刻重试。
- 审查 25 的两个场景：
  - 补收：重连时 `history` 出错一次，随即断线 13 小时，醒来前 2 小时用户在这个频道说了一句；醒来连上、`history` 能读了，这句照常进队列，不说「请重发」。
  - 核对：排着的一句在重连时 `exists` 出错一次，随即断线 1 小时；再连上时先核对一次，核对成了照常处理，不直接出队。
- R38d 已有的测试（含 18、20 条的）照常通过；没改到的行为不变。
- 「不要动ranch中corral的核心代码。」：`crates/corral/` 没有改动。
- 验证只做这些：上面三个场景各一条测试（修之前先跑，确认它因为这个问题失败）；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 `crates/pony/` 以外的任何文件（除本任务文件末尾）；不改 CONTRACT、DESIGN、设计稿；不加依赖。
- 不改审查 26 和其他「可以不改」各条，不顺手改别的行为。
- 不连 Slack、不发网络请求；`chats/slack.rs` 不动。
- 测试和你自己试的时候绝不调真实的 corral（PATH 上的、`~/.local/bin` 的），不读写 `~/.corral`、`~/.pony`；`HOME`、`PONY_HOME`、`CORRAL_HOME` 都指向临时目录。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch`、`pkill -f pony` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停测试里起的进程用记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r38d2-pony-retry` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：补收在 `Backfill` 持久化 `retry_at` 和间隔，旧状态文件缺字段时默认立即可试、初始间隔 2 秒；失败按 2、4、8… 秒退避，最多 300 秒，重连立即尝试且保留原下界及退避间隔。12 小时到期判断移到本次读取失败之后，读成仍照消息年龄处理。排队核对每次连上重置退避及 10 分钟期限，先核对一次。更新 README 和适配器接口注释；`pony status` 不变，paddock 不用动。
- 验证了什么：新增任务指定的三条测试，修前用 `cargo test -p pony --test core retry_` 跑一次，三条都因目标问题失败：60 秒补收调用 31 次而非 5 次；离线 13 小时后较新的消息没进队列；离线 1 小时后没有再次核对。已有 18、20 条测试保留，涉及旧规则的两条调整时间点和精确断言，继续覆盖期限边界、通知、出队、上限和重连。修后 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次；后两项通过。全量测试在未改动的 corral 测试 `raw_v2_events_keep_original_input_reply_and_retry_grace_with_v1` 失败：`send lab/main original --timeout 2` 返回 `not_delivered`，随后线程 join 失败，Cargo 提前停止，尚未运行到 pony；因此尚无修后 GREEN 证据。所有验证前台等到退出，HOME、PONY_HOME、CORRAL_HOME 使用临时目录，Cargo 离线运行。
- 拿主意的地方：没有新增设计决定。重连只强制立即补收一次，不清掉补收的首次尝试时间和退避间隔；核对按任务要求重新计时。待主控决定是否批准补跑验证：建议再跑一次 `cargo test --all-targets --no-fail-fast`，确保即使其他包失败也能得到 pony 的结果；本轮未自行追加或重跑。
- 没做的事：未改 corral、Slack 实现、依赖、CONTRACT、DESIGN、其他审查条目；未连 Slack、未调用户安装的 corral、未碰用户状态或 agent；未部署、合并或推送。验证尚未全部通过，等待主控处理上述验证缺口。
- 踩过的坑：第一次测试命令因附带 `rm -rf` 临时目录清理被工具拒绝，整条命令没有执行；改用 `rmdir` 清理空的隔离 HOME 后执行成功。全量测试遇到上述 corral 失败，遵守只跑一次的限制，没有尝试重跑或修改 corral。
