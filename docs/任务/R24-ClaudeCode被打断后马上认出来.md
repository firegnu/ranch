# 任务：Claude Code 被人按 Esc 打断后，corral 马上认出来

2026-10-19，ranch/main 交给 ranch/dev-r24（Codex，gpt-6-astra / high）。
路由：常规 / 交叉审查要 / 影响面：碰要害（路由：常规（置信 0.62）、拿不准、拿不准；主控按 R22 的经验定：同样碰共用状态判断和提醒，R22 交叉审查在这块挑出过两处真缺陷）
类型：Bug 修复
依据：R22 让 Codex、pi、amp 被打断后不再被当成做完，Claude Code 因为没有打断钩子留着没修。本轮让 corral 从 Claude Code 自己的对话记录里认出打断，接上 R22 已有的那套行为。别家不动。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做。除了下面「真实 agent 实测」写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-19（R22 部署后）：“没有claude code吗？”“claude code我觉得要解决。不然咱们再测试一次？”
- 10-19（主控说 Claude Code 被打断后主控要等 10 分钟甚至 1 小时才知道）：“这么长时间肯定不行啊。”

## 先读

- `AGENTS.md`「规矩」。
- `docs/任务/R22-人打断被委派的agent后主控误以为做完.md`（任务书、完成记录、主控审查）和 `docs/CONTRACT.md` 开头 R22 一节：`interrupted`、`wait` 的 `interrupted`、`reply` 的 `no_reply`、`--after` 跳过，这些行为照用，不改。
- `crates/corral/src/hooks.rs`（钩子记录哪些字段）、`crates/corral/src/events.rs`（共用状态机）、`crates/corral/src/cli.rs` 的 `status`、`crates/corral/src/agents/claude.rs`。
- `crates/corral/src/agents/agy.rs` 读对话记录的写法（`transcriptPath`），可以参照。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r24-claude-interrupted`，分支 `r24-claude-interrupted`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r24-claude-interrupted`。
- 只动 `crates/corral/`、两份技能里 R22 写的「Claude Code 认不出」那几句、`docs/CONTRACT.md` 的 R22 一节、本任务文件。

## 主控和用户 10-19 实测到的（Claude Code，本机现装版本）

测试 agent 用 `-- claude --dangerously-skip-permissions` 起，用户在真终端里按 Esc：

- **写字时被打断**（回复写到一半）：corral 一个事件都没收到，状态一直是 `working`，`last_event` 停在 `UserPromptSubmit`。对话记录（`~/.claude/projects/<目录>/<session_id>.jsonl`）在按 Esc 的同一秒多了一条 `type: "user"` 的记录，文字是 `[Request interrupted by user]`。之后 3 分钟内没有空闲通知（`Notification idle_prompt`），也没有别的事件。
- **跑命令时被打断**（`Bash` 在跑 `ping`）：corral 停在 `PreToolUse`，没有 `PostToolUse`、`Stop`。对话记录里先是那条命令的 `tool_result`（`toolUseResult` 为 `User rejected tool use`），紧接着一条 `type: "user"`，文字是 `[Request interrupted by user for tool use]`。
- 状态一直是 `working`，所以 `corral send` 报 `not_idle`，送不进新话；主控只能等提醒超时或后台等待的 `stopped-quiet`（600 秒）。
- 钩子的输入里有 `transcript_path`，但 corral 现在只记 `has_transcript`（`hooks.rs`），没记路径。

## 要做的

1. Claude Code 被人打断后，corral 在几秒内认出来：状态变 `idle`，`interrupted: true`，之后照 R22 的行为走：`wait` 返回 `interrupted`、`--after` 提醒不送、`reply` 报 `no_reply` 带 `interrupted: true`；`send` 能送进新话。
2. 只认对话记录里**最近一次输入之后**明确写下的打断标记（上面两种写法）。不按静默时间、读屏、输出内容猜。标记格式变了、读不到对话记录时，退回现在的行为（认不出），不能把正常结束认成打断。
   - 标记的时间必须晚于 corral 记下的最近一次输入（`UserPromptSubmit`）。人打断后马上又送了一句时，corral 可能先收到新输入、Claude Code 还没把这句写进对话记录，最后一条用户消息仍是旧标记；这时不能算打断（用户 10-19 同意的防法）。
3. 只对 Claude Code 生效，别家不动；不改 R22 已有的行为和公开字段。
4. 读对话记录要省：只在需要时读（比如 Claude Code 还是 `working` 时），只读末尾一段，不整份读。
5. 两份技能和 CONTRACT 里「Claude Code 认不出」的说法改成实际情况，写清对 paddock 的影响。
6. CONTRACT 写明新旧版本的边界：部署前就开着的 Claude Code，钩子还是旧程序、不记对话记录路径，认不出打断，要 `corral upgrade` 或重开才生效；旧版命令行忽略新字段。

## 怎么算做完

- 用一个 Claude Code 测试 agent，分别在它写字时、跑命令时按 Esc 打断：几秒内 `status` 是 `idle`、`interrupted: true`，主控挂的 `--after` 提醒不送、`wait` 返回 `interrupted`、`reply` 不给出上一轮的回复，`send` 能送进新话。
- 之后人在窗口里让它接着干，这一轮正常结束时提醒照送，回复是新的。
- 验证只做这些：
  - 针对测试：先写测试，确认它因为这个问题失败，再改；
  - 边角用例：正常结束照旧；打断后人马上接着输入（含新输入已到、对话记录还没写进去的那一刻）；对话记录读不到或格式不认识时照旧不认；别家行为不变；
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次；
  - 下面的真实 agent 实测一遍。
  
  觉得不够，在回复里说，不要自己加。

## 真实 agent 实测

- 用本分支编出来的 corral（打一个版本目录到临时位置，或用编译目录里的程序），隔离的 `CORRAL_HOME`，不部署。
- 测试 agent 名字 `ranch/test-interrupt-claude`，带 `--label role=test`、`--env CAIRN_DISABLE=1`，`-- claude --dangerously-skip-permissions`，模型用默认。活要小：写字时打断，可以让它不用工具写一篇长文；跑命令时打断，用一条要跑一两分钟的正常命令（比如 `ping -c 90 127.0.0.1`；Claude Code 不许在前台单独跑 `sleep`）。停之前存好证据，用完 `corral stop`。
- Esc 要在真终端里按，你按不了：起好、进入 `working` 后停下来，在回复里写明要用户在另一个终端运行的完整 `corral attach` 命令（带上隔离的 `CORRAL_HOME` 和程序路径）、按 Esc 后按 Ctrl+] 退出，以及要不要接着让它干；等主控转回「按过了」再看结果。两种打断可以一次排好，减少来回。
- 不要用 `corral keys` 送 Esc 来代替真人按键当验收依据；可以先自己预演，完成记录里分开写。
- 只读测试 agent 自己的那份对话记录，不读用户别的 Claude Code 会话。

## 不要做

- 不改别家的识别，不改 R22 已有的公开字段和行为，不新增状态值。
- 不改 Claude Code 的设置（`~/.claude/settings.json` 等），不改 pen、sandbox、fence、升级、attach。
- 不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不靠删测试、放宽断言、缩短超时来通过。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 实测发现标记的写法和上面不一样、或者打断后 Claude Code 还会写别的记录让判断不可靠，停下来报告，不要靠猜。
- 不合并到 main，不推送。只在 `r24-claude-interrupted` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：钩子保留 `transcript_path`，状态缓存只保存主会话的路径。Claude 适配器在 `working` 时最多读取对话记录末尾 64 KiB，只认本会话最近用户记录中的两种明确标记，且时间戳严格晚于最近一次 `UserPromptSubmit`。识别后复用 R22 的 Interrupt 处理并保存状态，后续空闲通知不清掉打断原因；新输入或正常结束照原规则更新。别家适配器默认不读对话记录，没有新增公开字段或状态值。更新 corral guide、两份技能相关句子和 CONTRACT 的识别范围、旧钩子升级边界；paddock 不用动。
- 针对验证：先加合成集成测试，实际看到 `status` 为 working、期望 idle 的失败，再实现至通过。覆盖两种标记、wait=interrupted、reply=no_reply/interrupted=true、提醒继续等、send 文字实际到达合成程序且输入钩子确认、新输入钩子先到而旧标记仍在、随后正常回复及原提醒送达。边角检查覆盖时间相等或早于新输入、缺文件、坏格式、未知标记、旧缓存重放、正常 Stop、别家不读该标记、有界末尾与不完整写入。
- 全套验证：`cargo test --all-targets` 首次因测试夹具的 send 确认预期写错中止，修正后补跑通过，170 passed、0 failed、11 ignored（原有跳过项）。`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过。收尾自查发现只做当前查询覆盖会在后续空闲通知时丢失打断原因，补充回归先得到 interrupted=false 的失败，再改为保存识别结果；最终 `--test interrupted` 10 项、`--lib events::` 4 项、protocol 的状态时间与缓存重放 1 项通过，corral 全 targets 的 clippy、fmt 和 diff 检查通过。收尾修正后没有重复全套或真人实测。
- 真人实测：同一个 `ranch/test-interrupt-claude`，隔离 CORRAL_HOME、本分支程序、role=test、CAIRN_DISABLE=1、默认模型及免确认参数。用户分别在写长文和前台 `ping -c 90 127.0.0.1` 时按 Esc，未用 keys 代替。两次均查到 idle/interrupted=true、wait=interrupted、reply=no_reply/interrupted=true，原 `--after` 记录一直 waiting/wait_done=false；对话记录分别为任务书中的两种标记，工具场景也有 User rejected tool use。第一次打断后普通 send（不带 force）成功确认下一句，清除打断标记并进入 Bash。用户最后在真终端输入继续指令，正常 Stop 后 interrupted=false、wait=idle、reply=R24-CONTINUED；同一条原提醒变为 sent、wait_result=idle，合成 `/bin/cat` 接收端收到文字。
- 实测证据边界：真人 Esc 后是在用户转述回来时查询，没有独立记录从按键到首次状态采样的秒数；查询同步读取标记，没有静默等待。最终正常输入的来源由用户确认，未把 `last_input_source` 当作真人输入的证明（退出 attach 后它显示 agent）。如主控要求严格的真人秒级计时证据，需要另行决定，本轮未加测。
- 拿主意的地方：沿用适配器边界，共用状态判断不写 Claude 特判；不增加依赖，用已有 libc 按 Claude 的 UTC 毫秒格式转换并拒绝无效日期。cursor 5 升到 6，防止旧 reader 已消费路径字段却未缓存；旧 reader 仍重放自己的缓存，事件格式不变。快照只改 cursor 版本。标记不完整、超出有界末尾或不认识时不猜，不按输出内容、静默时间或读屏推断。仅当前查询覆盖的初版改成保留已识别状态，以维持 R22 的回复和提醒保护。
- 没做的事：没有改 pen、sandbox、fence、升级或 attach，没有改 Claude 设置、其他仓库或别家识别，没有部署、安装技能、升级用户 agent、合并 main 或推送。没有开审查 agent。真实 Claude 和合成提醒接收端均已停止，隔离 corral ls 为空；证据与检查日志在 `/tmp/r24.zZTsxw`，未提交运行数据。本任务限定范围不含 DESIGN，主控合并时可补其 R22 后的 R24 说明。
- 踩过的坑：合成 `/bin/cat` 不会发 UserPromptSubmit，最初错误地期待 send 自动确认，导致首次全套中止；改为先看到输入实际到达，再补合成钩子，明确断言 confirmed=true。只在查询中覆盖状态会丢掉已识别的打断原因，收尾回归复现后改为缓存。没有把失败运行写成通过，也没有删测试、放宽断言或缩短超时。
- 主控需决定的事：上述真人秒级计时证据是否需要补充；其余没有新待决定项。本分支留给主控及规定的交叉审查，不自行合并。

### 返工 1

- 四条审查意见均已修复。新增内部 `turn_open`，由当前实例、主会话的输入及明确结束事件重放得到，空闲通知不关闭它；未结束的一轮即使状态已是 idle，也可从同一份对话记录恢复打断证据。正常 Stop/StopFailure、会话结束关闭该轮，换主会话重置，新输入仍用严格时间比较排除旧标记。不新增事件文件写入方。cursor 升到 7，避免沿用缺少轮次边界的旧缓存。
- 读对话记录改为 `O_NONBLOCK` 打开后对实际 FD 检查普通文件，FIFO 及指向它的符号链接立即退回未识别。显式 `isSidechain:true` 的用户记录即使 sessionId 相同也不作为打断证据。CONTRACT、guide、两份技能同步说明：调用方及提醒进程需用新版，该命名空间的 `.runtime/helper` 也需切到能记录路径的版本；新版 upgrade 会切换 helper，单纯重开不足。没有改 pen 或 helper 选择逻辑，paddock 不用动。
- 先补四条回归并运行：同版过期回写、旧版缓存重建后 interrupted=false；FIFO 查询超过 2 秒截止时间；同 sessionId 的侧链使状态错误变 idle，四条均在目标缺陷上失败。缓存回归控制写入顺序，并构造旧版 v5 缓存触发重放，不依赖真实旧程序或随机并发；同一用例还核对 wait/reply 保护，以及 Stop/StopFailure、新输入后再通知及重建不会误判。FIFO 用例对普通 FIFO 和符号链接分别检查 status 返回与 wait 的 0.2 秒超时，超出 2 秒就按子进程句柄终止并回收。
- 修正后只跑指定范围：interrupted 集成测试 14 项、Claude 适配器单元测试 1 项、events 回归 4 项、protocol 状态时间与缓存重放 1 项，全部通过；`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`、`git diff --check` 通过。快照仅更新 cursor 版本和新增内部 turn_open；去掉这两项后与提交前快照逐字比较一致，没有改其他期望值。
- 踩过的坑：FIFO 修复后的首次复跑发现新测试误把 wait 超时当作 result=timeout；实际约定是退出码 4、error=timeout，已按该约定修正断言，2 秒截止时间保留。初始 FIFO 阻塞的测试进程已按句柄杀掉并回收，合成 agent 由夹具停止。
- 未重复全套或真人实测，未打开真实 agent、读取真实 Claude 对话、部署、合并或推送；没有改主仓库审查文件。主控已决定不补真人精确计时，本次不重开该问题，没有新的待决定项。
