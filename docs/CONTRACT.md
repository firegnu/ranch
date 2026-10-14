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
  {"kind":"omp", …}
]}
```

- `agents`：corral 支持的每家 agent 一项，按 corral 的登记顺序（现在是 claude、codex、pi、omp）。以后加一家就多一项；前端照列表画，不要写死名单。
- `kind`：种类，也就是 `corral start NAME -- <程序>` 认的程序名，`status`／`ls` 里的 `kind`。
- `path`：按调用者的 PATH 找到的第一个可执行文件（只看绝对路径的目录），没找到为 `null`。从桌面启动的前端要带上登录 shell 的 PATH 再调。
- `version`：`<path> --version` 输出的第一行非空内容（去掉首尾空白）。各家并行跑，每家最多 3 秒，超时连同它起的子进程一起杀掉。没找到程序、超时、退出码非 0、没有输出时为 `null`，不影响 `ok` 和退出码。内容是各家程序自己的格式，只用来显示。
- `hooks`：corral 怎么给这家接上钩子。现在只有 `"launch"`：corral 启动 agent 时用启动参数或 agent 状态目录里的文件挂上，不写全局配置，所以没有要装、要卸的东西。以后有要装进全局配置的接法会用别的值，并配上相应的字段和命令。
- `skills`：这家 agent 会读到的、corral 能装技能的位置，按位置顺序（`claude` 在前）；一个位置可能几家共用（比如 `~/.agents/skills` Codex、pi、omp 都读）。只看用户级位置，不看项目目录；agent 自己专有、corral 不装的目录不列。
  - `target`：装这个位置要传给 `corral install-skills --target` 的值（`claude` 是 `$CLAUDE_CONFIG_DIR` 或 `~/.claude`，`codex` 是 `~/.agents`）。
  - `path`：corral 技能文件在这个位置的路径。
  - `status`：和 `corral install-skills --dry-run` 同一套判断。`create`：没装；`same`：已是这个 corral 带的最新内容；`overwrite`：有文件但内容不同（旧版或被改过），装会覆盖；`foreign`：路径上有符号链接等不归 corral 管的东西，`install-skills` 不会动它。
  - 要装或更新：用户确认后调 `corral install-skills --target <target> --yes`（同一个 target 装一次，共用它的几家都有了）。
- 读技能文件出错（比如没有权限）时整条命令按 corral 的规矩报 `ok:false`。
