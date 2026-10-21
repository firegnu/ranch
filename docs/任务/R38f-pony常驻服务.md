# 任务：R38f pony 装成常驻服务，修「分享消息」被漏掉

2026-10-21，ranch/main 交给 ranch/dev-r38f（Codex，gpt-6-astra / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.28）、拿不准、拿不准；要写用户全局目录 `~/Library/LaunchAgents`，又改 Slack 适配器，照 R38 定下的「适配器按碰要害处理、交叉审查」；按第 4 条读 `critical-other`，审查读 `review-claude`：Claude Code opus[1m] xhigh）
类型：功能变更
依据：R38 的实现第五件。加 `pony service install|uninstall`，让 pony 作为 launchd 用户服务常驻；顺带修 R38e 复核留下的建议改 13；写部署说明。本轮不连真实 Slack、不建 Slack 应用、不真的装服务（合并后用户在场时做）。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：「选 1，写 R38f 任务书吧。」（选 1：R38e 复核建议改 13 放进 R38f 一起修。）
- 10-21 设计定稿：后台常驻用 launchd 用户服务（设计稿「用户定的」第 2 条）。
- 底线：「不要动ranch中corral的核心代码。」
- R38 验收第 6 条：「Slack 的凭证只放在本机，不进任何仓库。」

## 先读

- `AGENTS.md`「规矩」：安装、测试不依赖真实 agent 和用户状态、验证、独立编译目录。
- `docs/DESIGN.md` §10「服务」那几条和 §7（全局安装的规矩：先列再写、只动自己的文件、能卸干净）。
- `docs/调研/R38a-Slack机器人设计.md`：3.4（plist 怎么写、怎么加载卸载），命令表里 `pony run`、`pony service` 两行。
- `docs/CONTRACT.md`「`pony status`（R38）」的 `service` 字段。
- `docs/任务/R38e-审查.md`「复核意见（第 1 轮）」第 13 条；`docs/任务/R38e-pony的Slack适配器.md` 末尾两段「给 R38f 的上线说明」（部署说明要用）。
- `docs/任务/R38c-审查.md` 主控决定里「删掉配置正常退出 0，launchd 不会再起」那句。
- corral 怎么原子写、怎么认 `foreign`：`crates/corral/src/integration.rs`，只读照着学，不改。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38f-pony-service`，分支 `r38f-pony-service`（已从 main 建好）。只动 `crates/pony/`、`Cargo.lock`（真要加依赖时），以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r38f-pony-service`。

## 要做的

1. **`pony service install|uninstall [--dry-run] [--yes]`**，照 DESIGN §10 和设计稿 3.4：
   - 不带 `--yes`（或带 `--dry-run`）只列要写的文件、内容要点和要跑的 `launchctl` 命令，什么都不改。
   - `install --yes`：原子写 `~/Library/LaunchAgents/local.ranch.pony.plist`（`ProgramArguments` 指向 `~/.local/bin/pony run` 的绝对路径，`RunAtLoad`，`KeepAlive.SuccessfulExit` 为 false，标准输出和错误写 `~/.pony/logs/launchd.log`），再 `launchctl bootstrap gui/<uid>`。已经装过的再装一次不出错。
   - `uninstall --yes`：`launchctl bootout` 后删掉这个文件，卸干净；没装过的卸一次不出错。
   - 那个位置已有不是 pony 写的文件：报 `foreign`，不动它。
   - 非 macOS 报 `unsupported`。
   - 输出 JSON，和 `pony status` 一样的 `ok`／`error` 风格；具体字段你定，写进 README。
2. **`pony status` 的 `service` 报真实状态**：pony 自己的 plist 在就是 `installed`，不在是 `not_installed`，非 macOS 照旧 `unsupported`。格式照 CONTRACT，不加字段。
3. **修「分享消息」被漏掉**（R38e 复核 13）：用户在 Slack 里用「分享消息」转发进来的那条（attachment 带 `is_share`），不能再被当成链接预览悄悄丢掉；至少照别的附件一样，告诉用户「附件没送」。只贴链接产生的链接预览照旧不算附件。
4. **部署说明**写进 `crates/pony/README.md`：装、卸服务；部署新版后用 `launchctl kickstart -k gui/<uid>/local.ranch.pony` 换到新程序；退回；删掉配置后 pony 正常退出、launchd 不会再起，放回配置后要 `kickstart` 或重新登录；日志在哪、看什么；代理什么时候要填。把 R38e 两段「给 R38f 的上线说明」整理进来，实测要看的几处列成清单。
5. 对 paddock：`pony status` 格式不变，`service` 开始报真实值。写进完成记录。

## 怎么算做完

- 「选 1，写 R38f 任务书吧。」：在 Slack 里用「分享消息」转发给 pony 的内容不再被悄悄漏掉。
- `pony service install`、`uninstall` 照上面「要做的」1 工作：不带 `--yes` 什么都不改；带了能装、能卸干净；别人的文件不动；`pony status` 的 `service` 跟着变。
- 「不要动ranch中corral的核心代码。」：`crates/corral/` 没有改动。
- 「Slack 的凭证只放在本机，不进任何仓库。」：plist、日志、命令输出、测试里都没有 token。
- 验证只做这些：上面 1、2、3 各有测试（3 修之前先跑，确认它因为这个问题失败）；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 测试和你自己试的时候，绝不跑真的 `launchctl`，绝不读写真实的 `~/Library/LaunchAgents`、`~/.pony`、`~/.corral`；`HOME`、`PONY_HOME`、`CORRAL_HOME` 都指向临时目录。怎么替换掉 `launchctl` 你定，但不能留一个能让正式程序改跑别的程序的口子（只在测试里生效，或者只能指向固定的系统路径）。
- 不真的装服务、不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包。
- 不连 Slack、不发网络请求、不建 Slack 应用、不找也不读真实凭证。
- 不改 `crates/pony/` 以外的文件（除本任务文件末尾）；不改 CONTRACT、DESIGN、设计稿；共用核心（`crates/pony/src/core/`）只为第 2 条改 `service` 的取值；`chats/mod.rs` 的接口不改，确实要改先问主控。
- 不改 R38e 审查里定了「这轮不改」的 8、10、11、12，不顺手改别的行为。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch`、`pkill -f pony` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停测试里起的进程用记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r38f-pony-service` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
