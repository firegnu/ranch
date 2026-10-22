# 任务：R41 先出设计——`ranch deploy` 一条命令部署

2026-10-22，ranch/main 交给 ranch/dev-r41（Claude Code，opus[1m] / xhigh）。
路由：高 / 交叉审查不要 / 影响面：看得见（照 R38a 的先例：本轮只出设计、不改代码，技能规定设计一律算看得见、不交叉审查；档位按第 2 条「本轮只出设计」定为高，类别界面和设计，读 `high-design`）
类型：设计
依据：R39 排第一的「一条命令部署」开工的第一步。本轮只出设计稿给用户定，不写代码、不改 DESIGN、不部署。
提示：围绕已知约束给出推荐及关键取舍；有实质备选时再比较，不凑方案数量。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21（R39 原 6.9，主控列、用户让记进 R39）：部署命令 `ranch deploy <版本>`：现在每次部署由主控写一串 `ln -sfn …` 交用户跑，有时还要重装技能、切共用 helper、kickstart pony，全靠 HANDOFF 里的经验。做成命令，先列要做什么、用户确认后一次做完、记下退回办法。
- 10-22 主控向用户说明的做法：`ranch deploy da45737` 先列出这次要做什么、不做什么（哪几个链接变了、技能要不要重装、pony 要不要重启），用户确认后一次做完，最后打印怎么退回；不碰正在跑的 agent，不切共用 helper，不打包。要先定：要不要 `--dry-run`、退回是不是同一条命令指向旧版本、中途失败已做的几步要不要自动退回。用户：「好的，按照你的计划走吧」。
- 底线（R38 起一直沿用）：「不要动ranch中corral的核心代码。」

## 先读

- `AGENTS.md`「规矩」里的「安装」「不要干扰用户正在用的 agent」两条。
- `HANDOFF.md`「本机安装」「部署时要记得」两节：现在部署要做的每一件事和长期有效的坑。
- `docs/DESIGN.md` 讲安装方式的部分（版本目录、链接、技能安装），以及 §10 里 pony 常驻服务一段。
- `tools/package/`：打包怎么产出版本目录、`BUILD.txt` 里记了什么。
- `crates/ranch/src/main.rs`：`ranch` 现在的子命令。
- 技能安装和插件状态的现有命令：`corral install-skills`、`ranch dispatch install-skills`、`corral integration status`（看它们的 `--dry-run` 或状态输出能不能拿来判断「要不要重装」）。
- `crates/pony/` 里 `pony service install|uninstall` 的做法（原子写、`foreign`、`--dry-run`、`--yes`），新命令可以照这个路子。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r41`，分支 `r41`（已从 main 建好）。
- 只新建一份设计稿 `docs/调研/R41-ranch-deploy设计.md`，并在本任务文件末尾写完成记录。不改其他文件。

## 要做的

写一页设计稿，讲清楚 `ranch deploy <版本>` 怎么做，给用户定。至少要回答：

1. **它做哪些事、怎么判断要不要做**：切哪几个链接（`corral`、`ranch`、`fence`、`pony`，程序没变的不切还是一起切）、技能要不要重装、pony 要不要 kickstart、plist 变了要不要 `pony service install`；靠什么判断（比如比对程序内容、技能 dry-run 结果、`BUILD.txt`）。
2. **用户怎么确认**：先打印的计划长什么样（给一个今天 R40a2 那次的例子），怎么确认（`--yes`、`--dry-run`），不在终端里交互也能用。
3. **退回**：是不是就是 `ranch deploy <旧版本>`；怎么知道上一版是哪个（要不要记一份部署记录，记在哪）。
4. **中途失败**：做到一半失败，已做的要不要自动撤回；最后状态怎么告诉用户。
5. **谁来跑**：用 `~/.local/bin/ranch`（旧版）还是新版本目录里的 `bin/ranch`；它会换掉自己的链接，有没有问题。
6. **不做的事**：不打包、不碰正在跑的 agent、不 `corral upgrade`、不切共用 helper（发现 helper 该切时只提醒）；说明为什么。
7. **对 paddock 的影响**：是不是只多一条命令；要不要写进 `docs/CONTRACT.md`，还是只给人用。
8. **怎么拆实现**：几件活、各自多大、要不要交叉审查。

拿不准、需要用户定的地方，在设计稿末尾列成「要用户定的」，每条给选项和你的建议。

## 怎么算做完

- 用户能照设计稿回答「要不要这样做」：上面 8 条都有答案，或者写明为什么留给用户定。
- 验证只做这些：`git diff --check`；设计稿里引用的现有命令和输出，自己运行一次核对过（只读的，比如 `--help`、`--dry-run`、`integration status`、`readlink`），写明哪些核对过。觉得不够，在回复里说，不要自己加。

## 不要做

- 不写代码，不改 `docs/DESIGN.md`、`AGENTS.md`、`HANDOFF.md`，不建新命令。
- 不部署：不切任何链接，不跑不带 `--dry-run` 的 `install-skills`、`pony service install`、`launchctl`，不 `corral upgrade`。
- 不碰用户现有的 agent；不读 `~/.pony/secrets`。
- 不要按项目名或路径批量杀进程（`pkill -f ranch`、`pkill -f corral`、`pkill -f pony` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r41` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、核对了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-22，ranch/dev-r41（Claude Code，opus[1m] / xhigh）。

- **做了什么**：新建 `docs/调研/R41-ranch-deploy设计.md`，8 条都有答案：先只读检查（版本目录、`BUILD.txt` 校验和、程序能跑、链接是不是链接）→ 打印「要做的、不做的、退回」→ 确认 → 切四个链接（没变的也切）→ 技能、全局插件有变才重装（问目标版本自己的程序 dry-run）→ pony 按服务状态、plist dry-run、正在跑的真实程序决定 `service install --yes`、kickstart 还是不动 → 核对 → 记 `~/.local/share/ranch/deploys.jsonl` → 打印退回命令。退回就是 `ranch deploy <旧版本>`；中途失败不自动撤回，重跑同一条命令接着做。附了 R40a2 那次的计划样子、失败时的样子和记录样例。末尾 7 条「要用户定的」，每条给了选项和推荐。没改其他文件。
- **核对了什么**：`git diff --check` 通过。设计稿里引用的命令都在本机只读跑过：四个链接的 readlink、3 份最近的 `BUILD.txt` 和全部 37 个版本目录的 `bin/`、helper 的 readlink 和 sha256、两份 `install-skills --dry-run`、`corral integration status`（3.5 秒）和按家 `integration install --dry-run`、`pony status`、`pony service install --dry-run`（新旧两版）、正在跑的 pony 的 `ps` 和 `lsof`、各程序的 `--version`/`--help`。逐条写在设计稿第 9 节。读了打包工具、两份技能安装、integration、helper、upgrade、pony service 的代码来确认行为。
- **拿主意的地方**：输出给人看的中文文字、不出一行 JSON（只给人用，不进 CONTRACT；要机器读的放记录里）；任何带 deploy 的 ranch 都能部署任何版本，碰到不认识的程序就拒绝、让用目标版本自己的 ranch；目标版本没有的程序链接不动、deploy 从不删链接；服务没装不替人装；pony 连没连上 Slack 不算成功条件，只等新程序起来（最多 30 秒）；每次部署开始前先记一行，防中途被 Ctrl-C；部署前链接混着时，按程序逐字节相同找出等价的版本当退回目标；全局插件（amp、agy）也纳入判断（列进「要用户定的」第 6 条）；实现拆两件、都按碰要害交叉审查（第 7 条）。
- **没做的事**：没写代码，没改 DESIGN、AGENTS、HANDOFF，没部署、没切链接，没跑任何 `launchctl`（只读的 `print` 也没跑，查正在跑的 pony 改用 `ps`、`lsof`），没跑不带 `--dry-run` 的安装，没读 `~/.pony/secrets`，没碰用户的 agent。没实测的：kickstart 后多久起来（30 秒是按 launchd 10 秒的重起间隔推的）、plist 为 `replace` 时的路径（本机是 `same`）、Linux 上的行为（只按代码推断 `pony service` 报 `unsupported`），这些只能在实现和第一次真部署时验。
- **踩过的坑**：本机 zsh 里 `ls` 是带颜色的别名，输出夹着控制字符，改用 `/bin/ls`；`echo ===` 和 `--include=*.rs` 被 zsh 当成展开报错，换了写法。

## 主控审查

- 设计稿 `docs/调研/R41-ranch-deploy设计.md` 回答了「要做的」8 条：做哪些事和怎么判断、确认方式、退回、中途失败、谁来跑、不做的事、对 paddock 的影响、怎么拆。引用的现有命令都在本机只读核对过（第 9 节），没有部署、没碰 agent、没读凭证。
- 主控同意设计稿末尾 7 条「要用户定的」里的推荐，交用户定。
- 只出设计、看得见，按规矩不交叉审查。
结果：一次过
