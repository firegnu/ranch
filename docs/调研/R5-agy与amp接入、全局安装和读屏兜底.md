# R5 调研：agy、amp 怎么接，全局安装和读屏兜底

2026-10-14，ranch 主控。任务 `docs/任务/R5-agy与amp接入调研.md`。只调研，不含实现代码；方案经用户定了再写进 DESIGN。

标记：**【查证】** 看了文档、程序或在临时 HOME 里实测；**【推测】** 有依据但没跑通；**【未知】** 要真跑一轮（要登录）才知道。

本轮实测都在临时 HOME（`/tmp/r5agy`、`/tmp/r5amp.*`）里做，没有启动 agy、amp 的会话，没有登录，没有动用户的 `~/.gemini`、`~/.config/amp`，没有碰在跑的 agent。

## 结论先说

1. **agy 和 amp 都不能在启动时临时挂钩子，只能走「全局安装」那一档。** 两家都有干净的「corral 自己一块」的位置：agy 是插件目录 `~/.gemini/config/plugins/corral/`，amp 是插件文件 `~/.config/amp/plugins/corral.ts`。装、卸只动这一块，不用改别人的文件。
2. **amp 能报全：** 工作中、空闲、等批准、回复原文、出错／取消都有。缺的只是首句不能从启动参数带进去，需要 corral 在启动后往终端里打字。
3. **agy 能报：** 工作中、空闲、会话 ID，回复要从 transcript 文件读（格式未知）。**等批准／等回答没有钩子**，要靠读屏补。
4. **全局钩子的「不是 corral 启动的就什么都不做」现成就有：** `corral __hook` 读不到 `CORRAL_EVENTS` 时直接返回。但有个坑要防：corral 管的 agent 里如果又跑了一个 agy／amp（比如 claude 用 shell 调了 `agy -p`），子进程会继承 `CORRAL_EVENTS`，把事件写进父 agent 的事件文件。要加一道「种类对得上才写」的检查（见三）。
5. **读屏兜底：** 建议放在 pen 里，用 `vt100` 库维护一份屏幕。规则按家写在适配器里，钩子优先，读屏只补钩子报不出的状态，状态里标来源。第一个用户是 agy 的「等批准」。
6. **建议的次序：** ① 全局安装框架 + agy 的钩子 → ② amp 插件 → ③ 读屏兜底（先给 agy 的等批准用）。理由和可调的地方见五。

## 一、agy（1.3.5，`~/.local/bin/agy`）

agy 是 Google Antigravity 的 CLI，Go 写的（程序里的源码路径是 `third_party/jetski/…`），**不是** `../gemini-cli`（TypeScript）的构建产物【推测，依据：程序字符串】。

### 能不能启动时挂钩子

- `agy --help` 没有指定配置目录、插件目录、钩子文件的参数【查证】。
- 程序字符串里**没有** `ANTIGRAVITY_CLI_CONFIG_DIR`，herdr 文档提的这个环境变量在 1.3.5 里找不到【查证：`strings` 无此串】。
- 结论：只能装进全局。可选的位置有：
  - `~/.gemini/config/hooks.json`：全局钩子文件，和用户自己的钩子、herdr 的钩子共用一个文件。改它等于改别人的文件。
  - `.agents/hooks.json`：项目目录，要先信任目录，而且等于往用户仓库写文件，不用。
  - **插件目录 `~/.gemini/config/plugins/corral/`**（`plugin.json` + `hooks.json`），**建议用这个**。

### 插件那条路【查证，临时 HOME 实测】

- 目录里一个 `plugin.json`（`{"name":"corral"}`）加一个 `hooks.json`，`agy plugin validate` 通过，显示 hooks 1 processed。
- `agy plugin install <目录>` 会把它拷到 `~/.gemini/config/plugins/corral/`，并多写一个 `~/.gemini/config/import_manifest.json`。
- **不经 `agy plugin install`、直接放目录也能被发现：** 删掉 `import_manifest.json` 后 `agy plugin list` 仍列出 corral，`enabled: true`。所以 corral 自己原子写这个目录就行，不用碰 agy 的共享文件。
- 用户在 agy 里 `agy plugin disable corral` 时，agy 把开关记在 `~/.gemini/config/config.json` 的 `plugins.corral.enabled`。corral 只读这个开关，用来在 status 里报「被停用」，不去写它。
- 运行时插件的 `hooks.json` 必须放在插件根目录【查证：内置文档】。

### 钩子能报什么

依据：程序内置文档 + 网上文档【查证到「有这些事件和字段」；真实触发时机和样子【未知】】。

| corral 需要 | agy 钩子 |
|---|---|
| 会话 ID | 每个事件 stdin 都有 `conversationId` |
| 收到输入、开始干活 | `PreInvocation`（`invocationNum`），**不带输入原文** |
| 工具 | `PreToolUse`／`PostToolUse`（`toolCall{name,args}`） |
| 这一轮结束 | `Stop`（`terminationReason`、`error`、`fullyIdle`）／`PostInvocation` |
| 回复原文 | 钩子里没有，要从 `transcriptPath` 指的文件里读，**格式【未知】** |
| 等批准／等回答 | **没有事件** → 读屏 |

要注意的：

- **不能挂 `PreToolUse`**：它要求返回一个决定，而且是「失败即拒绝」，脚本崩了或超时会拦下工具调用；用 `--dangerously-skip-permissions` 时它也不触发。corral 只挂 `PreInvocation`、`PostToolUse`、`Stop`（可能加 `SessionStart`、`PostInvocation`），都是只观察的。
- 钩子 stdin 不带事件名，要放在命令参数里：`helper __hook stop`，这和现在的做法一样。
- 钩子 stdin 是驼峰字段（`conversationId`），现在的 `__hook` 只认 `session_id` 这类蛇形字段，要按 agy 做一层翻译。翻译放在 agy 适配器里，可以走 v2 原始事件那条路（像 pi／omp）。
- 钩子能不能拿到 agent 的环境变量（`CORRAL_EVENTS`）【未知】：有资料说 agy 会清洗钩子环境；但 herdr 的 agy 脚本依赖 `HERDR_*` 环境变量，所以多半能拿到【推测】。**这一条决定方案成不成立，必须真跑一轮确认。** 拿不到的退路：钩子脚本按 `conversationId` 或 `workspacePaths` 去 `~/.corral` 里找是哪个 agent，这样更复杂，也不优雅。
- 钩子要不要输出 `{}`【未知】：herdr 的脚本总输出 `{}`；corral 的 `__hook` 什么都不输出，不知道 agy 认不认空输出，稳妥起见在 agy 这家输出 `{}`。
- 钩子默认超时 30 秒，`__hook` 只追加一行文件，没有问题。

### 首句和退出

- 首句：`agy -i "<首句>"`（`--prompt-interactive`）带着首句进交互模式【查证：`--help`】，可以做成适配器的启动参数（和别家一样走 `--prompt`）。
- 退出步骤【未知】：要实测 Ctrl-C、`/quit` 哪个好使，先用默认的 HUP／TERM。
- `--conversation <id>`／`-c` 能接着上次的会话，以后做「重开」有用【查证：`--help`】。

### 其他

- 自动更新：程序里有 `cli/updater/auto_updater.go`，有 `agy update` 子命令【查证】。怎么关自动更新【未知】，和 corral 无关，只是实测时要注意。
- 遥测：程序里有 `--disable_telemetry`、`EnableTelemetry` 字样【查证：字符串】，具体怎么设【未知】。
- herdr 对 agy 只拿会话 ID（它把 agy 列为 `session_identity_only`），状态全靠读屏【查证：herdr 源码 `src/detect/mod.rs`】。corral 用钩子能拿到工作中和空闲，比 herdr 多。

## 二、amp（0.0.1791950451-g165091，Homebrew `ampcode`）

### 插件从哪加载：旧结论仍成立

- 三处：项目 `.amp/plugins/*.ts`、系统 `~/.config/amp/plugins/*.ts`、工作区设置里的「全局插件」（限量实验）【查证：`amp plugins show-docs`】。
- 没有按一次启动加载插件的参数：`--help` 只有 `--settings-file`、`--mcp-config`，没有插件路径【查证】。程序里和插件有关的环境变量只有 `AMP_PLUGIN_NAME`、`AMP_PLUGIN_URI`、`AMP_PLUGIN_SOURCE_BASE64`（插件运行时内部用）和 `AMP_DISABLE_PLUGINS`（只在开发构建里认）【查证：`strings`】。
- 所以和 9 月的旧结论（Python corral `docs/ROADMAP.md` R5）一样：要么等 amp 支持按启动加载，要么接受全局插件的例外。用户 10-14 已经放宽了全局配置（DESIGN §7 的条件），**现在可以走 `~/.config/amp/plugins/corral.ts`**。

### 插件能报什么【查证：插件 API 文档；真实触发【未知】】

| corral 需要 | amp 插件 |
|---|---|
| 会话开始、会话 ID | `session.start{thread}`，`thread.id` 形如 `T-…` |
| 收到输入并带原文 | `agent.start{thread,message,id}` |
| 工具 | `tool.call`／`tool.result{status}` |
| 这一轮结束并带回复 | `agent.end{status: done/error/cancelled, messages}` |
| 等批准 | `ctx.thread.state` 可订阅：`idle`／`running`／`awaiting-approval`／`error` |
| 子会话 | 插件进程可能同时服务多个 thread，要按父 thread 过滤 |

- `tool.call` 可以只观察：返回 `{action:'allow'}`。但 allow 是不是会**跳过** amp 自己的权限确认【未知】。稳妥的做法是不挂 `tool.call`，只用 `tool.result` 和 `thread.state`。
- **插件进程继承 amp 的环境变量【查证】**：在临时 HOME 里用 `CORRAL_PROBE=yes amp plugins exec <插件> agent.end` 跑，插件在加载时读到了 `CORRAL_PROBE=yes`。所以插件可以用 `CORRAL_EVENTS` 判断「是不是 corral 启动的」。（顺带：`plugins exec` 送 `agent.end` 时插件的处理函数没被调用，原因没查；它能用来测「插件能加载、能读环境」，测不了事件。）
- 插件是 amp 用自带的 Bun 跑的、和 amp 进程同生共死的长驻进程【查证：插件运行时日志和文档】。装成系统插件后，**用户自己开的每个 amp 也会起一个这个进程**，只是什么都不做。成本：每个 amp 多一个 Bun 进程，内存几十 MB【推测】。要写在 status 和安装确认里让用户知道。
- 插件怎么把事件交给 corral：两种写法都行。一是插件里直接追加写 `CORRAL_EVENTS`（v2 原始事件，像 pi／omp 的 `hook_pi.ts`）；二是起 `.runtime/helper __hook`。建议第一种，和 pi／omp 一样，翻译放 `events/raw.rs` + amp 适配器。

### 首句、退出、服务器

- 首句带不进去：交互模式没有首句参数，`-x` 是执行模式（跑完就退）【查证：`--help`】。corral 现在 `--prompt` 是适配器的启动参数；amp 这家要么不支持 `--prompt`，要么 pen 等它就绪后往终端打字。后者是新机制，建议先不支持。
- 退出步骤【未知】，要实测。
- 会话存在 Amp 服务器上：thread 默认上传，有 `--visibility private/…`。对 corral 没影响（corral 只看本地事件），但用户在隐私项目里用 amp 要知道。
- `amp --help` 本身在临时 HOME 里会写 `~/.cache/amp/logs/cli.log`；amp 没有找到 `XDG_CONFIG_HOME` 相关的开关，只在 shell 片段字符串里出现【查证】。corral 不靠它。

## 三、全局安装那一档（agy、amp 共用）

### 装什么、装在哪

| 家 | 装的东西 | 位置 | 只动自己那块 |
|---|---|---|---|
| agy | 目录：`plugin.json`、`hooks.json` | `~/.gemini/config/plugins/corral/` | 整个目录归 corral；不碰 `hooks.json` 全局文件、`config.json`、`import_manifest.json` |
| amp | 文件：`corral.ts` | `~/.config/amp/plugins/corral.ts` | 只这一个文件 |

- 内容由 corral 程序里带着（和技能文件一样，编进程序），里面写一个版本号（比如 `corral-integration: 1`）。版本号只在钩子内容变了时加一，不跟 corral 提交走，免得每次升级都提示「旧了」。
- 原子写：先写同目录的临时文件或目录，再 rename；agy 的目录先写 `corral.tmp-<uuid>/`，再整体换掉。
- 卸干净：删掉这一个目录或文件；如果用户在 agy 里停用过，`config.json` 里留的 `plugins.corral` 一项不归 corral，不删，在说明里写清楚。
- 不覆盖不是自己的东西：位置上已有不是 corral 写的内容（没有版本标记、或是符号链接），就报 `foreign`，`install` 不动它，和 `install-skills` 一个规矩。
- **钩子里不写死 corral 的路径。** 钩子命令只引用环境变量，让 pen 在启动时告诉它 helper 在哪：

  ```sh
  # agy 的 hooks.json 里的 command（示意，不是最终写法）
  [ -n "$CORRAL_EVENTS" ] && [ "$CORRAL_KIND" = agy ] && exec "$CORRAL_HOOK" __hook stop; echo '{}'
  ```

  - 不在 corral 里时：两个条件不满足，输出 `{}` 就走，不依赖 corral 装没装、在哪。用户卸了 corral，钩子也不会报错。
  - corral 升级、换版本目录：`CORRAL_HOOK` 由 pen 设成 `~/.corral/.runtime/helper`（它本来就随升级切换），全局文件不用动。
  - `CORRAL_KIND` 是新加的环境变量（pen 按适配器的种类设）。它挡住「corral 管的 claude 里又跑了一个 agy」的情况：子进程会继承父 agent 的 `CORRAL_EVENTS`，没有这道检查就会把事件写进父 agent 的文件。`__hook` 自己也可以再查一遍（事件文件所在 agent 的 kind）。
  - amp 的插件同理，在 TS 里判断 `process.env.CORRAL_EVENTS` 和 `CORRAL_KIND === 'amp'`，不满足就不注册任何处理函数。
- 现有四家不受影响：仍是 `launch`，不装任何全局东西。

### 命令

```
corral integration install   --agent <kind> [--dry-run | --yes]
corral integration uninstall --agent <kind> [--dry-run | --yes]
```

- 和 `install-skills` 一样：不带 `--yes` 只报要做什么，不写；`--dry-run` 明确只看。
- 只接受 `hooks` 为 `global` 的种类；对 `launch` 的种类报 `usage`。
- 输出一行 JSON，`{"ok":true,"agent":"agy","path":"…","action":"create|overwrite|remove|none"}`。

### `integration status` 怎么表示（兼容 R4 的约定）

R4 的约定已经写了「前端遇到不认识的 `hooks` 值要能显示」「以后加一家就多一项」，所以下面都是兼容的新增：

```json
{"kind":"agy","path":"/Users/u/.local/bin/agy","version":"1.3.5","hooks":"global",
 "install":{"path":"/Users/u/.gemini/config/plugins/corral","status":"same","version":1,"enabled":true},
 "screen":true,
 "skills":[…]}
```

- `hooks: "global"`：要装进全局配置才能接上；配套字段 `install`。
- `install.status`：和技能同一套词。`create`：没装；`same`：装的是这个 corral 带的版本；`overwrite`：装了但版本旧或内容被改过（前端显示 Update）；`foreign`：位置上有不归 corral 的东西，不会动。
- `install.version`：装着的版本号，读不到为 `null`。
- `install.enabled`：agy 专有，读 agy 自己的 `config.json`；用户在 agy 里停用了是 `false`。其他家没有这一项，或为 `null`。
- `screen`：这家有没有读屏规则（四、做了以后才有）。可以晚点加。
- paddock 的按钮：`create` → Install；`overwrite` → Update；`same` → Uninstall；`foreign` → 只显示说明。按下后调上面的命令，带 `--yes`，再重新调 status。
- **对 paddock 的影响：兼容的新增。** paddock 在自己的任务里接 Install／Update／Uninstall；不接的话，也只是多两行显示 `global`。

## 四、读屏兜底

### 现在的 pen【查证：`crates/corral/src/pen.rs`、`terminal.rs`】

- pen 托着 PTY，把输出存进 512 KB 的原始字节环（`pen.rs` 约 623 行），`corral` 读历史用的就是它。
- `terminal.rs::Terminal` 只跟踪标题（OSC 0/2）、几种模式（括号粘贴、kitty 键盘等），没有屏幕模型。它随 `corral upgrade` 的快照序列化。
- `status` 已经带 `title`。对 claude（`✳ …`）、amp（标题里有转圈的盲文字符、`Plugin confirmation needed`）来说，标题本身就是便宜的信号，不用屏幕模型就能用。

### 放哪：建议放 pen 里

- 读时现算（每次 `status` 把 512 KB 原始字节重放一遍）：实现简单，但每次都要重放；屏幕被清过或重绘的程序，从环里截出来的一段起点不在干净状态，会画错。
- **pen 里常驻一份屏幕（建议）**：输出来一段喂一段，开销小（只喂增量）；状态变化可以立刻写进事件；`status` 只读结果。代价是 pen 多占内存（一屏几十 KB）。
- `corral upgrade` 快照：**不带屏幕**。新 pen 起来后把环里最后一段（比如 64 KB）重放一遍重建屏幕就行，读屏是兜底，错一两秒没关系。这样快照格式不变，和旧版 pen 双向兼容。

### 终端模拟库

| 库 | 许可 | 最近更新 | 依赖 | 喂什么 | 评价 |
|---|---|---|---|---|---|
| **vt100 0.16** | MIT | 2025-07 | `vte`、`unicode-width`、`itoa` | 字节 | 用的人最多（1400 万下载），接口就是「喂字节、取屏幕文字」，正合用。**建议**。 |
| avt 0.18 | Apache-2.0 | 2026-05 | `rgb`、`unicode-width` | `&str` | asciinema 在用，更新更勤；只收字符串，pen 要自己处理被截断的 UTF-8。备选。 |
| alacritty_terminal 0.26 | Apache-2.0 | 2026-04 | 重 | 字节 | 太重，不用。 |
| termwiz 0.23 | MIT | 2025-03 | 重 | 字节 | 太重，不用。 |
| libghostty-vt 0.2 | MIT/Apache | 2026-09 | Zig 编的 C 库 | 字节 | herdr 用的，最准；但要 Zig 工具链，不符合「成熟、好装」，不用。 |

都不依赖 ratatui、crossterm【查证：crates.io 依赖列表】。

### 规则怎么写

借 herdr 的思路，不搬它的代码（Apache-2.0，ranch 不加许可证）：

- 每家一组规则，写在适配器里（Rust 常量数据），不另外下发、不联网更新（herdr 会从 herdr.dev 拉新规则，corral 不做）。
- 一条规则：状态（`blocked`／`working`／`idle`）、优先级、看哪块（标题；屏幕最下面 n 行非空的；整屏）、怎么匹配（包含哪些字、正则、都要有、不能有）。
- 例子（来自 herdr 对 agy 的规则，只作参考）：最下面几行同时出现 `navigate` 和 `tab amend` → 等批准；`esc to cancel` → 工作中；`? for shortcuts` → 空闲。
- 只在屏幕内容变了以后、停了一小会儿（比如 300 ms，herdr 也是这么做的）再算，不每字节都算。

### 钩子优先，读屏补空缺

herdr 把各家分三档：钩子说了算（pi、omp 等）、钩子只给会话 ID（agy 等）、只看屏幕【查证：herdr `src/detect/mod.rs`】。corral 的做法建议：

- 钩子报得出的状态以钩子为准；读屏只补钩子报不出的那几样。适配器写明补哪几样，比如 agy 只补 `blocked`。
- 读屏判出 `blocked` 后，下一个钩子事件（工具结果、回合结束）就把它清掉。
- 不认识的程序：没有钩子，全靠读屏，只能用通用规则（比如标题），多半还是 `unknown`。
- `status` 里加 `state_source: "hooks" | "screen"`，前端可以显示「推测」。

### 风险和成本

- **界面一改规则就失效，而且是静默失效。** 要两样东西：
  - 屏幕快照测试：存几份真实屏幕的文字（脱敏），测规则能判对。
  - 冒烟：照旧 Python corral 的 R6，升级上游后真跑一轮，看状态对不对。
- CPU：只喂增量，静止时零开销；agent 狂刷输出时多一份解析，相对 PTY 转发本身不大【推测】。
- 内存：每个 pen 一屏加少量回滚，几十到几百 KB【推测】。
- 对在跑的 agent：新 pen 才有读屏；旧 pen 照旧。`corral upgrade` 换 pen 后从环重建，不影响。

## 五、分步和建议先做哪一步

| 步 | 做什么 | 拿到什么 | 依赖 |
|---|---|---|---|
| ① | 全局安装框架（`integration install/uninstall`、status 的 `global`／`install`、`CORRAL_KIND`／`CORRAL_HOOK` 环境变量）+ agy 适配器（钩子：工作中、空闲、会话 ID；回复读 transcript） | agy 能管；paddock 能显示 Install 按钮 | **要一次真实登录的 agy**，确认钩子拿得到环境变量、transcript 格式 |
| ② | amp 适配器（系统插件，v2 原始事件） | amp 能管，状态全 | 同上，要一次真实登录的 amp |
| ③ | 读屏兜底（pen 里 vt100 屏幕 + 规则 + `state_source`），先给 agy 的 `blocked` | agy 等批准能报；以后别家也能用 | 不依赖登录，可以先做框架；规则要真实屏幕 |

建议次序 ① → ② → ③：

- 用户先提的是 agy（P5-88），而且 ① 把两家共用的安装框架定下来，② 只是再加一家。
- 也可以 ② 先做：amp 的插件能拿到全部状态，做完马上有完整效果；agy 没有读屏就缺「等批准」。如果你更急着用 amp，就 ② → ① → ③。
- ③ 和 ①② 不冲突，可以并行；但它对 pen 改动最大，单独一件做、单独部署比较稳。

每一步动手前照规矩先写任务文件给你看。

## 六、要你定的事

1. **接受这两个全局位置吗？** agy 的 `~/.gemini/config/plugins/corral/` 和 amp 的 `~/.config/amp/plugins/corral.ts`。都是经你确认才装、只动自己那块、不是 corral 启动的就什么都不做、能卸干净。amp 那个会让你自己开的每个 amp 多一个闲置的 Bun 进程。
2. **真实一轮怎么测？** 钩子的环境变量、transcript 格式、退出步骤都得登录后真跑才知道。主控建议：在临时 HOME 的测试实例（`ranch/test-*`，`--label role=test`）里你自己登录一次，主控跑完就删整个临时目录，不碰你的真实配置。agy 的登录如果存在系统钥匙串里【未知】，要先查清楚会不会碰到你的真实登录。
3. **先做哪一步？** 主控建议 ① agy → ② amp → ③ 读屏；要是更急着用 amp，就 ② 先。
4. **读屏用的库：** 建议 `vt100`（MIT，成熟，喂字节）；备选 `avt`（更新更勤，只收字符串）。
5. **amp 首句：** 先不支持 `--prompt`（启动后手动发第一句），还是要 pen 等它就绪后自动打字？主控建议先不支持。

## 附：依据

- agy：`agy --help`、`agy plugin --help`、程序字符串（内置的插件和钩子文档）；临时 HOME 里 `plugin validate`／`install`／`list`／`disable` 实测；网上的 Antigravity CLI hooks 文档。
- amp：`amp --help`、`amp plugins --help`、`amp plugins exec --help`、`amp plugins show-docs`（10-06 版）；程序字符串；临时 HOME 里 `plugins exec` 实测环境变量继承。
- herdr 0.9.4 源码（只读，GitHub v0.9.4 标签）：`src/detect/mod.rs`、`src/detect/manifests/antigravity.toml`、`amp.toml`，以及它的 agy 集成脚本。
- 旧结论：`../corral/docs/ROADMAP.md` R5（2026-09-17）。
- corral：`crates/corral/src/hooks.rs`（`CORRAL_EVENTS` 不在就返回）、`environment.rs`（pen 设的 `CORRAL_*`）、`pen.rs`（512 KB 环）、`terminal.rs`。
- crates.io：vt100、avt、alacritty_terminal、termwiz、libghostty-vt 的版本、许可、依赖。
