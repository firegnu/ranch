# 任务：R48b 把 Kimi Code 没测到的真跑测完

2026-10-24，ranch/main 交给 ranch/dev-r48b（Codex，中档：gpt-6-astra / high）。
路由：常规 / 交叉审查不要 / 影响面：看得见（路由：轻（置信 0.0）、要、碰要害；本轮是不改代码的调研，按 corral-dispatch 第 3 节一律算看得见、不交叉审查；推翻：路由判轻会落到 pi、omp，但这件要临时改用户的全局配置、用真账号细致操作终端，上一轮同类活 R48 由中档 Codex 一次做好，按中档派）
类型：调研
依据：R48 调研留下的【未知】里，凡是正式接入（阶段 1）要靠它定方案的，本轮用真账号测清；结果写成补测报告。不写 corral 代码，不正式安装任何集成。
提示：区分已查证事实、推测和未知，给出直接依据；结论限定在本轮调查范围。
你是被委派的 agent：照本文件做。除了下面写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-24，看完 R48 报告后：「1: 我觉得要做到现在已经接入的agent的所有功能（但是可以分阶段做，有几个阶段你要给我列一下）。2: 钩子的问题，你要给我讲清楚，是不是kimi code和现在的几种都不匹配，要单独写？3: 继续使用我现在的账号 4. 测试完」。
- 10-24：「另外herdr也接入kimi code了。你也可以参考」。
- 10-24，主控问补测能不能临时往 `~/.kimi-code/config.toml` 插一段带标记的探针钩子块（只在 corral 开的测试 Kimi 里生效，测完删掉）、额度给多少，用户选：「可以，最多 15 轮」（选项说明：动手前先备份 config.toml，测完只删探针块和探针脚本，不动别的配置）。

## 先读

- `docs/调研/R48-接入Kimi-Code.md` 全文，尤其各处【未知】和「要用户定的」。
- `docs/任务/R48-接入Kimi-Code.md` 末尾「完成记录」「主控审查」。
- `docs/DESIGN.md` §7 里 R6 全局安装、R7 读屏兜底、R8 接入 agy、R22–R26 被人打断几条。

## 主控查到的（10-24）

- 用户决定（R48 报告「要用户定的」）：接入要做到现有几家的全部功能，可以分阶段；继续用现在的账号，接受 Kimi 自己刷新配置和令牌。
- 主控给用户列的阶段：0 补测（本任务）→ 1 基本接入（适配器、钩子安装、状态、回复、首句、停止）→ 2 派活（分工表、提问题目给 pony）→ 3 sandbox → 4 Slack 按钮（可选）。
- herdr 0.9.5（`/opt/homebrew/bin/herdr`，已装，用户在用）已经接了 Kimi，用户机器上**没装**它的 kimi 集成（`herdr integration status` 报 `kimi: not installed`）。从它的程序字符串里看到的接法：
  - 脚本放 `~/.kimi-code/hooks/herdr-agent-state.sh`（`HERDR_INTEGRATION_VERSION=8`），脚本先查 `HERDR_ENV`、`HERDR_SOCKET_PATH`、`HERDR_PANE_ID`，不是 herdr 开的就退出；
  - `config.toml` 里插一段 `# >>> herdr kimi integration` … `# <<< herdr kimi integration` 的块，挂 `SessionStart`、`PreToolUse`（matcher 分 `^(?!AskUserQuestion$).*$` 和 `^AskUserQuestion$`）、`PostToolUse`、`PostToolUseFailure`、`SubagentStart`、`PreCompact`、`PermissionRequest`、`Interrupt` 等；
  - 另有 kimi 的读屏规则（id `kimi`，版本 `2026.06.10.1`）：批准框（`run this command?`、`write this file?`、`apply these edits?` 等）、提问框（`select`、`esc cancel`、`question`）、转圈算工作中。
  - 可以用 `strings` 读 herdr 程序对照；**不要**运行 `herdr integration install kimi`。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r48b`，分支 `r48b`（已从 main 建好）。只写 `docs/调研/R48b-Kimi-Code补测.md` 和本任务文件末尾。
- 探针脚本、记录、实测工作目录放 `/tmp/r48b-*` 之类的临时目录，用完删掉（记录先摘进报告）。

## 要做的

测清下面几样，写成 `docs/调研/R48b-Kimi-Code补测.md`，开头「结论先说」，每条标【查证】【推测】【未知】并写依据；R48 报告里哪条结论被推翻或补全，逐条写明。

1. **钩子实际收到什么**：挂一个探针，覆盖 Kimi 2.3.1 能挂的全部钩子事件，记下每次的事件名、stdin 全文（凭证类字段不抄）、触发时间和先后、钩子进程的父进程链（钩子的父进程是不是 kimi，祖父是不是 corral 的 pen）、继承到的 `CORRAL_*` 环境变量名、钩子会不会挡住 Kimi。覆盖：会话开始、送一句话、调工具（真的执行，不只是嘴上说做了）、正常结束、出错（能造就造）、等批准、提问框、框上 Esc、运行中 Esc、子 agent（能触发就测）、压缩（能触发就测）。
2. **画面**：默认模式下真实的批准框、提问框长什么样（最下面几行原文）、终端标题怎么变，和 herdr 的读屏规则对照。
3. **标记块留不留得住**：Kimi 运行中自己改写 `config.toml` 时，探针块（含首尾注释标记）还在不在、有没有被挪动或改格式；Kimi 改了哪些字段。
4. **停止**：有一个真在跑的 Bash 子进程（如 `sleep`）时，分别用 TERM、HUP 停 Kimi，它起的子进程退不退干净、各用多久；会不会触发 `SessionEnd` 钩子。
5. **首句和就绪**：Kimi 刚启动、还没会话时，从终端送第一句的可靠办法（含以 `-` 开头的句子）；`--session <id>` 能不能提前建好会话；什么时刻可以认为它已经能收话。
6. **派活要用的启动方式**：`--auto` 加 `KIMI_CODE_TRUST_WORKSPACE=1` 能不能不弹任何框地完成一件小的真实改文件的活；`--auto` 下提问工具是什么行为。

最后写「对阶段 1 的建议」：钩子怎么装（照 herdr 的标记块，还是别的）、挂哪些事件、各事件对应 corral 的什么状态、回复从哪拿、读屏规则要哪些、停止步骤、首句怎么送。只写建议，不改 DESIGN。

实测的边界：

- 真账号，全部加起来**不超过 15 轮**用户输入。工作目录用临时目录，不在 ranch 或别的仓库里跑。用默认模型，不切更贵的模型。
- 改 `config.toml` 之前先备份到临时目录。探针块用首尾注释标记包好（如 `# >>> corral r48b probe` / `# <<< corral r48b probe`），探针脚本放临时目录、用绝对路径引用；脚本先查一个只有测试 agent 才有的环境变量（如 `R48B_PROBE_DIR`），没有就立刻退出，不输出任何东西。
- 测完只删探针块（和临时目录），不要拿备份整份覆盖回去（Kimi 自己的正常改动要留着）。删完和备份对比，把剩下的差别写进报告。
- 测试 agent 用 corral 开：名字 `ranch/test-r48b-<几个字>`，带 `--label role=test`、`--env CAIRN_DISABLE=1`，探针的环境变量用 `--env` 传，`--` 后面是 `kimi`（加要测的参数）。用 `corral read`、`corral keys`、`corral send` 操作，用完 `corral stop`。最多同时开一个。
- 测停止时，只对自己开的测试 agent 或自己记下 PID 的进程发信号。

## 怎么算做完

- 报告写完，上面 1–6 条和「对阶段 1 的建议」都有，每条有标记和依据；测不出来的写明试了什么、为什么不行。
- `~/.kimi-code/config.toml` 里没有探针块了，临时目录删了，测试 agent 都关了。
- 验证只做这些：`git diff --check`；`grep` 确认 `config.toml` 里没有 `r48b`。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 corral、ranch 的代码，不加适配器，不改 DESIGN，不跑 `corral integration install`，不装技能。
- 不运行 `herdr integration install` 或 `uninstall`，不改 herdr 的配置。
- 除了探针块，不改 `~/.kimi-code/` 里的配置；不往 `~/.kimi-code/hooks/`、`plugins/` 里放文件；不登出、不重新登录；不读、不往报告里抄凭证、token、`device_id`、`server.token` 的内容。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。只操作自己开的 `ranch/test-r48b-*`。
- 不按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f kimi` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉；用户自己可能也开着 kimi。停自己起的进程用记下的 PID。
- 不超过 15 轮。
- 碰到要拿主意的事（包括额度不够、探针块被 Kimi 弄坏了删不干净、要用户在场点框或登录），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r48b` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、真跑了几轮、拿主意的地方、没查清的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
