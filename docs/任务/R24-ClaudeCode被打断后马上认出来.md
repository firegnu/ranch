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
3. 只对 Claude Code 生效，别家不动；不改 R22 已有的行为和公开字段。
4. 读对话记录要省：只在需要时读（比如 Claude Code 还是 `working` 时），只读末尾一段，不整份读。
5. 两份技能和 CONTRACT 里「Claude Code 认不出」的说法改成实际情况，写清对 paddock 的影响。

## 怎么算做完

- 用一个 Claude Code 测试 agent，分别在它写字时、跑命令时按 Esc 打断：几秒内 `status` 是 `idle`、`interrupted: true`，主控挂的 `--after` 提醒不送、`wait` 返回 `interrupted`、`reply` 不给出上一轮的回复，`send` 能送进新话。
- 之后人在窗口里让它接着干，这一轮正常结束时提醒照送，回复是新的。
- 验证只做这些：
  - 针对测试：先写测试，确认它因为这个问题失败，再改；
  - 边角用例：正常结束照旧；打断后人马上接着输入；对话记录读不到或格式不认识时照旧不认；别家行为不变；
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
