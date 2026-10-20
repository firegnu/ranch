# 任务：分工表加改表命令、格子说明、可选项和写入防冲突，给 paddock 设置页可视化编辑用

2026-10-20，ranch/main 交给 ranch/dev-r35（Codex，gpt-6-astra / high）。
路由：重 / 交叉审查不要 / 影响面：改行为（路由：重（置信 0.91）、拿不准、拿不准；交叉审查和影响面由主控判：只写用户的 `~/.ranch/dispatch.toml`，写坏了能看出、能手改回，不涉及数据模型、安全；防冲突只是写前比对，不是并发和事务）
类型：功能变更
依据：R31 让主控派活时现查分工表，表可以由本机 `~/.ranch/dispatch.toml` 覆盖，但只能手改文件。用户要在 paddock 的设置里用连线图看、改这张表；按规矩前端只调 ranch 的命令，不读写 ranch 的文件。本轮在 ranch 这边补上 paddock 要用的：改表命令、每格一句说明、可选项、写入防冲突。不改分工（那是 R34，往后放），不做 paddock 的界面。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-20：「我有一个想法，想在paddock中settings有一个专门的面板可以使用那种连线的方式去展示这个关系，而且可以定制修改。修改后关联的表格也就修改了（我觉得甚至都不用重新读取skill，因为表格的修改是实时读取的）」
- 10-20：主控说 ranch 要补「改表命令、格子说明、可选项清单、写入防冲突」，用户：「R34往后放，先把这个拆解出来，你的任务你做，paddock的任务padck做」

## 先读

- `AGENTS.md`「规矩」，特别是「前端只用公开约定」「改动要写明对 paddock 的影响」「实现中要改设计，先改 DESIGN.md」。
- `docs/CONTRACT.md`「`ranch dispatch cell` / `table`（R31）」一节。
- `docs/任务/R31-再研究下路由，主要是omp和pi的分工.md` 的完成记录。
- `crates/dispatch/src/table.rs`、`crates/dispatch/resources/dispatch.toml`、`crates/ranch/src/main.rs`、`crates/ranch/tests/dispatch_table.rs`。
- `crates/dispatch/resources/corral-dispatch/SKILL.md` 第 3 节：各格对应分工表哪一条，说明照它写。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r35-dispatch-edit`，分支 `r35-dispatch-edit`（已从 main 建好）。只动 `crates/dispatch/`（技能文字除外）、`crates/ranch/`、`docs/DESIGN.md`、`docs/CONTRACT.md`、`docs/派活路由规则.md` 里写读表的地方、`Cargo.lock`，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r35-dispatch-edit`。

## 要做的

1. **改表命令**：改某一格的这几样：交给哪几家、模型、强度、交叉审查格子、返工格子（每样都能清成「没有 / 各自默认」）；把某一格恢复成默认（删掉本机文件里对这一格的覆盖）。写进 `~/.ranch/dispatch.toml` 时只动那一格，文件里别的内容、顺序和注释原样保留；文件或目录不存在就新建。写之前用读表的同一套检查，不合格就拒绝、说明哪里错、文件不动；写入要么整个成功要么不动，不留半个文件。各家的启动命令模板不能用命令改，只读。
2. **格子说明**：默认表每格加一句中文说明，照技能第 3 节那一条的意思写（例如「其余：照方案实现、修 bug、补测试、批量机械改动、查事实的调研」）。本机文件里也能写、能覆盖。`cell`、`table` 输出带上它。
3. **可选项**：`table` 输出带上每家能选哪些强度、建议哪些模型，给界面做下拉用。默认：Claude Code 强度 medium、high、xhigh，建议模型 `opus[1m]`、`sonnet`；Codex 强度 medium、high、xhigh，建议模型 `gpt-6-astra`；pi、omp、amp、agy 不分强度、不带模型。放在数据里，本机文件能改。改表命令拒绝那一家不认的强度；模型不限于建议列表。
4. **写入防冲突**：`table` 输出带上本机文件当前的版本（文件不存在也有一个值）。改表命令可以带上这个版本，文件在这之间被改过（用户手改、别的窗口改）就拒绝，回一个能和别的错误区分开的错误，文件不动。不带版本照常写。
5. 读表的现有输出、错误和退出码保留，只做兼容新增。先改 DESIGN，在 CONTRACT 里写清新命令、参数、JSON 字段、错误和退出码（paddock 照这个做界面），再实现。写清对 paddock 的影响。

## 怎么算做完

- 用户原话：「可以定制修改。修改后关联的表格也就修改了」：用 ranch 的命令改一格，`~/.ranch/dispatch.toml` 跟着变，下一次 `ranch dispatch cell` 读到的就是新的；恢复默认后读到的回到默认。
- 用户原话：「甚至都不用重新读取skill，因为表格的修改是实时读取的」：改表不需要重装技能、不需要编译。
- paddock 只调 ranch 的命令、读 JSON，就能画出每格连到谁（含说明、审查、返工）、给出下拉选项、发现文件被别处改过。
- 验证只做这些：
  - 先写测试再实现：隔离 HOME 下，改一格只那一格变、文件里别的内容和注释还在；文件不存在时新建；恢复默认；不合格的改动（不认识的家、那家不认的强度、指向不存在的格子、返工绕成圈）被拒且文件不动；版本对不上被拒且文件不动；说明和可选项出现在输出里。改程序前先跑一次看它失败。
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次，加 `git diff --check`。
  - 编译目录里的 `ranch` 在隔离 HOME 下改一格、读一格、恢复默认，看输出。
  - 觉得不够，在回复里说，不要自己加。

## 不要做

- 不改分工：默认表各格的家、模型、强度、审查、返工都不变（只加说明和可选项）。不改技能文字。那是 R34。
- 不改 `ranch dispatch route`。
- 不改 paddock，不做界面。
- 不动真实的 `~/.ranch`、`~/.claude`、`~/.agents`、`~/.gemini`；不装技能。
- 不改 corral、fence。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 保留格式写 TOML 做不到（会丢用户的内容或注释），停下来报告，说清原因和可选办法，不要自己换别的办法。
- 不合并到 main，不推送。只在 `r35-dispatch-edit` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：新增 `ranch dispatch edit <格子>`、`reset <格子>`，局部修改或删除本机覆盖，模型、强度、审查、返工、可选家都可清空，说明也可修改。写前用读表的全表校验，拒绝未知家、不支持的强度、未知引用和循环；使用 `toml_edit` 保留原键、注释和顺序，同目录临时文件同步后原子替换。默认 12 格新增中文说明，各家新增数据驱动的强度与建议模型，原分工和默认启动命令不变。`table.local.version` 与可选 `--if-version` 提供写前冲突检查，冲突返回 `conflict` / 退出 3。先更新 DESIGN / CONTRACT 后实现，路由规则读表说明和 README 已同步。
- 验证了什么：按 CLI / 隔离 HOME 分步先写测试再实现；各步分别因缺说明、未知 edit 命令、不能清空模板字段、未拒绝非法强度、缺版本字段失败，再实现后通过。全量 `cargo test --all-targets` 跑一次，196 通过、0 失败、11 个原有测试忽略；`cargo clippy --all-targets -- -D warnings` 跑一次通过。主控决定格式限制后追加点号键拒写测试，先确认旧实现意外成功，再补保护，相关 `cargo test -p ranch --test dispatch_table` 11 项全过（包含原 R31 回归）；未重跑全量与 clippy。最后 `cargo fmt --check` 通过。编译目录的 ranch 在临时 HOME 下完成改一格、读一格、恢复默认，输出和落盘内容正确，临时目录已清理。提交前 `git diff --check` 通过。
- 拿主意的地方：`--agents` 使用 JSON 字符串数组，其他字段用空字符串清空；模型和强度清空时省略对应模板参数及标签，仍由每家自身默认决定。版本是原文件字节的 SHA-256，不存在为 `missing`；开始读取及最后替换前比对，不承诺并发事务。写成功回复整张表及新版本。对 paddock 是兼容新增命令、字段和独立错误，设置页按 CONTRACT 接；说明/选项写进本机表后，读写方都须使用新版 ranch，技能不用重装。
- 主控决定：发现 `toml_edit` 不保证交错点号键顺序后暂停并报告；用户要求交主控决定，已用 corral 联系 `ranch/main`。主控选择继续用库、不自写源码位置替换，写前先核对未修改原文解析再输出是否逐字节相同；不相同就报 `format_not_preserved` / 退出 1、文件不动，提示手改或换普通 `[cells.NAME]` 写法。按要求补了拒写测试和 README / CONTRACT 限制说明；只读仍支持这些合法 TOML。命令新建覆盖使用普通表写法。
- 没做的事：未改默认分工、技能文字、route、corral、fence、paddock；未操作真实配置或用户 agent（仅按用户明确授权向主控询问实现取舍），未安装、部署、合并或推送。没有扩大任务验证预算；只追加了主控明确要求的一条格式拒写测试。
- 踩过的坑：直接用 `toml_edit` 替换键会丢键前注释，保留测试抓到后改为保留原键、只替换值；库对交错点号键会重排，已按主控决定增加原样往返检查。联系主控时最初写了任务单的 agent 名称，实际名称带 `-1`，随后已补正，主控确认收到。
- 要主控决定的事：没有新增待定事项；格式边界已由主控决定。实现留在本分支，待主控审查。
