# 任务：`corral send` 加一个插话参数，agent 干活时也能把话插进去

2026-11-09，ranch/main 交给 ranch/dev-r57b（Codex，gpt-6-astra / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.98）、拿不准、拿不准；主控判：改所有调用方共用的送话路径、栏位进程协议和新旧版兼容，误按会替人选框里的选项，属核心规则，用户 11-09 同意按碰要害派并交叉审查）
类型：功能变更
依据：本轮给 `corral send` 加显式插话参数，按各家适配器插进当前这一轮；不加参数的 `send`、`send --after` 行为不变。不改 pony、paddock。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent 替你干活（实测用的测试 agent 除外）。

## 用户原话

- 11-09：「现在委派出去的agent在工作的时候是没有办法send过去东西的，也就是说不能插话。我的理解是这块设计的有问题，我理解的是是可以插话的。」
- 11-09（主控说明做法后）：「你的意思是加一个插话的参数。加这个参数就是插话。不加就是之前正常的？」主控答是；用户：「可以，写个实现任务的草稿给我看」

## 先读

- `docs/调研/R57-干活时插话.md` 全文：各家插话用什么键、corral 能不能确认送达、误按风险、做法建议。本任务照它的「推荐实现的边界」做，下面写了的以下面为准。
- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」和「过渡期新旧版 corral 双向兼容」。
- `crates/corral/src/cli.rs` 的 `send_once`、`deliver`、`confirmation`；`crates/corral/src/pen.rs` 处理 `send` 的地方；`crates/corral/src/agents/` 下各家适配器。
- `docs/CONTRACT.md` 里 `send` 的约定。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r57b-send-steer`，分支 `r57b-send-steer`（已从 main 建好）。只动 `crates/corral/`、`docs/DESIGN.md`、`docs/CONTRACT.md` 和本文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r57b-send-steer`。

## 要做的

1. **先改设计**：照 AGENTS.md，先在 `docs/DESIGN.md` 写清这个参数的做法和理由，再写代码；`docs/CONTRACT.md` 写清用法、结果和退出码；`crates/corral/resources/AGENT_USAGE.md`、`SKILL.md` 里「send 被拒绝时」补一句干活时可以加这个参数插话。参数名自己定（报告里举的例子是 `--steer`），写进 DESIGN。
2. **加参数后的行为**：
   - 对方 `idle`：和不加一样，照常送。
   - 对方 `working`：按这家适配器声明的提交方式，把话插进当前这一轮（报告实测：Claude Code、Codex、omp 用回车，Kimi 用 Ctrl+S）。
   - 对方 `blocked`、`starting`、暂停中，或者这一家还不支持插话：照旧拒绝，退出码和现在一样说清是哪种原因；不支持的要能和 `not_idle` 区分开，让调用方知道等空闲再送。
   - 「30 秒内有人在窗口里操作过」照旧挡，`--force` 还是只跳过这一道。
3. **送达确认**（用户 11-09 同意）：沿用现在按输入事件确认的办法。Claude Code、Codex、omp 实测能及时确认，先开放这三家。pi、Kimi 话能收到但现在确认不了：能给出可靠的送达证据就一起开放，给不出就先不开放，在完成记录里写明缺什么。amp、canter、agy 这次不开放。
4. **误按保护**（用户 11-09 接受剩下的时间窗）：话打进去之后、按提交键之前，再查一次状态；这时已经是 `blocked` 或状态有变，就不按提交键，返回能看出「文字已写进去、没提交」的结果，不自动清草稿、不重送。剩下的那点时间窗消不掉，在 DESIGN 里照实写。
5. **不加参数时一切照旧**：`send`、`send --after` 的现有行为、输出字段、退出码不变；新旧版 corral 混用时，旧版碰到新参数报用法错误，新版对不支持插话的旧 pen 明确拒绝，不退回去裸发按键。

## 怎么算做完

- 用户原话：「加这个参数就是插话。不加就是之前正常的。」
- 加参数时上面第 2–4 条的每种情况各有测试（用合成 agent，不用真 agent）：idle 照送、working 按适配器的键送并确认、blocked／starting／暂停／不支持各自拒绝、30 秒人工操作照旧挡、提交前变成 blocked 不按提交键；旧 pen 没有这项能力时拒绝。
- 不加参数时 `send`、`send --after` 的现有测试全过，没改断言、没放宽超时。
- Claude Code、Codex、omp 各开一个测试 agent，在 working 时用新参数插一句，确认送达并照做一次（pi、Kimi 开放的话同样各一次）。
- 验证只做这些：上面的测试；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 pony、paddock，不改派活技能（`crates/dispatch/resources/corral-dispatch/`）；它们要不要用插话，另开任务。
- 不改 `send --after` 的提醒：提醒照旧等接收方空闲再送。
- 不碰 `corral ls` 里不是你开的 agent；实测用 `ranch/test-<名字>`、`--label role=test`、临时目录，用完 `corral stop`。
- 不要按项目名或路径批量杀进程（`pkill -f <项目名>` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送，不打包部署，不对任何 agent 跑 `corral upgrade`。只在 `r57b-send-steer` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
