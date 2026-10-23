# ranch 设计

## 1. 是什么

每台机器、每个用户只能有一份的运行时：数据和全局位置只能有一个主人。用户 10-05 定：把它们从 Saddle 里拉出来，独立成 ranch；Saddle（TUI）和 paddock（GPUI）都只是前端（来历见 `../paddock/docs/背景与决策记录.md` §6g–§6j）。

| 部分 | 来源 | 状态 |
| --- | --- | --- |
| corral（agent 运行时，命令 `corral`） | Saddle `crates/corral-core`，提交 `a31dea2` | 第 1 步 |
| dispatch（路由命令 `ranch dispatch route`、`corral-dispatch` 技能，不带遥测） | Saddle `plugins/dispatch` | 第 2 步 |
| 遥测、Drover、插件系统 | Saddle | 不迁；用户 10-05 定砍掉，Saddle 删除代码，数据留在磁盘上 |
| fence（sandbox，命令 `fence`，`corral start --sandbox` 调它，接 srt） | 新做 | 设计中，见 §8 |
| pony（聊天软件机器人，命令 `pony`，每台机器一份、后台常驻） | 新做（R38） | 第一期，见 §10 |

## 2. 与前端的关系

- 前端只调用 ranch 装好的命令，读 JSON 输出，不在 Cargo 里引用 ranch 的包，不读内部状态目录。各自的依赖、锁文件、工具链互不进入。
- 全局位置归 ranch：`~/.corral`、`~/.local/bin/corral`、corral 技能（`~/.claude/skills/corral`、`~/.agents/skills/corral`，R8 起还有 `~/.gemini/config/skills/corral`），以后还有 `~/.local/bin/ranch` 和 `corral-dispatch` 技能目录。Saddle、paddock 不再打包、安装这些。
- pony（R38）的位置也归 ranch：`~/.pony/`（配置、凭证、状态、日志，见 §10）、`~/.local/bin/pony`、`~/Library/LaunchAgents/local.ranch.pony.plist`（launchd 用户服务）。前端同样只通过命令用它：paddock 读 `pony status` 的 JSON（`docs/CONTRACT.md`），不读 `~/.pony`。
- Saddle 是保底版（用户 10-05）：不再加新功能。用户 10-07 基本不用 Saddle 了，定“保底规矩放松吧，Saddle 不用再写影响了”：改 ranch 时只按 AGENTS.md 的规矩写明对 paddock 的影响，不再写对 Saddle 的影响，也不再问 Saddle 跟不跟。
- 单独分发 paddock 时，打包带上 ranch 的某个版本（paddock DESIGN §7）。

## 3. 安装

- 打包工具（Rust）生成不可变版本目录 `~/.local/share/ranch/versions/<ranch 提交>/`：`bin/corral`、`bin/ranch`、`share/corral/`（随程序的资源和设计文档）、`BUILD.txt`（提交、目标平台、工作区是否干净、校验和）。目录已存在就拒绝；只生成目录，不切链接、不装技能。
- R17b 起版本目录还有 `bin/fence`、`share/fence/`（`runner.mjs`、记着打包时 node 路径和版本的 `runtime.toml`）、`lib/srt/`（固定版本的 srt）；打包机器要有 node ≥ 22.12 和 npm，没有就打包失败。corral 用同一版本目录里的 fence，前端调 PATH 上的 `fence`，所以部署时 `~/.local/bin/fence` 和别的链接一起切。
- R38 起版本目录还有 `bin/pony`。部署时 `~/.local/bin/corral`、`~/.local/bin/ranch`、`~/.local/bin/fence`、`~/.local/bin/pony` 一起切；第一次装 pony 的常驻服务（`pony service install`）仍是用户在场时的单独操作，之后由 `ranch deploy` 按需更换定义或重启。没有 `~/.pony/config.toml` 的机器上 pony 什么都不连（别的机器默认关着，见 §10）。
- 切换 `~/.local/bin/corral`、`~/.local/bin/ranch` 和安装技能（`corral install-skills`、`ranch dispatch install-skills`）是单独的部署操作，用户在场时做。已在运行的 agent 由启动它的那份 corral 管理；要换过来用 `corral upgrade`（用户 10-05 定：第 1 步切换时不做）。
- R41b 起部署用 `ranch deploy <版本> [--dry-run] [--yes]`：检查不可变版本目录的 `BUILD.txt`、程序校验和和可运行性，先打印计划，用户在普通终端确认后拿部署锁、复查链接，再一起切目标版本包含的程序链接；技能有变才重装，全局插件只重装本机已装且有变的。目标没有的程序链接、foreign 技能和插件保留；不打包、不删版本、不碰在跑的 agent、不 `corral upgrade`、不切共用 helper（只提醒）。R41c 起再按下面的规则处理 pony，最后核对。`--dry-run` 只预览；非终端且没有 `--yes` 时退出 1、什么都不写。中途失败不自动撤回，列出完成、失败、未做的步骤，修好后重跑同一条命令会跳过已完成的部分。真部署开始、结束各记一行到 `~/.local/share/ranch/deploys.jsonl`，下次计划会显示上一条记录及中断提示；每行单独按字节解析，截断在中文字符中间也只把那一行当作不完整。退回用打印出的当前 deploy 程序绝对路径运行 `ranch deploy <旧版本>`。混合链接按程序内容推算上一版，算不出来则打印逐个链接的恢复命令。只新增给人用的中文命令，不进 CONTRACT，paddock 不用动。

- pony：问目标版本的 `pony status` 和 `pony service install --dry-run`，用 pid 查真实程序（macOS `proc_pidpath`）并比 sha256。重启计划列出运行版本，不在版本目录内则列真实程序路径。服务未装或非 macOS 只切链接；服务装着但没在跑时说明 `last_exit`，父进程不是 launchd 时提醒自行重启，这两种都不重启、不替换定义。跳过 replace 时写明定义没换、给出会启动 pony 的安装命令，手动运行的须先自行停止。在跑的 launchd 服务若 plist 为 `replace`，用目标版本的 `pony service install --yes`；plist 相同且程序有变才 `launchctl kickstart -k gui/<uid>/local.ranch.pony`，程序相同不重启。服务装着而目标不支持 `service` 则拒绝，要求先用当前版本卸服务。重启后每半秒查状态，最多 30 秒，确认 `running:true`、pid 换了、真实程序与目标内容一致；Slack 连接状态照实打印，不作为成功条件。pony 的计划、核对和执行结果都入部署记录；已执行 pony 操作后失败才报告当前运行状态与日志，没在跑就把这句放在最前面，并列出 `~/.pony/logs/launchd.log`、`pony.log`。不只凭 plist 存在就声称 launchd 会重试，正常退出也不这样提示。
- 重跑未完成的同一目标部署时，沿用那次记录里的上一版和退回办法，并在计划中说明，不能把已切过的链接当成原始状态。当前链接已经等于目标版本时，退回办法取最近一次去该目标的部署记录。pony 每次都按实际运行进程判断，所以链接已切过但旧程序仍在跑时，续跑仍会列出重启。若未完成记录里 `pony_service` 失败且 `pony.before.running` 为 true，说明原本在跑的服务可能被 deploy 卸下：失败汇报提示先用目标版本 `pony service install --yes` 恢复；续跑也会列出并在确认后执行该安装，即使 plist 已是 same、pony 已停。再次失败仍保留最初在跑的记录；原本就没在跑的服务仍不启动。
- 追加部署记录前，文件非空且末字节不是换行时先补换行，保留旧半行，同时让新的 started／结束记录各自可解析。

### 在另一台机器（Linux）上装和升级（R10，10-16）

用户的远端多半是 Linux 或 VPS，ssh 能连上。macOS（aarch64）和 Linux（x86_64）都编得过、测得过；Linux 上的实测机器用 pi、omp 走过 start、钩子状态、send、reply、wait、pause/resume、经 ssh 的 attach、stop。

- **装**：在那台机器上取 ranch 的代码，`cargo run -p ranch-package` 打出版本目录（`BUILD.txt` 的目标是那台机器的平台）；用户在场时切 `~/.local/bin/corral`、`~/.local/bin/ranch` 的链接，跑 `corral install-skills`、`ranch dispatch install-skills`。和本机同一套规矩。
- **升级**：同上，再加 `corral upgrade --all`，把在跑的 agent 换到新版本。
- **代码怎么过去**：远端仓库设 `receive.denyCurrentBranch updateInstead`，本机用 `git push <host>:<目录> <分支>` 直接推过去，不经 GitHub。
- **前端不装、不升级远端**：那是用户在场时的部署操作。前端只读远端的状态（连不连得上、corral 在哪、`--version`）。
- **corral 不需要在非交互 ssh 的 PATH 里**：`~/.local/bin` 常常只在交互登录 shell 的 PATH 里。前端第一次连上时找一次（先 `command -v corral`，找不到试 `$HOME/.local/bin/corral`），看 `corral --version` 的 `contract` 是不是 `"1"`，记下绝对路径，以后直接用这个路径调。agent 程序（如装在 nvm 目录里的 pi）也不必在那个 PATH 里：corral 开 agent 时用 `$SHELL -l -i` 重建登录环境。
- **attach**：`ssh -tt -e none <host> <corral 绝对路径> attach <名字>`。`-tt` 是因为 `corral attach` 要求两头都是终端；`-e none` 关掉 ssh 的 `~` 转义，免得行首打 `~.` 断开。本机这头必须是真终端，窗口大小随 ssh 送过去；本机不是终端时远端拿到的是 0×0，agent 画不出东西。

## 4. 步骤

1. **corral**（任务：`../paddock/docs/任务/M2-ranch与corral.md`）：迁入、打包工具、切换 `~/.local/bin/corral`；Saddle 侧改成用 ranch 的 corral（需求交 Saddle 主控）。做到 Saddle 和 paddock 都能用为止（用户 10-05：“先把corral从saddle中拉出去，直到saddle能够和新的corral一块工作”）。
2. **dispatch**（任务：`../paddock/docs/任务/M3-dispatch进ranch.md`）：路由做成 `ranch dispatch route`，去掉遥测；`corral-dispatch` 技能归 ranch 安装（`ranch dispatch install-skills`），删去遥测部分、命令改成 ranch 的；Saddle 删掉内置 dispatch 插件、放掉技能归属（需求交 Saddle 主控）。

## 5. 待定

- （已定，用户 10-05）只再拆 dispatch（不带遥测），之后转回 paddock 开发；遥测、Drover、整个插件系统砍掉（Saddle 删代码），两个前端只靠 ranch 的 corral 和 dispatch。
- （已定，用户 10-15）~~插件协议的共享方式~~：不再需要，由 §7 的适配器结构取代（用户 10-14 定各家 agent 按代码级别的插件类型接入，编进 corral，不做运行时加载的插件协议）。
- （已定，用户 10-20、10-21）范围从「只再拆 dispatch」扩到聊天软件机器人 pony（R38）：放进 ranch，和 corral 一样每台机器一份、后台常驻，不开 paddock 也能用；照 §7 的思路按代码级插件式设计，分两期；见 §10。

## 6. corral 暂停（冻结）（用户 10-07）

用户在 paddock 提出：“agent能否有一个pause的功能。pause就是暂停而不是停止……不让其对外有连接或者之类的行为”，选“冻结”；用法是下班或网络不好时，活都做完后一键把全部 agent 冻住，第二天继续，不用一个个关掉重开（来历和原话见 `../paddock/docs/DESIGN.md` §13「agent 暂停（冻结）」）。任务 `docs/任务/R1-corral暂停与继续.md`。

- `corral pause NAME`／`corral resume NAME`：pen 对 agent 的进程组和另开进程组的子孙进程发 `SIGSTOP`，继续时发 `SIGCONT`。不改 agent 程序、配置和文件，对 claude、codex、pi、omp 和任何程序都一样。
- `status`／`ls` 多 `paused`、`paused_at`，原有 `state` 不变（兼容新增）。暂停中 `send`／`keys` 报 `paused`，attach 输入丢掉；`wait`、`--after` 不把暂停当作做完或安静；`stop` 先解冻再停；原地升级后保持暂停。
- 旧版 pen 不认 pause，报 `unsupported`，要先 `corral upgrade`；不在命令行这边直接发信号。
- 冻结范围（交叉审查 R1，用户 10-07 接受）：冻 agent 的子孙和它们所在会话里的所有进程，冻完逐个确认停住；冻结那一刻正好另开新会话、父进程又同时退出的进程（守护进程两次 fork）会漏掉。macOS 没有系统级冻结一组进程的手段（Linux 有 cgroup freezer），轮询进程表堵不死这个空隙；用户只在活都做完后暂停，这种情况一般不会发生。
- 对前端：满足下面的部署条件后，Saddle 不跟也能用（暂停的 agent 显示原来的状态，send 收到 `paused`），用户 10-07 定不跟；paddock 在 P5-33 接界面。
- 兼容边界（交叉审查 R6 指出）：旧版 corral 的命令行和后台提醒进程不读 `paused`，对暂停中的 agent 会误判：旧 `wait` 报 idle 或 stopped-quiet，旧 `send --after` 提前送或把等待记成 unknown（旧 send 被拒、旧 stop 正常）。新版无法替旧客户端纠正，所以靠部署次序保证不混用：只有换成新 pen 的 agent 才能暂停；先切 `~/.local/bin/corral`，再 `corral upgrade --all`（同一条命令把 pen 和持久提醒都交给新版），逐项 complete；还要确认没有旧版 corral 的进程还在跑（`ps` 查旧版本目录下 `corral` 的 `wait`、`__after` 等进程：切链接前就起的 `wait` 不会被换掉，无记录的旧提醒也不在 `upgrade --all` 能发现的范围内），有就等它们结束或让发起者用新入口重开。这些都满足之后才开始用暂停。Saddle 默认按 PATH 找 `corral`，切链接后即是新版；配置里固定了旧版本路径的不在保护之内。用户 10-07 同意先切链接、再 upgrade 的处理（“R6 我同意”），复核补上“确认没有旧版进程还在跑”后再次同意。

## 7. agent 适配器（用户 10-14）

用户 10-14（paddock P5-94「Settings 里一键适配 agent」讨论时，经 paddock/main 转达）：“各个agent是按照这插件的方式接入到ranch的（代码级别的插件类型）。”“先恢复适配器结构，全局配置可以放宽。”任务 `docs/任务/R3-corral恢复适配器结构.md`。

- 结构：`crates/corral/src/agents/`，`mod.rs` 是 `Adapter` 接口和登记表 `ADAPTERS`，每家一个文件（现在 claude、codex、pi、omp、amp、agy），沿用 Python 版 corral `src/corral/agents/` 的做法。每家写明：种类（程序的文件名）、启动时加的钩子参数、停止步骤、读哪些技能位置（R4 起；见下）、原始事件（v2，pi／omp）里的分家规则。共用的代码（pen、cli、skills、events）只查登记表，不写某一家的名字；不认识的程序不加钩子、不能带 `--prompt`、状态为 `unknown`、按默认步骤停。
- 加一家：加 `<种类>.rs`、在 `ADAPTERS` 登记一行。适配器往 agent 状态目录写文件的（如 pi 的 `hook_pi.ts`），文件名还要加进 `state::FILES`：agent 名字检查和清理都靠它，加进去后同名的 agent 名字不再合法（测试会查漏）。新事件名要用到共用状态机（如 codex 的 `Interrupt`）时，改 `events.rs`，它是 v1 事件格式的一部分。
- 技能位置（R4）：按位置登记（`skills.rs` 的 `TARGETS`：`claude` 是 `$CLAUDE_CONFIG_DIR` 或 `~/.claude`，`codex` 是几家共用的 `~/.agents`），各家适配器声明自己读哪些位置（claude 读 `claude`；codex、pi 读 `codex`；omp 两个都读）。R3 时位置挂在 claude、codex 两家上，pi、omp 报没有，不符合事实。`--target codex` 的名字沿用，没改公开用法。R8 加 `gemini`：`~/.gemini/config`，agy 只从这里读用户级技能；在项目里 agy 读 `.agents`，所以 `gemini` 没有项目位置（`--project` 带 `--target gemini` 报 `usage`，`all` 带 `--project` 时跳过它）。agy 读 `gemini`。用户 10-14 选“两份都装”：corral 和 corral-dispatch 两份技能都多装这一处。
- `corral integration status`（R4）由登记表生成，给前端（paddock Settings）列出各家：程序、版本、钩子怎么接（`launch`；amp、agy 是 `global`）、技能装没装；格式见 `docs/CONTRACT.md`。
- 共用、不归某一家的：钩子写事件（`__hook`）、`.runtime/helper`、v1／v2 事件格式和读取、pi／omp 共有的原始事件翻译（`events/raw.rs`）。
- 全局配置：到 R3 为止，钩子只随 corral 启动的那个 agent 生效（启动参数或状态目录里的文件），不写任何全局配置。用户 10-14 同意放宽，条件：经用户确认才装；钩子不在 corral 启动的 agent 里就什么都不做；只动自己那块；原子写；能卸干净。R3 不改代码，留给以后装全局钩子那件。
- 全局安装（R6，先给 amp）：不能在启动时挂钩子的 agent，适配器声明一个 `Global`（配置目录、文件、内容），`corral integration install|uninstall --agent <种类>` 经用户确认后原子写或删这一个文件，别的不动；那里已有不是 corral 写的东西（没有 `CORRAL_INTEGRATION = <n>` 版本号，或者那家配置目录及以下有符号链接；配置根目录本身是链接不算，常见于 dotfiles）就报 `foreign`、不动；用户确认后重新判断一次，变了就什么都不写，新建用硬链接、目标已有文件就失败。装上的文件只在 `CORRAL_EVENTS` 有值且 `CORRAL_KIND` 是它那家时才做事；`CORRAL_KIND`、`CORRAL_CWD` 只给这类 agent 设，现有四家的环境不变（`CORRAL_KIND` 挡住别的 agent 里跑的同种程序）。pen 还给这类 agent 设 `CORRAL_PEN_PID`，插件确认自己那个 agent 的父进程就是这个 pen，挡住 corral 管的 amp 里面直接再跑的 amp（环境全是继承的，R6 审查发现）。amp 装在 `$XDG_CONFIG_HOME/amp/plugins/corral.ts`（默认 `~/.config`），插件直接写 v1 事件，读事件和状态机不改；不挂 `tool.call`（要表态，可能绕过 amp 自己的权限）；只记顶层 thread（查不出是不是顶层时这次不记、下次再查）；amp 打开或切回一个 thread 时重新报会话开始，后台别的 thread 的事件不抢主会话；amp 不支持 `--prompt`。真实 amp 实测后：插件一加载就报会话开始（占位 id），corral 才肯 send 第一句；amp 还在加载时会跳过那一轮的 `agent.start`，插件从 thread 最后一条用户消息或这一轮结束时补记输入；插件接口在等批准时仍报 `running`，所以适配器可以声明「标题说在等用户」（`title_blocked`，amp 是 `◆ Input needed` 开头），状态为 working／idle 时报成 blocked，只对声明了的适配器生效（用户 10-14 选的做法，读屏兜底之前的最小一步）。amp 只能用 R6 及以后的 corral 操作，旧版 corral 启动的 amp 要重开，见 CONTRACT。用户 10-14 定：先 amp，然后读屏兜底，再 agy；桌面版 Amp 可能也加载这个插件，用户接受，由 paddock 在 Settings 里讲清楚。来历：`docs/调研/R5-agy与amp接入、全局安装和读屏兜底.md`、`docs/任务/R6-接入amp.md`。
- 读屏兜底（R7，给 agy 这类钩子报不全的 agent 用）：适配器可以声明读屏规则（`screen_rules`，默认没有；一条规则看屏幕最下面 n 行非空的或整屏，「都要有」「不能有」哪些字；行指屏幕行，长行折成几行就算几行）。只有声明了规则的那家，pen 才用 `vt100` 常驻一份屏幕、喂输出、跟着改大小，并在它的内部状态回复里给出屏幕文字（`screen`，兼容新增）；别家的 pen 不建屏幕、不多花开销。读屏不能拖垮 pen（R7 审查发现 vt100 0.16.2 在缩窄截断宽字符时会 panic，未闭合的 OSC 会无限占内存）：解析器 panic 就丢掉这份屏幕、退回钩子的状态，pen 和 agent 照常；OSC、DCS 等控制字符串（不画字，标题另有 `terminal.rs` 跟踪）在喂给 vt100 之前就丢掉。命令行算状态时（`status`、`wait`、`send` 的空闲检查、提醒都经过这一处），钩子报 working／idle 而屏幕命中规则，就报 `blocked`；别的状态以钩子为准。每次按当前屏幕现算，不锁存：提示框没了就回到钩子的状态。和 R6 的标题规则合在同一处（`agents::waits_on_terminal`）。`corral upgrade` 的快照不带屏幕，新 pen 从输出环（512 KB）按当前大小重放，是近似：期间改过大小或提示早已滚出输出环时会和原屏幕不同，直到 agent 重绘；新版命令行碰到没有 `screen` 的旧 pen 就不读屏。这一件只建机制，没有哪家声明规则；规则只判 `blocked`，agy 的规则和是否加 `state_source` 在接 agy 时按真实画面定（用户 10-14）。
- 接入 agy（R8，Google Antigravity 的命令行版）：agy 不能为一次运行挂钩子，走全局安装。
  - `Global` 扩成可以装一个目录：`path` 加 `files`。
    - 目录里只有 corral 的文件才算 corral 的；写到一半留下的 `.corral-*.tmp` 普通文件不算，卸载时一起删，删不掉目录就给警告。
    - 判断时记下每个文件在不在，写的时候照判断来：判断为没有的只用硬链接新建，期间冒出了文件就失败，不覆盖（R8 审查发现原先按写时有没有来定，会覆盖竞争中出现的别人的文件，amp 也是）。
    - 每个文件各自原子写；按次序写，让 agent 认出插件的 `plugin.json` 最后写，卸的时候它先删，空了再删目录。
    - amp 照旧一个文件。
  - 插件是 `~/.gemini/config/plugins/corral/`，里面只有 `plugin.json` 和 `hooks.json`。
    - 只挂 `SessionStart`、`PreInvocation`、`PostToolUse`、`Stop` 四个只观察的事件。
    - 不挂 `PreToolUse`：它要表态，出错会拦下工具。
  - 每条钩子是一行 shell：`CORRAL_KIND` 是 agy、`$CORRAL_HELPER` 可执行，才 exec `"$CORRAL_HELPER" __hook agy:<事件>`；否则 `echo {}`。agy 要求每个钩子都输出 JSON。
  - `CORRAL_HELPER` 由 pen 给 agy 设（R8 审查发现）：命名空间的 `.runtime/helper` 只在 `corral upgrade` 时切换，部署新版后、升级之前它还指着不认识 `agy:` 入口的旧版，新开的 agy 会一直 `starting`。所以共享 helper 正好是启动它的这份 corral 时用 helper（之后随 upgrade 走，和别家一样），否则直接用这份 corral 程序本身（这个 agy 就一直用它，清理旧版本时要算上）。不替用户切共享 helper。
  - `__hook <种类>:<事件>` 是全局安装的钩子共用的入口：
    - 查 `CORRAL_EVENTS`、`CORRAL_KIND`，以及钩子的父进程（agy）的父进程是不是 `CORRAL_PEN_PID`；
    - 再交给适配器的 `hook` 翻译成 v1 事件；
    - 记不记都输出 `{}`。
  - 翻译：
    - `SessionStart` → 会话开始。
    - `PreInvocation` 里 `invocationNum` 为 0（一轮的第一次调模型）→ `UserPromptSubmit`，提示词取 transcript 里最后一条用户输入的 `<USER_REQUEST>` 正文；取不到就不写提示词。
    - `PostToolUse` → 工具。
    - `Stop` 按 `terminationReason`：用户取消 → `Interrupt`；出错 → `StopFailure`；别的 → `Stop`，回复取 transcript 里最后一条用户输入之后、最后一段非空的模型文字。
    - `session_id` 用 `conversationId`。
  - 和 amp 一样直接出 v1 事件，状态机不改。任务里原写走 pi／omp 的 v2 原始事件，实现时改成这样，更简单。
  - 真实 agy 摸底（10-14，用户在场，真实 HOME，用临时探针插件，用完删掉）：
    - 钩子继承 agy 的环境；钩子的父进程是 agy，祖父是 pen。
    - 钩子同步执行，会挡住 agy，默认超时 30 秒（corral 设 10 秒）。
    - `SessionStart` 只在第一句话送进去时才触发，所以 agy 必须带 `--prompt`，不带就一直 `starting`。corral 用一个参数 `--prompt-interactive=<首句>` 传进去，首句以 `-` 开头也不会被当成选项。
    - agy 不设终端标题。批准框最下面一行是 `↑/↓ Navigate · tab Amend · ctrl+g edit/expand command`；信任目录、登录对话框是 `Navigate · enter Confirm`。所以读屏规则是：最下面 4 行非空的里同时有 `↑/↓ Navigate` 和 `tab Amend` 两个按键提示（R8 审查：只要两个词的话，回复正文提到它们就会误判）。这是 R7 机制第一次用上。不加 `state_source`（用户 10-14）。
    - HUP 一次就停干净，连同它起的 MCP 子进程。停止步骤：HUP 等 3 秒，再 TERM 等 3 秒。
    - 登录令牌在系统钥匙串里（go-keyring），临时 HOME 隔不开，用户改选在真实 HOME 实测。
    - `agy plugin validate` 认 corral 的插件目录。
  - `integration status` 的 agy 一项多报 `install.enabled`（用户 10-14 同意）：只读 agy 的 `config.json` 里 `plugins.corral.enabled`，不写。
  - 派遣：点名才派，用默认模式、不分档；派出去干活带 `--dangerously-skip-permissions`（用户 10-14）。
  - 钩子的开销（R8 审查）：环境不全（`CORRAL_EVENTS`、`CORRAL_KIND`、`CORRAL_PEN_PID`、`CORRAL_INSTANCE`、`CORRAL_CWD` 缺一个）时什么都不读；钩子输入最多读 16 MiB；transcript 逐行读，只解析用户输入和模型文字这两种行，只留这一轮要的。128 MiB 的 transcript 一次钩子约 30 ms、峰值内存约 3 MB（原先整份解析，审查测到约 346 MiB）。
  - 已知限制：
    - agy 有子 agent（`invoke_subagent`、浏览器子 agent），它们的会话会不会触发 `SessionStart` 钩子没有核实。如果会，子会话会被当成主会话（和 Claude Code 的 `/clear` 一样，最近一次会话开始成为主会话），之后主会话的事件不再算，状态可能偏。没有改成「只认第一个会话」，因为 agy 有 `/clear`（在同一进程里开新对话），那样会跟丢。遇到再按真实情况改。
    - 钩子的 shell 里 `exec` 失败（helper 可执行但跑不起来）时不会再走 `echo {}`，agy 拿不到 JSON。helper 是 corral 自己的程序或指向它的链接（原子替换），正常不会出现；为它改掉 `exec` 会多一层 shell，要连父进程检查一起改，这次不做。
  - 来历：`docs/任务/R8-接入agy.md`。
- agy 派活修整（R13，agy 1.4.0，用户 10-17）：
  - 抢跑：一次真实派活里 agy 登录慢（11 秒，平时 5–6 秒），`--prompt-interactive` 的首句没等登录就送出，那一轮没挂上钩子和技能，`SessionStart` 丢了；钩子 6 分钟后才恢复，事件全积在 `pending`，状态一直 `starting`。10 次实测没复现，首句的送法不改。
  - 状态兜底：适配器可以声明「认领会话」（`adopts_session`，只有 agy）。命令行读事件时，还没有主会话、`pending` 里有带 transcript、目录相同的会话，就把最早那条事件的会话当主会话，按时间补算它积压的事件（不先置为 idle）。之后照旧：再来的 `SessionStart` 仍按原规则换主会话。认领发生在读事件时，所以已经积压了事件的 agy，换上新版命令行就能恢复。别家不变。
    - 带首句启动的 agent，「首句还没处理就算 `starting`」的保护（`status`）对认领会话的适配器只在最后一个事件是 `SessionStart` 时生效：首句的钩子可能和会话开始一起丢了，之后来了工具、结束等事件，说明这一轮已经在走（R13 审查发现）。
  - 信任目录：agy 只信任列表里的目录本身，不认上级目录；派活每件活一个新 worktree，每次都要人点「信任」。Claude Code 往上找上级目录，Codex 归到所属主仓库，都不弹。适配器可以声明信任列表（`Trust`：配置文件、里面的字符串数组字段；agy 是 `<HOME>/.gemini/antigravity-cli/settings.json` 的 `trustedWorkspaces`），pen 替它补上「上级目录信任过就算信任」：
    - HOME 用 agent 拿到的那个（`--env HOME=` 也算）。记录和锁放在 `<HOME>/.corral/.runtime/trust.json`、`trust.lock`，跟着配置文件走，不跟 `CORRAL_HOME`：几份 `CORRAL_HOME` 共用一份 agy 配置时也共用一份记录（R13 审查发现）。
    - pen 启动 agent 之前：工作目录（规范化后）不在列表里、某个上级目录在列表里，就加进去；上级目录都不在列表里就不加，照旧弹框。记录先写成「待确认」，列表改好后再确认，没加就删掉这条记录。中途断了留下的「待确认」记录，之后清掉时只删记录、不动列表：corral 不删拿不准是不是自己加的条目，代价是最多留下一行没人认的路径（R13 复核发现：原先断在中间，会把用户本来就信任的目录当成自己加的删掉）。记录里有配置文件、字段、路径、`CORRAL_HOME`、agent 名字、实例、pen 的进程号。agent 起不来时立刻删掉。
    - 已经有 corral 别的活着的实例为同一路径记过：只加一条记录，共用；都退出了才从列表删。
    - agent 退出后（pen 的主循环结束）：列表先删、记录后删；列表删失败，或文件还在却读不了、变成了符号链接、结构认不出，记录都留着，下次再清（R13 复核发现：原先当成删完了，那条就永远没人清）。文件不在了算删完。只删一份同名条目。
    - 兜底：每次加之前，清掉 agent 已经不在、或目录已不存在的记录。判断 agent 在不在：它状态目录的 `meta.json` 实例对得上、`agent_pid` 还在；`meta.json` 还没写（pen 正在启动）时看 pen 的进程号还在不在（R13 审查发现：原先把正在启动的当成残留清掉了）。进程号被复用时，残留会多留一阵，到目录删掉为止。
    - 不写的情况：文件不存在、是符号链接（dotfiles 常见，替换会断开链接）、不是 JSON 对象、字段存在却不是字符串数组。
    - 写法：同目录临时文件写好、保留原权限；改名前再看一次，还得是同一个普通文件（设备号、inode 不变，没换成符号链接）、内容和最初读到的一样，否则整个重来（最多 5 次）（R13 复核发现：只比内容时，被换成同内容的符号链接会被覆盖）。agy 和用户编辑器不认 corral 的锁，这一步只能把丢别人改动的窗口缩到「再看」和「改名」之间；用户 10-17 接受这个窗口。对象键会按字母重排（agy 自己写回时用它的顺序），值不变。
    - 等锁最多 5 秒，等不到就放弃这次（agy 照旧弹框）。出错只记进 pen.log，不影响启动。
    - 已知限制（用户 10-17 接受）：corral 记着的目录，在这条记录删掉之前用户自己取消再重新信任，agent 正常退出或之后清残留时照样会删掉它（列表只存路径，分不出是谁加的；R13 复核发现不只是 pen 被强杀时）；后果是那个目录下次再弹一次信任框。
    - 用户 10-17 同意 corral 为它开的 agy 改写 agy 的信任列表；用户自己开的 agy 不受影响。
  - corral 技能里开 agy 改成 `-- agy --dangerously-skip-permissions`，和派活技能一致；Claude Code 不改（用户 10-17）。
  - 来历：`docs/任务/R13-agy启动抢跑与免确认.md`。
- 被人打断的一轮（R22，用户 10-19）：人在被委派 agent 的窗口里按 Esc，原先和正常结束一样置为 `idle`，提醒照送、`reply` 给上一轮的回复，主控误以为做完。现在共用状态机记下 `interrupted`（状态仍是 `idle`，不加状态值，paddock 不用动；新输入清掉；pi 关扩展对话框的空闲恢复不清）；`wait` 返回 `interrupted`，`reply` 报 `no_reply` 带 `interrupted: true`，`send --after` 只在「idle 且被打断」时跳过、`blocked` 照送、超时照送。只认明确事件，不按输出、静默时间或读屏猜：Codex、amp（连按两次 Esc）、pi（采集器补记 `agent_settled.aborted`）已真人实测；omp 当时没找到标记（R25 补上）、agy 和 Claude Code（R24 补上）没有打断事件，保留限制（用户在开发 agent 窗口里同意）。内部 cursor 缓存 4 → 5，新旧版本交替读时各自重放。来历：`docs/任务/R22-人打断被委派的agent后主控误以为做完.md`。
- Claude Code 被人打断（R24，用户 10-19：“这么长时间肯定不行啊”）：Claude Code 被 Esc 打断时不触发任何钩子，只在自己的对话记录里写 `[Request interrupted by user]` 或 `[Request interrupted by user for tool use]`。钩子记下 `transcript_path`；算状态时，这一轮还没收到明确结束事件（内部 `turn_open`），或已认出的打断被晚到的工具钩子拉回 `working`，才读对话记录末尾最多 64 KiB，只认本主会话、非侧链、时间严格晚于最近一次输入的那条标记，认出来按 `Interrupt` 处理。对话记录是持久证据：缓存被别的 reader 冲掉或新旧版本交替重建，也能重新认出同一轮；正常 Stop/StopFailure、会话结束关上这一轮，不会被旧标记改判。非阻塞打开、对打开的 FD 查普通文件。生效要该命名空间的 `.runtime/helper` 指向新版（`corral upgrade` 才切），单纯重开 Claude Code 不够。来历：`docs/任务/R24-ClaudeCode被打断后马上认出来.md`。
- Claude Code 框上打断和题目（R43，用户 10-22）：R24 的读取门槛扩到 `blocked`，包括权限框、AskUserQuestion、ExitPlanMode 被 Esc，以及批准后仍残留 blocked 的运行中打断；仍只认同一轮的明确对话标记，框开着不猜。仅 AskUserQuestion 的 PermissionRequest 钩子保留题目白名单，`status.questions` 只在该提问框 blocked 时给出题干、选项说明和多选标志，序列化总量最多 16 KiB，截断显式标记；不记其他工具参数。事件保持 v1，cursor 升版后新旧 reader 各自重放；旧钩子没有题目时留空。公开字段是兼容新增，paddock 可以不接，打断状态修正无需适配。来历：`docs/任务/R43-corral认出框上被Esc和记下提问题目.md`。
- omp 被人打断（R25，用户 10-19）：omp 在生成回复或调用工具时按 Esc 会中止整轮，最后一条 assistant 消息带 `stopReason: "aborted"`、`errorMessage: "Interrupted by user"` 和 `errorId` 的 UserInterrupt 位（67108864）。reader 在最终 `agent_end`（不是 `willContinue`）的最后一条 assistant 三个标记齐全时按 `Interrupt` 处理，其余中止、出错、重试照旧；跑命令时 Esc 只停命令、模型接着正常结束的不算打断。采集器早已原样记下 `messages`，只改 reader，omp 不用重开，helper 不用切。内部 cursor 缓存 7 → 8。来历：`docs/任务/R25-omp被打断后主控认得出.md`。
- agy 被人打断（R26 摸底，用户 10-19）：agy 1.4.2 的钩子说明写着用户取消时 `Stop` 带 `USER_CANCELED`（corral 早已转成 `Interrupt`），但写字时、命令在跑时按 Esc 实测都不发 `Stop`，对话记录没有取消标记，只有 agy 自己的诊断日志记一句 `Cancelling conversation <会话号>`。日志不是约定格式、会轮转，没接；agy 被打断仍认不出，`interrupted: false` 不保证没被打断。来历：`docs/调研/R26-agy被打断时报什么.md`。

## 8. sandbox（用户 10-18）

用户 10-18：“先接 srt（但是要支持linux）。 另外默认委派出去的不要家srt围栏（默认保持现在）。还有架构设计上我希望还是类似插件式的（以后可以很方便的把sandbox剥离出来成为像cairn这样独立的服务）。”

- 围栏式：agent 照常在本机跑，由 srt（Anthropic sandbox-runtime）按规则管读、写、上网，一律默认不许、按需放行。不做容器式、云端。
- 支持 macOS 和 Linux。
- 是 `corral start` 的一个选项，不加就是现在的样子；派出去的 agent 默认不加。
  - R18（用户 10-19：“可以通过paddock选中的状态改为sandbox跑”）：给正在跑的 agent（一般是主控）设一个「派出去的套哪个档案」，`corral child-sandbox NAME [<档案> | --clear]`，随时可改，从它下一次 `corral start` 起生效；默认没有设定，保持上面的默认。设定是 corral 自己的状态（agent 状态目录的 `child-sandbox.json`，带 instance，只认这一次运行），corral 不读任何前端的东西。`start` 凭调用者的 `CORRAL_NAME`、`CORRAL_INSTANCE` 读当下设定；显式 `--sandbox`／`--no-sandbox` 优先。设了就要么生效、要么报错：确认调用者是否同一次运行只看本地运行锁和 `meta.json`，不问 pen（交叉审查发现问 pen 时 pen 卡住会悄悄不套墙）；确认不了报 `child_sandbox_unreadable`。fence 的插件边界不变。
- 插件式：sandbox 叫 fence（用户 10-18：“同意B，名字就叫fence”），放在 ranch 仓库里单独一块（`crates/fence`），出自己的命令 `fence`；corral 不在 Cargo 里依赖它，只按一条窄约定调它，以后能整块剥离成独立服务。
- 定下的边界（R17b 方案，用户 10-18：“五条都同意”；全文 `docs/调研/R17b-fence方案.md`）：
  - corral 只调两条命令：起 pen 前 `fence check`，pen 里用 `fence run … -- <原命令>` 代替直接起 agent。fence 用同一版本目录里的那份（可用 `CORRAL_FENCE` 改），剥离时只改这一处。出错一律启动失败，不悄悄不套墙。
  - fence 用 Node 小脚本 `runner.mjs` 调 srt 的库接口起 agent（和 `hook_pi.ts` 同类，是跑在 srt 运行时里的代码，不算工具），被拦记录单独写进 `~/.fence/runs/<实例>/`，由 `fence denials` 读。agent 的临时目录 `${tmp}` 在 `${XDG_CACHE_HOME:-~/.cache}/fence/tmp/<实例>`（R17b 实现时主控定：srt 的禁止写压过放行写，放在整个禁止写的 `~/.fence` 里就写不进；方案原写 `runs/<实例>/tmp`）。规则文件和数据里的禁止写连同符号链接途中每个节点、最终目标一起保护（R17b 交叉审查）。
  - 规则是 TOML：内置档案（只有 `dev`）、各家数据、模型服务编进 fence；用户在 `~/.fence/` 下放同名文件整份替换，`~/.fence/local.toml` 补本机的；仓库补充是工作区顶层的 `.fence.toml`，只许额外放行，墙里写不了。路径用 `${…}` 占位符、不许通配符，macOS 专有项写在 `[macos]` 段。Codex 默认开 trustd（削弱网络隔离，用户同意）。
  - 套了墙的 agent 在墙里不能再开 agent：PATH 上的 `~/.local/bin/corral` 在 HOME 禁读里，通常报 127；corral 程序读得到时报 `sandbox`（退出码 6）。不为凑出 6 放开 HOME（R19，用户 10-19 同意）。
  - R20（用户 10-19，paddock 的测试要在临时目录建 socket）：`dev` 默认放行各实例 `${tmp}` 下的 unix socket（bind、connect），macOS 用 srt 的 `allowUnixSockets` 按路径放行；Linux 的 srt 只能全开或全关，不放开，`check` 给 warning，留 R17c。srt 会把墙内 `TMPDIR` 改成 `CLAUDE_CODE_TMPDIR`，没有就用各墙共用的 `/tmp/claude`；`dev` 给四家都设 `CLAUDE_CODE_TMPDIR=${tmp}`，墙内 `TMPDIR` 都是自己的临时目录。
  - R29（2026-10-20）：四家查资料统一连接免 key、有限频的 `https://mcp.exa.ai/mcp`，使用 `web_search_exa` 和 `web_fetch_exa`，由 Exa 服务器读取目标网页。Claude Code 用 `--mcp-config`，Codex 用 `-c mcp_servers.fence_exa_${run_nonce}`；pi 用一次性扩展调用内置 `registerMcpServer`；omp 的内置 Exa 搜索不足以覆盖读网页，因此用 `--plugin-dir` 临时 MCP 插件。接入参数仍由种类数据提供；所需资源编进 fence，在实例 `${tmp}/fence-exa` 写出并随临时目录清理，不修改各家全局配置、不做 fence 上网网关。
    - Codex 会递归合并同名 MCP 表，不能用固定名称改 URL，否则可能将原有认证头带往 Exa。`${run_nonce}` 在每次 `run` 时由系统随机源生成 128 位随机值（32 位十六进制），`check` 只显示 `<运行时才有>`；fence 不读取 Codex 配置查重。工具名称因此为 `mcp__fence_exa_<随机值>__web_search_exa`／`web_fetch_exa`，按当次工具目录发现，不硬编码完整名称。Claude 的会话配置整项覆盖、pi 的配置同名项优先、omp 的插件名称空间均未发现这种认证字段递归继承，三家接法和工具名保持不变。
    - `dev` 只新增 `mcp.exa.ai`、`github.com`、`api.github.com`、`raw.githubusercontent.com`、`codeload.github.com`、`objects.githubusercontent.com`、`docs.rs`、`doc.rust-lang.org`、`static.rust-lang.org`。GitHub 和 Rust 文档供开发直接读取；其他站交 Exa 抓取，避免放开任意互联网访问。
    - pi 为免认证 Exa 显式提供空 `Authorization` 头，关闭 pi 默认的 MCP OAuth 凭证加载；否则即使服务不要求登录，pi 也会尝试创建墙内禁止写的 `~/.pi/agent/mcp-auth.json`。不提供 token、不放开该路径。
    - omp 的 MCP URL 显式带 `?tools=web_search_exa,web_fetch_exa`，以保留内置搜索未覆盖的读网页工具；不带工具列表的 Exa MCP 配置会被 omp 自动过滤。仍只访问 `mcp.exa.ai`，不引入其他服务。
    - 推送不挡、不做推送实测。第一轮撤销钥匙串读取使 Claude 无法登录；用户随后明确取消推送限制，认为平时直接使用 Claude Code、Codex 也有同类风险，且常用 bypass。各家读写路径（包括 Claude 的钥匙串读取）保持原样，不新增凭证处理；是否允许推送由任务书约束。
    - 对 paddock：兼容新增规则和种类启动参数，命令、JSON 格式、退出码不变，paddock 不用动；随 fence 部署对新运行生效。同名用户档案和种类仍整份替换内置数据，不自动改用户规则。
  - R30（2026-10-20）：查资料接哪些 MCP 服务器改成数据，替代 R29 写死的资源；fence 的 Rust 程序不识别具体服务商。
    - `data/mcp/<名字>.toml` 与模型服务同样编入程序；`~/.fence/mcp/<名字>.toml` 同名整份替换。内容为 HTTP MCP 的 `url` 和可选的 `[auth]`（`header`、`prefix`），不含 key。服务器名字仅用 ASCII 字母、数字、下划线、连字符。`dev` 的 `mcp = ["exa"]` 默认仍接 Exa；本机 `local.toml` 顶层 `mcp = ["别家"]` 整份替换选择，空列表关闭。换服务商只加一份服务器数据、改这一行，不必覆盖档案或四家种类。
    - 选中的服务器从 URL 自动提取放行的域名（显式非默认端口一起保留）；不再在 `dev.domains` 重复 Exa。内置 Exa URL 带 `tools=web_search_exa,web_fetch_exa`，四家共用这一地址，保留 omp 所需的显式工具列表；不再提供各客户端用不上的其他 Exa 工具。
    - 每家种类的 `[mcp]` 写每台服务器的 `args` 和 `[mcp.files]` 文件内容模板；资源放 `${tmp}/mcp/<名字>/`。模板使用服务器名、URL、JSON/TOML 请求头及临时目录占位符，参数仍按数组传递，不经 shell。Codex 保留每次运行随机后缀，pi 模板保留空 Authorization 默认值再叠加该服务器认证，omp 模板写插件 manifest 和 MCP 配置。只为当前种类、选中的服务器生成资源。
    - key 是墙外 `$FENCE_HOME/keys/<服务器名字>` 的单行文件，建议目录 0700、文件 0600；fence 强制禁止墙内读取 keys 目录及解析符号链接得到的真实路径，并沿用整个 FENCE_HOME 的禁止写。只有 `run` 在墙外读取选中服务器的 key，按该服务器 `auth.header` 和 `auth.prefix` 组成请求头，不注入其他服务器或全局认证。没有 key 文件即免 key；有 key 却缺认证格式、空 key 或非法请求头则启动失败，不回退成免认证。`check` 不读取 key，认证内容只在运行时生成，不进检查输出或运行记录。
    - R30 返工：Claude 的 `--mcp-config` 只传 0600 配置文件路径；Codex 模板声明 `headers_via_env = true`，用 `env_http_headers` 引用本次、该服务器专用的随机环境变量名。真实值只通过 `Command::env` 传给子进程，不进入可序列化的规则环境、启动参数或运行记录；Codex server 名仍带随机后缀。其他两家仍写 0600 临时文件。
    - 凭证只支持普通 ASCII HTTP 请求头字面值，组合后的值拒绝任何 `$`，也拒绝以 `!` 开头或含非法控制字符的值，防止客户端再次插值或求值。生成的临时文件和专用环境会进墙，agent 能看到，也能主动复制发送；这不是保护 key 不被 agent 访问的网关。客户端连接和重定向行为仍由其原生 MCP 实现负责，fence 只给对应 server 配置认证。
    - key 源禁读独立于服务器选择：每次枚举 `keys/` 全部条目，将原路径和符号链接真实目标加入禁读；改选或 `mcp = []` 不解除旧 key 的保护。仍只读取被选中服务器的 key 内容，`check` 只看路径元数据。
    - 准备阶段失败立即清理本次新建的实例记录和临时目录，启动前已存在的目录、文件或链接不删除；清理失败附带错误，不能声称已清掉。成功交给 runner 且留下 `run.json`、`exit.json` 的运行，仍在以后启动时按结束超过七天回收；异常退出缺少完整记录的残留不保证自动回收。
    - 对 paddock：命令、退出码及现有 JSON 字段保留；`profiles` 兼容新增 `mcp` 服务器列表、`check/run` 的来源列表新增 `mcp`，paddock 不用动。只在 fence 新运行生效，不修改用户全局配置；用户整份覆盖的旧种类需自行采用 `[mcp]` 模板才能切换。
  - `packed-refs.lock` 报错不消除：提交不受影响，删除已打包的分支、`gc`、建新分支或 worktree 在墙里会失败。
- 来历：`docs/任务/R17-agent放进sandbox.md`、`docs/调研/R17a-srt小试验.md`、`docs/调研/R17b-fence方案.md`。

## 9. 派活分工数据（R31，2026-10-20）

R31 先把分工搬进数据表；R34（2026-10-21）改为按档位和类别选格子，再通过命令读取当下生效的分工。路由的判法、验证预算、任务书写法和交叉审查的判定标准不变。

- `crates/dispatch/resources/dispatch.toml` 编进 ranch。`agents` 按名字定义各家的 `argv`、`labels` 字符串数组，模板只支持 `${model}`、`${effort}`；`cells` 按名字定义 `agents`（可任选的家）、可选的 `model`、`effort`、`cross_review`、`rework`。名字和数量由数据决定，Rust 不识别具体家或格子。
- 每次调用读取 `$HOME/.ranch/dispatch.toml`，同名家、同名格子按字段覆盖；数组整项替换，未写的字段和条目继承默认，也可加新家、新格子。可选字符串写 `""` 表示清除，不能删除内置条目。不读项目文件，读表命令不写用户配置，不缓存。
- 文件不存在才用默认；读不了、格式错误、未知键、引用不存在、交叉审查或返工引用构成环（含混合环）、模板无法展开都报错，给出文件与字段位置，不退回默认。读取一格也先校验整张表。
- `ranch dispatch cell <名字>` 返回这一格的家、模型、强度、完整启动命令（shell 字符串和 argv）、标签，以及交叉审查格子的命令和标签、返工格子名。`ranch dispatch table` 返回全部生效的家模板与格子，并标记定义来源 `builtin` / `user`；格子引用的家另标来源，避免只改家时误以为启动命令仍全来自默认。
- 默认分工是四档 × 两类：`low-design` / `low-other`（低）、`middle-design` / `middle-other`（中）、`high-design` / `high-other`（高）、`critical-design` / `critical-other`（碰要害）。类别分别为「界面和设计」「其余」。低档均给 pi 或 omp，各自默认模型；中档界面和设计给 Claude Code `opus[1m]` high，其余给 Codex `gpt-6-astra` high；高、碰要害用对应家的 xhigh。
- 档位按第一条命中判定：改代码且碰要害 → 碰要害；本轮只出方案或做取舍 → 高；路由重或跨好几个文件、原因不明的 bug → 高；路由轻且不碰要害、改动点可列出、无待定设计 → 低；项目代码之外的杂活 → 低；其余 → 中。查事实、定位问题的调研同样判档；涉及界面或产品取舍的调研按只出设计判高。界面、设计及涉及界面或产品取舍的调研属于「界面和设计」，其余属于「其余」。
- 只有碰要害两格配置交叉审查：`critical-design` 指向 `review-codex`（astra xhigh），`critical-other` 指向 `review-claude`（opus xhigh），按实际开发者换家审。低档的 `rework` 指向同类别中档，中档指向同类别高档，高和碰要害没有升档格子；两格审查入口及 `amp`、`agy` 点名入口均无后续关系。
- 技能仍先遵从用户和项目指定，未指定部分通过读表补齐；主控只在用户明说或任务文件写了 `执行：主控` 时自己做。每档最多交回原 agent 返工一次，再不过就读升档格子，在原 worktree、原分支新开 agent 接着提交做；原 agent 的完成记录先留在任务文件，再关闭，不用问用户。没有升档格子的返工一次再不过，停下问用户。结果行记录一次过、返工次数、升到的档位或最高档返工仍不达标；用户要时主控按档位和家汇总，用户复盘，主控不自行调规则。
- Claude Code、Codex 只用 high、xhigh；Claude Code 不用 `max` 强度和 `fable` 模型，Codex 不用 `ultra`，用户点名也先说明、不开。取消旧试用和定额汇总。读表失败或所需 `choices` 为空就停下告诉用户、不派。amp、agy 仍只在点名时派；amp 不带首句参数、agy 必须带首句参数，以及 `CAIRN_DISABLE=1` 保持原规则。数据只提供信息，不执行 agent 命令、不自动返工或审查。
- 对 paddock：R34 的分工格子名全换，可选强度从三项减为 high、xhigh 两项；命令、JSON 结构、标签、退出码和状态目录不变。设置页按 `table` 返回的格子名和选项画，不硬编码旧名字；ranch 与技能须一起更新。本机旧格覆盖不会自动迁移，旧强度覆盖可能不再通过校验，部署时需核对本机表；本轮不改用户配置。之后改本机表即在下一次读取生效，不编译、不重装技能。

### 设置页编辑分工表（R35，2026-10-20）

- 新增 `ranch dispatch edit <名字>`，用参数局部修改一格；`reset <名字>` 删除本机对这一格的覆盖，回到内置默认。只改已存在的格子，不编辑家模板。`agents` 可以清成空数组；模型、强度、审查和返工可以清成空字符串。模型或强度为空时，不传对应的模板参数及标签，让各家使用自己的默认。
- 每格新增中文 `description`；每家新增 `efforts`（允许的强度）和 `models`（建议模型），都放在数据中，可由本机文件覆盖。R35 当时不改分工，现行默认按上面的 R34。非空强度必须被所选各家接受；模型不受建议列表限制。
- `table.local.version` 是本机文件原始内容的 SHA-256 标识，不存在时为 `missing`。编辑和恢复可带 `--if-version`；开始读取时及原子替换前比对，发现变化报独立的 `conflict`。这是写前检查，不是锁或事务，最后一次检查和替换之间仍有窗口；不带版本则照常写。
- 写入先按读表的同一套逻辑校验整个候选表；用保留格式的 TOML 编辑只动目标格，其他内容、顺序和注释保留。同目录临时文件写完后原子替换，不留下半份目标文件。目录和文件缺失时新建。失败不改原文件。
- 格式保留边界（主控 R35 实现中定）：继续用 `toml_edit`，不自写源码替换器。修改前先将原文解析再输出，必须逐字节相同，否则报 `format_not_preserved`、原文件不动。例如交错的点号键会被解析器重排，需用户手改或改成普通 `[cells.NAME]` 写法；命令新建格子用普通表写法。读表仍可读这些合法 TOML。
- 对 paddock：旧调用不用改；新设置页用 `table` 取得说明、关系、下拉选项和版本，用 `edit` / `reset` 保存，成功回复整张新表，冲突后重新读取。须部署支持 R35 的 ranch；不改技能、不改 route，不需重装技能。旧版读表器不认识新增数据字段，使用本机说明或选项覆盖前应让读写方都换到新版。

## 10. 聊天软件机器人 pony（R38，2026-10-21）

用户 10-19 提：「再加一个draft task，可以接入slack等社交软件，通过社交软件AT的方式进行交互开发」；10-20 定放进 ranch（「接受你的建议，甲」），和 corral 一样每台机器一份、后台常驻，不开 paddock 也能用；按 ranch 接 agent 的插件式设计（「我觉得应该像ranch接不同agent那样的设计，类似插件的形式」），分两期（「同意，分两期」）；底线「不要动ranch中corral的核心代码」。10-21 定名字 pony（Pony Express，送信的马）。设计稿 `docs/调研/R38a-Slack机器人设计.md`，用户 10-21 定稿；这一节写定论和理由，Slack 的接法、权限、实测项和其余细节以设计稿为准。需求和验收 1–7 见 `docs/任务/R38-从Slack等聊天软件at着agent开发.md`；第 7 条是 paddock 的 Settings，数据照 `docs/CONTRACT.md` 的 `pony status`。

### 范围

- 第一期（R38c–R38f）：在绑了仓库的频道里 @ 机器人说话，送给已经在跑的 agent，这一轮结束把回复贴回同一线程，在线程里接着回就是接着聊；主控（`<仓库>/main`）每一轮的新回复、绑了频道的仓库里任何 agent 的 Needs you（`blocked`）在对应频道提醒，在提醒的线程里回就是送给它；`@pony ?` 看绑了频道的仓库里哪些 agent 在干活、在等、暂停、空闲；长回复分段贴；给 paddock 的只读命令 `pony status`。
- 第二期另开任务：在聊天里开、停 agent。
- 第一期不做：私聊、斜杠命令、按钮、附件内容、内容过滤、多用户、多机器、改了消息跟着改、判断用户在不在电脑前（用户 10-21 定）。

### 结构：单独一块，和 corral 只走命令

- 单独的包 `crates/pony`、命令 `pony`，打进版本目录的 `bin/pony`，部署时切 `~/.local/bin/pony`。
- 和 corral 只通过公开命令来往：`corral ls`、`status`（含 R43 的只读 `questions`）、`reply`、`send`，读它们的 JSON；R44 加 `read`（纯文字，只拿屏幕末尾给人看），R45 加 `keys`（只按 Esc，amp 两个 Esc）。Cargo 里不依赖 `corral-core`、`ranch-dispatch`、`fence`，不读 `~/.corral`。R44、R45 不改 corral 的代码，只消费现有公开命令；题目采集由用户另行同意的 R43 提供。
- 找 corral：配置里的 `corral`（绝对路径），没有就先 PATH，再 `~/.local/bin/corral`（launchd 起的程序 PATH 只有系统目录，会落到后一个）；起来时看 `corral --version` 的 `contract` 是不是 `"1"`。
- 照 §8 fence 的剥离办法：以后整块拆出去时把 `crates/pony` 挪到自己的仓库，自己打包、部署；corral 一侧本来就没有关于它的代码，不用改。

### 插件式

- `crates/pony/src/`：`main.rs`、`corral.rs`（调 corral 命令、解析 JSON）、`core/`（共用核心）、`chats/`（每家聊天软件一个文件）。`chats/mod.rs` 是 `Chat` 接口和登记表 `CHATS`，现在只有 `slack`。
- 接口管：`kind`（配置里的段名、凭证文件名、`pony status` 里的值）、凭证的键、读这家的配置、连接和重连、把收到的消息翻成共用格式（频道、消息 id、所在线程、发话人、有没有 @、去掉 @ 和转义后的正文、附件、删除）、上限（一条多少字、有没有线程）、发消息、频道信息。没有线程的平台退成「每句都要 @、写明 agent，回复以 `<全名>:` 开头直接发在频道里」；第一期用不到，只在接口里留着。
- 共用核心管配置、凭证、权限、路由、线程表、送话和排队、盯 agent、提醒和去重、分段、状态、日志、`pony status`。核心源码不写任何一家的名字。
- 接第二家（比如飞书）只加 `chats/<名字>.rs` 和 `CHATS` 一行，核心不改。核心测试用只登记在测试里的内存假适配器；另有一条测试断言核心源码里不出现已登记适配器的 `kind`。

### 用户 10-21 定的

1. 主控新回复和提醒每一轮都发到 Slack；提醒和每一轮回复的第一段 @ 用户，pony 不判断用户在不在电脑前，手机吵不吵靠 Slack 自己的通知设置。用户原话：「先这么定，如果以后嫌吵，咱们再改。」R38f 上线实测时核对 Slack 有没有「电脑上不活跃时才推到手机」这类设置。
2. 后台常驻用 launchd 用户服务：`pony service install` 写 `~/Library/LaunchAgents/local.ranch.pony.plist` 并加载，登录就起、崩了 10 秒后重起、没配置时正常退出不再重起；只动自己这一个文件，用户在场、先 `--dry-run` 看、确认才写，能卸干净。不用 pony 自己转后台（`pony start`/`stop`）：那样重启、注销后不会自己起来，崩了也没人重起。第一期只在这台 Mac 上开，Linux 以后另做 systemd 用户服务。
3. Needs you 提醒带题目或屏幕末尾 20 行（R44）；R45 只按 Esc：「停」和提问框回答；不批准任何框。整句「停」或「打断」只打断当前一轮（框上即拒绝），amp 连按两次，agy 不支持。按前核对状态、消息时间和最近 30 秒的人工输入；离线补收的指令不按。在 Claude Code 提问提醒的线程里回答，核对实例、blocked、AskUserQuestion 和 state_started 仍相同后按一次 Esc，等空闲再沿原队列送回答，并加一句回答来历。这句说明带上提醒里各题的选项字母对照（多题按题号分开，字母与提醒一致，只列选项名），用户的原话紧跟在后、一字不改；题目被截断时注明题目不完整，对照超过上限（回答长度的两倍，至少 80、最多 300 字）时截断并注明。只有 Esc 命令成功才补「关闭提问框不表示放弃作答」；核对不过或按键失败只写「这是对刚才提问的回答」，回答仍排着。自动关框时提示「关掉提问框后就送」「提问框已关，回答这就送过去」，不说拒绝答案。同线程排着的回答都送出后清掉提问标记，后续按普通话处理；打断提示只合并同线程的一条，其他线程仍各自收到结果。按前存盘防重按；每秒查状态、最多 15 秒，超时照实说明、不重按，回答继续排着。原来的理由仍适用：误按会批准危险命令，所以权限批准要回电脑，pony 只用 Esc。
4. Slack 客户端自己接，用 ureq（HTTP，ranch 已在用）+ tungstenite（WebSocket，同步），不用 slack-morphism：pony 只用 4 个 Web API 加 Socket Mode，不值得为它引入一百多个包和一个异步运行时；本机可能要走代理，slack-morphism 的 Socket Mode 不支持。代价是多写约 450 行（另有测试）。
5. agent 正在干活时又来了话，排队：线程里回 `→ X（正在干活，这句排在第 n 个，这一轮结束就送）`，真送出去时说「已送给 X（排了 12 分钟）」；删掉还在排队的那条 Slack 消息就是撤回。排队的话存在 `state.json`，重启后还在。比「不送」好在手机上不用等着回头再发；过时的话可以撤回。
6. 名字 pony：包 `crates/pony`、命令 `pony`、目录 `~/.pony`。
7. paddock 的 P5-147 在 R38b 把 `pony status` 写进 CONTRACT 之后开工：数据先定下来，没配置的机器显示「未配置」，以后只做兼容新增。
8. 离线时的消息补收：Mac 睡着或 pony 没在跑时用户发的话，重连后从 Slack 历史补收并照常送，用已有的 `channels:history`、`groups:history`，不加权限；送给 agent 时开头加一句说明（「这是你 09:12 在 Slack 发的，pony 当时离线，现在才送到」）；发出超过 12 小时的不自动送，在它的线程里回「这句是 12 小时以前发的，没送；要送请重发」；第一次起来（没有记下的 `ts`）不补收，免得把频道里的旧话都送进去。

### 第一期的规矩

- **配置**：`~/.pony/config.toml`（可用 `PONY_HOME` 指定目录），初次配置由用户写，不含凭证。顶层只认 `corral`、`proxy` 和已登记适配器的段；每段必须有 `owner`、`channels`；`repo` 按 corral 名字一段的写法（ASCII 字母或数字开头，只含字母、数字和 `._-`）；同一家里一个频道只绑一个仓库、一个仓库也只绑一个频道，提醒发到哪才唯一。用频道 ID 不用频道名：改名不影响，也不用为了按名字查频道多要权限；名字由 pony 连上后查出来给 status。没有这个文件就什么都不连。
- **频道内绑定（R40a）**：owner 在未绑定的私有频道里发 `@pony 绑 <仓库名>`，把绑定加进这一家的 `channels`。只认「绑」加一个仓库名，不做解绑、改绑；已绑频道里这句话照旧送给主控。先用适配器查询，只有 `private: Some(true)` 才写；公开频道说明主控回复会带代码和路径、整个工作区可见，请建私有频道；查不到隐私属性也拒绝。仓库名非法、已绑别的频道、当前配置有错、磁盘原文与正在用的不同、写盘失败均在线程说明且不写；重复绑定显示原频道名，查不到才用 ID。不检查 agent 是否存在；成功回「已绑 <仓库>」及同 `?` 写法的主控状态，没在跑就说明。绑定成功后，这条消息的线程就记成 `<仓库>/main` 的线程（主控没在跑也记，R40a2），之后在里面回话带不带 @ 都照线程规则送；绑定时还没有实例号的，第一次送话不算「重开过」。写入保留注释、顺序和其余内容，支持一行数组和 `[[<家>.channels]]`；末尾缺换行时允许补一个换行，冲突检查仍比较原文。其他无法原样保留的格式报 `format_not_preserved`，回复说明格式问题、请手动添加，原文件不动。符号链接、硬链接或对当前用户只读的配置也拒绝，提示手动添加，不替换链接或顺着链接写。候选内容先过原有配置校验，同目录临时文件写好、同步、保留原权限，改名前再检查链接、写权限并比较原文。重读配置或凭证校验失败时禁用绑定，提示先修配置；重读成功才恢复。所有绑定拒绝记 `bind_failed <code>`，错误码经 `safe_code` 过滤，不记配置、路径或原始错误。写完不改内存配置，交给每 2 秒重读及原有重连生效。
- **随改随生效**：运行中每轮检查文件有没有变，变了就重读；新内容有错就继续用旧的，status 报 `config: "invalid"` 和原因；改了 `proxy` 或这一家的段就重连。代理要写在配置里：launchd 起的程序不继承 shell 的环境变量，ureq、tungstenite 也不读 macOS 系统设置里的代理；部署时先试直连，连不上再填 `proxy`。
- **只认 owner**：只处理配置里的 `owner` 发的消息；别人的消息不回、不送、不记内容（日志只记「忽略了一条别人的消息」）。没绑仓库的频道里只认上述绑定命令，@ 机器人说别的回「这个频道没绑仓库。要绑，发 `@pony 绑 <仓库名>`」，不列仓库；绑定写盘成功后才能查该仓库的主控状态，其余没绑仓库的 agent 连 `status` 都不调，一个字都不发到 Slack。
- **路由**：频道顶层 @ 机器人开一段新对话；在这条消息的线程里回，送给线程对应的 agent（整句只有 `?`、「停」或「打断」的除外）。线程里回复带不带 @ 都行；不认识的线程里 @ 就当新对话。只有 `?` 或 `？` 看状态；整句「停」或「打断」停线程对应的 agent，明确跨仓库送话建立的线程也放行。频道顶层停主控，顶层后面只跟一个本频道仓库的 agent 名字时停那个 agent；别的仓库回「只能停本频道仓库的 agent，没按」，像 agent 名但不存在时回「没有 X 这个 agent，没按」，都不转给主控。「停一下，先别提交」或「停 一下」仍是普通话。其余取 @ 之后第一个词：带斜杠的全名只送给绑了频道的仓库，没有这个 agent 就回「没有 ranch/foo 这个 agent，没送」，不转给主控；同仓库的短名送给对应 agent；其余整句送给 `<仓库>/main`，主控没在跑就回「paddock/main 没在跑，没送」。送之前先在线程里回一行 `→ <全名>`；送的就是用户打的字（`corral send <全名> -- <正文>`，不加 `--force`），例外是离线补收和提问框回答开头加一句说明（见本节用户定的第 8、3 条）；空的回一段用法。
- **送不进去时**：先 `corral status`。`idle` 立刻送；`working`、`starting`、`blocked`、暂停中排队，不替人 resume；`unknown` 照送（等不到回复）；不在了不送。`corral send` 的结果：`not_idle` 放回队首；`human_active` 30 秒后再试（不加 `--force`）；`paused` 排着；`not_delivered` 不重送，回「送了但 X 没收到（可能停在菜单或对话框上），没重送」；其他回 corral 的 `error` 和 `message`。排队的话按顺序送，一轮一句。agent 退出了，排着的都不送，在各自线程里说。
- **一轮结束贴回**：一个循环每 2 秒 `ls`、`status`，相邻两轮都是 `idle`、`[state, last_event, last_event_at]` 不变、`turn_started` 和上次处理过的不同，就算这一轮结束，调 `corral reply`。是 pony 送的话开的，就照分段贴回原线程（`no_reply` 带 `interrupted` 说被打断，退出了说没等到回复）；不是 pony 开的，按提醒的规则判断。不用 `wait`：一个循环同时管贴回和提醒，用 `wait` 每段对话要一直开着一个进程。
- **提醒**：主控新回复（这一轮不是 pony 开的、`reply.at` 比记下的新）发在频道顶层，第一段在那儿，其余在它的线程里；这条线程对应这个主控。Needs you（相邻两轮都 `blocked`）只发在该 agent 所在仓库绑定的频道顶层。R44：Claude Code 提问框有 `status.questions` 时按顺序列出所有题目、单选或可多选、字母编号的选项及说明；`truncated:true` 提示「题目不完整，请回电脑看」。其他情况附 `corral read --bytes 4000` 的末尾 20 行，连续空行并成一行、放进代码块、不遮盖；读不到仍发原提醒。R45 末尾说明：提问框在线程写答案就送；其他框回「停」拒绝、批准回电脑，其他话排着；agy 仍回电脑处理。模板见设计稿 §3.8。去重记号（实例、处理过的 `turn_started`、发过的 `reply.at`、提醒过的 `(实例, state_started)`）存在 `state.json`；提问提醒另记实例、state_started 和是否已处理，同一线程连答不重复关框。离开 `blocked` 再进来算新的一次；agent 重开从头记；pony 停着时结束的轮，重启后每个 agent 最多补发最近一条，开头写「（pony 不在线时）」。
- **分段**：不截、不删、不改字，上限由适配器给（Slack 10,000 字）。按行走，认围栏代码块（三个反引号或三个波浪号）；断点先用代码块外的空行、行尾，其次代码块里的行尾（本段末补上收尾的反引号，下一段用同样语言重开），一行超长就硬断；表格尽量整块放。多于一段每段末尾加 `(i/N)`；其余发在同一个线程里。发送失败对半再切（最小 1,000 字），还不行就报出来，不悄悄丢。
- **看状态**：`@pony ?` 只列绑了频道的仓库，这个频道的仓库排在最前。R44：干活、等你、空闲、暂停都带时长，分别用 `turn_started`、`state_started`（缺失时用 pony 首次看到这次 blocked 的时间）、`reply.at`（没有回复用 `state_started`）、`paused_at`；`starting`、`unknown` 保留各自说法。本频道仓库再带标题（去首尾空白和开头转圈符号，最多 40 字，空或只是程序名不显示）、在用的工具；空闲时查 `reply`，带第一行非空文字（最多 60 字），被打断写「上一轮被打断」。别的仓库只显示状态和时长，空闲用已知回复时间、没有则用 `state_started`，不为摘要额外查回复。排着话的加「排着 n 句」。
- **重启后**：`~/.pony/state.json`（0600）由拿着锁的那一份写。线程和 agent 的对应（按 30 天没用删掉；再回就说「这个线程太久没用，对应关系已清掉，请在频道里重新 @ 我」，不猜）、在等回复的对话、排队的话、去重记号、最近处理过的消息 `ts` 都还在；重连时按 `ts` 补收离线期间的话（用户定的第 8 条）。
- **Slack 接法**：Socket Mode，pony 往外连 wss，本机不要公网地址。app 级 token 换临时地址；每个事件先把这条消息记进状态文件再按 `envelope_id` 回执（Slack 要求 3 秒内，最多重发 3 次），按 `ts` 去重；`hello` 里的 `num_connections` 大于 1 说明还有别的连接（另一台机器或没退干净的 pony），status 报 `other_connections`；每几小时一次的 `disconnect` 就重新取地址再连，网络错按 1、2、4…60 秒退避，30 秒 ping、90 秒没消息算断；每次重连重读凭证。只订 `message.channels`、`message.groups`，不订 `app_mention`（同一句 @ 会来两次）。权限只有 `chat:write`、`channels:history`、`groups:history`、`channels:read`、`groups:read`。建议用私有频道：仓库的代码、路径会发进去；机器人要邀请进频道才收得到。
- **发消息**：`chat.postMessage` 带 `thread_ts`，正文用 markdown 块，另给一行 `text` 当通知里那句；关掉链接预览（免得 Slack 的服务器去抓回复里私有仓库的链接）；要提醒的按用户定的第 1 条 @ 用户。每频道约每秒 1 条，429 照 `Retry-After` 等。

### 常驻、状态、凭证、日志

- `pony run` 前台跑，launchd 用它，调试时也能在终端直接跑。同一个 `PONY_HOME`（默认 `~/.pony`，测试用隔离的）只能有一份：`~/.pony/run/lock` 拿不到就退出 1，launchd 10 秒后再试，等前一份退了自然接上。没有配置文件就记一行日志、退出 0。`pony status` 只查锁、不拿锁。
- 服务：`pony service install|uninstall [--dry-run] [--yes]`，非 macOS 报 `unsupported`。plist 的 `ProgramArguments` 指向 `~/.local/bin/pony run`，`RunAtLoad`，`KeepAlive.SuccessfulExit` 为 false（退出码非 0 才重起）；标准输出、错误写 `~/.pony/logs/launchd.log`，只接 panic。写法照 corral integration：原子写，那里已有不是 pony 写的东西就报 `foreign`、不动。
- `~/.pony/`：`config.toml`、`secrets/<kind>.toml`、`state.json`、`run/`、`logs/`。目录 0700、凭证文件 0600、属于当前用户、是普通文件；不是就不连，报 `secrets_too_open`，拒绝符号链接，每次连之前都查。凭证只放本机，不进仓库、不进 `pony status`、不进日志。不选钥匙串：macOS 按程序授权，pony 每次部署换新程序，人不在电脑前时会卡在弹框；文件加权限检查，和 fence 存 key 一样。
- 日志 `~/.pony/logs/pony.log`，满 1 MiB 轮转留 3 份。记连接变化、路由到哪个 agent、频道 ID、消息 `ts`、corral 和 Slack 的错误码；不记消息和回复正文、任何 token、`Authorization` 头、wss 地址（地址里带一次性的 ticket）。

### 给 paddock 的 `pony status`

- 只读本机文件（配置、`run/status.json`、锁），不联网、不调 corral、不碰 agent，几毫秒，可以每几秒调一次。成功退出 0、一行 JSON；没配置、没在跑、连不上都是 `ok:true` 里的数据，不是错误；只有参数错是 `usage`、退出 2。不输出 token、用户 ID、消息内容。
- 字段：`config`、`config_message`、`service`、`running`、`pid`、`started`、`last_exit`、`chats[]`（`connection`、`since`、`reason`、`message`、`workspace`、`bot`、`other_connections`、`channels[]` 的 `id`、`name`、`private`、`repo`、`problem`）。格式、取值和显示办法见 `docs/CONTRACT.md` 的「`pony status`（R38）」。
- 前端找命令：PATH 上找不到 `pony`、`~/.local/bin/pony` 也没有，就是「未安装」，和找 corral 的办法一样。

### 对 paddock 的影响

- R40a 在频道内新增绑定入口，配置格式和 `pony status` 格式不变，旧版 pony 也认新增的绑定；新频道重读后照常出现在 `channels`，paddock 不用动。
- 只是兼容新增：新命令 `pony status`。corral 的命令、JSON、`~/.corral` 都不变，paddock 现有功能不用动。
- R38b 合并后 paddock 就可以照 CONTRACT 开 P5-147（用户 10-21 定）；R38c 部署之前 PATH 上还没有 `pony`，Settings 显示「未安装」。
- pony 送进去的话在 corral 里是 `last_input_source: "send"`，和 paddock 自己送的一样；两边都遵守 `not_idle`、`human_active`，不会硬送。主控的回复会同时出现在 paddock 收件箱和 Slack 里。

### 实现怎么拆

- R38c 包和外壳：`--version`、`status`、`run` 外壳，配置和凭证的读取、校验、权限检查，单实例锁、状态文件、日志轮转，打包工具把 `bin/pony` 放进版本目录；在隔离的 `PONY_HOME` 里测 status 的各种情况（没配置、配置错、凭证太宽、在跑、没在跑）。部署后 `pony status` 是真命令，没配置时报 `missing`。
- R38d 共用核心：路由、线程表、送话和排队、盯 agent 和提醒、去重、分段、`?`、重启恢复；用内存假适配器和假 corral 测验收 1–5 的逻辑，另加核心源码不出现适配器名的测试。
- R38e Slack 适配器：Socket Mode（代理、回执、重连、`hello` 里的连接数）、4 个 Web API、事件翻译、markdown 块、限频；和 R38d 可以同时做。
- R38f 常驻和上线：`pony service install|uninstall`、部署说明；用户在场时建 Slack 应用、写 `~/.pony`、装服务，实测验收 1–6（含 markdown 块里 `<!channel>` 和长回复的样子、要不要代理、Slack 的通知设置）。
- 每件跑 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。测试不连 Slack、不碰真实 agent：corral 那层用内存假实现，或用工作区编出的 corral 和 `/bin/cat` 这类合成程序，HOME、`CORRAL_HOME`、`PONY_HOME` 都隔离。设计稿 §8 有逐件的依赖和对 paddock 的影响。

来历：`docs/任务/R38-从Slack等聊天软件at着agent开发.md`、`docs/调研/R38a-Slack机器人设计.md`。
