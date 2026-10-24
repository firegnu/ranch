# 任务：R48d Kimi Code 进派活表、题目给 pony（阶段 2）

2026-10-24，ranch/main 交给 ranch/dev-r48d（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：常规（置信 0.59）、要、碰要害；要改派活表的公开格式，按碰要害做）
类型：功能变更
依据：让主控能按点名把活派给 Kimi Code（派活表、两份技能），并让 corral 报出 Kimi 提问框的题目，pony 提醒里能显示；Slack 里回答 Kimi 的题目、按钮，以及 sandbox，不在本轮。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做。除了下面写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-24：「我觉得要做到现在已经接入的agent的所有功能（但是可以分阶段做，有几个阶段你要给我列一下）」。主控列的阶段：0 补测 → 1 基本接入 → 2 派活（分工表加 Kimi、提问框题目给 pony）→ 3 sandbox → 4 Slack 按钮（可选）。阶段 1（R48c）已合并、部署。
- 10-24：「模型我是自己改的。你不用管。」（派 Kimi 不指定模型，用用户自己设的默认。）
- 10-24，主控问三件事，用户都选了推荐：
  - Kimi 怎么派：「点名才派」（像 amp、agy 加一个 kimi 格子，只有用户或任务文件点名 Kimi 时才派；还没评估过写代码的水平，先不进默认格子）。
  - 派出去的 Kimi 怎么免掉「是否信任这个目录」：「家模板加环境变量」（派活表的家模板新增可选的环境变量字段，kimi 写 `KIMI_CODE_TRUST_WORKSPACE=1`，只对派出去的 Kimi 生效、不写 Kimi 的信任记录；公开格式兼容新增，paddock 设置页以后可以显示）。
  - 题目给 pony 做到哪：「只显示题目」（corral status 给出 Kimi 的题目，和 Claude Code 一样的格式，pony 提醒里就能显示；在 Slack 线程里回答、按钮留到阶段 4）。

## 先读

- `docs/DESIGN.md` §7 最后一条「接入 Kimi Code（R48）」，和讲派活表的部分（R31、R34、R35）。
- `docs/CONTRACT.md`：「`ranch dispatch cell` / `table`」「`ranch dispatch edit` / `reset`」「Kimi Code 阶段 1（R48c）」，以及 R43 的 `status.questions`。
- `docs/调研/R48b-Kimi-Code补测.md` §1（提问框的 `PreToolUse(AskUserQuestion)` 带完整 questions）、§2（提问框画面）、§6（`--auto` 下提问工具被拒）。
- 代码：`crates/dispatch/resources/dispatch.toml`（`agents.amp`、`agents.agy`、`cells.amp`、`cells.agy`）、`crates/dispatch/` 读表和输出 `cell` 的代码、`crates/dispatch/resources/corral-dispatch/SKILL.md`（讲 amp、agy 的几处）、`crates/corral/resources/SKILL.md` 和 `AGENT_USAGE.md`（讲 amp、agy 的几处）、`crates/corral/src/hooks.rs` 和 `events.rs`、`cli.rs` 里 `questions` 的来龙去脉、`crates/corral/src/agents/kimi.rs`。
- `crates/pony/src/corral.rs`、`core/engine.rs` 里读 `questions` 的地方（只读，看 pony 现在怎么用它）。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r48d`，分支 `r48d`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r48d`。
- 只动 `crates/dispatch/`、`crates/corral/`、`crates/ranch/` 里和派活表有关的部分（主控 10-24 开工后补：派活表的测试在 `crates/ranch/tests/dispatch_table.rs`）；`crates/pony/` 里题目展示的部分（主控 10-24 开工后补：pony 写死只给 claude 显示题目；只放开展示，回答、按钮、自动送答案、Esc 核对仍只对 claude）、`docs/DESIGN.md`、`docs/CONTRACT.md`、本任务文件末尾。

## 要做的

1. **派活表加 Kimi。** 家模板加一个可选的环境变量字段（没有它的家行为、输出不变）；`cell` 和 `table` 的 `choices[]` 里带上它，`ranch dispatch edit`、`reset` 和本机覆盖表 `~/.ranch/dispatch.toml` 照常能用。加 `agents.kimi`（Kimi 免确认的启动参数，`--auto`；环境变量 `KIMI_CODE_TRUST_WORKSPACE=1`；不带模型、强度，标签为空）和 `cells.kimi`（说明照 amp、agy：只在用户或任务文件点名 Kimi 时使用）。
2. **两份技能。** corral-dispatch 技能：把 Kimi 和 amp、agy 并列写进「点名才派」的几处，写清派 Kimi 时把读表给的环境变量每项传一个 `--env`、照常带 `--prompt`（Kimi 支持）。corral 技能（`SKILL.md`、`AGENT_USAGE.md`）：写怎么开 Kimi、一直 `starting` 时怎么查（`corral integration status`，没装就 `corral integration install --agent kimi`，不要自己装）。照现有写法，不改别家的说法。
3. **题目给 pony。** Kimi 在提问框上 `blocked` 时，`corral status` 给出 `questions`，格式、截断、只在提问框开着时给出，都和 R43 给 Claude Code 的一样；问题答完或取消就不再给。pony 不改代码就能在提醒里显示题目的，就不改 pony；改 pony 才行的，停下来问主控。
4. CONTRACT、DESIGN 照实现补上，写明对 paddock 的影响（派活表多一个可选字段、多一个格子；`status.questions` 多一家会给）。

## 怎么算做完

- 用户要的结果（10-24）：Kimi 点名才派，派的时候带 `KIMI_CODE_TRUST_WORKSPACE=1`、用用户自己的默认模型；Kimi 在提问框上时 corral status 给出题目，pony 提醒里能显示；Slack 里回答和按钮留到阶段 4。
- 验证只做这些：每项新行为一条针对的自动化测试；边角用例：没有环境变量字段的家输出和以前完全一样，本机覆盖表给家加或改环境变量、`edit`、`reset` 后照样对，环境变量字段格式不对时报 `invalid_data`；`questions` 只在 Kimi 提问框开着时给出，答完、取消、运行中 Esc 后不再给，截断和 R43 一样；用合成的 status 确认 pony 现有代码解析得出 Kimi 的题目；项目标准检查跑一次：`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`；真实 Kimi 照下面的边界走两条主路径：照读表给的命令和环境变量开一个 Kimi 做一件小的改文件的活、不弹任何框；普通模式开一个 Kimi 让它用提问工具，`corral status` 给出题目。觉得不够，在回复里说，不要自己加。

## 真实 Kimi 实测的边界

- 用用户已登录的账号，全部不超过 6 轮很短的输入，工作目录用临时目录。
- 用户的 `~/.kimi-code/config.toml` 里已经装着 corral 的块（R48c 部署后装的），不要卸、不要改；用本分支编出来的 corral 开测试 agent（部署的 corral 和本分支的钩子入口相同，本分支 corral 开的 Kimi 用的是本分支的程序）。
- 测试 agent：名字 `ranch/test-r48d-<几个字>`，带 `--label role=test`、`--env CAIRN_DISABLE=1`，用完 `corral stop`。最多同时开一个。
- 自动化测试不碰真实 HOME、真实 `~/.ranch/dispatch.toml` 和真实 Kimi：用隔离的 HOME、CORRAL_HOME、`KIMI_CODE_HOME` 和合成程序。

## 不要做

- 不把 Kimi 放进任何默认档位格子；不改别家的模板、格子和技能里别家的说法。
- 不改 pony 的代码（要改才能显示题目时停下来问）；不做 Slack 里回答 Kimi 的题目、按钮；不做 fence。
- 不改 PATH 上装好的 `corral`、`ranch`，不打包、不部署，不装技能（`install-skills`），不切共用 helper；不改用户的 `~/.ranch/dispatch.toml`。
- 不卸、不改 `~/.kimi-code/config.toml` 里 corral 的块和别的配置；不改用户的 Kimi 模型；不读、不往仓库里抄凭证、token、`device_id`、`server.token` 的内容。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。只操作自己开的 `ranch/test-r48d-*`。
- 不按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f kimi` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉；用户自己可能也开着 kimi。停自己起的进程用记下的 PID。
- 不放宽现有测试的断言、不缩短超时、不删测试来过。
- 碰到要拿主意的事（包括要改公开格式里已有字段的含义、要改 pony、额度不够），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r48d` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、真跑了几轮、拿主意的地方、没做的事、对 paddock 的影响、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
