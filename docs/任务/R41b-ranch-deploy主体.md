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

## 完成记录

- 做了什么：新增面向人的中文 `ranch deploy <版本> [--dry-run] [--yes]`，完成 BUILD 校验、程序可运行性检查、计划和确认、部署锁与确认后复查、逐个原子切链接、按目标版本命令检查并重装有变化的技能和已装插件。foreign 保留，未装插件不装，helper 只提醒；pony 只比较链接程序并提示手动重启。开始和结束各写一行部署记录，失败停止、不自动撤回，列出已完成／失败／未做和当前链接，支持重跑。混合链接按字节内容推算上一版，否则列逐个链接的恢复命令；退回固定使用开始时解析的当前 ranch 绝对路径。已更新 DESIGN §3 和 pony README 的指定部署说明。对 paddock 无影响，不进 CONTRACT。
- 验证了什么：先写 CLI 预览检查，观察到缺少 deploy 命令、退出 2 的 RED；预览通过后补正常部署检查，观察到执行部分未实现、退出 1 的 RED，再补实现转绿。最终 6 个部署集成测试覆盖任务列出的正常部署、技能／已装插件按需更新、foreign 保留、不安装新插件、两行记录和退回、dry-run 不写、非终端未确认不写、首次建链接、同版本无事可做、三类拒绝、混合链接推算和逐个恢复、中途失败及重跑。全部使用隔离 HOME、CORRAL_HOME、PONY_HOME、临时 versions 和匹配的 BUILD.txt；使用工作区 corral／ranch 与 Rust 合成程序，安装只落在隔离目录。`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次并通过（318 passed、11 个既有 ignored）；编译目录始终为 `.target/r41b`。自查 diff 范围和 `git diff --check` 通过。
- 拿主意的地方：没有需要改变设计或交主控决定的事项。沿用已有依赖（sha2、libc、shell-words、tempfile），没有升级依赖；旧程序明确报 usage／unsupported 才跳过可选检查，真正的检查错误停止部署。原先缺失的链接在逐个退回说明中给出 `rm --`；其余使用 `ln -sfn --`，路径均做 shell 引用。部署锁保留文件、靠文件锁释放，不删锁文件。
- 没做的事：没有改 corral、pony 源码、fence、dispatch 的行为，没有查 pony 进程、调 launchctl、装服务、动 helper 或任何真实 agent。没有在真实 HOME 下运行 deploy（含 dry-run）或安装命令；没有打包、部署、合并或推送，也没有扩大指定验证范围。交叉审查和合并留给主控。
- 踩过的坑：最初用短测试名加 `--exact` 筛到了 0 个测试，改为前缀筛选后才取得有效 RED；没有把空测试运行当验证。混合链接夹具起初用了本机不存在的 `/bin/true`，改用任务允许的 `/bin/cat` 后通过；这是夹具路径错误，没有作为目标行为的 RED。

## 返工记录

2026-10-22，按主控对交叉审查意见的决定返工第 1 次。

- 必须改 1：计划时读取部署记录。最近的部署未完成且目标相同时，沿用那次的 `before_version` 和完整 `rollback`，在计划里说明；当前链接已经等于目标版本时，取最近一次去该目标的记录，避免退回指向目标自己。续跑的开始、结束记录均保留这一退回来源，逐个链接的退回办法也不因部分链接已切换而丢失。DESIGN §3 补明这条规则。
- 建议改 1–4：仅把 `integration status` 的预检超时改为 15 秒，其他预检仍为 5 秒；子进程已被回收时不再向原进程组发信号；pony 仅在 macOS、原来已有链接且程序改变时提醒 kickstart，首装改为「pony 第一次装，服务另装，deploy 不装」；BUILD.txt 声明了但 bin 中缺失的程序一律拒绝，原本没声明 pony 的旧版本仍可部署。
- 采纳的三处输出：链接计划优先显示版本名、指向别处时显示路径；失败汇报的当前链接不带 JSON 引号；上次部署时间转换为本地时区，记录仍写 UTC。确认只认 y、重复报错、测试依赖先编好的 corral 均按主控决定保留；输出细节没有另补独立用例。
- 验证：先只给已有失败续跑测试追加断言，确认最后输出仍为 `deploy new` 而失败，再改实现转绿；同时断言结束记录的 rollback、before_version 仍指 old，并检查完成后再次预览仍保留 old。新增只有 started 记录且部分链接已切换时保留完整逐个退回办法的用例。四条建议各补一例并经历有效 RED → GREEN：6 秒插件状态检查、已回收组长留下输出管道时不再杀组、pony 首装与当前平台提醒、BUILD 清单程序缺失及合法旧包。所有部署均在隔离 HOME、CORRAL_HOME、PONY_HOME 和临时 versions 中；管道持有者是 7 秒自行结束的 Rust 合成程序，测试等待其结束标记，不操作真实 agent。平台用例在本机 macOS 执行，Linux 条件随测试所在平台断言，未做 Linux 实机验证。
- 验证预算：除上述新增用例（含新增断言）外，`cargo test --offline --all-targets`、`cargo clippy --offline --all-targets -- -D warnings`、`cargo fmt --check` 各执行一次并通过；全量 323 passed、0 failed、11 个既有 ignored，部署测试共 11 个通过。编译目录始终为 `.target/r41b`，自查范围及 `git diff --check` 通过。没有改依赖和 corral、pony 源码、fence、dispatch，没有部署、合并或推送；对 paddock 无影响，无需主控另作决定。
