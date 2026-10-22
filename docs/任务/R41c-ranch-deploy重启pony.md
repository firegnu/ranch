# 任务：R41c `ranch deploy` 自动重启 pony

2026-10-22，ranch/main 交给 ranch/dev-r41c（Codex，gpt-6-astra / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.27）、要、碰要害；按第 1 条读 `critical-other`，审查读 `review-claude`：Claude Code opus[1m] xhigh）

类型：功能变更
依据：照 R41 设计稿 1.4 把 R41b 里「要你手动重启 pony」那一行换成 deploy 自己判断、自己做；顺带修 R41b 复核留下的部署记录读法。不改别的。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21（R39，主控列、用户让记进去）：「部署命令 `ranch deploy <版本>`：现在每次部署由主控写一串 `ln -sfn …` 交用户跑，有时还要重装技能、切共用 helper、kickstart pony，全靠 HANDOFF 里的经验。做成命令，先列要做什么、用户确认后一次做完、记下退回办法。」
- 10-22：设计稿 7 条全部按推荐，其中第 7 条：拆成 R41b（主体）和 R41c（pony 那几步），都按碰要害做、交叉审查。用户：「好的，那就按照你的建议干吧」。
- 10-22：「其实就是加一个脚本呗。其他都不动，我理解没错吧？」主控答：corral、pony、fence 的代码、paddock、在跑的 agent、共用 helper 都不动。

## 先读

- `docs/调研/R41-ranch-deploy设计.md` 第 1.4 节（整节）、1.6（顺序）、第 4 节（失败）、第 5 节最后一条（Linux）。
- `crates/ranch/src/deploy.rs`：R41b 现在怎么列 pony 提醒、怎么记步骤和失败、怎么续跑。
- `docs/任务/R41b-审查.md` 末尾「复核意见（第 1 轮）」的建议改 1、2，以及「主控对复核意见（第 1 轮）的决定」。
- `crates/pony/src/service.rs`、`crates/pony/README.md`「部署与退回」：`pony status`、`pony service install --dry-run/--yes` 的输出，launchd 服务名，以及测试里怎么把 launchctl 换成假的。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r41c`，分支 `r41c`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r41c`。
- 只动 `crates/ranch/`、`Cargo.lock`（只在确实要时）、`docs/DESIGN.md` §3 讲部署的那段、`crates/pony/README.md`「部署与退回」里 pony 重启那步，以及本任务文件末尾。

## 要做的

1. 照设计稿 1.4 的表判断 pony 这一步：服务没装或不是 macOS 只切链接；plist 要换就用目标版本的 `pony service install --yes`；plist 相同、在跑、正在跑的程序和目标不同就 `launchctl kickstart -k gui/<uid>/local.ranch.pony`；程序相同不重启；服务装着没在跑、在跑但父进程不是 launchd 都不重启并说明；服务装着而目标版本的 pony 不认 `service` 就拒绝。「正在跑的程序」按 pid 查真实程序，比对 sha256，不只看链接。
2. 重启后照 1.4 等新的 pony 起来（最多 30 秒，`running`、pid 换了、真实程序是目标的），Slack 连没连上照实打印、不算成功条件；失败照第 4 节停下汇报，把「pony 现在没在跑」放在最前面，给出日志位置。
3. 计划、核对、部署记录里都有 pony 这一步；续跑时如果 pony 还在跑旧程序，照样列出重启（R41b 复核建议改 2，按进程比对自然覆盖）。
4. R41b 复核建议改 1：`deploys.jsonl` 按字节读、按行解析，某一行不是合法 UTF-8 或 JSON 就当作不完整的一行，不要让整条命令失败。
5. 文档：DESIGN §3 讲部署那段和 pony README「部署与退回」里 pony 重启那步，改成由 `ranch deploy` 做。

## 怎么算做完

- 「做成命令，先列要做什么、用户确认后一次做完」：pony 程序变了时，`ranch deploy` 的计划里列出重启 pony，确认后自己做完并核对新的 pony 起来了，用户不用再手敲 `launchctl`。
- 「其他都不动」：`crates/corral/`、`crates/pony/src/`、`crates/fence/` 没有改动；不碰在跑的 agent，不 `corral upgrade`，不切共用 helper。
- 验证只做这些：
  - 针对这件活的测试：1.4 表里每一行各一个用例（launchctl、查进程这些外部调用照 `pony service` 的做法换成假的，测试里绝不调真的 `launchctl`）；重启后等不到新 pony 的失败汇报；续跑时 pony 仍是旧程序会列出重启；`deploys.jsonl` 末行截断在中文字符中间时 `--dry-run` 照常、显示末行不完整。
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。
  - 觉得不够，在回复里说，不要自己加。
- 对 paddock：无影响。

## 不要做

- 不改 corral、pony、fence 的代码；不升级依赖，新依赖只用工作区已有的。
- 不装 pony 服务（服务没装时不替人装），不写 `~/.pony`，不读 `~/.pony/secrets`。
- 测试和开发中绝不碰真实的 `~/.local/bin`、`~/.local/share/ranch`、`~/.pony`、`~/Library/LaunchAgents`：不在真 HOME 下跑 `ranch deploy`（`--dry-run` 也不跑），不跑真的 `launchctl`（包括只读的 `print`）、`pony service install`、`install-skills`。用户在用的 pony 服务正在跑，误操作会让 Slack 断掉。
- 不碰用户现有的 agent；不部署。
- 不要按项目名或路径批量杀进程（`pkill -f ranch`、`pkill -f corral`、`pkill -f pony` 这类）：主控、别的 agent 和用户的 pony 的进程命令行里都带着项目名和路径，一条命令能把它们全杀掉。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r41c` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
