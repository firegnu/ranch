# 任务：给一个 agent 设定「它派出去的 agent 套哪个档案」，随时可改

10-19，ranch/main 交给 ranch/dev-r18（Codex，gpt-6-astra / high）。
路由：常规 / 交叉审查要 / 影响面：碰要害（路由：常规（置信 0.35）、要、碰要害）
类型：功能变更
依据：paddock/main 10-19 按用户要求起草、转给 ranch/main；ranch/main 定了命令、字段和存放位置（见「要做的」）。本轮只做 corral 这一侧，不做 paddock 的勾（P5-141），不改 fence。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话（10-19，在 paddock 讨论 P5-120 之后）
- “我的想法是在paddock中默认委派出去的不是sandbox中跑的，但是我勾选了某项之后就变成sandbox了，但是ranch中默认是不再sandbox中跑的。但是可以通过paddock选中的状态改为sandbox跑。我想要的是这个效果”
- 问“这个主控的勾选是启动主控之后的，还是中间随时可以变”后，要随时可改。
- 不要靠改 `CLAUDE.md` 让主控记规矩：“还有更加优雅的办法吗？这个要改claude.md”
- “我不想corral反向关联paddock”
- 设置由 corral 自己保存：“选甲，写草稿吧”
- ranch/main 提的 `--no-sandbox`、设定时先跑 `fence check`、验收照抄原话，用户都同意（经 paddock/main 转达：“按你的建议转给ranch吧”）。

## 先读
- `docs/CONTRACT.md`「sandbox 与 `fence`（R17b）」一节，以及 `corral start`、`status`、`ls` 的现有输出。
- `docs/DESIGN.md` §8（sandbox）。
- `crates/corral/src/cli.rs` 里 `start` 和 `--sandbox` 的处理、`FENCE_RUN` 防护；`crates/corral/src/state.rs` 的 `FILES`、`list()` 的清理；`labels.json` 带 `instance` 的做法（照它）。
- `CLAUDE.md`「规矩」全部。

## 要做的
用户要的效果：
- ranch 默认不变：主控派出去的 agent 不套墙，`corral start` 不加 `--sandbox` 就是现在的样子。
- 可以给一个**正在跑**的 agent（一般是主控）设定：它**之后**用 `corral start` 派出去的 agent 套某个 fence 档案，或者不套；随时能改，从下一次 `corral start` 起生效。已经在跑的 agent 不受影响。
- 设定的这个 agent 自己不套墙。
- corral 不知道 paddock，不读 paddock 的任何东西；paddock、别的前端、人在终端里都用同一条公开命令。

ranch/main 定的做法：
1. **新命令** `corral child-sandbox NAME`：
   - `corral child-sandbox NAME <档案>`：设定；
   - `corral child-sandbox NAME --clear`：清除；
   - `corral child-sandbox NAME`：只查。
   - 三种都输出一行 JSON：`{"ok":true,"name":…,"instance":…,"child_sandbox":{"profile":"dev"}}`，没设是 `"child_sandbox":null`。NAME 不在跑报 `not_found`（和别的命令一样）。
   - 设定时先调一次 `fence check --profile <档案>`（不带种类），不过就报 `sandbox_failed`、不存；找不到 fence 报 `sandbox_unavailable`。
2. **存在哪**：这个 agent 的状态目录里一个新文件 `child-sandbox.json`，带 `instance`，像 `labels.json` 那样只认同一次运行；加进 `FILES`，agent 结束随目录清掉。不经 pen，旧版 pen 的 agent 也能设。
3. **`corral ls`、`corral status`** 每个 agent 加 `child_sandbox` 字段，取值同上；没设、旧记录、instance 对不上都是 `null`。
4. **`corral start` 怎么用它**：调用者带着 `CORRAL_NAME` 和 `CORRAL_INSTANCE`（corral 给每个 agent 都设）时，读这个 agent 当下的 `child-sandbox.json`：
   - instance 对得上且设了档案：等同带了 `--sandbox <档案>`，后面照现有 `--sandbox` 走（`fence check`、错误码都一样）；
   - 没设、没有这个文件、instance 对不上、调用者不在跑：和现在完全一样，不多出失败；
   - 文件在、instance 对得上但读不了或格式错：报新错误 `child_sandbox_unreadable`（退出码 1），不起 agent，不当成不套。
5. **显式参数优先**：带了 `--sandbox <档案>` 用它；新加 `--no-sandbox`，这次不套（主控偶尔有要联网、要推送的活）。两个一起带报 `usage`。
6. `corral start` 的输出和以后 `status` 里的 `sandbox` 照旧。是不是从设定来的，不另加字段。
7. **CONTRACT** 写清楚：命令、字段、新错误码、优先次序；兼容（新增；旧 CLI 不认 `child-sandbox` 命令、不读这个文件，从旧 CLI 发的 `corral start` 不会套墙；旧 CLI 的 `ls` 不清这个文件，靠 instance 对不上兜底，下次新 CLI 清）。
8. **对 paddock 的影响**：兼容新增；paddock 在 P5-141 里调 `corral child-sandbox`、读 `ls` 的 `child_sandbox` 做勾。写进完成记录。

## 在哪里干活
- worktree：`../ranch-worktrees/r18-child-sandbox`，分支 `r18-child-sandbox`（已从 main 建好）。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r18-child-sandbox`。
- 会动：`crates/corral/`、`docs/CONTRACT.md`。DESIGN 有要改的在完成记录里写，不要自己改。

## 怎么算做完
- 用户原话：“但是可以通过paddock选中的状态改为sandbox跑。我想要的是这个效果”“选甲”。
- 验证只做这些：
  - 针对这件活的测试（隔离 HOME、CORRAL_HOME、FENCE_HOME，合成程序和假 `fence` 脚本，不起真实 agent），覆盖这些边角：设定 → 子 agent 套墙；清除 → 不套；没设、没有文件、instance 对不上（同名重开后）、调用者已不在跑 → 和现在一样；文件格式坏 → `child_sandbox_unreadable` 且不起 agent；显式 `--sandbox`／`--no-sandbox` 压过设定；两个一起带 → `usage`；设定时 `fence check` 不过 → `sandbox_failed` 且不存；没装 fence → `sandbox_unavailable`；设定只影响之后的 `start`，已在跑的子 agent 的 `sandbox` 不变。
  - 合并前：`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。
  - 觉得不够，在回复里说，不要自己加。

## 不要做
- 不改 fence（`crates/fence/`），不改 corral-dispatch、corral 技能，不改 paddock。
- 不读 paddock 的任何文件或配置。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`、`child-sandbox`，不 attach 打字。测试用隔离的 HOME、CORRAL_HOME、FENCE_HOME 和合成程序（如 `/bin/cat`、假 `fence` 脚本）。
- 不靠删测试、放宽断言、缩短超时来通过。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 认调用者只靠 `CORRAL_NAME`、`CORRAL_INSTANCE` 走不通时（比如发现某家 agent 的子进程拿不到这两个变量），停下来报告，不要自己换别的认法。
- 不合并到 main，不推送。只在 `r18-child-sandbox` 上提交。

## 做完
在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
