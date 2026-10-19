# corral 公开约定：ranch 的新增

corral 的基础约定（命令、参数、JSON 输出、退出码、`~/.corral` 格式）仍以原 corral 仓库 `6923da1` 的 `docs/CONTRACT.md` 为准（`corral guide` 也这么指）。这里写 ranch 在它之上新增、给前端用的部分；以前的新增（`pause`／`resume`、`paused` 字段）在 `docs/DESIGN.md` §6 和 `crates/corral/README.md`。

前端读这里写的字段，遇到没写的字段要忽略，遇到不认识的取值（比如 `hooks`、`status` 的新值）要能显示、不报错。

## `corral integration status`（R4）

只读：不写任何文件，不碰在跑的 agent；只会对找到的程序各跑一次 `--version`。要等各家的 `--version`，一般一两秒、最多三秒多：在打开设置页、用户刷新或装完技能后调，不要放进每秒一次的刷新。和其他命令一样只输出一行 JSON，没有 `--json` 参数。多余的参数、其他子命令报 `usage`，退出码 1。

```json
{"ok":true,"agents":[
  {"kind":"claude","path":"/Users/u/.local/bin/claude","version":"2.1.299 (Claude Code)","hooks":"launch",
   "skills":[{"target":"claude","path":"/Users/u/.claude/skills/corral/SKILL.md","status":"same"}]},
  {"kind":"codex","path":null,"version":null,"hooks":"launch",
   "skills":[{"target":"codex","path":"/Users/u/.agents/skills/corral/SKILL.md","status":"create"}]},
  {"kind":"pi", …},
  {"kind":"omp", …},
  {"kind":"amp", …},
  {"kind":"agy", …}
]}
```

- `agents`：corral 支持的每家 agent 一项，按 corral 的登记顺序（现在是 claude、codex、pi、omp、amp、agy）。以后加一家就多一项；前端照列表画，不要写死名单。
- `kind`：种类，也就是 `corral start NAME -- <程序>` 认的程序名，`status`／`ls` 里的 `kind`。
- `path`：按调用者的 PATH 找到的第一个可执行文件（只看绝对路径的目录），没找到为 `null`。从桌面启动的前端要带上登录 shell 的 PATH 再调。
- `version`：`<path> --version` 输出的第一行非空内容（去掉首尾空白）。各家并行跑，每家最多 3 秒，超时连同它起的子进程一起杀掉。没找到程序、超时、退出码非 0、没有输出时为 `null`，不影响 `ok` 和退出码。内容是各家程序自己的格式，只用来显示。
- `hooks`：corral 怎么给这家接上钩子。`"launch"`：corral 启动 agent 时用启动参数或 agent 状态目录里的文件挂上，不写全局配置，没有要装、要卸的东西。`"global"`（R6 起）：要在这家自己的配置目录里装一个文件或目录，见下面「要装进全局配置的 agent」。以后可能还有别的值。
- `skills`：这家 agent 会读到的、corral 能装技能的位置，按位置顺序（`claude` 在前）；一个位置可能几家共用（比如 `~/.agents/skills` Codex、pi、omp 都读）。只看用户级位置，不看项目目录；agent 自己专有、corral 不装的目录不列。
  - `target`：装这个位置要传给 `corral install-skills --target` 的值（`claude` 是 `$CLAUDE_CONFIG_DIR` 或 `~/.claude`，`codex` 是 `~/.agents`，`gemini` 是 `~/.gemini/config`，R8 起，agy 只从这里读用户级技能）。`ranch dispatch install-skills --target` 认同样的值。
  - `path`：corral 技能文件在这个位置的路径。
  - `status`：和 `corral install-skills --dry-run` 同一套判断。`create`：没装；`same`：已是这个 corral 带的最新内容；`overwrite`：有文件但内容不同（旧版或被改过），装会覆盖；`foreign`：路径上有符号链接等不归 corral 管的东西，`install-skills` 不会动它。
  - 要装或更新：用户确认后调 `corral install-skills --target <target> --yes`（同一个 target 装一次，共用它的几家都有了）。
- 读技能文件出错（比如没有权限）时整条命令按 corral 的规矩报 `ok:false`。

### 要装进全局配置的 agent（R6 起：amp；R8 起：agy）

有的 agent 不能在启动时临时挂钩子，只能在它自己的配置目录里装一个文件或一个目录（现在是 amp 和 agy）。这样的一项 `hooks` 是 `"global"`，多一个 `install` 字段：

```json
{"kind":"amp","path":"/opt/homebrew/bin/amp","version":"0.0.1791964847-g13f24a","hooks":"global",
 "install":{"path":"/Users/u/.config/amp/plugins/corral.ts","status":"same","version":1},
 "skills":[…]}
{"kind":"agy","path":"/Users/u/.local/bin/agy","version":"1.3.5","hooks":"global",
 "install":{"path":"/Users/u/.gemini/config/plugins/corral","status":"same","version":1,"enabled":true},
 "skills":[{"target":"gemini","path":"/Users/u/.gemini/config/skills/corral/SKILL.md","status":"same"}]}
```

- `install.path`：corral 装的那个文件或目录。
  - amp 是文件 `$XDG_CONFIG_HOME/amp/plugins/corral.ts`（`XDG_CONFIG_HOME` 没设或不是绝对路径时用 `~/.config`）。
  - agy 是插件目录 `~/.gemini/config/plugins/corral`，里面只有 corral 的 `hooks.json`、`plugin.json`。
- `install.status`：和技能同一套词。
  - `create`：没装，前端显示 Install。
  - `same`：装的就是这个 corral 带的内容，前端显示 Uninstall。
  - `overwrite`：装的是 corral 的旧版或被改过，装会覆盖，前端显示 Update。
  - `foreign`：那里有不归 corral 的东西，装、卸都不动它，前端只显示说明。包括：
    - 同名但不是 corral 写的文件（没有版本号）；
    - 该是文件的地方是目录，或该是目录的地方是文件；
    - agy 的插件目录里有 corral 那两个文件以外的东西（corral 写到一半留下的临时文件 `.corral-*.tmp` 不算，卸载时一起删）；
    - 那家配置目录及以下有符号链接：amp 是 `amp`、`amp/plugins` 和文件本身，agy 是 `~/.gemini/config`、`plugins`、`plugins/corral` 和里面的文件。再往上的目录（`~/.config`、`$XDG_CONFIG_HOME`、`~/.gemini`）本身是链接不算。
- `install.version`：装着的文件里写的版本号（`CORRAL_INTEGRATION = <n>`），没装或不是 corral 的为 `null`。目录里有几个文件时取第一个在的文件里的。版本号只在插件内容变了时加一，不跟 corral 的版本走。
- `install.enabled`（R8 起，只有 agy 有）：这家 agent 自己是不是开着 corral 的插件。
  - `false`：用户在 agy 里停用了它（`agy plugin disable corral`，记在 agy 的 `~/.gemini/config/config.json`）。这时装着也不起作用，corral 启动的 agy 会一直是 `starting`。前端要显示「已停用」，告诉用户用 `agy plugin enable corral` 重新启用。corral 不写 agy 的 `config.json`。
  - `true`：没停用（没装时也是 `true`）。
  - `null`：读不了 agy 的 `config.json`，不知道。
  - 别的 agent 没有这个字段，前端不要当成 `false`。
- 装上后，只有之后启动的 agent 才会加载；已经开着的要重开。
- 装上的文件只在 corral 启动的那家 agent 里起作用：要看环境变量 `CORRAL_EVENTS`、`CORRAL_KIND`，并确认它那个 agent 进程的父进程就是 `CORRAL_PEN_PID` 指的 pen。以下情况都什么都不做：
  - 用户自己开的；
  - 别的程序里跑的；
  - 在 corral 管的 amp／agy 里面再直接运行的同种 agent（环境是继承来的）。
- 也会加载这个插件的其他程序：
  - amp 的桌面版可能也会加载它，同样什么都不做；但每个加载了它的 amp 会多一个闲置的插件进程，前端要讲清楚。
  - agy 的插件是给 agy 跑的几条 shell 命令，不常驻进程。不在 corral 里时，每个事件只多跑一次 `sh` 并回 `{}`，agy 照常往下走。Antigravity IDE 和 agy 共用 `~/.gemini/config`，会不会加载这个插件没有核对（ranch 不碰 IDE）；加载了也一样什么都不记。
- `corral start --prompt`：
  - amp 不支持（报 `usage`），要启动后再发第一句。start 返回时插件不一定加载好了：先 `corral wait <名字>` 等到 `idle` 再 `send`。状态一直是 `starting`，可能是插件没装、没加载成功，或者 amp 是装插件之前开的（要重开）。
  - agy 支持，而且**必须带**：agy 在收到第一句话之前不触发任何钩子，不带 `--prompt` 启动的 agy 一直是 `starting`，`send` 报 `not_idle`。corral 用一个参数 `--prompt-interactive=<首句>` 传给 agy。带了首句还一直是 `starting`，可能是：
    - 插件没装或被停用（看 `install.status`、`install.enabled`）；
    - agy 是装插件之前开的；
    - agy 停在「是否信任这个目录」或登录的对话框上（要用户 attach 去点）。R13 起，工作目录的上级目录已被用户信任时，corral 启动 agy 前会把工作目录加进 agy 的信任列表（`~/.gemini/antigravity-cli/settings.json` 的 `trustedWorkspaces`），agy 退出后删掉，这时不再弹信任框；上级目录都没被信任的照旧弹。corral 自己加的记在 `~/.corral/.runtime/trust.json`，只删这些；例外是 agy 还开着时用户对同一目录取消再重新信任，退出时也会删掉（列表只存路径，分不出来）。退回到 R13 之前的 corral 时，它开的 agy 不加也不删；R13 的 pen 换成旧版后，agy 退出时不删，留到之后新版下次加条目时清掉。
  - R13 起，agy 的「会话开始」丢了（agy 登录慢时首句抢在钩子挂上之前送出）也不会一直 `starting`：钩子恢复后，corral 把 agy 报来的会话当主会话，状态照常变化。钩子缺席的那一段仍是 `starting`。
- 版本要求：amp 要用 R6 及以后的 corral 操作，agy 要用 R8 及以后的。
  - 旧版 corral 不认识它们：`status` 报 `unknown`，`wait` 返回 `unknown`，`send` 不检查是否空闲。
  - 旧版 corral 启动的 amp／agy 没有 `CORRAL_KIND` 等变量，插件不工作，新版看它一直是 `starting`。
  - `corral upgrade` 只换 pen，补不上已经在跑的 agent 的环境，要经用户同意重新启动。这和现有四家「不升级 pen 也能用新版命令」是两回事。
  - agy 的钩子不依赖共享 helper 先切到新版：部署新版 corral 后，不用先 `corral upgrade` 就能开 agy（那个 agy 的钩子用启动它的那份 corral）。
- 插件报不出「等批准」时，corral 看终端，在钩子报 `working`／`idle` 时报成 `blocked`（`status`、`wait` 一样）：
  - amp 看它设的终端标题（以 `◆ Input needed` 开头）。
  - agy 不设标题，corral 读屏：屏幕最下面 4 行非空的里同时有 `↑/↓ Navigate` 和 `tab Amend` 这两个按键提示时，就是在等批准（agy 的批准框最下面是 `↑/↓ Navigate · tab Amend · …`；回复正文里只出现这两个词不算）。
  - 每次按当前屏幕现算，批准框没了就回到钩子的状态。信任目录、登录这类对话框不算。
  - 不说明状态是钩子报的还是读屏推测的（用户 10-14 定，不加 `state_source`）。

## `corral integration install|uninstall`（R6）

```
corral integration install   --agent <kind> [--dry-run] [--yes]
corral integration uninstall --agent <kind> [--dry-run] [--yes]
```

- 只对 `hooks` 为 `"global"` 的 agent；对别的种类、不认识的种类、缺 `--agent`、多余参数都报 `usage`，退出码 1。
- 只写或删 `install.path` 那一个文件，或者那个目录和里面 corral 的文件，别的都不动（上一级目录不删）。每个文件的写都是原子的：同目录下先写临时文件再改名。目录里有几个文件时按次序写，最后写让 agent 认出插件的那个（agy 是 `plugin.json`），卸的时候它先删。计划里没有的文件只新建、不覆盖：判断之后那里冒出了别人的文件，就报错、不动它（已经写好的 corral 文件留着，再判断时整个目录算 `foreign`）。卸载删不掉目录时（比如期间有人放进了别的文件），`written` 仍为 `true`，`warnings` 里说明目录留下了。
- `--dry-run`：只报会怎样，不写。
- 不带 `--yes`：终端里会问用户；不在终端里（前端调）报 `confirmation_required`，不写。前端要先让用户确认，再带 `--yes` 调。
- 输出一行 JSON：

```json
{"ok":true,"action":"install","agent":"amp","path":"/Users/u/.config/amp/plugins/corral.ts",
 "status":"create","dry_run":false,"written":true,"warnings":[]}
```

  - `action`：`install` 或 `uninstall`。
  - `status`：做之前的状态。装：`create`／`same`／`overwrite`／`foreign`；卸：`remove`（是 corral 的，会删）／`absent`（没有）／`foreign`（不是 corral 的，不动）。
  - `written`：这次有没有真的写或删。`same`、`absent`、`foreign` 和 `--dry-run` 都是 `false`。
  - `warnings`：`foreign` 时说明那里的东西不归 corral，没动它。
- 做完后再调一次 `integration status` 刷新显示。

## sandbox 与 `fence`（R17b）

这些都是兼容新增。paddock 当前用法不用改；P5-120 接入档案选择、卡片小锁、拒绝记录及 Settings 规则展示。前端通过 PATH 找 `fence`；远端先 `command -v fence`，再试 `~/.local/bin/fence`，检查 `--version` 的 `contract`。未安装时不提供 sandbox 选项。

### `corral start NAME [--sandbox <档案> | --no-sandbox] … -- <命令>`

加 `--sandbox` 就通过 fence 起 agent；没设子 agent 档案时，不加参数保持原行为，派活技能不默认加。R18 起也可继承调用者当下的子 agent 档案，或用 `--no-sandbox` 让这一次不套墙（见下节）。corral 不依赖 fence 包，只调用以下命令：

```text
fence check --profile <档案> --kind <原种类> --cwd <目录>
fence run --profile <档案> --kind <原种类> --agent <名字> --instance <实例>
          [--read <路径>]… [--write <路径>]… -- <含钩子与首句的原命令>
```

先用绝对路径的 `CORRAL_FENCE`，否则用 corral 真实程序所在目录的 `fence`；CLI 检查成功后把该路径传给 pen。fence 不认识 corral 的状态目录，所需管道和 helper 只经 `--read/--write` 传入。

`start`、`status`、`ls` 中每个 agent、`where` 增加 `sandbox:{"profile":"dev"}`；未套墙或旧格式记录为 `null`。`meta.json` 增加 `sandbox:{"profile":"dev","fence":"/absolute/path/fence"}`，`argv` 仍是原来的 agent 启动参数，不含 fence 包装。原种类识别、钩子与首句不变；stop/pause/resume 沿用 pen 的进程管理。

新增启动错误（退出码 1，均不起 pen）：

- `sandbox_unavailable`：找不到或无法执行 fence，或 `CORRAL_FENCE` 非绝对路径。
- `sandbox_failed`：fence 检查未同时返回退出码 0 与 `ok:true`；`fence` 字段携带其原始 JSON 错误。

墙内继承 `FENCE_RUN`，再调 corral 用户命令报 `sandbox`，退出码 6，message 为 `running inside fence`；`__hook`、`guide` 仍可用。因此这个阶段墙内 agent 不能再派活。

检查与运行之间条件变化、srt 初始化失败时，start 可能已经返回成功；随后 agent 停止，退出码 125，原因在终端中，可用 `corral read` 读取。fence 不会回退成无 sandbox 运行。runner 正常结束时透传原程序的退出码或信号；runner 意外先死时 pen 清理剩余进程组。

### `corral child-sandbox NAME [<档案> | --clear]`（R18）

给正在运行的 NAME 设定它之后通过 `corral start` 派出去的 agent 用哪个 fence 档案。随时可改，从下一次 start 起生效；不改变 NAME 自己或已经运行的子 agent。默认没有设定。corral 自己保存设置，不依赖任何前端或技能指令。

- `corral child-sandbox NAME <档案>`：先执行一次 `fence check --profile <档案>`（不带种类），成功才保存。fence 的定位和检查成功条件与 `start --sandbox` 一致；失败报 `sandbox_failed`、找不到或无法执行报 `sandbox_unavailable`，退出码 1，原设置不变。
- `corral child-sandbox NAME --clear`：清除设定，不需要 fence。
- `corral child-sandbox NAME`：只查，不需要 fence。

三种操作成功都输出一行 JSON、退出码 0：

```json
{"ok":true,"name":"ranch/main","instance":"0123abcd4567","child_sandbox":{"profile":"dev"}}
```

没有设定时 `child_sandbox:null`。NAME 不在运行时三种操作都报 `not_found`、退出码 2。档案与 `--clear` 同时给、多余或缺少参数报 `usage`、退出码 1。

`status` 和 `ls` 的每个 agent 增加 `child_sandbox`，同样为 `{"profile":"dev"}` 或 `null`。没设、旧记录没有这个文件、记录的 instance 对不上、记录读不了或格式坏时，查询显示 `null`；启动时对坏记录的处理更严格，见下文。

设置保存在 agent 状态目录的 `child-sandbox.json`，原子写入，内容为 `{"instance":"0123abcd4567","child_sandbox":{"profile":"dev"}}`；清除写成同 instance 的 `child_sandbox:null`。只认同一次运行，不向 pen 增加请求；旧 pen 管的 agent 也可设定。文件列入状态目录的清理清单，agent 退出后由新版 `ls` 随目录清理。

`corral start` 的优先次序：

1. 同时带 `--sandbox <档案>` 与 `--no-sandbox`，报 `usage`（退出码 1），不起 agent。
2. 显式 `--sandbox <档案>` 使用指定档案；显式 `--no-sandbox` 这次不套墙。两者都会跳过调用者的设置读取。
3. 否则，以调用环境中的 `CORRAL_NAME`、`CORRAL_INSTANCE` 识别调用者，确认它仍在运行且 instance 相同，读取它当下的 `child-sandbox.json`。记录 instance 匹配且设了档案，等同 `--sandbox <档案>`，照常再次执行带种类、工作目录的 fence 检查，错误码也相同。
4. 没有这两个环境变量、调用者不在跑、instance 不匹配、没有文件或已清除，都保持原行为，不额外失败。

确认调用者仍是同一次运行后，文件在但读不了、JSON 无法解析、没有有效 instance，或匹配 instance 的设置格式错误，报 `child_sandbox_unreadable`（退出码 1），不起 agent，不当作不套墙。能读出不同 instance 的旧记录直接忽略。坏记录可用设定或清除命令覆盖。

`start` 输出和之后 `status` 的 `sandbox` 仍表示这个 agent 实际使用的档案，不另加是否继承的字段。`FENCE_RUN` 防护不变，墙内也不能靠 `--no-sandbox` 再派活。

兼容新增：旧 CLI 不认识 `child-sandbox` 命令和 `--no-sandbox`，也不读取这个文件；用旧 CLI 发起的 `start` 不会继承档案。旧 CLI 的 `ls` 不清理这个文件，依靠 instance 不匹配防止同名重开后误用，留待下次新版 CLI 清理。要让设定生效，派活所用的 CLI 必须是 R18 或更新版本，无需升级父 agent 的 pen。

对 paddock：P5-141 调用 `corral child-sandbox`，读取 `ls` 的 `child_sandbox` 做勾选；paddock 现有调用保持兼容，在自己的任务中接入。

### fence 输出约定

前端命令均只输出一行 JSON；成功 `ok:true`、退出码 0；失败 `ok:false`、`error`、`message`、退出码 1。忽略未知字段、显示未知取值。内部 `run` 不属于前端命令，启动失败向 stderr 输出、退出码 125；启动后的终端与退出状态属于 agent。

`fence --version`：

```json
{"ok":true,"version":"0.1.0","contract":"1","srt":"0.0.80","node":"v24.13.1"}
```

`node` 是打包时记录的版本；尚未打包、没有运行时记录时为 `null`，能否运行交给 `check`。

`fence profiles`：只读，列出档案、种类数据和模型服务。

```json
{"ok":true,"profiles":[{"name":"dev","description":"在工作目录里改代码、编译、测试、提交；HOME 默认不可读。Codex 在 macOS 开启 trustd，会削弱网络隔离","source":"builtin","path":null,"overrides_builtin":false}],"kinds":[{"kind":"claude","source":"builtin","services":["anthropic"]},{"kind":"codex","source":"builtin","services":["openai"]},{"kind":"omp","source":"builtin","services":[]},{"kind":"pi","source":"builtin","services":[]}],"services":[{"name":"anthropic","source":"builtin"},{"name":"deepseek","source":"builtin"},{"name":"openai","source":"builtin"}],"local":{"path":"/Users/u/.fence/local.toml","exists":false}}
```

`source` 为 `builtin` 或 `user`。用户同名文件整份替换内置档案，`path` 为该路径、`overrides_builtin:true`。读不了或数据格式错误仍列出该项，多一个 `error` 对象；整条命令仍为 `ok:true`。

`fence check --profile <档案> [--kind <种类>] [--cwd <目录>]`：只读，不创建运行记录或 mkdir 目录。cwd 默认当前目录；不带 kind 只检查档案和本机补充；带 kind 再合并种类、模型服务与工作区 `.fence.toml`。检查程序、固定 node/srt 和 Linux 的 bwrap/socat/rg。

```json
{"ok":true,"profile":"dev","kind":"codex","platform":"macos","cwd":"/Users/u/w/x","layers":[{"source":"profile","name":"dev","origin":"builtin"},{"source":"agent","name":"codex","origin":"builtin"},{"source":"service","name":"openai","origin":"builtin"},{"source":"repo","path":"/Users/u/w/x/.fence.toml"}],"settings":{"network":{"allowedDomains":["chatgpt.com"],"deniedDomains":[]},"filesystem":{"denyRead":["/Users/u"],"allowRead":["/Users/u/w/x"],"allowWrite":["/Users/u/w/x"],"denyWrite":["/Users/u/w/x/.fence.toml","/Users/u/.fence"]},"allowPty":true,"enableWeakerNetworkIsolation":true},"env":{"TMPDIR":"<运行时才有>"},"args":["-c","projects={\"/Users/u/w/x\"={trust_level=\"trusted\"}}"],"skipped":[],"warnings":["weaker_network_isolation: 削弱网络隔离（com.apple.trustd.agent）"],"runtime":{"node":{"path":"/Users/u/.nvm/versions/node/v24.13.1/bin/node","version":"v24.13.1","ok":true},"srt":{"version":"0.0.80","ok":true}}}
```

示例的路径和域名列表已缩短。`settings` 为实际合并规则；运行时专有的 `${tmp}` 显示为 `<运行时才有>`。`skipped:[{entry,reason}]` 列出缺值或不存在的读路径，`warnings` 列出环境覆盖与平台差异。Codex 默认 trustd 规则会削弱 macOS 网络隔离。

错误码：`usage`、`unknown_profile`、`invalid_data`（附 `problems:[{source,path,message}]`）、`unsupported_kind`（附 `supported:[…]`）、`no_service`、`program_not_found`、`bad_cwd`、`runtime_missing`、`not_found`。amp/agy 本轮无数据，使用 sandbox 时返回 `unsupported_kind`。

`check` 在合并时校验域名和可选端口，错误返回 `invalid_data`，附规则来源和 `domains[索引]`；`run` 复用同一校验，在创建运行记录和启动程序前失败。允许 `*.example.com`、`example.com:443`、`[::1]`、`[2001:db8::1]:443` 等 srt 格式；端口为 1–65535，不带前导零。

`fence denials --agent <名字> [--instance <实例>] [--limit <n>]`：只读，未给实例时取该名字最近一次运行；前端宜传 `corral status` 的 instance，避免重名混淆。limit 默认 200，取最后 n 条并保持时间顺序；没有匹配记录返回 `not_found`。

```json
{"ok":true,"agent":"ranch/dev-x","instance":"0123abcd4567","profile":"dev","kind":"codex","started":1760760000.1,"running":false,"exit":{"code":0,"signal":null,"t":1760760200.1},"monitors":{"network":true,"files":true},"denials":[{"t":1760760123.4,"type":"network","target":"example.com:443","detail":"deny network-outbound example.com:443 (not on the allow list)"}],"truncated":false}
```

类型为 `network`、`file-read`、`file-write`、`other`；不认识目标时 `target:null`。running 表示 runner 尚在运行；exit 来自 runner 的退出记录，没有则为 null。网络记录来自 srt 代理，macOS 文件记录来自系统日志，Linux 文件记录是 seccomp 对写尝试的诊断提示，路径可能不准且没有读记录。srt 没有监视器健康接口，因此 `files` 只有收到文件事件后才为 true；false 表示尚未证实能记录，文件拒绝还可看 agent 终端。

Linux 的 `deny <syscall> <路径>` 事件归为 `file-write`，`target` 为上游报告的路径，收到后同样置 `monitors.files:true`；不因此把写尝试诊断视为准确的文件访问审计。

记录在 `$FENCE_HOME/runs/<实例>/`（默认 `~/.fence/runs/`），由墙外的 fence/runner 写：`run.json`、0600 的 `settings.json`、`denials.jsonl`、`monitor.json`、`exit.json`。拒绝日志最多 5 MiB，满了停写并报 `truncated:true`；不保存完整 agent 命令或首句。`--limit` 仅限制返回条数，不改变 truncated。文件事件可能漏记，`files:true` 只证明收到了文件事件，不保证覆盖每次拒绝；`truncated:false` 也不代表上游日志没有遗漏。

临时目录在 `${XDG_CACHE_HOME:-~/.cache}/fence/tmp/<实例>`，与记录分开，起 agent 前创建并按档案放行读写。每次启动清理已结束超过 7 天的记录和它对应的临时目录。整个 `$FENCE_HOME` 及工作区 `.fence.toml` 始终禁止写。

这些强制保护路径以及数据层所有 `deny_write` 同时保护原路径、解析途中每一个符号链接节点和最终真实目标，包含中间目录组件及多跳链接，防止通过替换中间链接更换规则。末级路径尚不存在时，解析已有父目录后保留不存在的后缀；链接自身的目标必须存在，断链、循环或其他解析错误返回 `invalid_data`，拒绝启动。

新 CLI 读旧 metadata 得到 sandbox:null；旧 CLI 忽略新增字段，但不能使用 `--sandbox`。新旧 pen 升级快照原样携带 metadata，runner 不随 pen 升级切换版本；仍有 runner 使用的旧版本目录不能清理。
