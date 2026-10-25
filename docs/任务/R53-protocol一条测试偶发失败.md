# 任务：protocol.rs 一条测试别偶发失败

2026-10-25，ranch/main 交给 ranch/dev-r53（Codex，中档：gpt-6-astra / high）。
路由：常规 / 交叉审查不要 / 影响面：改行为（路由：常规（置信 0.73）、拿不准、拿不准；只改测试、不碰程序，主控定不交叉审查、按改行为验）
类型：Bug 修复
依据：本轮只修这一条测试的偶发失败；corral 程序行为不改。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-25：主控问要不要另开小活修 `protocol.rs` 那条偶发失败的测试，用户：「测试那个也安排修一下」。

## 先读

- `HANDOFF.md`「已知没修的」里「全套测试偶有 1 个失败」那一条。
- `crates/corral/tests/protocol.rs` 的 `injected_hooks_use_the_namespace_entry_and_adapter_arguments_are_preserved`。
- `docs/任务/R52-两处测试的残留和偶发失败.md` 的完成记录（同类问题，sandbox 测试的修法可参考）。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r53-protocol-flaky`，分支 `r53-protocol-flaky`（已从 main 建好）。只动 `crates/corral/tests/protocol.rs`（必要时它用的测试辅助代码），以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r53-protocol-flaky`。

## 要做的

`injected_hooks_use_the_namespace_entry_and_adapter_arguments_are_preserved` 在 R51 复核时全套测试里失败过一次（越界 panic），单独复跑 3 次都过。当时的猜测是：假 claude 用 shell 重定向写 `events.args`，测试只等文件存在就读，内容没写完。猜测没有核实。找出原因，让它不再偶发失败。

## 怎么算做完

- 写清失败的原因和依据；修好后这条测试反复跑不再失败。
- 验证只做这些：修之前先用能看出问题的办法确认问题在（能稳定做出失败最好，做不出就在完成记录里写清依据）；修后这条测试连跑 20 次；最后 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 `crates/corral/src/` 里的程序代码。查下来原因在程序里，停下来报告，不要自己改。
- 不改别的测试；HANDOFF 那一条里提到的其他偶发失败不在本轮范围。
- 不靠删测试、放宽断言或单纯加长超时来通过。
- 不碰用户的 agent：不对 `corral ls` 里现有的 agent 做 `stop`、`send`、`keys`、`upgrade`、`attach`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r53-protocol-flaky` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：只改目标测试的假 claude 脚本，先把参数写到同目录的 `events.args.tmp`，`printf` 成功完成后用 `mv` 原子发布 `events.args`。原有的文件存在检查因此只能看到完整内容；所有参数、hook、upgrade 断言及 3 秒等待上限均保留。没有改程序行为或公开约定，paddock 不用动。
- 原因与修前证据：原脚本的 shell 重定向先创建 `events.args`，然后才执行 `printf`；测试只等文件存在，可能读到空内容，随后 `args[2..]` 越界。修前临时改为 `{ sleep 0.2; printf ...; } > "$CORRAL_EVENTS.args"`，只放大创建文件与写入之间已有的调度窗口。运行 `cargo test --test protocol injected_hooks_use_the_namespace_entry_and_adapter_arguments_are_preserved -- --exact --nocapture`，在原断言处复现 `range start index 2 out of range for slice of length 0`（1 failed，退出码 101）。这确认了实际存在的竞态并复现同类越界；没有 R51 当次的完整运行现场，不能追溯证明历史失败的全部时序。临时停顿已移除。
- 验证了什么：全部 Cargo 命令使用 `.target/r53-protocol-flaky` 独立编译目录并在前台等到退出。修后目标测试以 `--exact` 连续运行 20 次，20 次全部通过；最后 `cargo test --all-targets` 一次通过（502 passed、17 ignored、0 failed），`cargo clippy --all-targets -- -D warnings` 一次通过，`cargo fmt --check` 一次通过。自查 diff 只有本测试的脚本、说明注释及本完成记录，没有诊断停顿、断言放宽或超时修改。
- 拿主意的地方：采用同目录临时文件写完再发布，直接消除读半份文件的窗口，不引入固定等待或额外辅助代码。没有需要主控决定的事。
- 没做的事：不改 `src/`、其他测试、HANDOFF、设计文档或其他仓库；不碰用户 agent，不合并、不推送、不部署，只在任务分支提交。按任务验证预算，没有额外扩展测试。
- 踩过的坑：没有。
