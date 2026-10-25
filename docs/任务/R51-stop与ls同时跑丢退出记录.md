# 任务：`corral stop` 和 `corral ls` 同时跑时，stop 也拿得到退出记录

2026-10-25，ranch/main 交给 ranch/dev-r51（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.85）、要、碰要害）
类型：Bug 修复
依据：本轮让 `stop` 在和 `ls` 竞争时仍输出这次的退出记录；不改 `ls` 清理目录的规则以外的行为，不改公开字段。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-24（R48e 审查后）：「记草稿，以后再做。」
- 10-25：主控列下一步时把 R51 列为第 2 件，用户：「2.可以做」。

## 先读

- `docs/任务/R48e-审查.md` 交叉审查意见第 1 条（发现经过和实测数字）。
- `docs/CONTRACT.md` 里 `corral stop` 的输出（`exit_code`、`stopped_by`、`remaining_pids`，以及「为 null 表示没取到记录」那句）。
- `crates/corral/src/cli.rs` 的 stop、`crates/corral/src/state.rs` 的 `list()`、`crates/corral/src/pen.rs` 写 `exit.json` 和放锁的次序。

## 现象

- pen 停下时先写 `exit.json`，再删 socket、放锁。`corral stop` 要等 socket 连不上、锁也拿得到才去读 `exit.json`。
- 锁一放开，同时在跑的 `corral ls`（`state.rs` 的 `list()`：status 失败且拿得到锁时删掉整个目录）可能抢在 `stop` 下一次检查（每 50 ms）之前把目录连 `exit.json` 一起删掉。`stop` 就输出 `"exit_code":null,"stopped_by":null`，Kimi 的 `remaining_pids` 也跟着丢。
- paddock 侧栏默认每 1 秒跑一次 `corral ls`，pony 也调 `ls`。审查实测：隔离环境里循环 start/stop `/bin/cat`，后台每秒跑一次 `ls` 时 200 次里有 8 次（4%）拿不到；`ls` 不停地跑时 40 次全拿不到；不跑 `ls` 时全正常。
- 所有家的 agent 都会碰上。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r51-stop-exit-record`，分支 `r51-stop-exit-record`（已从 main 建好）。只动 `crates/corral/`，以及必要时 `docs/CONTRACT.md` 里 stop 那几句和本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r51-stop-exit-record`。
- 另有一件 R52 在 `r52-test-cleanup` 分支并行，只动 `crates/corral/tests/lifecycle.rs`、`crates/corral/tests/sandbox.rs`；你不要动这两个文件。

## 要做的

- `corral stop` 正常停下一个 agent 时，即使同时有 `corral ls` 在跑，也输出这次停止的 `exit_code`、`stopped_by`（Kimi 还有 `remaining_pids`），不再是 null。
- 没有竞争时输出和现在完全一样。
- 审查给过一个改法，供参考，不必照做：`stop` 的等待循环里，每次连不上 socket 就读一次 `exit.json`，记下 instance 匹配的那份；锁放开后再读不到时，用记下的那份。依据是 pen 先写 `exit.json` 再删 socket 和放锁，`ls` 又必须先拿到锁才能删。

## 对 paddock 的影响

只改内部实现：命令、字段、退出码不变，竞争时原来的 null 变成真实值，paddock 不用动。旧版 corral 同时操作同一批 agent 时，行为和现在一样。

## 怎么算做完

- 一条能做出这个竞争的测试：改之前失败（stop 拿到 null），改之后通过。审查建议的做法是在 `pen.rs` 的 `final_kill_wait_is_bounded_and_records_remaining_processes` 里 `drop(lock)` 之后跑一次 `corral ls`。
- 边角：同名 agent 停了又马上重开时，`stop` 不会拿上一个实例的记录充当这一次的。
- 验证只做这些：上面那条测试先红后绿，边角那条各一条测试；然后 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改公开字段、退出码，不改 `ls` 的输出。
- 不碰用户的 agent：不对 `corral ls` 里现有的 agent 做 `stop`、`send`、`keys`、`upgrade`、`attach`。测试用隔离的 HOME、CORRAL_HOME 和 `/bin/cat`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID。
- 碰到要拿主意的事（包括要改 `ls` 删目录的规则、要改 pen 写记录的次序、要动公开约定），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r51-stop-exit-record` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
