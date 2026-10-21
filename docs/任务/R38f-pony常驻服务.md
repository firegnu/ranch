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

## 完成记录

2026-10-21，ranch/dev-r38f，分支 `r38f-pony-service`。

### 做了什么

- 加 `service.rs` 和命令入口：`pony service install|uninstall [--dry-run] [--yes]`。默认只列计划，dry-run 优先；原子写自有 plist、加载、重复装卸、foreign 保护、非 macOS unsupported 和一行 JSON 输出均已接入。程序、日志路径按任务固定为 HOME 下的默认位置，正式程序只执行 `/bin/launchctl`，没有可替换程序的环境变量或配置。
- `pony status` 的 `service` 按自有 plist 是否存在取值；核心仅改这一处。对 paddock：JSON 格式不变，`service` 开始报真实值，paddock 不用动；运行状态仍看 `running`，不拿文件存在冒充进程已启动。
- 修分享消息：attachment 带 `is_share: true` 时，即使带 `from_url`／`original_url` 也计为附件，沿用核心的「附件没送」提示；只贴链接产生的预览仍不计附件。
- README 写清命令与 JSON 字段、安装和卸载、kickstart 换版、退回、删配置后的恢复、日志及代理；整理 R38e 两段上线说明和复核第 13 条为实测清单。
- 自查 diff：只动 `crates/pony/` 和本文件末尾；没有新依赖，`Cargo.lock`、`crates/corral/`、`chats/mod.rs`、CONTRACT、DESIGN、设计稿都未改。

### 验证了什么

- 分享消息测试先跑，确认因「share 被当成 preview」断言失败；只改附件判断后通过。同一测试保留粘贴链接不算附件的断言，现有核心附件提示测试也在全量验证中通过。
- 服务 CLI 测试先确认旧程序因不认识命令返回 usage 失败；装卸测试先确认 apply 尚未实现失败，然后实现到通过。5 条服务单元测试覆盖：只读预览、重复装卸和命令顺序、日志保留与权限、原子更新、外来文件／目录／链接、并发变化复查、launchctl 报错保留文件及已卸载状态。CLI 测试覆盖默认预览、dry-run 与 yes 同给、参数错误、自有／外来／链接／缺失 plist 对 status 的影响。macOS 的 plutil 实际解析生成的 XML，核对包括特殊字符路径在内的字段。
- 指定的最终检查各实际运行一次：`cargo test --all-targets --offline`（279 通过、0 失败、11 个原有忽略）、`cargo clippy --all-targets --offline -- -D warnings`、`cargo fmt --check`，全部通过。全量测试的 HOME、PONY_HOME、CORRAL_HOME 指向临时目录；其他命令子进程也沿用已有隔离 fixture，服务单元测试直接传入临时 HOME 和 launchctl 替身，不读取真实用户状态。
- 所有命令在前台结束，使用本分支独立 CARGO_TARGET_DIR；没有调用真实 launchctl、连接真实 Slack 或读取真实凭证。非 macOS 分支有条件测试，但本轮运行平台是 macOS，未另做 Linux 验证。

### 拿主意的地方

- 沿用 corral 的归属标记思路：自有 plist 以独立一行 `PONY_SERVICE = 1` XML 注释识别；服务路径下的链接视为 foreign，HOME 本身允许是链接。新文件只硬链接到空位，自有文件更新用 rename；操作前和 bootout 后复查，变了不覆盖、不删除。
- 重装同内容不重写文件。bootstrap 失败后，仅 exact service target 的 print 成功才视为已加载；其余报错并保留 plist，供排查和重试。bootout 只把成功或 ESRCH 当成可删文件，其他错误保留文件。卸载不删用户配置、凭证、状态、日志或共享目录。
- 服务使用任务指定的默认 HOME 路径，不把调用者的 PONY_HOME、代理、凭证复制到 plist；加载前创建或核对私有日志目录／日志，防止 launchd 因日志路径不存在而启动失败。命令输出不透传 launchctl 文本。没有需要主控另作决定的事。

### 没做的事

没有部署、打包、切链接、安装真实服务、建 Slack 应用、发外网请求、读取真实凭证或操作用户 agent；没有改 R38e 留着不改的 8、10、11、12，也没有扩适配器接口。未合并、推送，交叉审查与真实上线由主控安排；README 的实测清单尚未实测。

### 踩过的坑

第一次准备全量测试的命令带 `rm -rf` 清理临时目录的 trap，被工具的自动审批检查拒绝，命令和测试都未启动。改成只用 `rmdir` 删除本次创建的空目录后，前台完整跑完并清理成功；没有因此重复运行全量测试。其余没有非预期的试错失败。
