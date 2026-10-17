# 任务（草稿）：agy 派活修整：状态兜底、免确认、信任目录

2026-10-17 起草，ranch 主控（ranch/main）自己做。第 1 部分（实测）已做完；第 2–4 部分用户 10-17 看过，开工。
执行：主控
路由：常规 / 交叉审查要 / 影响面：碰要害（路由不可用：调用被 Claude Code 自动权限检查拦下，没有拿到结果。第 4 部分要改写用户的 agy 配置，多个 pen 可能同时写，算碰要害，交叉审查照规矩给 Codex astra / xhigh；第 2、3 部分单看是「改行为」）
类型：第 1 部分调研实测（已完成）；第 2 部分改技能内容；第 3、4 部分功能变更

## 用户要的

- 10-17：“现在我会委派一些活到agy，查一下委派agy整条路径有没有不合适的地方”
- 10-17：“先测一下，写任务文件”
- 10-17：“另外还有一个问题，委派出去的agy的permission我觉得应该是类似claude或者codex这种是bypass的”
- 10-17：“5次，Claude 不动，第2部分等实测出来之后，和以后的修复一起部署。”
- 10-17：“在同一个目录里重测5次，那5行也删掉”
- 10-17（信任框）：“关于3， 现在我看codex或者claude code并不会出现这个问题”；“但是这样的话，agy会增长很快吧？”；主控答「开 agy 时加、它退出时删、下次加时清掉目录已不在的」后：“好，写进任务文件”

## 现场（10-17 主控排查所得）

- paddock 主控 16:23:30 派出 `paddock/dev-card-details-1`（agy 1.4.0，带 `--dangerously-skip-permissions --prompt-interactive=…`）。agy 一直在干活，但：
  - 前 6 分钟 corral 一条钩子事件都没收到；16:29:51 之后才有 `UserPromptSubmit`、`PostToolUse`，后来也有了 `Stop`；
  - `SessionStart` 始终没来，事件都积在 `pending`，状态一直是 `starting`（paddock 显示叹号）。`corral wait` 等不到 `idle`，DONE 提醒不会来，`corral reply` 拿不到回复。
- agy 日志（`~/.gemini/antigravity-cli/log/cli-20261017_162330.log`）：首句 16:23:40.28 就送进了会话，登录到 16:23:41.06 才完成；那次组装提示词时 `skills`、`mcp_servers`、`user_rules` 三段都是空的。16:29:51 重新组装后，钩子才开始工作。
- agy 10-17 自己升到了 1.4.0（R8 时是 1.3.5）。
- 免确认：派活技能里 agy 已经是 `agy --dangerously-skip-permissions`，上面那个 agy 的日志也写着 `auto-approving all tool permissions`。corral 技能（`crates/corral/resources/SKILL.md`、`AGENT_USAGE.md`）里开 agy 只写 `-- agy`，不免确认；同一处 Codex 是 `--yolo`、omp 是 `--approval-mode yolo`、Claude Code 是 `claude`（也不免确认，用户定不动）。
- 信任框：三家各有一份信任记录（Claude Code `~/.claude.json` 的 `hasTrustDialogAccepted`，Codex `~/.codex/config.toml` 的 `trust_level`，agy `~/.gemini/antigravity-cli/settings.json` 的 `trustedWorkspaces`），区别在怎么查：
  - Claude Code 往上找上级目录，Codex 归到所属主仓库，所以派活开的 worktree 不弹框，记录也不随 worktree 增长（Claude Code 记录里 worktree 只有 5 条、Codex 1 条，都不是 ranch、paddock 派活的）；
  - agy 只认目录本身。列表里有 `/Users/firegnu`、`~/Developer/personal_projs`，新目录照样弹。agy 的命令行参数和程序里都没找到跳过信任框或认上级目录的选项。

## 要做的

### 1. 实测抢跑（已完成，见文末「实测记录」）

10 次 0 次复现；推测是 agy 登录慢时不等登录就送首句。暂不改首句的送法。

### 2. corral 技能里 agy 改免确认

- `crates/corral/resources/SKILL.md`、`AGENT_USAGE.md` 里开 agy 的命令改成 `-- agy --dangerously-skip-permissions`，和派活技能一致。Claude Code 不动。

### 3. agy 的状态兜底（缺了「会话开始」也能跟上）

- 现在：没收到 `SessionStart` 就没有主会话，之后的事件全积在 `pending`，状态停在 `starting`。
- 改成：**只对 agy**（适配器声明，共用代码不写 agy 的名字），还没有主会话时，收到 agy 的事件就把它的 `session_id` 当主会话，积压的事件照常算上。已有主会话时照旧，不改别家。
- 管不到：钩子完全没挂上的那一段，状态仍是 `starting`；那一轮 agy 没加载技能和 MCP，是 agy 自己的问题。
- 已在跑、积压了事件的 agy（如 `paddock/dev-card-details-1`）部署新版后会不会直接恢复，实测确认，结果写进完成记录。

### 4. 派 agy 时替它补上「上级目录信任过就算信任」

- **加**：corral 开 agy 时（pen 启动 agy 之前），这个工作目录不在 agy 的 `trustedWorkspaces` 里、而它的某个上级目录在里面，就把工作目录加进去，并在 corral 自己的记录里记下「这条是 corral 加的」。上级目录都没被信任的，不加，照旧弹框由人点。
- **删**：这个 agy 退出时（`corral stop` 或它自己退出），pen 删掉自己加的那条。
- **兜底**：pen 被强杀没删成的，下次 corral 再加条目时，顺手清掉 corral 以前加的、目录已经不存在的那些。
- **只动自己加的**：用户自己点过信任的条目一概不删不改；文件里别的设置原样保留。
- 写法照全局安装的规矩：原子写（临时文件加改名），保留文件权限；多个 pen 同时写要互斥，不能互相覆盖；文件读不了、不是合法 JSON、或结构认不出时，什么都不写，照旧弹框。
- 要用真实 agy 实测：agy 运行时自己也会写这个文件（比如用户点信任时），要确认它不会拿内存里的旧内容把 corral 加的或删的那条覆盖掉；会的话停下来找用户商量。
- 动手前先在 `docs/DESIGN.md` §7 写明：corral 会为它开的 agy 改写 agy 的信任列表，范围和条件同上。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r13-agy`，分支 `r13-agy`。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r13-agy`。
- 真实 agy 实测：自己开 `ranch/test-agy-<n>`（`--label role=test`），**固定用同一个测试目录**，用完 `corral stop`、删目录。第 4 部分的实测会往用户的 agy 信任列表里加删条目，开测前告诉用户。

## 不做

- 不碰正在跑的 `paddock/dev-card-details-1` 和其他现有 agent，只读 `status`。
- 不改首句的送法（`--prompt-interactive`）。
- 不改 Claude Code、Codex 的信任记录，也不改它们在 corral 技能里的启动命令。
- 不改派活技能。

## 怎么算做完

- 用户 10-17：“验收按你的建议”。即：
  1. 第 2 部分：corral 技能里 agy 的命令带 `--dangerously-skip-permissions`；
  2. 第 3 部分：一条测试，造「没有 `SessionStart`、只有后续事件」的 agy 事件流，修之前停在 `starting`、修之后到 `idle` 并拿到回复；别家缺 `SessionStart` 时行为不变；
  3. 第 4 部分：测试覆盖加、退出时删、清掉残留、不动用户的条目、上级目录未信任时不加、文件坏了不写、并发写不丢；真实 agy 在一个新目录里开起来不弹信任框，退出后那条已删；
  4. `cargo test --all-targets`、clippy、fmt 全过；交叉审查到「可以合并」；新版本目录打好。

## 部署

- 第 2–4 部分一起打一个版本目录，部署（切链接、`corral install-skills`）由用户在普通终端里做（用户 10-17：第 2 部分和以后的修复一起部署）。

## 用户定的（10-17，第 2–4 部分）

- “第3部分也做，验收按你的建议，开工吧。”
- 复核第二轮后（“按照建议走”）：接受改名前最后一次检查和改名之间极短的窗口（agy 或编辑器恰好这时写设置文件，它们的改动会丢）；接受「agent 运行期间用户对同一目录取消再重新信任，退出时会被删」作为已知限制，第 4 部分「用户自己点过信任的条目一概不删不改」在这一点上放宽。

## 对 paddock 的影响

- 第 2 部分：技能内容有兼容的改动，paddock 不用动。
- 第 3 部分：只改内部实现，paddock 不用动；agy 的状态不再一直是 `starting`。
- 第 4 部分：corral 开 agy 时多了改写 agy 信任列表这件事，属于兼容的新增行为，paddock 不用动。paddock 用旧版 corral 开的 agy 照旧弹框。
- 在修好之前：paddock 派给 agy 的活，主控可能一直等不到完成，要人去看；每次派 agy 都要点一次信任。

## 实测记录（10-17，主控）

测试 agy 都是 1.4.0，命令照派活技能（`--dangerously-skip-permissions`、`--prompt`），首句「只回复一句：你好。最后一行写 DONE。」，用完都已 `corral stop`，临时目录已删。

| 组 | 目录 | 信任框 | 次数 | 首句在登录之后 | `SessionStart` 到了 | 拿到 DONE |
|---|---|---|---|---|---|---|
| 一 | 每次新建（`.agy-test-1`…`5`） | 每次都弹，用户在 paddock 里点掉 | 5 | 5（晚 0.8–2.1 秒） | 5 | 5 |
| 二 | 同一个已信任目录（`.agy-test-1`） | 不弹 | 5 | 5（晚 1.4–2.9 秒） | 5 | 5 |

- **抢跑没有复现**（10 次 0 次）。
- 对照启动日志的顺序：正常时「新建对话 → 约 1 秒后登录完成 → 再 1–3 秒送首句」；paddock 那次登录用了 11 秒（正常 5–6 秒），首句在新建对话后 5.4 秒、登录完成之前就送了。推测：agy 等登录有上限，登录慢时不等就送，这一轮便没有钩子和技能。10 次里登录都不慢，所以这条没法证实，也说不清出现频率。目前观察到的是 11 次里出 1 次。
- **agy 的信任只认目录本身，不继承上级目录**；点过一次后同一目录不再弹。派活每件活一个新 worktree，所以每次派给 agy 都要人点一次信任。
- 第一组 5 次每次新建目录，是主控测试设计的失误，用户因此点了 5 次信任。agy 把这 5 个目录记进了 `trustedWorkspaces`。用户要求删掉，主控去改时被 Claude Code 自动权限检查拦下，给了用户一条命令自己删。

## 完成记录（10-17，主控）

### 做了什么
- 第 2 部分：`crates/corral/resources/SKILL.md`、`AGENT_USAGE.md` 里开 agy 改成 `-- agy --dangerously-skip-permissions`。
- 第 3 部分：`Adapter::adopts_session`（只有 agy 为真）。`events::read` 多一个参数，读完事件后若还没有主会话，就认领 `pending` 里最早一条（带 transcript、本目录）事件的会话，按时间补算积压事件。`SessionStart` 的处理抽成 `make_main`，行为不变。
- 第 4 部分：新模块 `trust.rs`；`Adapter::trust` 声明信任列表（agy：`~/.gemini/antigravity-cli/settings.json` 的 `trustedWorkspaces`）。pen 在启动 agent 前调用 `trust::add`（HOME 用 agent 拿到的那个），agent 起不来或主循环结束（agent 已退出）后调用 `trust::remove`；出错只写 pen.log。记录在 `<HOME>/.corral/.runtime/trust.json`（配置文件、字段、路径、`CORRAL_HOME`、名字、实例、pen 进程号；加之前先记成「待确认」），加删都在同目录 `trust.lock` 的 `flock` 里做（最多等 5 秒）。改设置文件：临时文件写好，改名前核对还是同一个普通文件、内容没变，否则重来。细则见 DESIGN §7 R13。
- 先改了 `docs/DESIGN.md` §7（R13 一条）和 `docs/CONTRACT.md`（agy 的 `--prompt` 一节）。

### 验证了什么
- 第 3 部分先写测试再改：`tests/adapters.rs` 新增 `agy_that_lost_its_session_start_still_reports_its_turns`（假 agy 不发 `SessionStart`），改之前卡在 `starting`（失败），改之后到 `idle` 并拿到回复；`events.rs` 单元测试覆盖别家不认领、认领顺序、不认领别的目录、认领后 `SessionStart` 照旧换主会话。
- 第 4 部分：`trust.rs` 14 条单元测试：加删、不动用户条目、上级目录没信任不加、坏文件和符号链接不写、清残留、正在启动的不清、两个实例共用目录、几份 `CORRAL_HOME` 共用记录、加删失败的回滚和保留、写的同时被改不丢、12 线程并发加删（临时去掉加锁，3 次都失败）；复核第二轮后新增 3 条（加到一半断了不删列表里的目录、设置文件读不了时记录留着、写的同时被换成符号链接或别的文件不覆盖），都先在旧代码上确认因目标缺陷失败；复核第三轮后再加 1 条（删的时候文件被换掉、没删成，不能当成删完丢掉记录），把判断改回旧写法时失败。`tests/adapters.rs` 新增 `agy_is_trusted_under_a_trusted_directory_while_it_runs`，走真实 pen：claude 不动列表，agy 开着时有条目、停掉后没了。
- `cargo test --all-targets --no-fail-fast`：合并前（c282d8c 之后）122 通过、0 失败、3 跳过。复核第二轮修改后那次 `protocol` 一组的 `concurrent_unique_starts_and_listing_preserve_live_locks` 卡住一次（已知的 macOS 并发启动问题，悬着的 R9），按进程号停掉后单独连跑 3 次、整组重跑都通过；`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过。
- 真实 agy 1.4.0（分支编出的 corral，用户的真实 HOME，用户知情）：`ranch/test-agy-a`、`ranch/test-agy-b` 各开在一个新目录，都没弹信任框，都拿到 DONE；先停 b 只删 b，再停 a 删 a；设置文件内容、权限和测试前一致，没留临时文件，用户原有的条目（含没删的 5 条 `.agy-test-*`）没动。第一轮审查修改后用 `ranch/test-agy-c` 重测，结果相同。测试 agent 已停、目录已删。
- 交叉审查（Codex astra xhigh，`ranch/review-r13-1`，记录在 `docs/任务/R13-审查.md`）：第一轮 7 条必须改、3 条建议改，改于 3f96e01；第二轮剩 4 条必须改，用户定了其中两处取舍（见「用户定的」），其余照改（d18f3b9）；第三轮剩 1 条必须改（删的过程中文件被换掉时误当删完），照改（c282d8c）；用户要求再复核一轮，第四轮结论「可以合并」，0 条必须改、0 条建议改。

### 拿主意的地方
- 认领放在「读事件」时做，不放在钩子里：这样已经积压了事件的 agy 换上新版命令行就能恢复，不用等它的钩子换版本。
- 删条目的时机跟 agent 退出走，不跟 worktree 走：清 worktree 是主控用 git 做的，corral 看不到；派活技能本来就要求清 worktree 时一并关掉住在里面的 agent。
- 判断「agent 还活着」用 `meta.json` 的实例和 `agent_pid`：`corral upgrade` 换 pen 时 agent 进程不变，`meta.json` 会被新 pen 改写，实例不变。不去抢 agent 的 `lock`，免得和正在启动的同名 agent 争锁。
- 写回设置文件时对象键按字母重排（serde_json 不保序；开 `preserve_order` 会改整个 corral 的 JSON 输出顺序，不值得）。值不变；agy 自己写回时会用它的顺序。
- 加进列表的是规范化后的路径（macOS 的 `/tmp` 会变成 `/private/tmp`）；真实 agy 认这个形式（实测不弹框）。
- 没有实测「另一个先开的 agy 拿内存里的旧列表写回」：会这样写的只有 agy 自己改设置的时候（如用户在它里面点信任）。后果是 corral 加的条目被冲掉（那个 agy 下次再开会弹框），或 corral 删掉的条目被写回（多一行不再用到的路径）。没为它加机制。
- corral 改名前最后一次检查和改名之间，agy 或编辑器恰好写设置文件，它们这次的改动会丢；corral 的锁管不到它们，用户接受这个窗口。
- 列表只存路径：agent 运行期间用户对同一目录取消再重新信任，退出时会被删（用户接受，DESIGN 写为已知限制）。除此之外，corral 不删拿不准是不是自己加的条目：加到一半断了，宁可多留一行。

### 没做的
- 首句的送法（`--prompt-interactive`）没改，抢跑没复现。
- 没核实 agy 子 agent 会不会触发 `SessionStart`（R8 的已知限制，照旧）。
- 第 4 部分要求的「agy 运行时自己写设置文件会不会拿旧内容覆盖」没做针对性实测：要让另一个 agy 在运行中保存设置（如点信任），会往用户的信任列表里留下 agy 自己加、corral 不能删的条目。可能的后果已写在「拿主意的地方」，用户已接受并发写的窗口。
