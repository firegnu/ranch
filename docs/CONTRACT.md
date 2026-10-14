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
  {"kind":"amp", …}
]}
```

- `agents`：corral 支持的每家 agent 一项，按 corral 的登记顺序（现在是 claude、codex、pi、omp、amp）。以后加一家就多一项；前端照列表画，不要写死名单。
- `kind`：种类，也就是 `corral start NAME -- <程序>` 认的程序名，`status`／`ls` 里的 `kind`。
- `path`：按调用者的 PATH 找到的第一个可执行文件（只看绝对路径的目录），没找到为 `null`。从桌面启动的前端要带上登录 shell 的 PATH 再调。
- `version`：`<path> --version` 输出的第一行非空内容（去掉首尾空白）。各家并行跑，每家最多 3 秒，超时连同它起的子进程一起杀掉。没找到程序、超时、退出码非 0、没有输出时为 `null`，不影响 `ok` 和退出码。内容是各家程序自己的格式，只用来显示。
- `hooks`：corral 怎么给这家接上钩子。`"launch"`：corral 启动 agent 时用启动参数或 agent 状态目录里的文件挂上，不写全局配置，没有要装、要卸的东西。`"global"`（R6 起）：要在这家自己的配置目录里装一个文件，见下面「要装进全局配置的 agent」。以后可能还有别的值。
- `skills`：这家 agent 会读到的、corral 能装技能的位置，按位置顺序（`claude` 在前）；一个位置可能几家共用（比如 `~/.agents/skills` Codex、pi、omp 都读）。只看用户级位置，不看项目目录；agent 自己专有、corral 不装的目录不列。
  - `target`：装这个位置要传给 `corral install-skills --target` 的值（`claude` 是 `$CLAUDE_CONFIG_DIR` 或 `~/.claude`，`codex` 是 `~/.agents`）。
  - `path`：corral 技能文件在这个位置的路径。
  - `status`：和 `corral install-skills --dry-run` 同一套判断。`create`：没装；`same`：已是这个 corral 带的最新内容；`overwrite`：有文件但内容不同（旧版或被改过），装会覆盖；`foreign`：路径上有符号链接等不归 corral 管的东西，`install-skills` 不会动它。
  - 要装或更新：用户确认后调 `corral install-skills --target <target> --yes`（同一个 target 装一次，共用它的几家都有了）。
- 读技能文件出错（比如没有权限）时整条命令按 corral 的规矩报 `ok:false`。

### 要装进全局配置的 agent（R6 起：amp）

有的 agent 不能在启动时临时挂钩子，只能在它自己的配置目录里装一个文件（现在只有 amp）。这样的一项 `hooks` 是 `"global"`，多一个 `install` 字段：

```json
{"kind":"amp","path":"/opt/homebrew/bin/amp","version":"0.0.1791964847-g13f24a","hooks":"global",
 "install":{"path":"/Users/u/.config/amp/plugins/corral.ts","status":"same","version":1},
 "skills":[…]}
```

- `install.path`：corral 装的那个文件。amp 是 `$XDG_CONFIG_HOME/amp/plugins/corral.ts`（`XDG_CONFIG_HOME` 没设或不是绝对路径时用 `~/.config`）。
- `install.status`：和技能同一套词。
  - `create`：没装，前端显示 Install。
  - `same`：装的就是这个 corral 带的内容，前端显示 Uninstall。
  - `overwrite`：装的是 corral 的旧版或被改过，装会覆盖，前端显示 Update。
  - `foreign`：那里有不归 corral 的东西（同名但不是 corral 写的文件、同名目录，或者那家配置目录及以下是符号链接，amp 是 `amp`、`amp/plugins` 和文件本身；配置根目录 `~/.config` 或 `$XDG_CONFIG_HOME` 本身是链接不算），装、卸都不动它，前端只显示说明。
- `install.version`：装着的文件里写的版本号（`CORRAL_INTEGRATION = <n>`），没装或不是 corral 的为 `null`。版本号只在插件内容变了时加一，不跟 corral 的版本走。
- 装上后，只有之后启动的 agent 才会加载；已经开着的要重开。
- 装上的文件只在 corral 启动的那家 agent 里起作用（看环境变量 `CORRAL_EVENTS`、`CORRAL_KIND`，并确认它那个 agent 进程的父进程就是 `CORRAL_PEN_PID` 指的 pen），用户自己开的、别的程序里跑的、corral 管的 amp 里面再直接运行的 amp（环境是继承来的）都什么都不做。amp 的桌面版可能也会加载它，同样什么都不做；但每个加载了它的 amp 会多一个闲置的插件进程，前端要讲清楚。
- `hooks` 为 `"global"` 的 agent 不支持 `corral start --prompt`（报 `usage`），要启动后再发第一句。start 返回时插件不一定加载好了：先 `corral wait <名字>` 等到 `idle` 再 `send`。状态一直是 `starting`，可能是插件没装、没加载成功，或者 amp 是装插件之前开的（要重开）。
- amp 要用 R6 及以后的 corral 操作。旧版 corral 不认识 amp：`status` 报 `unknown`，`wait` 返回 `unknown`，`send` 不检查是否空闲。旧版 corral 启动的 amp 没有 `CORRAL_KIND` 等变量，插件不工作，新版看它一直是 `starting`；`corral upgrade` 只换 pen，补不上已经在跑的 amp 的环境，要经用户同意重新启动。这和现有四家「不升级 pen 也能用新版命令」是两回事。
- amp 的插件报不出「等批准」：corral 看 amp 设的终端标题（以 `◆ Input needed` 开头）把 `working`／`idle` 报成 `blocked`（`status`、`wait` 一样）。

## `corral integration install|uninstall`（R6）

```
corral integration install   --agent <kind> [--dry-run] [--yes]
corral integration uninstall --agent <kind> [--dry-run] [--yes]
```

- 只对 `hooks` 为 `"global"` 的 agent；对别的种类、不认识的种类、缺 `--agent`、多余参数都报 `usage`，退出码 1。
- 只写或删 `install.path` 那一个文件，别的都不动（目录不删）。写是原子的：同目录下先写临时文件再改名。
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
