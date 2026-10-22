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

## 完成记录

2026-10-22，ranch/dev-r41c。

- 做了什么：将 pony 纳入 deploy 的计划、确认、执行、核对和部署记录。按真实进程路径与目标程序的 sha256 判断重启；plist 为 replace 时调用目标版本的 service install，same 且程序有变时 kickstart。重启后每半秒检查，最多等 30 秒，要求 running、新 pid 和目标程序；连接状态照实打印。失败先报 pony 当前状态与日志位置，保留已完成步骤和退回办法。续跑重新检查进程，不依赖已切换的链接。deploys.jsonl 改为按字节分行解析，坏 UTF-8／JSON 只影响该行；更新了 DESIGN §3 和 pony README 的部署与退回说明。对 paddock 无影响。
- 验证了什么：先确认中文截断测试因 UTF-8 读取失败，pony 重启、plist 替换、停止／手动运行等用例也在实现对应行为前出现目标失败，再逐项实现。部署相关测试共 22 个通过（含测试子进程入口）；覆盖判断表每行、相同内容不同路径、重启等待三个条件、Slack connecting、完整 30 秒超时汇报、续跑和中文截断。测试清空继承环境，使用隔离 HOME／CORRAL_HOME／PONY_HOME、合成程序及假的 launchctl／进程查询，不调用真实 launchctl。`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次，均通过；全量测试保留原有 ignored 用例，没有开启额外验证。命令均等待前台完成，编译目录为 `.target/r41c`。
- 拿主意的地方：遵循任务明确的「没在跑／父进程不是 launchd 都不重启」，即使 plist 为 replace，也不替换这两类服务。旧版 c3a0b86 的 status 固定报 not_installed，因此额外检查 plist 是否存在，避免漏掉「服务装着、目标不认 service」的拒绝条件。系统调用通过函数注入给测试替换，生产程序没有测试环境变量开关。service install 自身可能等 bootout 30 秒，外层命令限 45 秒，完成后再开始规定的 30 秒启动核对。部署记录只保留进程、退出与连接状态，不复制频道／工作区资料。没有需要主控决定的事项。
- 没做的事：没有修改 corral、pony/src、fence、依赖或 Cargo.lock；没有操作真实服务、用户 agent、共用 helper 或真实安装目录；没有部署、合并、推送、更新 HANDOFF，也没有开其他 agent。交叉审查和合并留给主控。
- 踩过的坑：集成测试通过 `#[path]` 复用 deploy 命令入口时，Rust 查找子模块的相对位置与主程序编译时不同，最初报 E0583；改为显式指定 deploy/command.rs、deploy/pony.rs 后解决。该编译失败没有当作行为测试的 RED 证据。

### 返工（第 1 次）

2026-10-22，按主仓库 `R41c-审查.md` 末尾主控决定执行；该审查文件只读，未修改。

- 改了什么：采纳必须改 1 的两部分。服务安装失败后，汇报及「接着做」都明确提示定义可能已卸下、先运行目标版本的 `pony service install --yes`。沿用同一目标的未完成记录时，若 `pony_service` 失败且 `pony.before.running` 为 true，即使当前已停、plist 已是 same，也列出服务恢复，确认后执行并等待核对；连续失败仍保留最初在跑的事实和 pid。执行前仍用当前状态复查，手动运行和原本就停止的服务继续不动。
- 改了什么：采纳建议改 1–3。跳过 replace 时说明定义没换、给出会启动 pony 的安装命令，手动运行时加上先停止的提醒；计划显示真实运行程序所属版本或外部路径；`Home::command` 加断言，真实二进制测试入口拒绝临时 HOME 内已有 pony plist，服务用例必须走注入替身的 `pony_run`。
- 改了什么：采纳可以不改 3、4 的指定部分。追加部署记录前检查末字节、按需补换行，避免新的 started 粘进旧半行；只有实际尝试 pony 操作后才在失败汇报中查询状态、给日志位置，不再凭 plist 存在声称 launchd 会重试，正常退出也不这样提示。同步 DESIGN §3 和 pony 部署说明；对 paddock 无影响。
- 验证了什么：先用假的 service install 复现「旧服务已卸、新 plist 已写、bootstrap 失败」，两个用例分别因错误恢复提示和续跑「没有要做的」失败，修复后通过；其中续跑用例连续失败两次再成功，验证恢复依据保留。记录补换行用例先复现三行而非四行，修复后确认旧半行保留、started／done 各自可解析。版本／路径提示、两类跳过 replace 的提示也先确认缺失再补齐；原有用例增加了未动 pony 不报状态日志、正常退出不声称重试的断言。
- 验证了什么：只运行本次相关用例和 `cargo test -p ranch --all-targets`（部署 26 个、dispatch 11 个，全通过）；`cargo clippy -p ranch --all-targets -- -D warnings`、`cargo fmt --check -p ranch` 各一次通过。沿用 `.target/r41c`，命令均等待前台完成。没有运行本轮范围外的全仓测试，没有真实 launchctl 或真实服务操作。
- 拿主意／没做／坑：只落实主控采纳项；未改 PONY_HOME 继承或父进程识别方式，未改 corral、pony/src、fence、依赖和 Cargo.lock，未部署、合并或推送。没有新的主控待决定事项。本轮没有额外的工具或环境问题。
