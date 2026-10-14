# ranch 设计

## 1. 是什么

每台机器、每个用户只能有一份的运行时：数据和全局位置只能有一个主人。用户 10-05 定：把它们从 Saddle 里拉出来，独立成 ranch；Saddle（TUI）和 paddock（GPUI）都只是前端（来历见 `../paddock/docs/背景与决策记录.md` §6g–§6j）。

| 部分 | 来源 | 状态 |
| --- | --- | --- |
| corral（agent 运行时，命令 `corral`） | Saddle `crates/corral-core`，提交 `a31dea2` | 第 1 步 |
| dispatch（路由命令 `ranch dispatch route`、`corral-dispatch` 技能，不带遥测） | Saddle `plugins/dispatch` | 第 2 步 |
| 遥测、Drover、插件系统 | Saddle | 不迁；用户 10-05 定砍掉，Saddle 删除代码，数据留在磁盘上 |

## 2. 与前端的关系

- 前端只调用 ranch 装好的命令，读 JSON 输出，不在 Cargo 里引用 ranch 的包，不读内部状态目录。各自的依赖、锁文件、工具链互不进入。
- 全局位置归 ranch：`~/.corral`、`~/.local/bin/corral`、corral 技能（`~/.claude/skills/corral`、`~/.agents/skills/corral`），以后还有 `~/.local/bin/ranch` 和 `corral-dispatch` 技能目录。Saddle、paddock 不再打包、安装这些。
- Saddle 是保底版（用户 10-05）：不再加新功能。用户 10-07 基本不用 Saddle 了，定“保底规矩放松吧，Saddle 不用再写影响了”：改 ranch 时只按 AGENTS.md 的规矩写明对 paddock 的影响，不再写对 Saddle 的影响，也不再问 Saddle 跟不跟。
- 单独分发 paddock 时，打包带上 ranch 的某个版本（paddock DESIGN §7）。

## 3. 安装

- 打包工具（Rust）生成不可变版本目录 `~/.local/share/ranch/versions/<ranch 提交>/`：`bin/corral`、`bin/ranch`、`share/corral/`（随程序的资源和设计文档）、`BUILD.txt`（提交、目标平台、工作区是否干净、校验和）。目录已存在就拒绝；只生成目录，不切链接、不装技能。
- 切换 `~/.local/bin/corral`、`~/.local/bin/ranch` 和安装技能（`corral install-skills`、`ranch dispatch install-skills`）是单独的部署操作，用户在场时做。已在运行的 agent 由启动它的那份 corral 管理；要换过来用 `corral upgrade`（用户 10-05 定：第 1 步切换时不做）。

## 4. 步骤

1. **corral**（任务：`../paddock/docs/任务/M2-ranch与corral.md`）：迁入、打包工具、切换 `~/.local/bin/corral`；Saddle 侧改成用 ranch 的 corral（需求交 Saddle 主控）。做到 Saddle 和 paddock 都能用为止（用户 10-05：“先把corral从saddle中拉出去，直到saddle能够和新的corral一块工作”）。
2. **dispatch**（任务：`../paddock/docs/任务/M3-dispatch进ranch.md`）：路由做成 `ranch dispatch route`，去掉遥测；`corral-dispatch` 技能归 ranch 安装（`ranch dispatch install-skills`），删去遥测部分、命令改成 ranch 的；Saddle 删掉内置 dispatch 插件、放掉技能归属（需求交 Saddle 主控）。

## 5. 待定

- （已定，用户 10-05）只再拆 dispatch（不带遥测），之后转回 paddock 开发；遥测、Drover、整个插件系统砍掉（Saddle 删代码），两个前端只靠 ranch 的 corral 和 dispatch。
- 插件协议的共享方式。

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

- 结构：`crates/corral/src/agents/`，`mod.rs` 是 `Adapter` 接口和登记表 `ADAPTERS`，每家一个文件（现在 claude、codex、pi、omp、amp），沿用 Python 版 corral `src/corral/agents/` 的做法。每家写明：种类（程序的文件名）、启动时加的钩子参数、停止步骤、读哪些技能位置（R4 起；见下）、原始事件（v2，pi／omp）里的分家规则。共用的代码（pen、cli、skills、events）只查登记表，不写某一家的名字；不认识的程序不加钩子、不能带 `--prompt`、状态为 `unknown`、按默认步骤停。
- 加一家：加 `<种类>.rs`、在 `ADAPTERS` 登记一行。适配器往 agent 状态目录写文件的（如 pi 的 `hook_pi.ts`），文件名还要加进 `state::FILES`：agent 名字检查和清理都靠它，加进去后同名的 agent 名字不再合法（测试会查漏）。新事件名要用到共用状态机（如 codex 的 `Interrupt`）时，改 `events.rs`，它是 v1 事件格式的一部分。
- 技能位置（R4）：按位置登记（`skills.rs` 的 `TARGETS`：`claude` 是 `$CLAUDE_CONFIG_DIR` 或 `~/.claude`，`codex` 是几家共用的 `~/.agents`），各家适配器声明自己读哪些位置（claude 读 `claude`；codex、pi 读 `codex`；omp 两个都读）。R3 时位置挂在 claude、codex 两家上，pi、omp 报没有，不符合事实。`--target codex` 的名字沿用，没改公开用法。
- `corral integration status`（R4）由登记表生成，给前端（paddock Settings）列出各家：程序、版本、钩子怎么接（现在都是 `launch`）、技能装没装；格式见 `docs/CONTRACT.md`。
- 共用、不归某一家的：钩子写事件（`__hook`）、`.runtime/helper`、v1／v2 事件格式和读取、pi／omp 共有的原始事件翻译（`events/raw.rs`）。
- 全局配置：到 R3 为止，钩子只随 corral 启动的那个 agent 生效（启动参数或状态目录里的文件），不写任何全局配置。用户 10-14 同意放宽，条件：经用户确认才装；钩子不在 corral 启动的 agent 里就什么都不做；只动自己那块；原子写；能卸干净。R3 不改代码，留给以后装全局钩子那件。
- 全局安装（R6，先给 amp）：不能在启动时挂钩子的 agent，适配器声明一个 `Global`（配置目录、文件、内容），`corral integration install|uninstall --agent <种类>` 经用户确认后原子写或删这一个文件，别的不动；那里已有不是 corral 写的东西（没有 `CORRAL_INTEGRATION = <n>` 版本号，或者那家配置目录及以下有符号链接；配置根目录本身是链接不算，常见于 dotfiles）就报 `foreign`、不动；用户确认后重新判断一次，变了就什么都不写，新建用硬链接、目标已有文件就失败。装上的文件只在 `CORRAL_EVENTS` 有值且 `CORRAL_KIND` 是它那家时才做事；`CORRAL_KIND`、`CORRAL_CWD` 只给这类 agent 设，现有四家的环境不变（`CORRAL_KIND` 挡住别的 agent 里跑的同种程序）。pen 还给这类 agent 设 `CORRAL_PEN_PID`，插件确认自己那个 agent 的父进程就是这个 pen，挡住 corral 管的 amp 里面直接再跑的 amp（环境全是继承的，R6 审查发现）。amp 装在 `$XDG_CONFIG_HOME/amp/plugins/corral.ts`（默认 `~/.config`），插件直接写 v1 事件，读事件和状态机不改；不挂 `tool.call`（要表态，可能绕过 amp 自己的权限）；只记顶层 thread（查不出是不是顶层时这次不记、下次再查）；amp 打开或切回一个 thread 时重新报会话开始，后台别的 thread 的事件不抢主会话；amp 不支持 `--prompt`。真实 amp 实测后：插件一加载就报会话开始（占位 id），corral 才肯 send 第一句；amp 还在加载时会跳过那一轮的 `agent.start`，插件从 thread 最后一条用户消息或这一轮结束时补记输入；插件接口在等批准时仍报 `running`，所以适配器可以声明「标题说在等用户」（`title_blocked`，amp 是 `◆ Input needed` 开头），状态为 working／idle 时报成 blocked，只对声明了的适配器生效（用户 10-14 选的做法，读屏兜底之前的最小一步）。amp 只能用 R6 及以后的 corral 操作，旧版 corral 启动的 amp 要重开，见 CONTRACT。用户 10-14 定：先 amp，然后读屏兜底，再 agy；桌面版 Amp 可能也加载这个插件，用户接受，由 paddock 在 Settings 里讲清楚。来历：`docs/调研/R5-agy与amp接入、全局安装和读屏兜底.md`、`docs/任务/R6-接入amp.md`。
- 读屏兜底（R7，给 agy 这类钩子报不全的 agent 用）：适配器可以声明读屏规则（`screen_rules`，默认没有；一条规则看屏幕最下面 n 行非空的或整屏，「都要有」「不能有」哪些字）。只有声明了规则的那家，pen 才用 `vt100` 常驻一份屏幕、喂输出、跟着改大小，并在它的内部状态回复里给出屏幕文字（`screen`，兼容新增）；别家的 pen 不建屏幕、不多花开销。命令行算状态时（`status`、`wait`、`send` 的空闲检查、提醒都经过这一处），钩子报 working／idle 而屏幕命中规则，就报 `blocked`；别的状态以钩子为准。每次按当前屏幕现算，不锁存：提示框没了就回到钩子的状态。和 R6 的标题规则合在同一处（`agents::waits_on_terminal`）。`corral upgrade` 的快照不带屏幕，新 pen 从输出环（512 KB）重建；新版命令行碰到没有 `screen` 的旧 pen 就不读屏。这一件只建机制，没有哪家声明规则；规则只判 `blocked`，agy 的规则和是否加 `state_source` 在接 agy 时按真实画面定（用户 10-14）。
