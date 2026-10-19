# ranch 设计

## 1. 是什么

每台机器、每个用户只能有一份的运行时：数据和全局位置只能有一个主人。用户 10-05 定：把它们从 Saddle 里拉出来，独立成 ranch；Saddle（TUI）和 paddock（GPUI）都只是前端（来历见 `../paddock/docs/背景与决策记录.md` §6g–§6j）。

| 部分 | 来源 | 状态 |
| --- | --- | --- |
| corral（agent 运行时，命令 `corral`） | Saddle `crates/corral-core`，提交 `a31dea2` | 第 1 步 |
| dispatch（路由命令 `ranch dispatch route`、`corral-dispatch` 技能，不带遥测） | Saddle `plugins/dispatch` | 第 2 步 |
| 遥测、Drover、插件系统 | Saddle | 不迁；用户 10-05 定砍掉，Saddle 删除代码，数据留在磁盘上 |
| fence（sandbox，命令 `fence`，`corral start --sandbox` 调它，接 srt） | 新做 | 设计中，见 §8 |

## 2. 与前端的关系

- 前端只调用 ranch 装好的命令，读 JSON 输出，不在 Cargo 里引用 ranch 的包，不读内部状态目录。各自的依赖、锁文件、工具链互不进入。
- 全局位置归 ranch：`~/.corral`、`~/.local/bin/corral`、corral 技能（`~/.claude/skills/corral`、`~/.agents/skills/corral`，R8 起还有 `~/.gemini/config/skills/corral`），以后还有 `~/.local/bin/ranch` 和 `corral-dispatch` 技能目录。Saddle、paddock 不再打包、安装这些。
- Saddle 是保底版（用户 10-05）：不再加新功能。用户 10-07 基本不用 Saddle 了，定“保底规矩放松吧，Saddle 不用再写影响了”：改 ranch 时只按 AGENTS.md 的规矩写明对 paddock 的影响，不再写对 Saddle 的影响，也不再问 Saddle 跟不跟。
- 单独分发 paddock 时，打包带上 ranch 的某个版本（paddock DESIGN §7）。

## 3. 安装

- 打包工具（Rust）生成不可变版本目录 `~/.local/share/ranch/versions/<ranch 提交>/`：`bin/corral`、`bin/ranch`、`share/corral/`（随程序的资源和设计文档）、`BUILD.txt`（提交、目标平台、工作区是否干净、校验和）。目录已存在就拒绝；只生成目录，不切链接、不装技能。
- R17b 起版本目录还有 `bin/fence`、`share/fence/`（`runner.mjs`、记着打包时 node 路径和版本的 `runtime.toml`）、`lib/srt/`（固定版本的 srt）；打包机器要有 node ≥ 22.12 和 npm，没有就打包失败。corral 用同一版本目录里的 fence，前端调 PATH 上的 `fence`，所以部署时 `~/.local/bin/fence` 和另外两个链接一起切。
- 切换 `~/.local/bin/corral`、`~/.local/bin/ranch` 和安装技能（`corral install-skills`、`ranch dispatch install-skills`）是单独的部署操作，用户在场时做。已在运行的 agent 由启动它的那份 corral 管理；要换过来用 `corral upgrade`（用户 10-05 定：第 1 步切换时不做）。

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
- omp 被人打断（R25，用户 10-19）：omp 在生成回复或调用工具时按 Esc 会中止整轮，最后一条 assistant 消息带 `stopReason: "aborted"`、`errorMessage: "Interrupted by user"` 和 `errorId` 的 UserInterrupt 位（67108864）。reader 在最终 `agent_end`（不是 `willContinue`）的最后一条 assistant 三个标记齐全时按 `Interrupt` 处理，其余中止、出错、重试照旧；跑命令时 Esc 只停命令、模型接着正常结束的不算打断。采集器早已原样记下 `messages`，只改 reader，omp 不用重开，helper 不用切。内部 cursor 缓存 7 → 8。来历：`docs/任务/R25-omp被打断后主控认得出.md`。

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
  - `packed-refs.lock` 报错不消除：提交不受影响，删除已打包的分支、`gc`、建新分支或 worktree 在墙里会失败。
- 来历：`docs/任务/R17-agent放进sandbox.md`、`docs/调研/R17a-srt小试验.md`、`docs/调研/R17b-fence方案.md`。
