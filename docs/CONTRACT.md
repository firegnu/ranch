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
