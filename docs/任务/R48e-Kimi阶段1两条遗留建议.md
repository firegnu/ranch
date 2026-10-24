# 任务：R48e Kimi 阶段 1 的两条遗留建议

2026-10-24，ranch/main 交给 ranch/dev-r48e（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：常规 / 交叉审查要 / 影响面：碰要害（路由：常规（置信 0.25）、要、碰要害）
类型：功能变更
依据：恢复每轮第一次读 Kimi 状态的速度，并让 `corral stop` 停 Kimi 有残留进程时在输出里给出；不改别家、不改钩子。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-24：「那两条建议能先做吗？」（指 R48c 交叉审查「复核 1」新问题 1、2。）
- 10-24，主控问验收和 stop 的结果，用户选了推荐：照下面「怎么算做完」两条验收；有残留时 `corral stop` 算成功，带上残留 PID。

## 先读

- `docs/任务/R48c-审查.md`「复核 1」的「新问题」第 1、2 条：现象、位置、改法。
- `docs/CONTRACT.md` 里 `stop` 和 Kimi 停止的说法（搜 `remaining_pids`）；`docs/DESIGN.md` §7 Kimi 停止那条。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r48e`，分支 `r48e`（已从 main 建好）。只动 `crates/corral/`（`src/agents/kimi.rs`、`src/pen.rs`、`src/cli.rs` 及对应测试）、`docs/CONTRACT.md`、`docs/DESIGN.md`，以及本任务文件末尾的记录。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r48e`。

## 要做的

1. **每轮第一次读 Kimi 状态不再变慢。** `crates/corral/src/agents/kimi.rs` 读主 wire 的行筛选，照审查的改法：每行只做一次 UTF-8 转换（合法 UTF-8 时不复制），再按关键字查找；不要在每种类型里各转一遍。筛出的事件和现在完全一样。
2. **`corral stop` 能看出没停干净。** 停 Kimi 到 5 秒上限仍有残留子孙进程时，`corral stop` 的 JSON 输出带上 `remaining_pids`（PID 数组，兼容新增字段），仍是 `ok:true`、退出码不变。没有残留时 `stop` 输出和现在逐字一样；别家的 `stop` 不变。`exit.json`、`pen.log` 里照旧记。CONTRACT 改成写在 `stop` 输出里，写明对 paddock 的影响（兼容新增，不用改）；DESIGN §7 对应那条跟着改。

## 怎么算做完

- 同一份 50 MB 合成 wire，第一次读的用时不慢于 R48c 之前（审查测的旧版约 37 ms，现在约 129 ms），读出的状态和回复不变。
- 合成一个关不掉的子进程，`corral stop` 输出带 `remaining_pids`；正常停止时输出不变。
- 都用测试验，不开真 Kimi。
- 验证只做这些：上面两条各一个针对的测试（第 2 条先写测试，确认在现在的代码上因为没有 `remaining_pids` 而失败）；第 1 条的用时对比在临时测试或临时程序里做，结果写进完成记录，不把计时断言放进套件；再跑一次 `cargo test --all-targets --no-fail-fast`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。可以加边角用例：残留进程在上限前刚好退掉、`stop` 时 pen 已经不在。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 Kimi 以外 agent 的读状态、停止逻辑；不改钩子（`hooks.rs`）。
- 不处理审查「复核 1」第 3 条（停止上限单测要实等约 10 秒）。
- 不改 PATH 上装好的 `corral`、`ranch`，不打包、不部署，不装技能，不切共用 helper。
- 不开真 Kimi；不读、不改用户的 `~/.kimi-code`、`~/.corral`、`~/.ranch`、`~/.pony`。测试用隔离的 HOME、CORRAL_HOME、KIMI_CODE_HOME 和合成程序。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。
- 不按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f kimi` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉；用户自己可能也开着 kimi。停自己起的进程用记下的 PID。
- 不放宽现有测试的断言、不缩短超时、不删测试来过。
- 碰到要拿主意的事（包括要改公开格式里已有字段的含义），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r48e` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么（含第 1 条前后用时）、拿主意的地方、没做的事、对 paddock 的影响、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-24，ranch/dev-r48e，在 `r48e` 分支完成并自查。

- **做了什么**：主 wire 每个完整行只调用一次 `String::from_utf8_lossy`，合法 UTF-8 借用原字节，再以 `contains` 筛选原来的七种关键字；JSON 仍从原字节解析，损坏行不会变成有效事件。`stop` 沿用 pen 的退出记录，仅对子孙停止适配器（目前只有 Kimi）、同一实例且非空的残留数组增加 `remaining_pids`；`ok:true`、命令退出码和原有字段不变。停止步骤、`exit.json`、`pen.log` 的生产代码未改。同步更新 CONTRACT 和 DESIGN §7。
- **针对验证**：先扩展 `final_kill_wait_is_bounded_and_records_remaining_processes`，通过隔离 HOME、CORRAL_HOME、KIMI_CODE_HOME 的子进程执行 CLI `stop` 路径。使用原有最后一次 KILL 后仍活着的 `/bin/sleep` 快照，实等 5 秒；旧代码明确因返回值缺少 `remaining_pids` 失败，修复后父进程已退出／尚未退出两种情况均通过，原来的时限、日志及退出码断言全部保留。`stop_waits_for_the_kimi_command_tree_and_leaves_unrelated_processes_alive` 补上正常停止 stdout 的逐字节比较并通过。`only_the_main_wire_finishes_a_turn_and_supplies_its_reply` 加入长 UTF-8 无关行、含关键字但 UTF-8 损坏的行及 `tool.result`，修改筛选前后均通过，状态、最后一步回复、主子轮区分和半行处理不变。
- **首次读取用时**：临时 Rust 测试直接比较实际 `wire_events` 与审查所用旧版按行 `contains` 实现（`154c2fb`）。同一份 50,000,000 字节合成 wire，以约 64 KiB 的无关 `model.context` 行填充，包含主轮输入、工具、文字和完成事件；release 编译，预热文件读取，每次用空 cursor，交替调用各 21 次，记录中位数，不设计时断言。修改前：旧基线 **28.660 ms**、本分支 **145.705 ms**；修改后：旧基线 **28.275 ms**、本分支 **28.271 ms**，恢复至旧版水平。每次均断言相同的 Stop 事件、`final reply` 和 Bash 工具；临时测试、程序和 wire 已删除，未进入套件。
- **全套验证**：命令均在前台执行并等到结束，编译目录统一为 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r48e`。一次 `cargo test --all-targets --no-fail-fast`：**465 通过、15 跳过、0 失败**（新增的 1 个 ignored 项是由停止上限测试实际调用的隔离 CLI 子进程入口）。`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`、`git diff --check` 全部通过。
- **拿主意的地方**：没有需要主控新增决定的事。复用原有适配器的 `stop_descendants` 声明与实例匹配，不在共用 CLI 写 Kimi 名字，不改停止机制或已有公开字段的含义；继续采用现有快照模拟 KILL 后仍存活的进程，不开真 Kimi、不要求特权。
- **没做的事**：未处理审查第 3 条的实等约 10 秒，未加计时断言；未改其他 agent 或钩子，未读写用户真实状态，未打包、部署、安装技能、切 helper、合并或推送。HANDOFF 不在本任务改动范围，留给主控合并后更新已知问题。
- **对 paddock 的影响**：`stop.remaining_pids` 是兼容新增的可选字段，无残留时不出现，paddock 现有调用不用改；如需显示残留，可直接用命令输出，无须读内部目录。
- **踩过的坑**：没有。
