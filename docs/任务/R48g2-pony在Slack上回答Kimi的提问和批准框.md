# 任务：R48g2 pony 在 Slack 上回答 Kimi 的提问框、批准框，「拒绝」「停」能停下 Kimi

2026-10-24，ranch/main 交给 ranch/dev-r48g2（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.82）、要、碰要害）
类型：功能变更
依据：照 R48g1 方案和用户的 5 条决定，让 pony 能在 Slack 上回答 Kimi 的提问框、批准框，「拒绝」「停」能停下 Kimi；corral 给 Kimi 补兼容新增的只读字段。不碰真实 Slack，不部署。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent（下面写明的测试 agent 除外）。

## 用户原话

- 10-24：「1: 我觉得要做到现在已经接入的agent的所有功能（但是可以分阶段做，有几个阶段你要给我列一下）……」。第 4 阶段是 Slack 回答和按钮。
- 10-24：「R48g 先做吧」；主控问三件事，用户都选推荐：提问框、批准框都做，和 Claude Code 对齐，顺带处理后台提问漏掉、提问线程里「停」停不下来；先出方案，用户定了再实现；真 Kimi 实测方案、实现各 5–10 轮短任务。
- 10-24，R48g1 方案（`docs/调研/R48g-Kimi提问和批准框.md`）的 5 件事，用户都选推荐：
  1. 前台提问：两次 Esc 关框并停下这一轮，再把答案当文字送（同 Claude Code）；选项按钮也走这条路。
  2. 后台提问：Slack 列题目、给按钮；等它空下来，在框里按键答，按完核对 `resolved`。
  3. 批准框「拒绝」和停在框上时的「停」：两次 Esc，拒绝并停下这一轮。
  4. corral 改动 §5.3 a–e 都做（e 回退选择器报 `blocked` 也做）。
  5. 非 Bash 的批准第一版在 Slack 上只给「拒绝」；「允许这一次」只给 Bash 命令框。

## 先读

- `docs/调研/R48g-Kimi提问和批准框.md`：全文，重点 §1.1（按键）、§2、§3.4、§4、§5（方案、防误按、corral 要补的、Slack 样子）、§6（要改的地方和测试）、「未知」「用户的决定」。
- `docs/任务/R45-*.md`、`R47a`、`R47b`、`R47c`、`R47d` 的任务文件和审查：Claude Code 那套怎么做的、防误按怎么核对。
- `docs/CONTRACT.md` 里 `status` 的 `questions`、`command`、Kimi 的部分；`docs/DESIGN.md` §7 Kimi、§10 pony。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r48g2`，分支 `r48g2`（已从 main 建好）。只动 `crates/corral/`（Kimi 适配器和 `status` 合并，见报告 §6）、`crates/pony/`、`docs/DESIGN.md`、`docs/CONTRACT.md`、pony README，以及本任务文件末尾的记录。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r48g2`。

## 要做的

照报告 §5、§6 和「用户的决定」做：

1. **corral**：`status` 对 Kimi 兼容新增 §5.3 a–e：Bash 批准框的 `command`；后台提问的 `questions`（带 `background:true`）和 `state_started`；最近一次框的结果 `resolved`；非 Bash 批准框的 `action`；回退选择器开着时报 `blocked`。只从主会话 wire 读，不改钩子，不需要换共用 helper。新旧 corral 交替读同一个 agent 要能用（照 R48d 升游标版本）。
2. **pony 在 Kimi 上**：
   - 前台提问：线程里写答案、选项按钮，都同 Claude Code，只是关框按两次 Esc；提醒末尾那句换成 Claude Code 那句；
   - 后台提问：照报告 §5.4 的样子列题目、给按钮；在干活时排着，空下来再在框里按键答，按完核对 `resolved`，对不上就照实说；
   - 批准框：Bash 命令框给「允许这一次」（按 `1`，命令用 `status.command` 核对）和「拒绝」；非 Bash 只给「拒绝」，显示 `action`；「本会话都允许」一类永远不给；
   - 「拒绝」「停」：Kimi 停在框上（提问、批准、后台提问）按两次 Esc，干活中按一次，空闲不按；
   - 防误按照报告 §5.2：按之前核对、按之后确认、用户已在电脑上答过、过期和连点；
   - 旧 corral 没有新字段时，退回「回电脑处理」，不按键。
3. 文档：CONTRACT、DESIGN、pony README 跟着写，写明对 paddock 的影响（公开约定只有兼容新增）。

## 怎么算做完

- 用户定的 5 条（上面「用户原话」最后一条）都做到。
- 「顺带处理后台提问漏掉、提问线程里『停』停不下来」：后台提问在 Slack 上有题目、能答；Kimi 提问框和批准框上回「停」能停下这一轮。
- Claude Code、Codex、amp、agy、pi、omp 在 pony 里的行为不变。
- 验证只做这些：
  - corral、pony 各自针对的测试，照报告 §6 列的用例（合成 wire、假 corral、内存聊天适配器，隔离 HOME、CORRAL_HOME、KIMI_CODE_HOME、PONY_HOME），先写测试确认现在失败；边角：两个框同时开、旧游标升级和新旧交替重放、旧 corral 没有新字段、命令截断、危险命令；
  - 真 Kimi 在测试 agent 上按 pony 会送的按键序列走一遍：前台提问线程回答、前台多题、后台提问空闲时按键答（加一次「它在干活时开着后台提问」）、Bash 批准允许、批准拒绝、提问框上「停」，记下每条怎么看出来的；
  - `cargo test --all-targets --no-fail-fast`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。
  觉得不够，在回复里说，不要自己加。

## 真 Kimi 实测的边界

- 额度：5–10 轮短任务，超了先停下来问主控。
- 只在自己开的测试 agent `ranch/test-r48g2-<用途>` 上做（`--label role=test`，cwd 用 worktree 外的临时 git 仓库，隔离的 `CORRAL_HOME`，普通模式不加 `--auto`，可以加 `--env KIMI_CODE_TRUST_WORKSPACE=1`，不指定模型），用完 `corral stop`。用本分支编出的 corral。
- Kimi 用用户的真实 `~/.kimi-code`，不改配置和凭证，不读、不往仓库里抄凭证、token、`device_id`、`server.token` 的内容。只让 Kimi 做无害的事，批准框只在临时目录里的命令上允许。
- 不碰真实 Slack：不发消息、不点按钮，不改 `~/.pony`，不动 pony 服务。Slack 上的真机实测由主控在部署后和用户一起做。

## 不要做

- 不给 Kimi 的非 Bash 批准框、危险命令、「本会话都允许」一类任何「允许」按钮。
- 不改 Kimi 的钩子、corral 的共用代码里和 Kimi 无关的部分、fence、dispatch；不升级依赖。
- 不改 PATH 上装好的程序，不打包、不部署，不装技能，不切共用 helper。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。
- 不按项目名或路径批量杀进程（`pkill -f kimi`、`pkill -f corral`、`pkill -f pony` 这类）。停自己起的进程用记下的 PID。
- 不放宽现有测试的断言、不缩短超时、不删测试来过。
- 碰到要拿主意的事（例如报告「未知」里的情况实测和预想不一样、额度不够），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r48g2` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么（真 Kimi 用了几轮、每条怎么看出来的）、拿主意的地方、没做的事、对 paddock 的影响、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-24，ranch/dev-r48g2 实现并自查。

做了什么：corral 只改 Kimi 适配器与 status 合并，主 wire 按交互 id 保存跨轮后台题目，提供 command、action、resolved 和后台标记，回退选择器报 blocked，wire cursor 升到 2。pony 前台文字／选项走两次 Esc、确认 interrupted idle、带原前缀送答案；后台文字／单选／多选等主轮空闲后在框内填写，以 resolved 核对。Bash 只给单次允许，非 Bash、危险／截断／不可读命令不给允许；拒绝和停按框上两次、working 一次、idle 不按。按前核对、过期撤控件、连点、重启不重按和超时如实报告均覆盖。CONTRACT、DESIGN、pony README 已同步。

验证：针对的合成 wire 和假 corral／内存聊天测试先看到目标行为失败，再实现到通过；包含后台题目跨工具／结束／新轮保留、游标重建、两类框互相排队、命令和结果截断、旧 corral 降级、危险命令、前台确认超时、后台实际答案不符／超时、人工操作、实例和框变化、重复操作与重启。用改动前保存的 corral 二进制跑了显式兼容测试：新旧读取方交替三次，新版每次都重建后台题目，解决后两者清框。所有合成测试使用隔离目录，不依赖真实 agent。

真 Kimi 2.3.1 共 **9 轮用户短输入**（后台答案引起的自动通知轮不另算），普通模式、无指定模型，用本分支二进制、独立 CORRAL_HOME、临时 git cwd，测试 agent 为 `ranch/test-r48g2-ui`、`role=test`。按键按 pony 会发的序列直接调用 corral，不连接 Slack：

| 轮次 | 做法 | 看到的证据 |
|---|---|---|
| 1–2 | 前台颜色单选，先两次 Esc，再送带相同前缀的 `B` | 框有完整 questions 和单框信息；两次 Esc 后 idle、interrupted:true、answers:null；下一轮答「蓝」 |
| 3–4 | 前台三题，含单选和多选；两次 Esc 后送米饭／青菜、鸡腿／茶 | 三题及 multi_select 正确；打断确认后新轮完整复述三个答案 |
| 5 | 后台三题，等模型结束后按 `2 1 3 tab 2 1` | tool result 和主轮结束后同一 state_started、questions 仍在，running 从 true 到 false；resolved 分别为米饭、`青菜, 鸡腿`、茶，未截断，自动通知轮也复述一致 |
| 6 | 后台单题同时继续输出平方数，空闲后按 `3`、文字、Enter、`1` | 干活时背景题保持 blocked 且 running:true；空闲后变 false；Other 的 resolved 精确为「梨 R48G2」 |
| 7 | 临时目录 Bash `printf R48G2_ALLOWED > allow.txt && cat allow.txt`，只按 `1` | command 保留原始空格；最终 resolved.decision:approved，文件及回复均为 R48G2_ALLOWED |
| 8 | 临时目录写 deny.txt 的 Bash 框，两次 Esc | idle、interrupted:true、resolved.decision:rejected，deny.txt 不存在 |
| 9 | 前台提问框上模拟「停」，两次 Esc | idle、interrupted:true、对应 resolved.answers:null |

测试 agent 已 stop，隔离命名空间的 ls 为空；前后 config.toml、tui.toml 的 SHA-256 相同。未读取或复制凭证、token、device_id、server.token。实测原始状态和日志只留临时目录，不进仓库。

最终检查各一次通过：`cargo test --all-targets --no-fail-fast`（488 passed、17 ignored；兼容测试已按上文单独显式运行）、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`；`git diff --check` 通过。编译目录全程为 `.target/r48g2`，命令均前台完成。

拿主意的地方：为落实报告要求的「主轮已空闲、最前面是这道框、没有别的框」，兼容新增 `interaction:{kind,count,running}`；单靠 blocked 和 questions 无法区分。缺字段不猜，多框不远程按键。同类框显示最近一次，但计数保留所有未解决框。危险命令采用 Kimi 已知危险模式的保守文本识别，不引入 shell 分析库；引用危险字样也可能降级。前台答案即使确认超时仍须原轮 interrupted，另开新轮就撤掉旧答案并提示重发。这些实现细节已写入 DESIGN 和 CONTRACT，没有改用户的五项决定。

没做的事：未碰真实 Slack、`~/.pony`、pony 服务、已安装程序、钩子、共用 helper、依赖、fence、dispatch 或其他仓库；未打包、部署、合并 main、推送。真实 Slack 验收留给主控部署后与用户一起做。

对 paddock 的影响：公开约定只有兼容新增字段，原命令、状态值与退出码不变，现有调用不用改；若显示后台题目／交互结果，由 paddock 自己的任务接。新旧 corral 可交替读取，旧版仍有原来的后台题目遗漏；新版 pony 配旧 corral 会降级，不按键。

踩过的坑：原按钮清理会在第一次更新后丢掉内容，使后台／允许结果无法更新原消息，现改为确认结束后再清；给满 16 KiB 的后台题目加标记曾丢整题，改为从末尾收回空间、保留文本前缀；通用答案队列在 15 秒后仅凭 idle 会送出 Kimi 答案，已加原轮 interrupted 门槛。真机批准按 `1` 后立即读状态仍可能是 blocked、无 resolved，随后轮询才看到 approved，因此不能把按键返回成功当批准结果。均已验证修复／正确等待。

需要主控决定的事：没有；等待主控审查和后续部署。

### 审查后修改

2026-10-24，按主控决定处理审查意见第 1–7 条，第 8 条不改。审查者的 4 个临时测试仍在，已对照其场景改写为要求正确结果的正式回归；第 5、6 条另加对应复现。第 7 条是文档检查，先检出旧句、修后确认无匹配，不编造行为测试。

逐条修改与 RED → GREEN：

1. **子框计数**：新增 `review_child_dialogs_count_without_changing_main_state_or_command`，先失败于 count 仍为 1（应为 2）；pony 的 `review_child_dialogs_block_approval_background_answers_and_escapes` 又复现主会话 working、只有子框时仍按 Esc。现在读取同会话子 wire 的未解决交互 id，计入总数，只计数、不替代主会话的状态／命令／题目／结果；有子框时单次允许、后台答案及 Esc 都不执行。子计数读不到时 interaction 为 null，保持不能核对就不按。两项测试已通过。
2. **当前进程边界**：`review_resuming_a_session_ignores_previous_process_dialogs` 先复现旧前台框使新实例一直 blocked。现在用 pen 的本实例 started 过滤主、子 wire 的旧事件，覆盖旧前台、旧后台、旧批准及旧子框，新进程再出现的同 id 请求仍有效。wire cursor 升到 3 并核对 started，v1/v2 缓存均重放；不改钩子／helper。用例通过，恢复后 send 不再报 not_idle。现有合成时间从固定的 1970 年移到实际测试实例附近，仍保留原有相对顺序、毫秒边界和断言。
3. **答案队列出口**：选择主控允许的“明确说明并撤掉”，不把答案转换成普通指令。`review_kimi_normal_completion_releases_answer_and_following_message` 先复现到 600 秒仍无发送；现已验证电脑正常答完、以及 Esc 确认超时后原轮正常 idle 两种情况，旧答案不送并撤队，后面的“你好”可发送。确认中的 Esc 仍阻止提前送，正常收到原轮 interrupted 的前台文字／按钮答案流程不变。
4. **同类 FIFO**：`review_same_kind_dialogs_show_the_oldest_then_advance` 先失败于显示后一个框的时间。现在取最早未解决框；批准和提问均验证先显示 first，解决后 count 为 1、state_started 换成 second，给后续提醒新的等待标识。
5. **保留 resolved**：`review_resolved_survives_a_new_dialog_before_the_next_poll` 先失败于结果已为 null。现在新框不清上一结果；主框的新结果覆盖它，新用户输入才清除，子框结果不覆盖。批准与提问两种结果、task 通知轮立刻弹下一框、冷游标重放均通过；仍保留“最近一次结果”的接口，没有新增结果历史数组。
6. **提醒文案**：`review_kimi_notices_quote_commands_and_make_multiple_dialogs_manual` 先失败于命令没有代码块。现在命令中的 `*`、`_`、反引号放在代码块内；多框只提示回电脑，不给操作控件或“写答案／回停”的误导；Kimi 不再追加 Claude 的“认不出框的样子”句子。测试覆盖批准、多框后台题目及危险命令的提示。
7. **文档**：CONTRACT、DESIGN、pony README 同步子框计数、实例边界、FIFO、resolved 保留、正常结束撤队、命令代码块，并补上 Kimi 框上两次 Esc 与后台 `text:`／`tab`／`enter`。R48d 只展示题目的说法注明历史阶段，并指向当前 R48g2。旧句检查修前失败、修后通过。

验证范围：没有重跑 workspace 全套。Kimi 适配器相关测试一次 16 通过、1 个夹具失败、2 ignored；失败的是旧长题测试未关闭前台框就检查后来的后台框，依赖了已纠正的后进先显示。补上前台 resolved、保留原截断和前缀断言，定向重跑该用例与调整了时间的非法 UTF-8 用例，2 项通过；17 个非 ignored 用例均取得通过结果。另显式跑旧二进制交替读取测试，1 项通过。pony 的 Kimi 筛选 11 项通过，随后直接相关的 `question_`、`approval_`、`stop_`、`interrupt_`、`review_` 回归 69 项通过，包含审查新增用例和其他家的原有流程。`cargo clippy -p corral-core -p pony --all-targets -- -D warnings`、`cargo fmt --check`、`git diff --check` 均通过。所有命令前台完成，编译仍只用 `.target/r48g2`。

真机追加 **1 轮，总数 10 轮**：普通模式 Kimi 2.3.1，临时 git cwd、隔离 CORRAL_HOME、`ranch/test-r48g2-review`（role=test）。一条用户输入让主会话开后台单选、子 agent 请求 Bash 在临时目录写 child.txt。观察到主题和 agent-0 的 Bash 批准各有 request、没有 resolved，主轮结束后 status 为 blocked、count:2、running:false；未按任何批准、答案或 Esc。保留两个未解决框直接 stop，再以新实例 `--continue` 恢复同一会话（未追加用户输入／模型轮次），status 为 idle、questions／command／interaction 均 null，无幽灵框；恢复后新一轮的过滤由合成测试覆盖。child.txt 未生成，测试 agent 已再次 stop，隔离 ls 为空。config.toml、tui.toml 前后 SHA-256 相同，未读取或复制凭证和身份文件；原始证据留临时目录，不入库。

取舍和边界：正常结束的旧答案撤掉并提示重发；子框仅用于阻止按键；resolved 继续只保留最近一次主框结果。第 8 条维持旧 corral 缺字段时不按键、原来 working 打断按钮的显示行为也没改。paddock 的现有调用无需修改，公开字段结构保持兼容，计数与恢复状态得到纠正。未碰真实 Slack、安装程序、服务、其他仓库，也未部署、合并或推送。踩过的坑是上述旧 FIFO 夹具，以及实例时间边界要求合成 wire 采用相对测试进程的时间；原非法 UTF-8 事件也一起移入当前时间窗，保留其原检验意义。没有新增需要主控决定的事项。

### 复核后修改

2026-10-24，按主控决定只修复核新问题 1、2，3、4 不改。对照仍在的审查临时测试补正式回归：`review2_bad_old_child_line_nulls_interaction_forever` 先失败于旧残缺行使 count 为 null；修为末行等换行、完整坏行跳过后通过，并覆盖分段追加新请求、换行后计数、后续 resolved。`review2_kimi_background_null_interaction_reports_unverifiable` 先失败于撤后台答案的理由不实，现明确说“无法核对 Kimi 的框”，仍撤队且不按键、不发送。`review2_kimi_command_fence_closed_by_command_content` 先复现命令里的三连反引号提前关块，现围栏取命令最长连续反引号加一、至少三个，三连／五连分别用四连／六连，原文和允许按钮保留；测试通过。DESIGN 同步这两项决定。

验证只跑针对及直接相关回归：corral 的 `review2_`、`review_child_dialogs`、`review_resuming_a_session` 共 3 项通过；pony 的 `review2_`、`kimi_approval_`、`kimi_background_`、`review_child_dialogs`、`review_kimi_notices` 共 9 项通过。`cargo fmt --check`、`git diff --check` 通过，命令均前台完成，编译仍用 `.target/r48g2`。本次不用真 Kimi（累计仍为 10 轮），不跑全套；未碰真实 Slack、部署或其他仓库，只提交任务分支供主控合并。公开字段结构不变，paddock 不用动。

## 主控审查

2026-10-24，ranch/main。
- 实际开的是 `ranch/dev-r48g2-1`（Codex gpt-6-astra / xhigh）。首次提交 `aaeeec3`：看了 diff，只动了允许的目录；Kimi 批准核对实例、框开始时间、`last_tool`、轮次、命令原文、人工输入和单框；重跑全套 488 通过、17 跳过、0 失败。真 Kimi 9 轮。
- 交叉审查 `ranch/review-r48g2-1`（Claude Code opus[1m] / xhigh）：改完再合并，必须改 2 条（子 agent 的框没计数，可能误批准；上一个进程留下的幽灵框），建议改 5 条。主控交回改第 1–7 条，第 8 条不改；dev 在 `d69a62a` 改完，真 Kimi 再用 1 轮（共 10 轮），主控重跑全套 495 通过、0 失败。
- 复核：可以合并；新提 2 条建议改（子 wire 坏行让计数一直读不到、批准提醒代码块围栏被命令里的反引号提前关掉），用户定修完再合并，dev 在 `7577b97` 改完，主控看过 diff。另 2 条可以不改的不采纳（子 wire 每次全读目前几十毫秒；子框先关后主框不重发提醒，和「回电脑处理」一致）。
- 合并前主控在 `7577b97` 跑全套：497 通过、1 失败、17 跳过；失败的是 `crates/corral/tests/sandbox.rs` 的 `wrapped_agent_keeps_kind_hooks_prompt_and_lifecycle`（本任务没碰这个文件），单独复跑 3 次都通过，像是文件出现后内容还没写完就读的偶发，记进 HANDOFF 已知问题。clippy、fmt 通过。
- 真实 Slack 验收待部署后和用户一起做。
结果：返工 1 次
