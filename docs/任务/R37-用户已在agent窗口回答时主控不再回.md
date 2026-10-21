# 任务：用户已经在 agent 窗口里回答了，主控就不再回

2026-10-21，ranch/main 交给 ranch/dev-r37（Codex，gpt-6-astra / high）。
优先：高
路由：轻 / 交叉审查不要 / 影响面：看得见（路由：轻（置信 0.78）、拿不准、看得见；交叉审查由主控判：只改技能文字，不涉及数据和安全；推翻：强度用 high 不用 medium，用户 10-20：「我不需要medium」，派 R37 前用户也同意了 astra high）
类型：功能变更
依据：R36 让主控回答被委派 agent 在回复里提的问题，但没写用户已经在 agent 自己窗口里回答时怎么办，两边可能先后给出不同答复。本轮只改 corral-dispatch 技能文字和派活路由说明：主控送决定前先看用户是不是已经在回答，是就不送。不改程序。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21，用户贴来的问题：「你在 agent 窗口里直接回答它、主控又替它定了的情况，新规则没有写，两边可能给出不同的答复。……agent 一停下提问，主控马上会收到提醒。你要是正好也在它的窗口里回了，agent 可能先后收到两个不一样的答复。要堵上这个口子，可以让主控另开一件小活，补上「以你的回答为准」。」
- 主控提了下面「要做的」三条规矩，并说「正在打字还没回车、又超过 30 秒没打字」这个空隙这次不堵（要改程序）。用户：「同意，写任务书吧」。

## 先读

- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」。
- `crates/dispatch/resources/corral-dispatch/SKILL.md`：第 4 节「前台这次 wait 如果对方已经停了」那条，第 5 节「对方停了、仍在，回复最后一行不是 DONE，且回复里在问要拿主意的事」那条（R36 加的），第 8 节 R36 加的那句。
- `docs/派活路由规则.md` 8.1 节（R36 加的）。
- 现有机制，只读不改：`corral status` 的 `state`、`state_started`、`last_human_input`（有人在 attach 的窗口里打字的时间，paddock 里打字也算）；`corral send` 在对方 working 时报 `not_idle`，30 秒内有人在窗口里打过字时报 `human_active`（`crates/corral/src/pen.rs` 里 `human_active` 附近）。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r37-user-answer-wins`，分支 `r37-user-answer-wins`（已从 main 建好）。只动 `crates/dispatch/resources/corral-dispatch/SKILL.md`、`docs/派活路由规则.md`，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r37-user-answer-wins`。

## 要做的

在第 5 节 R36 加的那条（主控自己定、用 `corral send` 回给它）里，以及第 4 节、`docs/派活路由规则.md` 8.1 对应的地方，补上这三条，以用户的回答为准：

1. 主控送决定之前先运行一次 `corral status <名字>`：对方停下之后有人在它窗口里打过字（`last_human_input` 晚于这次停下的 `state_started`），或者它又在干活了，就当成用户已经在回答，主控不送，只按「长任务」一节重挂提醒。
2. `corral send` 报 `not_idle` 或 `human_active` 时同样处理：不送、不加 `--force`，重挂提醒。
3. 主控没送的时候告诉用户：agent 问了什么、主控原本打算怎么定。

先问用户的那几类（第 8 节）和请用户 attach 的对话框照旧，不受这三条影响。

## 怎么算做完

- 用户原话：「补上「以你的回答为准」」：技能第 4、5 节和说明文档写明上面三条，用户已经在 agent 窗口里回答时主控不再送决定。
- 验证只做这些：`git diff --check`；`cargo test -p ranch-dispatch` 一次；用 `grep` 看第 4、5 节和说明文档 8.1 的说法前后一致。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改程序、不加命令或字段，不改分工表；不去堵「正在打字还没回车」那个空隙。
- 不改 corral 通用技能和 `AGENT_USAGE.md`。
- 不改 R34 要改的那些（分档、交叉审查交给谁、返工升档、medium、最费的配置）。
- 不新增上面「要做的」以外的规矩，不加审查内容和验证要求。
- 不装技能，不动 `~/.claude`、`~/.agents`、`~/.gemini`、`~/.ranch`。
- 不改 paddock、corral 程序、fence。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r37-user-answer-wins` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：技能第 4、5 节和路由说明 8.1 补齐「以用户的回答为准」三条：送决定前再查 status；停下后有窗口输入或又在 working 时不送；send 报 not_idle / human_active 时不送、不加 --force；这些情况都重挂提醒，并告诉用户 agent 的问题和主控原本打算的决定。第 8 节先问用户的类别和 attach 对话框处理照旧。说明文档写明 paddock 不用动，更新已安装技能仍属单独部署。
- 验证了什么：`cargo test -p ranch-dispatch` 只跑一次，独立编译目录为 `.target/r37-user-answer-wins`，21 个单元测试通过，doc-tests 0 个；用 grep 核对技能第 4、5 节与说明 8.1 的条件、错误处理、告知内容一致；`git diff --check` 通过。仅改文档，不新增失败测试，未扩大验证预算；所有命令都在前台等到结束。
- 拿主意的地方：没有需要另定的取舍，照任务书沿用现有字段和发送保护；没有要主控决定的事。
- 没做的事：未改程序、分工表、通用 corral 技能或其他仓库，未处理「正在打字还没回车、又超过 30 秒没打字」的空隙；未安装、部署、操作用户 agent、合并 main 或推送。只在当前任务分支提交。
- 踩过的坑：长文批量读取时工具输出被截断，改为分段读取；修改和验证没有遇到失败。

## 主控审查

- 看了 diff：只动技能第 4、5 节、`docs/派活路由规则.md` 8.1 和本文件；「要做的」三条（送前查 status、`not_idle`/`human_active` 不硬送、没送时告诉用户）在三处都写到，先问用户的几类和 attach 对话框照旧；「不要做」的一件都没做（没改程序、通用 corral 技能、分工表）。
- 重跑：`cargo test -p ranch-dispatch` 21 通过，与对方说的一致；`git diff --check` 通过。
- 没有取舍要表态。实际名字是 `ranch/dev-r37-1`（`--unique` 补的后缀）。
- 新规矩和 R36 一起打包，部署后才生效。

结果：一次过
