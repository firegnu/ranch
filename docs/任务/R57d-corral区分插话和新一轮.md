# 任务：corral 状态里加一个「真正换一轮才变」的标记，让调用方分得清插话和新一轮

2026-11-09，ranch/main 交给 ranch/dev-r57d（Codex，gpt-6-astra / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.79）、要、碰要害）
类型：功能变更
依据：本轮给 corral 的 `status`（及 `ls`、`wait` 里的同一份状态）兼容新增一个字段，插话时不变、新一轮开始时才变；现有字段含义一律不变。不改 pony、paddock；R57c（pony 插话）等这件做完再接着做。用户 11-09 同意先做这件（「继续吧」）。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent 替你干活（实测用的测试 agent 除外）。

## 用户原话

- 11-09：「现在委派出去的agent在工作的时候是没有办法send过去东西的，也就是说不能插话。……我理解的是是可以插话的。」
- 11-09（R57c 开发停下报告：pony 每 2 秒查一次状态，分不清「同一轮里插了话」和「这一轮被打断后马上开了新一轮」，旧「打断」按钮可能按到新活上；主控建议先补 corral）：「我原则上统一1，但是会不会加了这个影响paddock，因为paddock已经在开发了」。主控答：只加字段、旧字段不变，CONTRACT 规定前端忽略不认识的字段，paddock 不受影响。

## 起因

- 插话会让 corral 刷新 `turn_started`（R57b 有意不改它的含义）。pony 有十几处靠 `turn_started` 判断「这一轮结束没有、回复贴哪、按钮还算不算数」。
- pony 每 2 秒轮询一次，两次之间「插了一句」和「被打断后马上开了新一轮」可能看到一模一样的 `status`、`reply`。只有逐条读事件的 corral 分得清：一句输入进来时，这一轮是还在进行，还是已经结束（正常结束、被打断、出错）。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r57d-run-id`，分支 `r57d-run-id`（已从 main 建好）。只动 `crates/corral/`、`docs/DESIGN.md`、`docs/CONTRACT.md` 和本文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r57d-run-id`。

## 先读

- `docs/DESIGN.md`「干活时显式插话（R57b）」一节，`docs/CONTRACT.md`「`corral send NAME TEXT --steer`（R57b）」一节。
- `docs/任务/R57c-pony干活时插话.md`：pony 要拿这个字段做什么。
- `crates/corral/src/events.rs` 及各家读事件的地方：`turn_started`、`interrupted`、`state` 怎么随事件变；内部 cursor 缓存的版本号和新旧版本交替读的规矩（R22、R25 等有先例）。

## 要做的

1. **先改设计**：照 AGENTS.md，先在 `docs/DESIGN.md` 写清新字段的含义、怎么算、各家的边界，再写代码；`docs/CONTRACT.md` 写清字段名、取值、和 `turn_started` 的区别，以及对 paddock 是兼容新增。字段名自己定，写进 DESIGN。
2. **含义**：一句输入进来时，这一轮还在进行（还没收到这一轮的结束、打断、出错事件）就不变；这一轮已经结束、或这是第一句，就换成新值。被打断后接着开的新一轮必须换新值，哪怕中间没有被观察到 idle。新值要能让调用方只靠比较是否相等就判断「是不是同一轮」。
3. **各家**：Claude Code、Codex、omp（支持插话的三家）必须准确；其余各家至少不比 `turn_started` 差（例如认不出结束事件的家，可以和 `turn_started` 一样变，DESIGN 里写明）。
4. **兼容**：`turn_started` 及所有现有字段、`wait`、`reply`、`send --after` 的行为不变。新旧版 corral 交替读同一个 agent 时各自正确（照先例给内部缓存升版本）。旧版 corral 输出里没有这个字段，调用方要能分辨「没有这个字段」。尽量不需要对在跑的 agent 做 `corral upgrade`；需要的话在 DESIGN 里写清为什么、不升级时会怎样。

## 怎么算做完

- 用户要的结果：pony 能靠这个字段分清插话和新一轮；paddock 不受影响。
- 新加测试（合成 agent、隔离 HOME 和 CORRAL_HOME）：同一轮里插话不变；正常结束后新输入换新值；被打断、出错后马上来的新输入换新值（中间不经过可观察的 idle 也要换）；Claude Code、Codex、omp 各自的事件都覆盖；新旧版本交替读同一个 agent 各自正确。
- 现有测试原样全过，不改断言、不放宽超时。
- Claude Code、Codex、omp 各开一个测试 agent 实测一次：干活时 `send --steer` 插话后字段不变，下一句新输入后字段变了。
- 验证只做这些：上面的测试和实测；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 pony、paddock、派活技能；不改 `turn_started` 等现有字段的含义。
- 不碰 `corral ls` 里不是你开的 agent，不对它们 `corral upgrade`；实测用 `ranch/test-<名字>`、`--label role=test`、临时目录，用完 `corral stop`。不读本机其他 agent 的事件日志和状态文件。
- 不要按项目名或路径批量杀进程（`pkill -f <项目名>` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送，不打包部署。只在 `r57d-run-id` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
