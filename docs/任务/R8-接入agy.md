# 任务：接入 agy（Google Antigravity 的命令行版）

2026-10-14 起草，ranch 主控（ranch/main）自己做。
路由：重 / 交叉审查要（用户 10-14 同意）/ 影响面：改行为（新增一家 agent；往用户的 `~/.gemini` 里装插件；`integration status` 可能多一个字段）
类型：功能变更
依据：R5 调研报告第一、三节（`docs/调研/R5-agy与amp接入、全局安装和读屏兜底.md`）、用户 10-14 定的五条（`docs/任务/R5-agy与amp接入调研.md` 末尾）、R6（全局安装框架、amp 的做法）、R7（读屏兜底）。这是第三阶段。
提示：**现有四家和 amp 的行为一点不变**；插件只动 corral 自己的目录；真实 agy 实测用户在场。

## 用户要的

- paddock P5-88：“加一个draft 任务，集成agy agent”。
- R5 第六节（用户 10-14 同意）：
  - 第 1 条：agy 装在 `~/.gemini/config/plugins/corral/`（`plugin.json` + `hooks.json`）。
  - 第 2 条：“真实一轮：在临时 HOME 的测试实例里由用户自己登录一次，用完删掉。动手前先查清 agy 的登录是不是存在系统钥匙串里，别动到用户平时用的那份。”
- 10-14：“读屏幕只是兜底给agy，amp这类用的”。
- 阶段次序：一 amp（R6）→ 二 读屏兜底（R7）→ 三 agy（这件）。

## 本机现状（主控已看，只看了目录名，没读文件内容）
- agy 1.3.5 在 `~/.local/bin/agy`。
- 用户的 `~/.gemini/` 里有 `antigravity`、`antigravity-cli`、`antigravity-ide` 等目录，`config/` 下已有 `hooks.json`、`hooks/`、`skills/`、`config.json`，还有登录相关的文件。也就是说用户平时在用 agy 或 Antigravity IDE，而且已经登录。
- IDE 和命令行可能共用 `~/.gemini/config`，所以 IDE 可能也会加载 corral 的插件（和桌面版 Amp 的情况一样，要核对）。

## 在哪里干活
- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r8-agy`，分支 `r8-agy`。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r8-agy`。

## 要做的

1. **先核对，再定细节**（结果写进完成记录）。只在临时 HOME 里试，不碰用户的 `~/.gemini`：
   - agy 的登录存在哪（系统钥匙串还是文件）：只看 agy 程序和文档的说法，不读用户的登录文件。
   - 钩子拿不拿得到 agy 的环境变量（`CORRAL_EVENTS` 等）。**这一条决定方案成不成立**；拿不到就停下来找用户商量。
   - 钩子要不要输出 `{}`；回复原文在 `transcriptPath` 指的文件里是什么格式；等批准时屏幕长什么样；怎么退出好使（Ctrl-C、`/quit`、HUP）。
   - agy 从哪些目录读技能（`~/.gemini/config/skills`？`~/.agents/skills`？）；IDE 会不会加载插件。
2. **全局安装框架扩成多文件**：R6 的 `Global` 只装一个文件；agy 的插件是一个目录（`plugin.json` + `hooks.json`）。装、卸、`foreign` 的判断、确认后重查都按整个目录做，只动 `plugins/corral/` 这一个目录。amp 照旧。
3. **agy 适配器**（`agents/agy.rs`）：
   - `--prompt` 用 `agy -i "<首句>"` 带进去。
   - 钩子只挂只观察的事件：`PreInvocation`（工作中）、`PostToolUse`（工具）、`Stop`（这一轮结束），可能加 `SessionStart`。**不挂 `PreToolUse`**：它要表态，脚本出错就会拦下工具调用。
   - 钩子的输入（驼峰字段 `conversationId` 等）在适配器里翻译成 corral 的事件，走 pi／omp 用的 v2 原始事件那条路。回复原文从 transcript 文件读。
   - 插件只在 corral 启动的 agy 里起作用：和 amp 一样看 `CORRAL_KIND`，并确认 agy 的父进程是 `CORRAL_PEN_PID` 指的 pen。其余情况（用户自己开的 agy、IDE、嵌套的 agy）什么都不做，并且要让 agy 照常往下走。
   - 读屏规则：按真实画面写「等批准」「等回答」的规则（R7 的机制第一次用上）。
   - 停止步骤按实测定；技能位置只登记核对过的。
4. **`integration status`**：agy 一项 `hooks: "global"`，`install{path,status,version}`，和 amp 一样（见下面「要用户定的」第 3 条）。
5. **技能文字**：corral 技能、`corral guide`、corral-dispatch 里加 agy 的用法（见下面「要用户定的」第 4 条）。
6. **文档**：CONTRACT、DESIGN §7、corral README。

## 不做
- 不写用户 `~/.gemini/config/` 里的共享文件（`hooks.json`、`config.json`、`import_manifest.json`），只读 `config.json` 里 corral 那一项开关（如果做第 3 条）。
- 不读用户的登录文件；不碰 Antigravity IDE；不碰用户开着的 agent。
- 不改 amp 和现有四家。

## 怎么算做完
- 用户没另说验收。主控建议下面几条，请用户定：
  1. 现有四家和 amp 不变：R3 快照不改；R6、R7 的测试照旧通过。
  2. 合成测试（隔离的 HOME、CORRAL_HOME，假的 agy 程序，不用真实 agy）：
     - 装、卸整个插件目录的各种状态；`foreign` 不动；卸干净；
     - 钩子事件翻译成工作中、工具、空闲，子 agent 的事件不算；
     - 读屏规则在合成画面上判出 blocked；
     - 不在 corral 里（没有变量、种类不对、父进程不是 pen）时什么都不做。
  3. 真实一轮（用户在场）：
     - 送一句话，看到工作中 → 空闲，回复对；
     - 试一次等批准，看到 blocked，批准后继续；
     - `stop` 停得干净；
     - 用完 stop，临时目录整个删掉。
  4. `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 全过。

## 部署（用户在场）
- 主控打包、核对；用户切 `~/.local/bin/corral`、`~/.local/bin/ranch`，R7 一起带上。
- 技能文字改了，要重装两份技能。
- 插件要不要装进你真实的 `~/.gemini/config/plugins/corral/`，由你决定（或者以后从 paddock 的 Settings 点）。

## 对 paddock 的影响（写进完成记录）
- 兼容新增：status 多一项 agy；install／uninstall 多支持 `--agent agy`。
- Settings 要讲清楚的和 amp 类似（IDE 是否也加载，核对后写）。
- New Agent 可以选 agy，并且可以带 Prompt（和 amp 不同）。

## 要用户定的
1. **真实一轮在哪测**：
   - 临时 HOME（R5 时你定的）：你在里面重新登录一次，用完整个删掉。不碰你平时用的 `~/.gemini`。前提是登录不在系统钥匙串里，否则可能碰到你的真实登录，要先查清。
   - 真实 HOME（R6 测 amp 的做法）：用你现有的登录，插件装进真实的 `~/.gemini/config/plugins/corral/`，你在场。省掉重新登录，但要往你在用的 `~/.gemini` 里放东西，IDE 也可能加载它。
   - 主控建议**临时 HOME**，按你原来定的来；查出登录在钥匙串里再找你商量。
2. **交叉审查**：主控建议**要一轮**（Codex）。又一次往你的全局配置里装东西，还第一次把读屏用到真实 agent 上。
3. **`integration status` 要不要报「插件被停用」**（`install.enabled`：你在 agy 里 `agy plugin disable corral` 时为 `false`；只读 agy 的 `config.json`，不写）：主控建议**加**。不加的话，你停用了插件，agy 会一直卡在 starting，界面也看不出原因。这是兼容新增。
4. **派遣**：主控建议和 amp 一样，「点名才派」，用默认模式、不分档。agy 有 `--dangerously-skip-permissions`，派出去干活时带上它，和 Claude Code、Codex 一样不弹权限框。
5. **`state_source`**（状态是钩子报的还是读屏推测的）：主控建议**还是不加**。等真用起来，读屏真的判错了再说。

## 用户定的（10-14）

“同意你的建议，五条都按你说的来，开始吧。”即：
1. 真实一轮在临时 HOME 里测，用户在里面登录一次，用完整个删掉；查出登录在系统钥匙串里再找用户商量。
2. 交叉审查要一轮（Codex）。
3. `integration status` 报插件被停用（`install.enabled`）。
4. 派遣：agy 点名才派，默认模式、不分档；派出去干活带 `--dangerously-skip-permissions`。
5. 不加 `state_source`。

核对（10-14）：agy 用 go-keyring 把登录令牌存在 macOS 系统钥匙串（程序里 `cli/backend/auth/keyring.go`、`KeyringTokenStorage`），临时 HOME 隔不开登录。主控问用户，用户改选：“真实 HOME，像测 amp 那样”（用现有的登录和真实的 `~/.gemini`，插件装进 `~/.gemini/config/plugins/corral/`，用户在场）。摸底先在临时 HOME 里试（不重新登录），能用就不往真实 `~/.gemini` 放东西。

技能（10-14）：agy 只从 `~/.gemini/config/skills` 读全局技能（`.agents/skills` 只在项目目录里读）。主控问用户，用户选“两份都装”：`corral install-skills` 和 `ranch dispatch install-skills` 都多装 `~/.gemini/config/skills`。

## 完成记录

## 真实一轮（10-14，用户在场）

用户：“可以开始，跑完卸掉”。

**环境**：真实 HOME，worktree 编出的 corral（`e5de242`），状态目录 `/tmp/r8agy.Vg4v/real`；测试 agent `ranch/test-agy`（`--label role=test`），工作目录是摸底时已信任的临时目录。

**安装**：`integration install --agent agy --yes` 写了 `~/.gemini/config/plugins/corral/{hooks.json,plugin.json}`，`agy plugin list` 显示 `corral (enabled)`、1 个钩子。

**验收**：
1. 带 `--prompt` 的第一句：
   - 头 6 秒左右是 `starting`（agy 在启动），之后 working → idle。
   - `reply` 是「2 加 3 等于 5。」。
   - 事件依次为 SessionStart、UserPromptSubmit（提示词是原话，`last_input_source` 是 `send`）、Stop（回复原文）。
2. `send` 一句要跑命令的话：
   - `send` 确认送达（`merged_with_draft:false`），之后转 working。
   - agy 弹批准框时报 `blocked`。屏幕最下面是 `> 1. Yes, run command`…`4. No, cancel`、`↑/↓ Navigate · tab Amend · ctrl+g edit/expand command`。
   - `keys enter`（选第 1 项，只批准这一次）后转 working，`last_tool` 是 `run_command`；之后 idle，回复是完整原文（多段文字和代码块）。
3. `stop`：`stopped_by:SIGHUP`，pen、agy 和 agy 起的两个子进程都退了。

**卸载**：
- `integration uninstall --agent agy --yes` 删了两个文件和 `corral` 目录，`agy plugin list` 显示 “No plugins installed.”。
- 安装时建出来的空 `plugins` 目录卸载不删（照约定只删 corral 自己那块），主控用 `rmdir` 删掉，恢复成原来没有 `plugins` 的样子。

### 做了什么
- **全局安装扩成可以装一个目录**（`Global` 加 `files`、`enabled`、`helper`）：
  - 目录里只能有 corral 的文件；
  - 每个文件原子写，`plugin.json` 最后写、先删；
  - 写入照判断时的结果来，判断为没有的只新建；
  - 卸载时清掉写到一半留下的 `.corral-*.tmp`。
  - amp 照旧一个文件。
- **agy 适配器**（`agents/agy.rs`）：
  - 首句用 `--prompt-interactive=<首句>`；
  - 停止步骤 HUP 3 秒、TERM 3 秒；
  - 技能位置 `gemini`；
  - 读屏规则：最下面 4 行非空的里同时有 `↑/↓ Navigate`、`tab Amend`；
  - 插件目录装在 `~/.gemini/config/plugins/corral/`；
  - `install.enabled` 读 agy 的 `config.json`；
  - 钩子翻译成 v1 事件，提示词和回复从 transcript 逐行取。
- **插件**（`resources/agy_hooks.json`、`agy_plugin.json`）：挂 SessionStart、PreInvocation、PostToolUse、Stop。每条钩子在 corral 启动的 agy 里 exec `"$CORRAL_HELPER" __hook agy:<事件>`，否则 `echo {}`。
- **`__hook <种类>:<事件>`**（`hooks.rs`）：
  - 环境齐全（`CORRAL_EVENTS`、`KIND`、`PEN_PID`、`INSTANCE`、`CWD`）、agy 的父进程是 pen，才读输入（最多 16 MiB）并记录；
  - 无论记不记都输出 `{}`。
- **pen 给用 helper 的那家设 `CORRAL_HELPER`**：共享 helper 正好是这份 corral 时用 helper，否则用这份 corral 本身。
- **技能多一个 `gemini` 位置**（`~/.gemini/config/skills`）：
  - `corral install-skills` 和 `ranch dispatch install-skills` 都认；
  - 带 `--project` 时 `gemini` 报 `usage`，`all` 跳过它。
- **文字**：
  - CONTRACT、DESIGN §7、corral README；
  - corral 技能、`corral guide`、corral-dispatch：agy 必带 `--prompt`，点名才派，派出去带 `--dangerously-skip-permissions`，卡在 `starting` 怎么查。

### 验证了什么
- `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`、`git diff --check` 全过。指定已部署的 `0e5bd19` 跑兼容测试（`CORRAL_COMPAT_BIN=… --test compat -- --ignored`）3 项也过。
- **现有四家和 amp 不变**：
  - R3 快照只多出 `all` 里的 gemini 一项（有意的改动，按用户“两份都装”，快照已重生成）；
  - R6 amp 测试、R7 读屏测试照旧通过。审查改动中途把 `CORRAL_HELPER` 也设给了 amp，被 R6 的 amp 测试当场抓住，已改成只给 agy。
- **合成测试**（隔离的 HOME、CORRAL_HOME，假 agy 照真插件里的命令跑钩子）：
  - 钩子翻译：首次调模型才算一轮开始；提示词和回复不取上一轮；取消记成中断、出错记成失败；取不到就不写。
  - 环境不全、种类不对、父进程不是 pen、在 agy 里再起的 agy：什么都不记，始终回 `{}`。
  - 带 `--prompt` 的一轮：`send` 确认，等批准判 `blocked`，工具，回复，HUP 停止。
  - 旧 helper 留在命名空间里时，新启动的 agy 照样能记。
  - 插件目录：装、更新、补文件、卸干净，`enabled` 三种值；各种 `foreign` 不动；判断后冒出的文件不覆盖；留下的临时文件卸载时清掉。
  - gemini 技能。
  - 读屏规则用真实一轮抓到的批准框：信任对话框、提示滚走、正文提到这两个词，都不算等批准。
- **临时改坏代码自查**：分别去掉父进程检查、helper 回退、按判断写入、`CORRAL_INSTANCE` 检查，对应测试都会失败。
- **资源**：128 MiB 的 transcript 一次钩子约 30 ms、峰值内存约 3 MB（release）。
- **真实一轮**：见上节，三项都过。
- **Codex 交叉审查**：见 `R8-审查.md`，共 7 条（必须改 2，建议改 5），处理见下。

### 审查意见怎么处理的
1. **（必须改）判断为新建的文件，写时冒出来会被覆盖**：属实（amp 也是）。改成写入照判断时的结果，新建只用硬链接；补了单元测试（amp 和 agy 的最后一个文件）。
2. **（必须改）共享 helper 还是旧版时，新开的 agy 一直 `starting`**：属实（helper 只在 `corral upgrade` 时切）。pen 给 agy 设 `CORRAL_HELPER`，插件改用它；不替用户切共享 helper。补了回归测试。
3. **（建议改）同一进程里别的会话会抢主会话**：
   - 没改代码。agy 有子 agent，它们会不会触发 `SessionStart` 没法离线核实。agy 也有 `/clear`，改成「只认第一个会话」会跟丢。
   - 和 Claude Code 一样，最近一次会话开始成为主会话。写进 DESIGN 的已知限制，遇到再按真实情况改。
4. **（建议改）transcript 每次整份读**：改成逐行读，只解析两种行、只留这一轮要的；先查环境再读输入，输入最多 16 MiB。
5. **（建议改）环境不全仍会记；exec 失败没有 `{}`**：
   - 前一半改了：`CORRAL_INSTANCE`、`CORRAL_CWD` 也必须有，补了用例。
   - 后一半没改：helper 是 corral 自己的程序或原子替换的链接，正常不会 exec 失败。去掉 `exec` 会多一层 shell，要连父进程检查一起改。写进已知限制。
6. **（建议改）正文里出现两个词就判 `blocked`**：规则改成两个按键提示，补了反例。
7. **（建议改）留下的临时文件让卸载不干净**：卸载时清掉 corral 自己的 `.corral-*.tmp` 普通文件，删不掉目录就给警告。名字像但不是的（`.corral-notes`、名为 `.corral-x.tmp` 的目录）仍算 `foreign`。

### 拿主意的地方
- **事件**：和 amp 一样翻译成 v1 事件，没走任务里写的 pi／omp 的 v2 原始事件，状态机不改。
- **首句**：用 `--prompt-interactive=<首句>` 一个参数，没用 `-i "<首句>"`，首句以 `-` 开头也不会被当成选项。
- **取不到时**：提示词取不到时不写（原先会被记成字符串 `"None"`）；工具名取不到时不写。
- **`CORRAL_HELPER`**：只给 `Global.helper` 为真的那家设，amp 的环境不变。
- **卸载**：只删 corral 的文件和 `plugins/corral`，上一级 `plugins` 不删（和 amp 一样）。真实一轮里这个空目录是主控用 `rmdir` 删的。

### 没做的事
- agy 子 agent 的会话会不会触发钩子、会不会抢主会话，没有实测（见审查第 3 条）。
- Antigravity IDE 会不会加载这个插件没有核对（不碰 IDE）。加载了也只是每个事件多跑一次 `sh`、回 `{}`。
- 钩子 shell 里 `exec` 失败的保底（见审查第 5 条）。
- macOS 上多线程同时起 `corral start` 时，管道描述符会漏进常驻的 pen，使调用方等不到输出结束。`protocol` 测试偶尔卡住就是这个原因，和这件无关，另开一件修（pen 启动时关掉继承来的多余描述符）。
- 不加 `state_source`（用户 10-14）。
- **摸底留在用户 `~/.gemini` 里的东西，没清，等用户定**：
  - agy 信任列表里的测试目录 `/private/tmp/r8agy.Vg4v/work`；
  - 测试对话记录：摸底的 `~/.gemini/antigravity-cli/brain/dc7a4646-…`、真实一轮的 `42d1db0b-…`（两句话在同一个对话里）。

### 对 paddock 的影响（兼容新增，paddock 在自己的任务里接）
- `integration status` 多一项 agy：`hooks:"global"`，`install` 多一个 `enabled` 字段（只有 agy 有）。`install.path` 是目录。
- `integration install|uninstall --agent agy`。卸载可能带 `warnings`（目录没删掉）。
- 技能位置多了 `gemini`：Settings 照 `skills` 列表画就行；装用 `corral install-skills --target gemini --yes`。
- New Agent 可以选 agy，而且**必须带 Prompt**（和 amp 相反）。
- Settings 要讲清楚：
  - 插件装在 agy 的配置目录，只在 corral 启动的 agy 里起作用；
  - Antigravity IDE 可能也会加载，什么都不做；
  - 停用时（`enabled:false`）提示 `agy plugin enable corral`。
- 状态 `blocked` 可能来自读屏，不另外标出（不加 `state_source`）。
