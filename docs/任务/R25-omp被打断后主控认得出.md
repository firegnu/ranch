# 任务：omp 被人按 Esc 打断后，主控认得出

2026-10-19，ranch/main 交给 ranch/dev-r25（Codex，gpt-6-astra / high）。
路由：常规 / 交叉审查要 / 影响面：碰要害（路由：常规（置信 0.62）、拿不准（核心规则 0.71、并发 0.7）、拿不准；推翻：同一处的 R22、R24 交叉审查都查出过要改的缺陷，判断错了会让正常结束被当成打断、提醒不送，算核心规则，按碰要害走）
类型：Bug 修复
依据：R22 让 Codex、pi、amp 被打断后主控不再误以为做完，R24 补上 Claude Code；omp 当时没找到可靠标记，跳过了。主控 10-19 查到 omp 有明确标记，本轮把它接到 R22 现有的「被打断」行为上，不改别的家、不改别的状态规则。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做。除了下面「真实 agent 实测」写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-19（R22）：“如果被委派的agent正在干活，我按下esc打断了他，这时候，主控会认为它做完了。这个要修复”
- 10-19：“查一下omp有没有中止的信号”；看了主控的结论后：“可以，写 R25 草稿给我看”。

## 先读

- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」。
- `docs/CONTRACT.md` 开头 R22 那一节（`interrupted`、`wait`、`reply`、提醒的规则，和 omp 的那句限制）。
- `crates/corral/src/events/raw.rs`：pi、omp 的原始事件转换，`agent_settled`、`agent_end` 两支，以及文件里现有的 `omp/*` 测试。
- `crates/corral/resources/hook_omp.ts`：omp 采集器记下了哪些字段。
- `docs/任务/R22-人打断被委派的agent后主控误以为做完.md` 末尾的完成记录里 omp 那几句。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r25-omp-interrupted`，分支 `r25-omp-interrupted`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r25-omp-interrupted`。
- 只动 `crates/corral/`、`crates/dispatch/resources/corral-dispatch/SKILL.md`、`docs/CONTRACT.md`、本任务文件。

## 主控查到的（10-19 读 omp 18.12.2 程序里的代码，加上本机 omp 会话记录；没用 corral 实测）

- omp 在生成回复或调用工具时按 Esc，会中止整轮。这一轮最后那条 assistant 消息带：
  - `stopReason: "aborted"`；
  - `errorMessage: "Interrupted by user"`；
  - `errorId` 里有「用户打断」那一位（omp 里 `UserInterrupt = 67108864`）。
  
  这条消息会出现在 `message_end` 的 `message` 里，也会出现在随后 `agent_end` 的 `messages` 里。
- 本机 `~/.omp/agent/sessions` 里以前按 Esc 留下的记录就是这样，`errorId` 为 67112960。另有一条别的中止，`errorMessage` 为 `"Request was aborted"`，`errorId` 里没有「用户打断」那一位。
- 现装采集器已经原样记下 `message`、`messages`，所以采集器不用改，omp 也不用重开。现在没认出来，是因为 `raw.rs` 把不是 `willContinue` 的 `agent_end` 一律转成 `Stop`。
- omp 在跑命令时按 Esc，只停那条命令，模型接着干、正常结束（R22 实测）。这不算打断这一轮，照旧当正常结束。

## 要做的

1. omp 的一轮以「被人打断」结束时，按 R22 已有的 `Interrupt` 行为处理：
   - 状态 `idle`、`interrupted: true`；
   - `wait` 返回 `interrupted`；
   - `reply` 返回 `no_reply` 并带 `interrupted: true`；
   - `--after` 提醒不送。
   
   「被人打断」只认上面那组明确标记。其他中止、出错、`willContinue` 的路径照旧，现有的重试延迟也照旧。
2. 更新说明里 omp 那几句：`corral guide`（`AGENT_USAGE.md`、`SKILL.md`）、corral-dispatch 技能、CONTRACT 的 R22 一节。写清对 paddock 的影响。

## 怎么算做完

- 在 omp 测试 agent 生成回复时按 Esc 打断：主控挂的 `--after` 提醒不送、`wait` 不返回 `idle`、`reply` 不给出上一轮的回复。
- 之后人在窗口里让它接着干，这一轮正常结束时提醒照送。
- 验证只做这些：
  - 针对测试：先写测试，确认它因为这个问题失败，再改；
  - 边角用例：
    - 正常结束照旧是 `Stop`；
    - `"Request was aborted"` 这类非用户打断的中止不算打断；
    - 跑命令时 Esc 只停命令、随后正常结束的不算打断；
    - 可重试的出错照旧走重试延迟；
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次；
  - 下面的真实 agent 实测一遍。

  觉得不够，在回复里说，不要自己加。

## 真实 agent 实测

- 用本分支编出来的 corral（打一个版本目录到临时位置，或用编译目录里的程序），隔离的 `CORRAL_HOME`，不部署。
- 测试 agent 名字 `ranch/test-interrupt-omp`，带 `--label role=test`、`--env CAIRN_DISABLE=1`，按 corral-dispatch 技能第 4 节的免确认参数起（`omp --approval-mode yolo`），用 omp 默认模型。活要小，但要让它生成的文字够长，好有时间在它写字时打断。存好证据，用完 `corral stop`。
- Esc 要在真终端里按，你按不了：起好、进入 `working` 后停下来，在回复里写明要用户在另一个终端用 `corral attach <名字>`（带上隔离的 `CORRAL_HOME`）按 Esc，以及打断后要不要接着让它干。等主控转回「按过了」再看结果。
- 不要用 `corral keys` 送 Esc 来代替真人按键当验收依据。可以用它先自己预演，但完成记录里要分开写。

## 不要做

- 不改 `hook_omp.ts` 采集器，不改别的家（Codex、Claude Code、pi、amp、agy）的识别，不新增状态值或公开字段，不改 pen、sandbox、fence、升级、attach。
- 不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不靠删测试、放宽断言、缩短超时来通过。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 实测发现 omp 被 Esc 打断时报的和上面「主控查到的」不一样，或者认不可靠，停下来报告，不要靠猜时间间隔、按文字模糊匹配之类的办法硬认。
- 不合并到 main，不推送。只在 `r25-omp-interrupted` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：omp 最终 `agent_end` 只检查最后一条 assistant 消息，三个明确标记齐全才转 `Interrupt`，沿用 R22 的状态、wait、reply 和提醒行为。保留 `willContinue`、普通结束、非用户中止、工具取消后正常结束和重试延迟。更新 AGENT_USAGE、corral 技能、corral-dispatch 技能和 CONTRACT，写明识别条件、部署边界及 paddock 不用动。
- 自动验证：先写原始事件回归测试，旧代码实际得到 Stop、期望 Interrupt，修复后通过。CLI 集成测试覆盖 wait=interrupted、reply=no_reply/interrupted=true、原提醒继续等待、下一轮正常结束后送达；该测试还先复现了旧 cursor 保留 interrupted=false，再升缓存版本修至通过。指定边角用例和原有 pi/omp 事件快照通过。`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次，均通过；全套 180 passed、0 failed、11 ignored（原有 3 项外部基线兼容测试、8 项环境限定测试），git diff --check 通过。
- 真人实测：使用本分支编出的程序副本、隔离 CORRAL_HOME，唯一真实测试实例为 `ranch/test-interrupt-omp`，omp 18.12.2 默认模型、`--approval-mode yolo`、`role=test`、`CAIRN_DISABLE=1`。先正常回复 R25-BASELINE，再开始长文本生成并挂 `r25-omp-after` 提醒；用户在真终端按 Esc，message_end 和 agent_end 的最后 assistant 都为 aborted / Interrupted by user / errorId=67112960。随后 status=idle/interrupted=true、wait=interrupted、reply 退出码 1/no_reply/interrupted=true，原提醒 wait_done=false、phase=waiting，没有送出。用户在窗口输入继续后，last_input_source=human、interrupted=false、wait=idle、reply=R25-CONTINUED；同一条提醒随后以 wait_result=idle、phase=confirmed、result.confirmed=true 送达，实例回复 R25-REMINDER-RECEIVED。没有用 keys 模拟 Esc 或继续，没有另开 agent。
- 拿主意的地方：只在最终 agent_end 判整轮中断，message_end 不提前结束一轮；errorId 按 UserInterrupt 位判断，不写死整个 67112960。内部 cursor 缓存 7 → 8，避免旧 reader 已把同一条原始事件读成 Stop 后继续误判；快照只机械更新缓存版本，原断言和事件格式 v1/v2 不变，新旧 reader 各自重放。提醒接收者也用同一个测试 omp，避开用户 agent 并遵守不另开 agent 的范围；正常结束后等待原有 30 秒真人输入保护期再送达，没有 force 绕过。
- 对 paddock：不新增状态值或公开字段，沿用 R22 的约定，paddock 不用动。调用方和提醒进程须用新版；omp 采集器原样保留，不用重开 omp。
- 没做的事：未改其他家识别、采集器、pen、sandbox、fence、升级、attach 或其他仓库；未部署、未装技能、未操作用户现有 agent、未合并 main、未推送。测试实例已 corral stop，隔离 corral ls 确认空。原始证据和检查日志留在 `/tmp/r25.2n6yJF`，不提交运行数据。主控及交叉审查留给 ranch/main 安排。
- 踩过的坑：真实提醒的接收者是可识别的 omp，送达后的最终 phase 是 confirmed；首次前台轮询照合成 /bin/cat 用例只等 sent，因而没把已经确认的送达当作成功。随后按 confirmed 回执及实际收到的 R25-REMINDER-RECEIVED 核实，没有重发提醒或换 request_id。
- 主控需决定的事：没有新增待决定项；等待主控审查和任务规定的交叉审查。

## 主控审查

- 10-19 ranch/main 看了 diff 和完成记录：只动了 `crates/corral`、两份技能、CONTRACT 和本文件，「不要做」里的一件都没做。`raw.rs` 只在 omp 最终 `agent_end` 的最后一条 assistant 三个标记齐全时转 `Interrupt`，其余路径照旧；快照只改了 cursor 7 → 8。
- 重跑 `cargo test --all-targets`：180 通过、0 失败、11 跳过，和完成记录一致。
- 「怎么算做完」：用户在真终端里于 omp 写字时按 Esc，提醒不送、`wait` 返回 `interrupted`、`reply` 不给旧回复；用户接着让它干、正常结束后原提醒照送。用户是直接在开发 agent 窗口里回的「按过了」「继续过了」。
- 取舍：同意 cursor 升到 8（旧 reader 已把同一轮读成 Stop，要重放）；同意 errorId 按位判断。
- 交叉审查（ranch/review-r25-1，Codex xhigh）：可以合并，必须改 0 条、建议改 0 条，见 `docs/任务/R25-审查.md`。
- 结果：一次过
