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
