# 任务：人按 Esc 打断被委派的 agent 后，主控不再误以为它做完了

2026-10-19，ranch/main 交给 ranch/dev-r22（Codex，gpt-6-astra / high）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.91）、要、碰要害）
类型：Bug 修复
依据：人在被委派 agent 的窗口里按 Esc 打断它，corral 把这一轮当成正常结束：`--after` 提醒照送，`reply` 给的还是上一轮的回复，主控就误以为做完了。本轮在 corral 里把「被人打断」和「正常结束」分开，不新增状态值，不改别的状态规则。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做。除了下面「真实 agent 实测」写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-19：“我在使用的时候，我发现一个问题，如果被委派的agent正在干活，我按下esc打断了他，这时候，主控会认为它做完了。这个要修复”
- 10-19：主控列了两种改法（甲只改技能文字；乙改 corral 的判断）和乙的四条（提醒不送、加字段和 `interrupted`、不新增状态值、Claude Code 先实测），用户：“可以，按乙定，补全任务书派出去吧。”

## 先读

- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」。
- `crates/corral/src/events.rs`（共用状态机）、`crates/corral/src/events/raw.rs`（pi、omp 的原始事件转换）、`crates/corral/src/after.rs`（`send --after`）、`crates/corral/src/cli.rs` 的 `status`、`turn_end`（`wait`）、`reply`。
- `crates/corral/src/agents/` 里 codex、agy 的 `Interrupt`，`crates/corral/resources/amp_plugin.ts` 的 `cancelled`。
- `docs/CONTRACT.md` 开头的通用规矩（前端忽略未知字段、显示未知取值）；`corral guide` 里 `wait`、`reply`、提醒的说法（源文件 `crates/corral/resources/AGENT_USAGE.md`、`SKILL.md`）。
- `crates/dispatch/resources/corral-dispatch/SKILL.md` 第 4、5 节里收到提醒后怎么判断的那几句。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r22-interrupted`，分支 `r22-interrupted`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r22-interrupted`。
- 只动 `crates/corral/`、`crates/dispatch/resources/corral-dispatch/SKILL.md`、`docs/CONTRACT.md`、本任务文件。

## 主控查到的（10-19 读代码，没实测）

- `events.rs`：`Stop`、`StopFailure`、`Interrupt` 一样处理，都把状态置为 `idle`；只有 `Stop` 更新 `reply`，`Interrupt` 不动它，`reply` 还是上一轮的。
- Codex 有 `Interrupt` 钩子；agy 的 `USER_CANCELED` 记成 `Interrupt`；amp 插件把 `cancelled` 记成 `Interrupt`。
- pi、omp：`raw.rs` 把 `agent_end` 一律转成 `Stop`，被打断（大概是 `stopReason` 为 `aborted` 一类）也当成正常结束。要实测确认打断时报的是什么。
- Claude Code 没有打断的钩子：被打断后大概停在 `working`，等它自己的 `idle_prompt` 通知才变 `idle`（`corral guide` 里 `stopped-quiet` 那句也提到这种情况）。推测，要实测。
- `after.rs`：等到 `idle` 就送；人 30 秒内在窗口里打过字时才先等一会儿。
- paddock 按 `idle`、`working`、`blocked`、`starting`、`exited` 认状态，认不出的显示「未知」；用 `corral reply` 做卡片的回复预览，报错时不显示预览。

## 要做的（用户 10-19 定）

1. **被打断的那一轮单独记下来**：状态照旧是 `idle`（不新增状态值），`status` 加一个字段 `interrupted`：最近结束的这一轮是被人打断的为 `true`，否则 `false`；下一轮开始（新输入进来）就回到 `false`。
2. **`wait`**：碰到被打断的这一轮，返回 `result: "interrupted"`，不返回 `idle`。
3. **`send --after` 提醒**：被打断的那一轮不送，接着等这个 agent 下一次正常结束一轮；等满 `--timeout` 时照旧送。人接着在窗口里让它干，这一轮正常结束时照送。
4. **`reply`**：最近这一轮是被打断的，不给出上一轮的回复：返回现有的 `no_reply` 错误，带上 `"interrupted": true`。
5. **哪几家**：报 `Interrupt` 的 Codex、agy、amp 照上面改；pi、omp 先实测打断时报什么，能可靠认出来就一起改；Claude Code 先实测，现有事件能可靠认出来才改，认不出来就不改，在完成记录里写明。
6. **技能和约定**：`corral guide`（`AGENT_USAGE.md`、`SKILL.md` 里相关的那几句）、corral-dispatch 技能里收到提醒后怎么判断的那几句、CONTRACT 加一节，写清新字段、新结果值、`reply` 的变化、对 paddock 的影响。

## 怎么算做完

- 在一个测试 agent 干活时按 Esc 打断：主控挂的 `--after` 提醒不送、`wait` 不返回 `idle`、`reply` 不给出上一轮的回复。
- 之后人在窗口里让它接着干，这一轮正常结束时提醒照送。
- 上面两条照「要做的」第 5 条，在改了的每一家上都过。
- 验证只做这些：
  - 针对测试：先写测试，确认它因为这个问题失败，再改；
  - 边角用例：正常结束的一轮照旧送提醒、`wait` 照旧返回 `idle`、`reply` 照旧给出回复；被打断后人接着干、正常结束；`StopFailure` 不算打断；
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次；
  - 下面的真实 agent 实测一遍。
  
  觉得不够，在回复里说，不要自己加。

## 真实 agent 实测

- 用本分支编出来的 corral（打一个版本目录到临时位置，或用编译目录里的程序），隔离的 `CORRAL_HOME`，不部署。
- 测试 agent 名字 `ranch/test-interrupt-<种类>`，带 `--label role=test`、`--env CAIRN_DISABLE=1`；各家照 corral-dispatch 技能第 4 节的免确认参数起，模型用各家默认，活要小（比如让它数到很大的数、每步停一下，好有时间打断）。一家做完、存好证据，再开下一家，用完 `corral stop`。
- Esc 要在真终端里按，你按不了：每家起好、进入 `working` 后停下来，在回复里写明要用户在另一个终端 `corral attach <名字>`（带上隔离的 `CORRAL_HOME`）里按 Esc，以及打断后要不要接着让它干；等主控转回「按过了」再看结果。几家可以一次排好一起请用户按，减少来回。
- 不要用 `corral keys` 送 Esc 来代替真人按键当验收依据；可以用它先自己预演，但完成记录里要分开写。

## 不要做

- 不新增状态值，不改 `blocked`、`starting`、`exited` 等别的状态规则，不改 pen、sandbox、fence、升级、attach。
- 不改 paddock（paddock 要不要显示「被打断」由它自己的任务做）。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不靠删测试、放宽断言、缩短超时来通过。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 某一家的打断怎么都认不可靠，停下来报告，不要靠猜时间间隔之类的办法硬认。
- 不合并到 main，不推送。只在 `r22-interrupted` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：共用状态机保留中断原因，status 新增 interrupted（状态仍 idle，新输入清 false），wait 返回 interrupted，reply 返回退出码 1/no_reply/interrupted=true，after 跳过中断这一轮并继续按原截止时间等待。pi 采集器补记原生 agent_settled.aborted，reader 据此转 Interrupt。更新 guide、corral 技能、dispatch 技能第 4/5 节和 CONTRACT，说明识别范围与 paddock 影响。
- 自动验证：共用路径先写 6 项集成测试，实际复现 wait 返回 idle、reply 返回 previous reply、提醒提前结束等待、缺少 interrupted，再修至全过；pi 的原生 aborted 转换测试也先红后绿。覆盖正常结束、打断后新输入再正常结束、StopFailure 不算打断、提醒超时照送。最后 cargo test --all-targets、cargo clippy --all-targets -- -D warnings、cargo fmt --check 各跑一次，均通过；全套 165 passed、0 failed、11 ignored（原有 3 项外部基线兼容测试和 8 项环境限定测试）。git diff --check 通过。
- 真人实测：均用本分支程序、隔离 CORRAL_HOME、role=test、CAIRN_DISABLE=1、各家默认模型，Esc 和继续输入均由用户在真终端操作，没有用 keys 模拟验收。Codex、pi、amp 的完整流程通过：中断后 status=idle/interrupted=true，wait=interrupted，reply=no_reply/interrupted=true，原提醒保持 waiting；真人输入只回复 R22-CONTINUED 后，last_input_source=human、interrupted=false、wait=idle、reply=R22-CONTINUED，同一条提醒随后送达。amp 的实际中断快捷键是连续两次 Esc。
- 实测限制与已定范围：omp 18.12.2 的 Esc 只取消当前 bash 工具，toolResult 仅有 isError=true、details={} 和文本 [Command aborted]，模型随后回复并以 stopReason=stop 结束；没有可靠结构化标记，用户确认本轮跳过 omp，不改转换。agy 的真人 Esc 在终端显示 Interrupted，但 events 停在 PostToolUse，没有 Stop/Interrupt，仍 working、wait 超时、reply 仍是基线；用户确认保留这个限制，已有 USER_CANCELED → Interrupt 映射和共用修复仍保留。Claude Code 的真人 Esc 后终端显示 Interrupted，约 80 秒后 events 仍停在 PreToolUse，没有结束事件，wait 超时、reply 仍是基线；按任务第 5 条不改其识别规则。不能把这些家写成已通过打断验收。
- 拿主意的地方：只用明确事件，不按工具输出、模型措辞、静默时间或读屏猜中断。pi 第一次真人打断实际报 stopReason=error，可靠原因在原生 agent_settled.aborted；查本机接口及实现后补采，并在重开的 pi 上实测 true/false。cursor 缓存升为 5，避免沿用旧 reader 丢失中断原因的缓存；事件格式仍为 v1/v2，旧版会重建自己的版本 4。快照只更新缓存版本、新增的 false 字段和 pi 采集器新增字段，没有放宽断言。旧命令/提醒进程仍有旧行为，旧 pi 采集器需重开，这些边界已写 CONTRACT。
- 对 paddock：新增字段、wait 结果值和 reply 错误附加字段属于兼容新增；原状态显示不用动，reply 报错后按原行为不显示预览即可。若要显示「被打断」，由 paddock 自己的任务接。
- 没做的事：未改 pen、sandbox、fence、升级、attach 或其他状态规则，未改别的仓库，未部署、未安装技能、未升级用户 agent、未合并 main、未推送。所有测试实例（含合成 /bin/cat 提醒接收者）均已停止，隔离 corral ls 确认空。原始证据和检查日志留在临时目录 /tmp/r22.YUwTZ3，不提交运行数据。
- 踩过的坑：首次测试命令误用二进制名 corral 作为 Cargo 包名，改成 corral-core 后才取得有效 RED；pi 原有采集器丢掉了真正可靠的 aborted 字段，需重开测试实例；omp 取消工具后会再调用模型；agy 未按预期报结束钩子；amp 首次因用户误键退出不计验收，按用户要求重开，随后发现一次 Esc 只弹快捷键提示，连续两次才发 Interrupt。未将这些未通过或误操作记录当成功。
- 主控需决定的事：没有新的待决定项。omp 跳过、agy 保留限制已获用户确认；本分支等待主控审查和规定的交叉审查，未自行派审查 agent。

### 返工 1

- 修复交叉审查的两条必须改：pi 的 ui_prompt_end 在 idle 时生成的内部 Stop 增加 idle_only 标记，只恢复空闲，保留 interrupted 和旧回复保护；新输入、真正的 Stop/StopFailure 仍按原约定清标记。after 保留 paused 的优先级，把中断抑制限定为 idle 且 interrupted=true，后续 blocked 仍按原稳定等待规则送提醒，restarted、超时截止时间和接收端保护未改。
- 各补一条合成回归测试并先运行：关闭对话框的测试因 interrupted=false（期望 true）失败；打开对话框的测试因提醒结果 timeout（期望 blocked）失败。修复后两条通过；关闭测试同时验证 wait=interrupted、reply=no_reply/interrupted=true、提醒不按 idle 提前送出，以及真正新输入和正常结束后恢复回复并送出原提醒。
- 只跑针对性和直接相关回归：`cargo test -p corral-core --test interrupted` 8 项，`cargo test -p corral-core --lib events::` 4 项，暂停提醒用例 `reminders_wait_for_a_paused_agent_on_either_side` 1 项，状态与缓存用例 `state_started_tracks_transitions_and_survives_events_and_cursor_replay` 1 项，全部通过；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过。没有重跑全套或真人实测，没有修改快照、删除测试或放宽断言。
- 拿主意的地方：idle_only 只在 reader 内部的转换事件上使用，不增加持久事件类型、状态值或公开字段；原 ui_prompt_end 的状态、last_event 等无关行为保持原样。paddock 不用动。本次没有新的坑或待主控决定事项；未部署、未合并、未推送。
