# 任务：把现有分工表改成数据表，主控派活时调命令读表；分工本身不变

2026-10-20，ranch/main 交给 ranch/dev-r31（Codex，gpt-6-astra / high）。
路由：常规 / 交叉审查不要 / 影响面：改行为（路由：常规（置信 0.35）、要、碰要害；推翻：只把技能里写死的「交给谁、什么模型、什么强度」搬进数据表、加读表命令，分工不变；读表结果主控每次都看得见，写错了改一行数据就能改回，不涉及数据模型、并发、安全；按改行为走，不交叉审查）
类型：重构
依据：现在 corral-dispatch 技能里写死了交给哪家、什么模型、什么强度、启动参数。技能编进 ranch，改一个字都要打新版本、重装技能。本轮只把这部分搬进数据表：ranch 自带默认表，用户本机放一份就能改；主控派活时调命令读表。默认表照现在的分工原样填，派活结果和现在一样。改分工是下一件 R34。
提示：围绕指定的结构目标调整，保持约定的外部行为不变。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-20：「我还有一个需求，这个需求一旦改了，我就要编译ranch或者分发skill。如果中间我想改，怎么弄？」
- 10-20：主控提议把分工表做成数据、派活时调命令查表。用户：「我觉得先做这个，把调整分工的从这里面剥离出去。」「把这个任务先分成两个任务书」「我的理解是两份任务书应该是一份是：改成表以及读表 一份是在现有的基础上改分工。」

## 先读

- `AGENTS.md`「规矩」，特别是「前端只用公开约定」「改动要写明对 paddock 的影响」「实现中要改设计，先改 DESIGN.md」。
- `crates/dispatch/resources/corral-dispatch/SKILL.md` 第 3 节（分工表、补模型和强度的规则、分档表、很费 token 的配置）、第 4 节（agent 命令表、`--label`）、第 6 节（交叉审查用哪一档）。
- `crates/dispatch/src/`（`route` 怎么出 JSON、怎么报错）、`crates/ranch/src/main.rs`（命令入口和用法）。
- `crates/fence/README.md` 开头：fence 的「内置数据 + 用户文件覆盖」怎么做，可以照它的思路。
- `docs/派活路由规则.md` 第 3、5 节。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r31-dispatch-table`，分支 `r31-dispatch-table`（已从 main 建好）。只动 `crates/dispatch/`、`crates/ranch/`、`docs/DESIGN.md`、`docs/CONTRACT.md`（如果 `ranch dispatch` 的命令写在那里）、`docs/派活路由规则.md`、`Cargo.lock`，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r31-dispatch-table`。

## 要做的

1. **数据表**：表里是一组有名字的格子。每格写：交给哪家（可以是 pi、omp 这样任选其一的两家）、模型、强度、交叉审查用哪一格（可以没有）、返工后换到哪一格（可以没有，没有就照技能停下问用户）。各家的启动命令和标签写一次，各格引用。格子的名字和个数由数据决定，程序里不写死某几个格子：下一件 R34 要把格子换成「档位 × 类别」，只改数据和技能文字，不改程序。
2. **默认表**编进 ranch，照现在技能第 3、4、6 节的分工原样填（见下面「默认表」）。
3. **本机覆盖**：`~/.ranch/dispatch.toml` 存在时，只写要改的格子或要改的某一家，其余照默认表；也可以加新格子。不存在就用默认表。文件写错（格式不对、不认识的键、引用不存在的家或格子、换格子绕成圈）直接报错、指出哪里错，不悄悄退回默认表。
4. **读表命令**：给一个格子名，回 JSON：交给哪家、完整的 agent 启动命令、`--label` 要带的标签；有交叉审查格子的，一并回它的启动命令和标签；有返工换格子的，回换到哪一格。另外能列出当前生效的整张表（每格来自默认还是本机文件），方便用户照着改。命令名、参数、JSON 字段由你定，风格照 `ranch dispatch route`，写进用法说明和 DESIGN、CONTRACT 里该写的地方。
5. **技能改成读表**：第 3 节的分工表还是那 9 条判断，每条的结果从写死的「家 / 模型 / 强度」改成「调读表命令取某一格」；分档表、第 4 节的 agent 命令表、第 6 节交叉审查的启动参数，都改成以读表命令回的为准。判断条件、「说得清」检查、sonnet 试用和「满 5 件汇总」、很费 token 的配置先问用户、返工、审查的规矩，文字意思都不变。`--env CAIRN_DISABLE=1`、首句写法、amp 不带 `--prompt`、agy 的写法照旧。
6. `docs/派活路由规则.md` 同步成读表的说法。写清对 paddock 的影响。

## 默认表（照现在的技能）

| 格子（对应现在分工表） | 交给 | 模型 / 强度 |
|---|---|---|
| 碰要害、设计和界面（第 4 条） | Claude Code | `opus[1m]` / high |
| 碰要害、其余（第 4 条） | Codex | `gpt-6-astra` / high |
| 杂活（第 5 条） | pi 或 omp | 各自默认，不带强度、标签 |
| 出方案（第 6 条） | Claude Code | `opus[1m]` / high |
| 说得清的界面活（第 7 条） | pi 或 omp | 各自默认 |
| 其余界面活（第 8 条，试用） | Claude Code | `sonnet` / high |
| 其余（第 9 条） | Codex | `gpt-6-astra` / medium |
| 其余，难的（第 9 条） | Codex | `gpt-6-astra` / high |
| 交叉审查（第 6 节） | Codex | `gpt-6-astra` / xhigh |

- 第 4–9 条的交叉审查格子：碰要害那两格指向「交叉审查」，其余没有。返工换格子：都没有（现在是返工两次停下问用户）。
- 第 3.1 节「点名只定了家时怎么补模型和强度」的规则照旧写在技能里；需要的话，表里可以再放「某家 medium / high」这类格子给它用，由你定。
- 启动命令（照现在第 4 节）：
  - Claude Code：`claude --dangerously-skip-permissions --model '<模型>' --effort <强度>`
  - Codex：`codex --yolo --disable memories -m <模型> -c 'model_reasoning_effort="<强度>"'`
  - pi：`pi`
  - omp：`omp --approval-mode yolo`
  - amp、agy 照现在第 4 节，只在点名时用。
- 标签：Claude Code、Codex 带 `effort=<强度>`、`model=<模型>`；pi、omp、amp、agy 不带。

## 怎么算做完

- 用户原话：「如果中间我想改，怎么弄？」：用户在 `~/.ranch/dispatch.toml` 改一格，不编译、不重装技能，下一次读表就是新结果。
- 用户原话：「一份是：改成表以及读表」：不放本机文件时，每一格读出来的家、模型、强度、启动命令、标签，和现在技能里写的一样；技能里不再写具体的模型名和启动参数，判断规则不变。
- 验证只做这些：
  - 先写测试再实现：默认表每一格读出的启动命令和标签与上表一致；本机文件只改一格时只那一格变；加新格子能读出来；上面列的几种写错都报错。测试用隔离的 HOME。改程序前先跑一次看它失败。
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次，加 `git diff --check`。
  - 编译目录里的 `ranch` 在隔离 HOME 下读一格、列整张表，看输出。
  - 觉得不够，在回复里说，不要自己加。

## 不要做

- 不改分工：不改哪条交给谁、什么模型、什么强度，不改判断条件、审查、返工的规矩。那是 R34。
- 不改 `ranch dispatch route` 的判法和它问的问题。
- 不动真实的 `~/.ranch`、`~/.claude`、`~/.agents`、`~/.gemini`；不装技能。
- 不改 corral、fence，不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 现在技能里哪一处没法原样放进表、读出来会和现在不一样，停下来报告，说清原因和可选办法，不要自己改分工去凑。
- 不合并到 main，不推送。只在 `r31-dispatch-table` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：新增编入 ranch 的 `crates/dispatch/resources/dispatch.toml`，默认九格保持原分工，另有 `claude-medium`、`amp`、`agy` 三格供点名时补缺省部分。新增 `ranch dispatch cell <名字>`、`ranch dispatch table`，返回家、模型、强度、完整 argv / shell 命令、标签、交叉审查启动信息、返工格子和来源。`~/.ranch/dispatch.toml` 按字段覆盖、支持新家和新格子，每次重新读取；格式错误、未知键、未知引用和引用循环均报错。技能九条判断改为读表，DESIGN §9、CONTRACT、路由规则和 dispatch README 已同步。
- 验证了什么：先写隔离 HOME 的 CLI 集成测试，改程序前运行，5 项全部因 `unknown command` 失败；实现后同一组 5 项全部通过，覆盖默认每格命令与标签、只改一格、只改一家、新名字与关系、错误格式/键/引用/循环。独立编译目录下 `cargo test --all-targets` 跑一次，191 通过、0 失败、11 个原有测试忽略；`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次通过。编译目录的 `ranch` 在临时 HOME 下读 `core-code`、列整表，输出正确，临时目录已移除。提交前 `git diff --check` 通过。
- 拿主意的地方：命令用 `cell` / `table`；家模板按 argv 数组保存，只支持模型和强度两个占位符，shell 命令由参数数组引用生成。局部覆盖按字段合并，数组整项替换，可选字符串用空字符串清除；不支持删除内置条目。格子和家模板分别标来源，审查信息只展开一层；校验交叉审查、返工及混合引用环。技能仅在 sonnet 试用计数和昂贵配置检查中保留必要的模型名称，分工和启动参数不再写死；返工规则不变，数据不自动执行换人。对 paddock 是兼容新增，现有调用不用改；首次需新版 ranch 与技能一起部署，此后改本机表不必编译或重装。
- 没做的事：没有调整分工或 route 判法，没有改 corral、fence、paddock，没有操作真实用户配置、现有 agent 或安装技能；没有部署、合并或推送。未增加任务预算之外的验证。
- 踩过的坑：没有。
- 要主控决定的事：没有；分工调整仍留 R34。
