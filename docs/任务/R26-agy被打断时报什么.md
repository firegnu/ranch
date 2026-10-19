# 任务：摸底——agy 被人按 Esc 打断时，到底报了什么

2026-10-19，ranch/main 交给 ranch/dev-r26（Codex，gpt-6-astra / medium）。
路由：轻 / 交叉审查不要 / 影响面：看得见（路由：轻（置信 0.59）、拿不准、看得见；本轮是不改代码的调研，按 corral-dispatch 第 3 节一律算看得见、不交叉审查）
类型：调研
依据：R22 实测 agy 跑命令时按 Esc，`Stop` 钩子没来，主控认不出打断；生成文字时按 Esc 没测过。本轮只摸清 agy 在这两种情况下报了什么，不改代码；改不改、怎么改，看了结果由用户定。
提示：区分已查证事实、推测和未知，给出直接依据；结论限定在本轮调查范围。
你是被委派的 agent：照本文件做。除了下面写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-19：“agy的打断能不能也查一下”；主控说了查到的和建议做一次小摸底后：“摸一下”。
- 10-14：“主要是想通过agy加进来读屏幕的能力，agy很少用。”

## 先读

- `AGENTS.md`「规矩」。
- `crates/corral/src/agents/agy.rs`：`hook` 怎么把 agy 的钩子转成 corral 事件（`Stop` 的 `USER_CANCELED` → `Interrupt`），`turn` 怎么读对话记录。
- `crates/corral/resources/agy_hooks.json`：钩子命令都是 `exec "$CORRAL_HELPER" __hook agy:<事件>`。
- `docs/任务/R22-人打断被委派的agent后主控误以为做完.md` 末尾完成记录里 agy 那几句。
- `docs/DESIGN.md` §7「接入 agy（R8）」「agy 派活修整（R13）」。

## 主控查到的（10-19，读 agy 1.4.2 程序和 R22 那次测试留下的记录；没新测）

- agy 程序里内嵌的钩子说明写着：`Stop` 钩子的输入带 `terminationReason`，用户取消时是 `"USER_CANCELED"`；另有 `fullyIdle`、`error` 等字段。
- R22 那次测试：agy 在跑一个前台命令时，人按了 Esc。
  - 对话记录（`~/.gemini/antigravity-cli/brain/<会话号>/.system_generated/logs/transcript_full.jsonl`）最后一步是还没出结果的命令调用，没有「已取消」的步骤或状态。
  - agy 自己的日志（`~/.gemini/antigravity-cli/log/cli-<时间>.log`）只记了一句 `Cancelling conversation <会话号> (killBackgroundTasks=false)`；这份日志不记钩子有没有跑。
  - corral 的事件停在 `PostToolUse`，没有 `Stop`。
- 钩子命令 exec 的是 `$CORRAL_HELPER`；用隔离的 `CORRAL_HOME` 时，它指向的程序在你自己手里。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r26-agy-interrupt-probe`，分支 `r26-agy-interrupt-probe`（已从 main 建好）。只新写 `docs/调研/R26-agy被打断时报什么.md` 和本任务文件的完成记录。
- 用已经装好的 `corral`（`~/.local/bin/`，`versions/06e2c27`），不用编译。

## 要弄清的

1. agy 生成文字（模型在写回复、还没调用工具）时按 Esc：有没有 `Stop` 钩子，原始输入里 `terminationReason` 等字段是什么；对话记录末尾和 agy 日志里留下了什么。
2. agy 跑命令时按 Esc：同上几样。和 R22 那次是不是一样。
3. 打断后人接着让它干、正常结束：`Stop` 钩子照常来不来，`terminationReason` 是什么。
4. 结论：corral 能不能靠明确的、agy 自己报的信号认出这两种打断；能的话，是哪个事件、哪个字段，现有转换够不够、还差什么，大概多大；不能的话，写明缺什么。

## 怎么算做完

- `docs/调研/R26-agy被打断时报什么.md` 写清上面 4 条，每条标明是实测、读代码还是推测，附上关键的原始钩子输入（删掉对话内容，只留字段名和枚举值）。
- 验证只做这些：照上面实测一遍，`git diff --check`。觉得不够，在回复里说，不要自己加。

## 测试 agent

- 名字 `ranch/test-interrupt-agy`，用隔离的 `CORRAL_HOME`，带 `--env CAIRN_DISABLE=1`。按 corral 技能的 agy 写法起：必须带 `--prompt`，不带 `--label`，最后是 `-- agy --dangerously-skip-permissions`。用 agy 默认模型。活要小，但要让它写的文字够长、跑的命令够久，好有时间按 Esc。
- 工作目录用 HOME 下的临时合成目录。用完删掉它、隔离的 `CORRAL_HOME`，以及你记下的原始数据里不打算写进报告的部分。
- Esc 要在真终端里按，你按不了。起好、进入 `working` 后停下来，在回复里写明要用户在另一个终端里做的事：
  - 用 `corral attach <名字>`（带上隔离的 `CORRAL_HOME`）接上去；
  - 在什么时候按 Esc；
  - 按完要不要接着让它干。
  
  等主控转回「按过了」再看结果。两种情况可以排在一次 attach 里做完，减少来回。
- 不要用 `corral keys` 送 Esc 来代替真人按键当结论依据。可以用它先自己预演，但报告里要分开写。
- pen 会替 corral 开的 agy 往 `~/.gemini/antigravity-cli/settings.json` 的 `trustedWorkspaces` 补信任目录（R13，用户同意过）。不要手改这个文件；在完成记录里写明加进去了哪些条目、退出后撤没撤回。
- agy 卡在登录框、信任框或别的对话框上，停下来报告，请主控转给用户去点；不要 attach 上去替它点。
- 停之前用 `corral read`/`reply` 存好证据，用完 `corral stop`。

## 不要做

- 不改 ranch 的代码，不改 corral 插件（`~/.gemini/config/plugins/corral/`），不改用户的 agy 设置。
- 不读用户别的 agy 会话：`~/.gemini/antigravity-cli/brain/`、`log/` 下只看你这个测试 agent 的会话号和日志。不读、不打印、不拷贝 `~/.gemini/` 下的凭证文件。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 不做 sandbox，不去 5090。
- 不合并到 main，不推送。只在 `r26-agy-interrupt-probe` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：完成指定测试 agent 的真人 Esc 调研，新增 `docs/调研/R26-agy被打断时报什么.md`，分别列出文字生成、命令运行期间取消、继续输入后正常结束的钩子、transcript 和 agy 日志证据，区分实测、读代码、推测和未知。两次取消均没收到 Stop；随后两次正常回复都收到 NO_TOOL_CALL，fullyIdle 分别为 true/false。仅日志明确记录取消请求，现有 USER_CANCELED → Interrupt 映射不缺分支。
- 验证了什么：只做任务指定的一遍真人实测和 `git diff --check`。采集器记录 8 个原始钩子，正常 Stop 能被原版 corral 接收；最终 reply 返回正确合成标记。停止前保存 read/reply，之后 corral stop 成功，隔离 ls 为空。没有用 keys 替代真人按键，没有编译或增加测试。
- 拿主意的地方：隔离 helper 在 start 后、首个钩子前换为临时 shell 采集器，记录 stdin 后 exec 原版 helper，未改插件或全局运行时。报告仅保留脱敏字段、枚举和关键时间，原始证据不提交。日志里的 Cancelling 只认作取消请求证据，不夸大为可靠终止协议；paddock 不用动。
- 没做的事：未改代码、用户设置或插件，未部署、合并、推送或操作其他 agent。pen 自动加入 `$HOME/r26-agy-probe.O38o74/work` 的信任条目，停止后已确认撤回。临时目录、隔离 CORRAL_HOME、采集器和原始证据，以及 agy 自己生成的本测试会话目录和对应日志均已清理。
- 踩过的坑：虽然请求前台 sleep 300，agy 工具的 WaitMsBeforeAsync=10000 仍在约 10 秒后把它转成后台任务；真人 Esc 时命令继续运行，不能宣称覆盖「同步等待阶段取消命令」。第一次“按过了”只有文字打断，核对记录后请用户完成后续操作，没有误算做完。初查 helper 时猜错源码文件名，随后按 rg 定位到 hooks.rs；启动阶段证据目录尚空时 zsh glob 报无匹配，首个钩子到来后采集正常，未影响测试。
- 要主控决定的事：是否另补同步工具等待阶段的真人实测，或继续调查日志作为取消信号的可靠性；本轮没有自行加测或实施修复。分支等待主控审查。
