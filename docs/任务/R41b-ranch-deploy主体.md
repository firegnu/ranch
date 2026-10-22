# 任务：R41b `ranch deploy` 主体：切链接、重装技能和插件、记录和退回

2026-10-22，ranch/main 交给 ranch/dev-r41b（Codex，gpt-6-astra / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.3）、拿不准、拿不准；会写用户的 `~/.local/bin` 和技能目录，设计稿和用户 10-22 定的第 7 条都是按碰要害做、交叉审查；按第 1 条读 `critical-other`，审查读 `review-claude`：Claude Code opus[1m] xhigh）

类型：功能变更
依据：照 R41 设计稿实现 `ranch deploy` 除 pony 重启以外的部分；pony 那几步是 R41c，这一件里只打印提醒。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21（R39，主控列、用户让记进去）：「部署命令 `ranch deploy <版本>`……做成命令，先列要做什么、用户确认后一次做完、记下退回办法。」
- 10-22：「其实就是加一个脚本呗。其他都不动，我理解没错吧？」主控答：是 `ranch` 里多一个子命令，corral、pony、fence 的代码、paddock、在跑的 agent、共用 helper 都不动。
- 10-22：设计稿末尾 7 条「要用户定的」全部按推荐（A）：中途失败不自动撤回；退回就是 `ranch deploy <旧版本>`；要 `--dry-run`；四个链接一起切；记录在 `~/.local/share/ranch/deploys.jsonl`；全局插件和技能一样有变才重装；拆成 R41b、R41c 两件。用户：「好的，那就按照你的建议干吧」。
- 底线：「不要动ranch中corral的核心代码。」

## 先读

- `docs/调研/R41-ranch-deploy设计.md` 全文。这件做第 1 节的 1.1、1.2、1.3、1.5、1.6，第 2、3、4、5 节；1.4（pony）不做，见下面「要做的」第 3 条。
- `AGENTS.md`「规矩」。
- `crates/ranch/src/main.rs`、`crates/dispatch` 的 `install-skills`（确认方式、`confirmation_required`），`crates/pony/src/service.rs`（原子写、可替换的外部调用，测试里不调真程序的做法）。
- `tools/package/src/main.rs`：`BUILD.txt` 的格式。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r41b`，分支 `r41b`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r41b`。
- 只动 `crates/ranch/`（要用 `crates/dispatch` 里的东西可以，但不改它的行为）、`Cargo.lock`、`docs/DESIGN.md` §3 讲部署的那段、`crates/pony/README.md`「部署与退回」里切链接和重装技能那几步，以及本任务文件末尾。

## 要做的

1. 照设计稿做 `ranch deploy <版本> [--dry-run] [--yes]`：先检查（1.1）、打印计划（第 2 节的样子）、确认、拿锁并复查、切四个链接（1.2）、技能和全局插件有变才重装（1.3）、helper 只提醒（1.5）、核对、两行部署记录（第 3 节）、打印退回命令；中途失败照第 4 节停下汇报，重跑同一条命令接着做。
2. 部署前链接混着时，照第 3 节按程序逐字节相同算出「部署前是哪一版」；算不出来就打印逐个链接的退回办法。
3. **pony 这一步只打印，不做**：目标版本的 pony 和正在用的不同，在「要做的」最后列一行「要你手动重启 pony：`launchctl kickstart -k gui/<uid>/local.ranch.pony`」，不执行任何 launchctl、不跑 `pony service install`；pony 没变就放在「不做的」里。查「正在用的」只比对链接指向的程序，不查进程（查进程是 R41c 的事）。
4. 文档：DESIGN §3 加一段「部署用 `ranch deploy`」（做什么、不做什么、退回、记录在哪）；pony README「部署与退回」里切链接、重装技能的步骤改成指向它，pony 重启那步暂时照旧手敲。

## 怎么算做完

- 「先列要做什么、用户确认后一次做完、记下退回办法」：`ranch deploy <版本>` 先打印计划，确认后做完切链接和需要的技能、插件重装，最后打印退回命令并写进部署记录。
- 「其他都不动」：`crates/corral/`、`crates/pony/src/`、`crates/fence/` 没有改动；不碰在跑的 agent，不 `corral upgrade`，不切共用 helper。
- 验证只做这些：
  - 针对这件活的测试，全部用隔离的 HOME、`CORRAL_HOME`、`PONY_HOME` 和临时的 `versions/`（放工作区编出的 corral、ranch 和合成程序，配对得上的 `BUILD.txt`）：正常部署（链接切好、技能有变才装、记录两行、退回命令对）；`--dry-run` 什么都不改；不在终端又没 `--yes` 时什么都不改、退出 1；已经是这个版本时没事可做；校验和对不上、`~/.local/bin` 里是普通文件、不认识的程序这几种拒绝；混着的链接算出上一版；中途失败停下、重跑接着做。
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。
  - 觉得不够，在回复里说，不要自己加。
- 对 paddock：无影响。只多一条给人用的命令，不进 CONTRACT。

## 不要做

- 不做 pony 的进程查询、kickstart、`pony service install`（R41c）。
- 不打包、不删版本目录、不装这台机器没装过的插件、不改 `~/.fence/local.toml`。
- 测试和开发中绝不碰真实的 `~/.local/bin`、`~/.local/share/ranch`、`~/.claude/skills` 等用户目录：不在真 HOME 下跑 `ranch deploy`（`--dry-run` 也不跑）、不跑真的 `install-skills`、`integration install`、`launchctl`。
- 不改 corral、pony、fence 的代码；不升级依赖，新依赖只用工作区已有的。
- 不碰用户现有的 agent；不部署。
- 不要按项目名或路径批量杀进程（`pkill -f ranch`、`pkill -f corral`、`pkill -f pony` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r41b` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 主控审查

- `d956512` 主控看过 diff、重跑 `cargo test --all-targets` 318 通过、0 失败、11 忽略，clippy、fmt 通过；corral、pony 源码、fence 没改。
- 交叉审查（`R41b-审查.md`）：必须改 1 条（失败后重跑做完，退回命令指向目标自己，退不回去），交回返工；同时采纳建议改 1–4 和三处输出细节。返工提交 `56061dc`，复核第 1 轮可以合并，审查者在隔离 HOME 里用本机真实版本副本实测失败、重跑、退回都对。
- 复核新提的两条建议改并进 R41c。
结果：返工 1 次
