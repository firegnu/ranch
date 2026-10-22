# 任务：R43 corral：Claude Code 在框上被 Esc 后认出空下来；记下提问框的完整题目

2026-10-22，ranch/main 交给 ranch/dev-r43（Codex，gpt-6-astra / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.86）、要、碰要害；按第 1 条读 `critical-other`，审查读 `review-claude`：Claude Code opus[1m] xhigh）
类型：功能变更
依据：R42「第一步」的第 1 件，只改 corral 这两处，给后面 pony 的两件活打底；pony 不在本件里改。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-22：主控建议「现在就为 Claude Code 选择框改一小块 corral，Slack 里列出完整题目，用文字回答加『停』」，并修 Claude Code 在框上被 Esc 拒绝后一直报 `blocked` 的误报。用户：「好的，那就按照你的意见来吧。」这两处是用户对底线「不要动ranch中corral的核心代码」明确开的口子，只限这两处。
- 10-22：主控说明做完后「agent 弹了选择题，Slack 提醒里列出完整的题目和选项，你在线程里回答就送到了」「回一句停就拒绝了……这一轮马上停，会话还在，你可以接着说话」。用户：「好的，那就开始吧。」

## 先读

- `docs/调研/R42t-确认框和打断实测.md`「Claude Code」整节：三种框按 Esc 后一直 `blocked`、`send` 报 `not_idle`；落盘事件没有 `tool_input`；事件和状态原样。
- `docs/调研/R42-手机上回答确认框设计.md` 第 6 节 (a)、(d) 和「用户的决定」。
- `docs/DESIGN.md` §7 里 R22、R24 两段（打断怎么记、Claude Code 怎么读对话记录认打断）；`docs/任务/R24-ClaudeCode被打断后马上认出来.md`。
- `crates/corral/src/hooks.rs`（钩子抄哪些字段）、`crates/corral/src/events.rs` 的 `observe_transcript`、`crates/corral/src/agents/claude.rs` 的 `transcript_interrupt`。
- `docs/CONTRACT.md` 里 `corral status` 的字段。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r43`，分支 `r43`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r43`。
- 只动 `crates/corral/`、`docs/CONTRACT.md`（`status` 新字段）、`docs/DESIGN.md` §7（在 R24 那段后补一段），以及本任务文件末尾。

## 要做的

1. **框上被 Esc 后认出空下来。** Claude Code 停在权限框、提问框（AskUserQuestion）、计划批准（ExitPlanMode）上被 Esc 拒绝，或者在框上批准后、工具运行中被 Esc 打断，corral 现在一直报 `blocked`（实测见 R42t）。改成：认出这次打断，状态变成 `idle`、`interrupted: true`，之后 `corral send` 能送进去。沿用 R24 的办法（读对话记录末尾，只认本主会话、非侧链、时间严格晚于最近一次输入的打断标记），不按输出、静默时间或读屏猜；人没按 Esc、框还开着时，照旧是 `blocked`。
2. **记下提问框的完整题目。** Claude Code 调 AskUserQuestion 时，钩子收到的原始输入里有 `tool_input.questions`（先在测试里或按官方文档确认它在哪个钩子、长什么样；拿不到就停下问主控）。只对这个工具，把题目记下来，`corral status` 多一个只读字段 `questions`：
   - 停在这个提问框上（`state` 是 `blocked`、`last_tool` 是 `AskUserQuestion`）时给出，其余时候没有或为 `null`；
   - 每道题：`header`、`question`、`multi_select`（布尔）、`options`（每项 `label`、`description`）；
   - 设一个总大小上限，超了截断并标明截断；别的工具的参数一律不记。
   - 写进 `docs/CONTRACT.md`，标明是兼容新增；旧版 corral 读新事件、新版读旧事件都不出错（过渡期两版并存）。

## 怎么算做完

- 「agent 弹了选择题，Slack 提醒里列出完整的题目和选项」：corral 这边，停在 Claude Code 提问框上时 `corral status` 的 `questions` 给出完整题目、选项、说明和是否多选。
- 「回一句停就拒绝了……这一轮马上停，会话还在，你可以接着说话」：corral 这边，Claude Code 在三种框上、或批准后运行中被 Esc，`status` 变成 `idle`、`interrupted: true`，`send` 能送进去。
- 「其他都不动」的边界：只改上面两处；别家的行为、现有字段和状态值都不变。
- 验证只做这些：
  - 针对这件活的测试：三种框上被 Esc、批准后运行中被 Esc 各一个用例，断言 `idle`/`interrupted`；框还开着、没按 Esc 时仍是 `blocked`；对话记录里只有更早的打断标记时不误判；`questions` 的单选、多选、多道题、超长截断、非 AskUserQuestion 不记各一个；新旧事件互读。用合成的钩子输入和对话记录，隔离 HOME、CORRAL_HOME。
  - 改完后开一个测试 Claude Code 实测一次（见「不要做」里测试 agent 的规矩），确认：提问框上 `status` 有 `questions`；Esc 后回到 `idle`、`send` 能送进去。实测要用这个分支编出的 corral，并让测试 agent 的钩子也走新程序（不切用户的共用 helper，见「不要做」），怎么做到写进完成记录。
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。
  - 觉得不够，在回复里说，不要自己加。
- 对 paddock：`status` 兼容新增 `questions` 字段，paddock 可以不接；Claude Code 被 Esc 后状态从误报的 `blocked` 变成 `idle`，显示更准，paddock 不用动。

## 不要做

- 不改 pony、fence、dispatch；不改上面两处以外的 corral 行为；不升级依赖。
- **不切用户的共用 helper**（`~/.corral/.runtime/helper`）、不切 `~/.local/bin` 链接、不 `corral upgrade` 用户的 agent、不部署。实测要用新程序时，用隔离的 `CORRAL_HOME`（临时目录）和这个分支编出的 corral 启动测试 agent。
- 测试 agent：名字 `ranch/test-r43-claude`，带 `--label role=test`、`--env CAIRN_DISABLE=1`、`--permission-mode default`，cwd 用 worktree 下自建的 `scratch/`（不提交，做完删掉）；只让它做无害的事（`ls`、`echo`、`ping -c 30 127.0.0.1`、AskUserQuestion 问测试题），不在框上选「以后都允许」；用完 `corral stop`。
- 不碰用户现有的 agent：`corral ls` 里不是你开的，一律不 `read`、`keys`、`send`、`stop`、`attach`。
- 不要按项目名或路径批量杀进程（`pkill -f ranch`、`pkill -f corral`、`pkill -f claude` 这类）：主控、别的 agent 和用户的 pony 的进程命令行里都带着项目名和路径，一条命令能把它们全杀掉。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事（包括钩子原始输入里拿不到题目、要动上面两处以外的东西），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r43` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：`observe_transcript` 将 R24 的读取门槛从 working 扩到 working／blocked，仍使用原来的轮次、主会话、非侧链、严格晚于输入的标记判断，不改解析器或别家适配器。AskUserQuestion 的 PermissionRequest 钩子仅记录题目白名单，`status.questions` 在当前提问框 blocked 时返回 `{items, truncated}`；单选、多选、多题及选项说明完整保留。离开该框或下一次旧钩子没有题目时清掉旧内容，通知保留当前内容。DESIGN §7 增补 R43，CONTRACT 写明字段、截断和新旧版本边界；paddock 可以不接题目，打断状态修正无需适配。
- 针对验证：先运行权限框回归，确认在预期 idle、实际 blocked 上失败；题目回归先在实际 null 上失败；总量回归先在 truncated=false 上失败，再分别最小实现至通过。新增用例覆盖权限框、AskUserQuestion、ExitPlanMode、批准后运行中被 Esc，均验证 idle/interrupted 和普通 send 实际到达合成 cat 并被输入钩子确认；同时覆盖框仍开着、早于或等于最近输入的旧标记不误判。题目覆盖单选、多选、多题、带中文和 JSON 转义的超长截断、其他工具参数不落盘。全部用隔离 HOME/CORRAL_HOME、合成钩子和对话记录。
- 新旧兼容与全套验证：保存修改前（基于 `219f8c2`）编出的 corral 为 `/tmp/r43.i98QjM/corral-before`，显式运行 `CORRAL_COMPAT_BIN=… cargo test -p corral-core --test interrupted claude_questions_new_and_old_clients_share_events -- --exact --ignored`，真实旧 reader 读新事件、新 reader 在旧缓存回写后重放，以及无题目的旧事件均通过。直接相关 Claude 用例 17 通过，另加该兼容用例 1 通过。最终 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各运行一次并通过；全套 347 通过、0 失败、12 跳过（原有 11 项及需要外部基线、已单独跑过的兼容项）。快照只将 cursor 8 改成 9，其他内容不变。自查 diff 和 `git diff --check` 通过。
- 真实实测：只开一个 `ranch/test-r43-claude`，role=test、CAIRN_DISABLE=1、default 权限模式，cwd 为本 worktree 的 scratch。使用本分支 `.target/r43/debug/corral`，`CORRAL_HOME=/tmp/r43.i98QjM/pens` 原本为空，首次启动自动创建其 `.runtime/helper`；readlink 已确认指向本分支程序。两道测试题（一单选、一多选）在 blocked 时完整出现在 questions。只对这个自建测试 agent 用 `corral keys … esc` 送一次 Esc，后续查询为 idle/interrupted=true，questions 缺省；普通 send 返回 confirmed=true，随后回复 `R43-RECEIVED`。本次是程序送 Esc，不宣称真人按键或精确秒级计时。测试 agent 已 stop，隔离 ls 为空，scratch 已删除；用户共用 helper 前后均为 aef2b4a，未切换。状态样本、回复、基线程序及检查日志留在 `/tmp/r43.i98QjM`，不提交运行数据。
- 拿主意的地方：按 [Claude Code 官方钩子文档](https://code.claude.com/docs/en/hooks#permissionrequest-input) 及 [AskUserQuestion 字段说明](https://code.claude.com/docs/en/hooks#pretooluse-input) 确认原始输入位置和形状，再由真实测试验证。只在 PermissionRequest 采集，避免重复保存 PreToolUse 参数，且接收之前钩子更新后的输入。上限定为 questions 紧凑 JSON 的 16 KiB（含结构和转义），按顺序保留 Unicode 字符前缀，省略放不下的后续项并置 truncated=true；不记录 answers。cursor 升到 9，防止沿用旧 reader 已消费事件却没保存题目的缓存；事件仍为 v1，不迁移原事件。
- 没做的事：未改 pony、fence、dispatch、pen、部署逻辑、依赖或其他仓库；未切用户 helper、bin 链接，未 upgrade、操作用户 agent、部署、合并 main 或推送。未另开开发或审查 agent。只在 r43 分支提交，交主控及规定的交叉审查。
- 踩过的坑：最初误用 Cargo 包名 corral，命令在包查找阶段失败；改成仓库实际包名 corral-core 后才取得有效 RED。真实 Esc 后紧接着的第一次查询仍是 blocked，下一次查询读到落盘标记后正常恢复；没有增加静默等待或猜测逻辑。
- 要主控决定的事：没有新的待决定项。
