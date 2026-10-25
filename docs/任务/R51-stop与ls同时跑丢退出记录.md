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

## 完成记录

2026-10-25，ranch/dev-r51。第一轮（提交 `53a7fe0`）停在需要主控决定的地方，当时只保存失败复现，不能合并；主控决定及后续实现见下文。

- 做了什么：在 `pen.rs` 的 `final_kill_wait_is_bounded_and_records_remaining_processes` 中加入真实 CLI `ls` 清理。pen 写好记录、仍持锁且 socket 尚在时，暂停本测试自己的 stop CLI 子进程；确认暂停后关闭 socket、放锁，等隔离环境中的 ls 清理完成，再恢复并回收 stop CLI。这个固定调度模拟 stop 在两次轮询之间没有获得 CPU，不改变 pen 写记录、删 socket、放锁的次序。原有残留进程、耗时、退出记录和日志断言保留，文件内容在 ls 删除前读取。所有子进程均等待结束。
- 验证了什么：使用指定 `CARGO_TARGET_DIR`，运行 `cargo test -p corral-core --lib pen::tests::final_kill_wait_is_bounded_and_records_remaining_processes -- --exact --nocapture`。原实现 RED：ls 成功且目录已删除，stop 输出 `exit_code:null`、`stopped_by:null`，缺少 `remaining_pids`，而预期是 `exit_code:0`、`stopped_by:SIGKILL` 和残留 PID。随后只试任务单参考方案：socket 连不上时先读并缓存 instance 匹配的 exit.json，再检查锁；同一条测试仍 RED，输出相同。两次均约 5.1 秒，测试命令均以前台完成、退出码 101；无效生产代码已撤回。
- 拿主意的地方：没有自行改变 ls 清理规则、pen 写记录次序或公开约定。已证实单靠 stop 轮询缓存不能保证记录可用：`写 exit.json → 删 socket → 放锁 → ls 删 exit.json` 可以全部发生在 stop 两次轮询之间。第一次看到 socket 不通时文件已经不存在，缓存无从建立。
- 需要主控决定：建议允许修改 stop 与 ls 的清理协调，让 stop 读完本实例退出记录前，ls 不能删除它；需同时明确旧版 ls 不参与新协调时的兼容边界。另一条路是 pen 通过停止请求的连接返回最终记录，但涉及内部协议和旧 pen 的兼容，改动更大。只缩短轮询或保留提前缓存仅能缩小窗口，达不到“不再丢”的要求，不建议作为修复交付。待主控决定后继续，将决定补记于此。
- 没做的事：尚未达到 GREEN，未加或运行同名重开边角测试，未运行全套测试、clippy、fmt 三项最终验证；未修改 CONTRACT、DESIGN、HANDOFF，未动 R52 的两个文件，未合并、推送、部署，也未操作用户 agent。生产实现与原来一致，paddock 目前不用动。
- 踩过的坑：审查参考方案所依赖的“pen 先写记录、再删 socket 和放锁”只保证写入先后，并不保证 stop 会在 ls 清理之前被调度。固定这个调度后，提前缓存也必然漏读，故撤回该尝试，没有继续替换方案。

### 主控决定后的实现（2026-10-25）

- 主控决定：采用「stop 与 ls 协调清理」，以文件锁实现，不用时间宽限，不改 pen 协议。stop 发请求前在 agent 目录持有专用排他 flock，读完 exit.json 才释放；ls 清理前非阻塞获取同一把锁，取不到就跳过且不列入输出。锁文件加进 `state::FILES`；stop 尽量删除自己的锁文件，中途退出由系统释放 flock，余下目录交后续 ls。旧版 ls 不认识这把锁，仍可能令新版 stop 返回 null；需要保证其遗留锁文件能回收、同名 agent 能重开。命令、字段、退出码不变，在 CONTRACT 写明旧版边界。
- 做了什么：实现 `stop.lock`，沿用已有带 inode 复核的非阻塞 flock。stop 等待另一持锁者时沿用 `--timeout` 和现有 timeout 错误；持锁对象负责返回、报错时释放。删除自身锁文件前还尝试获取运行锁并核对 inode，避免和另一个 stop、旧 ls 的删除互相穿插；运行锁已经被旧 ls 删掉时留下标记，供新版 ls 发现。新版 ls 同时识别运行锁或 stop.lock，回收孤立标记时重建并取得运行锁，避免删除同名新实例。pen 启动清理保留 stop.lock，避免重开时换掉仍在用的锁 inode；退出记录写入次序、停止步骤及协议未改。补了保留文件名检查及文件清单快照，CONTRACT 已补旧版 ls 边界。
- 验证了什么：命令均使用本任务的 `CARGO_TARGET_DIR`，并等待前台结束。
  - 原竞争测试先前已 RED，本轮 `cargo test -p corral-core --lib pen::tests::final_kill_wait_is_bounded_and_records_remaining_processes -- --exact --nocapture` GREEN（10.15 秒）。固定相同调度，ls 不列已停 agent、保留等待中的退出记录，stop 给出完整字段；stop 结束后自身锁文件消失，后一次 ls 清理目录。原有两种父进程状态、残留 PID、5–7 秒上限、日志及记录断言保留。
  - `cargo test -p corral-core --test protocol stop_does_not_reuse_exit_record_after_same_name_restart -- --exact --nocapture` 通过（1.20 秒）。合成 pen 在同名重开后故意保留旧实例记录，真实 CLI 不把旧退出码、停止步骤、残留 PID 用于当前实例；匹配实例时完整输出保持一致。
  - `cargo test -p corral-core --test protocol stop_lock_left_by_old_ls_allows_restart_and_eventual_cleanup -- --exact --nocapture` 通过（0.90 秒）。构造旧 ls 只留下 stop.lock 的状态：持锁期间新版 ls 不删除、不列出，同名 `/bin/cat` 可启动且不替换锁 inode；持锁者退出后新 stop、ls 可正常清理。另核对无 flock 持有者的孤立标记可直接被新版 ls 回收，不需先重开；这里用关闭锁描述符模拟失去持锁者，没有实际杀 stop 进程。
  - `cargo test --all-targets` 按预算跑一次：首个 lib 测试目标 43 通过、1 失败、1 跳过，失败是 `adapters_tables.json` 尚未加入已批准的 `stop.lock`，之后的目标因默认失败即停未执行。已只在预期清单加入这一项，保留原快照断言，删除测试生成的 `.actual.json`。**受“各跑一次”限制，未复跑，不能声称全套通过。**
  - `cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次，均通过。
- 拿主意的地方：实现沿用主控的锁方案，未采用提前缓存、宽限时间或协议改动。专用锁尽量在仍持锁时删除，以免另一 stop 拿到旧 inode 后锁文件被移除。旧 ls 留下的标记由之后的新版 ls 回收；如果始终只运行旧版 ls，它仍不认识标记，但不妨碍同名重开。公开输出及退出码保持，paddock 不用动。
- 没做的事：未补跑被快照失败挡住的全套检查，未改 HANDOFF、DESIGN、R52 的两个文件，未操作用户 agent，未合并、推送或部署。
- 踩过的坑：新增 `state::FILES` 条目时首次漏同步 `adapters_tables.json`，导致唯一一次全套验证提前停止；现已补齐预期数据，没有删除或放宽断言。
- 交主控：实现及指定针对性检查已完成，没有新的设计待定项；还需要主控安排或批准一次修正快照后的全套测试，补齐上述验证缺口后再判断可否合并。

### 交叉审查后返工

2026-10-25，ranch/dev-r51。已读主仓库 `docs/任务/R51-审查.md` 的「审查意见」，按本轮主控决定返工。前一轮全套验证缺口已由主控和交叉审查补齐：审查文件记录两边均为 500 通过、0 失败、17 跳过；这不是本轮重新运行的结果。

- 主控决定及做了什么：
  - 第 1 条必须改：记下是否等过 stop.lock。等过且停止请求得到 not_found 时保留原错误，继续持锁等待、读退出记录；instance 与开头 status 一致才走原成功输出，不一致或无记录返回保留的原 not_found。没等过锁时的请求错误处理不变。补了两个 stop 并发停止同一个合成 agent 的测试；合成 shell 忽略 HUP、TERM，约 6 秒后 KILL，两边必须退出成功、完整记录相同。
  - 第 2 条采纳：等锁、等 pen 停下共用一个 deadline，不再在发完停止请求后重新计算 timeout。
  - 第 3 条采纳：CONTRACT 新增独立的「stop 与 ls 的退出记录（R51）」，从 Kimi 小节移出通用说明。写明同时停止的返回、双向新旧混用、agent 目录的 stop.lock、保留名字 bad_name、stop 进行中 status 失败时 ls 不列该项，以及 paddock 不读目录、不用改。
  - 第 4 条采纳：ls 不带 create 获取 stop.lock；文件存在且被占则跳过，取得文件锁时保持原清理流程；文件不存在时只尝试已有运行锁，不建 stop.lock，也不新建运行锁。已有孤立 stop.lock 仍可通过重建并取得运行锁回收。扩展原遗留目录测试，核对启动中的项仍列 starting、不会被 ls 新建 stop.lock，结束后照常清理。
  - 第 5、6、7、8 条按决定不改代码：5、6 的显示行为和旧名字影响已写入 CONTRACT；不改合成 pen 的 accept 等待方式，不改同名重开时删除 exit.json 的已有窗口。未采用共享锁、时间宽限或 pen 协议改动。
- 验证了什么：全部使用本任务的 `CARGO_TARGET_DIR`，所有命令等待前台结束。
  - RED：`cargo test -p corral-core --test protocol stop_ -- --nocapture` 在改生产代码之前运行，2 通过、2 失败（6.97 秒）。双 stop 测试失败于后到调用返回 not_found；扩展的 ls 检查失败于启动中目录被新建 stop.lock，均为本次目标行为，而非测试夹具或语法错误。
  - GREEN：同一命令修改后 4 通过、0 失败（7.26 秒），包含双 stop、实例隔离、旧 ls 遗留目录及不新建锁、停止超时。
  - `cargo test -p corral-core --lib pen::tests::final_kill_wait_is_bounded_and_records_remaining_processes -- --exact --nocapture` 通过（10.09 秒）；`cargo test -p corral-core --test protocol concurrent_unique_starts_and_listing_preserve_live_locks -- --exact --nocapture` 通过（0.97 秒）。共 6 条直接相关测试通过。
  - `cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次，均通过。按本轮主控指示不跑全套。
- 拿主意的地方：全部沿用主控对八条意见的取舍；fallback 只作用于明确等过锁的 not_found，不把其他错误或其他实例的记录改成成功，没有新的待定事项。
- 没做的事：未改第 5–8 条对应的实现，未改其他仓库、R52 文件、DESIGN、HANDOFF；未操作用户 agent，未合并、推送、部署。
- 踩过的坑：没有；两条预期 RED 都直接复现了审查所述问题，采用指定改法后通过。

### 复核后返工

2026-10-25，ranch/dev-r51。已读主仓库 `docs/任务/R51-审查.md` 末尾「复核」，按主控决定以共享锁方案同时处理第 1、2 条，替代上一轮排他锁排队及 not_found fallback。

- 主控决定及做了什么：stop 对 stop.lock 取 `LOCK_SH`，几个 stop 可以同时持锁，拿到锁后各自立刻发停止请求。删除「等过锁」标志和容忍 not_found 的分支，恢复 status → 共享锁 → 停止请求 → 等待 → 读取 exit.json 的流程；取锁重试及等停止仍使用同一 deadline。ls 保持非阻塞 `LOCK_EX`，任一 stop 持锁即跳过，不主动创建 stop.lock。CONTRACT 相应写明共享／排他锁、多个 stop 均返回 ok:true、退出码 0 和同一份记录，去掉上一轮关于等过锁及 not_found 的两句。
- 释放与 inode 核对：StopLock 的 Drop 先在自己的描述符上尝试 `LOCK_EX | LOCK_NB`；任何失败都直接结束 Drop、关闭描述符，不再做删除判断，也不假设共享锁仍保留。升级成功后才取运行锁，并核对路径和所持文件 inode 一致再删除。取锁共用原来的 inode 复核循环：成功 flock 后路径已删或 inode 已换，就关闭旧描述符并重新打开、取锁；stop 的共享模式也走此循环，避免接受已删 inode 上的锁。没有改变 pen 协议、写退出记录的次序或 ls 的缺文件路径。
- 新测试：保留原有并发 stop 测试，新增 `stop_concurrent_callers_keep_the_record_across_listing`。合成 pen 通过真实 socket 接收两个 CLI 的 status／stop；共享锁实现收到第二个停止请求后，明确对本测试第二个 CLI 发 SIGSTOP，并用 waitpid 确认停住。随后写退出记录、关闭 pen socket、放运行锁，等第一个 stop 结束，跑真实 ls，再 SIGCONT 第二个，断言三个命令均退出码 0、两个 stop 的完整输出相同且记录非空，并确认后续 ls 能清理目录。当前排他锁基线无法发出第二个停止请求，测试在有界等待后暂停其等锁进程，仍执行同一段 ls 插入顺序；失败信号来自第二个 CLI 的真实退出码，不把夹具等待超时当作 RED。新测试的 CLI 子进程有 Drop 清理，断言失败时也只杀并回收自己启动的两个 PID。
- 验证了什么：全部使用本任务的 `CARGO_TARGET_DIR`，命令均在前台等待结束。
  - 有效 RED：生产代码仍为 `6999b3c` 时运行 `cargo test -p corral-core --test protocol stop_concurrent_callers_keep_the_record_across_listing -- --exact --nocapture`，第二个 stop 返回 `not_found`、`exited:null`、退出码 2，预期为 0，测试失败（5.83 秒）。
  - GREEN：共享锁修改后，同一命令通过（0.87 秒）。第二个 stop 已持锁且发出自己的停止请求，第一个结束和一次 ls 之后仍返回与第一个相同的记录。
  - `cargo test -p corral-core --test protocol stop_ -- --nocapture`：5 通过（6.72 秒），包含两条并发 stop、实例隔离、旧 ls 遗留目录及不新建锁、停止超时。
  - `cargo test -p corral-core --lib pen::tests::final_kill_wait_is_bounded_and_records_remaining_processes -- --exact --nocapture`：通过（10.17 秒）；`cargo test -p corral-core --test protocol concurrent_unique_starts_and_listing_preserve_live_locks -- --exact --nocapture`：通过（1.27 秒）。共 7 条直接相关测试通过。
  - `cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次，均通过；按主控指示不跑全套。
- 拿主意的地方：严格采用主控的共享锁方案，未选择记录缺失时返回 null 的排队补救，也未额外加入 pen 协议或重开行为改动。原有同名重开窗口、双向旧版混用边界保持。paddock 不用动，没有新的待定事项。
- 没做的事：未改其他仓库、R52 文件、DESIGN、HANDOFF，未操作用户 agent，未合并、推送或部署。
- 踩过的坑：新夹具最初给已关闭的空探测 socket 设置读取超时，macOS 返回 EINVAL（0.82 秒）；这次失败不算 RED。改用带统一截止时间的非阻塞读取，并给测试自己的 CLI 加 Drop 回收后，才取得上述真实退出码失败；生产修复之前没有把夹具错误当成目标缺陷。
