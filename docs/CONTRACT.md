# corral 公开约定：ranch 的新增

corral 的基础约定（命令、参数、JSON 输出、退出码、`~/.corral` 格式）仍以原 corral 仓库 `6923da1` 的 `docs/CONTRACT.md` 为准（`corral guide` 也这么指）。这里写 ranch 在它之上新增、给前端用的部分；以前的新增（`pause`／`resume`、`paused` 字段）在 `docs/DESIGN.md` §6 和 `crates/corral/README.md`。

前端读这里写的字段，遇到没写的字段要忽略，遇到不认识的取值（比如 `hooks`、`status` 的新值）要能显示、不报错。

## `corral terminal-colors [--fg COLOR --bg COLOR | --clear]`（R50）

设定本机新开的 agent 在没有客户端连着时，corral 拿什么颜色回答终端前景色、背景色查询（OSC 10、OSC 11）。

- `corral terminal-colors --fg '#4c4f69' --bg '#eff1f5'`：设定。COLOR 是 `#rrggbb`，大小写都行，输出一律小写；`--fg`、`--bg` 要一起给。
- `corral terminal-colors --clear`：清除。
- `corral terminal-colors`：只查。

成功都输出一行 JSON、退出码 0：

```json
{"ok":true,"terminal_colors":{"fg":"#4c4f69","bg":"#eff1f5"}}
```

没有设定时 `terminal_colors` 为 `null`。颜色格式不对、只给一个颜色、`--clear` 和颜色同时给，报 `usage`、退出码 1，原设置不变。设置存在 `$CORRAL_HOME/.runtime/terminal-colors.json`，写入是原子的。

只查时，设置文件的 JSON 或颜色内容无效，返回 `ok:false`、`error:"invalid_settings"`、退出码 1，message 包含文件路径和原因，原文件不变；这不表示命令不受支持。

从下一次 `corral start` 起生效，不管是谁调的（paddock、派活的 agent、终端里的用户），已经在跑的 agent 不变。start 起 pen 前只读一次；设置文件读不了或格式坏，当作没设置，启动照常成功，在原有 `warnings` 中提示。pen.log 记录本次颜色，不给 start、status、ls 增加颜色字段。

agent 启动后，pen 解析出 OSC 10/11 单色查询的那一刻，没有客户端 attach 且有颜色时才回答；BEL 结尾用 BEL 回，ST 结尾用 ST 回，格式如 `ESC ] 11 ; rgb:efef/f1f1/f5f5 ST`。有客户端 attach 就照旧由客户端回答，客户端不答时不补答。设置颜色的 OSC、多色连问及其他终端查询一概不答；没有设定时行为和以前一样。回答不受 send 的块间延时影响，也不插入正在写的块，不算人在打字。

`corral upgrade` 保留启动时颜色、待写回答和未完成的查询，不重新读取本机设置。快照 SCHEMA 不变：旧版升级到新版的 agent 没有颜色，继续不答；新版退回旧版时旧版忽略新增字段、不再回答。socket、attach、现有命令参数和 JSON 格式不变；旧 CLI 与新 pen、新 CLI 与旧 pen 可混用。

对 paddock：兼容新增，接入在 paddock 自己的任务做。

1. 启动时、终端主题变化时（包括跟随系统深浅变化），调用 `corral terminal-colors --fg <主题前景色> --bg <主题背景色>`；取回答 OSC 10/11 的 foreground、background，格式 `#rrggbb`。
2. 在远端机器开 agent 前，对那台机器的 corral 也调一次；每台机器各存一份。
3. 旧版不认识此命令，返回 `usage`、退出码 1；前端当作不支持，照常开 agent。
4. start、attach 等现有调用不用改。可在 Settings 说明已经在跑的 Codex 不跟随主题变化，要重开。

主控派活的 agent 读取同一份设置，不用改派活技能。本机从没设置过，就不答，和以前一样。

## `ranch dispatch cell` / `table`（R31）

只读，不调用路由服务、不启动 agent、不写文件。每次重新读取 `$HOME/.ranch/dispatch.toml`，不存在则用编入 ranch 的默认表。成功 stdout 一行 JSON、`ok:true`、退出码 0；数据错误退出码 1，`{"ok":false,"error":"invalid_data","message":"<文件路径>: <位置和原因>"}`；未知格子同样退出码 1，`error:"unknown_cell"`。命令参数错误沿用 ranch 的 `usage`、退出码 2。

`ranch dispatch cell <名字>`：

```json
{"ok":true,"cell":{"name":"middle-other","source":"builtin","agents":["codex"],"model":"gpt-6-astra","effort":"high","cross_review":null,"rework":"high-other","choices":[{"agent":"codex","source":"builtin","argv":["codex","--yolo","--disable","memories","-m","gpt-6-astra","-c","model_reasoning_effort=\"high\""],"command":"codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"high\"'","labels":["effort=high","model=gpt-6-astra"]}],"review":null}}
```

- `agents` 是可任选的家，按数据顺序返回；不自动挑选。`model`、`effort` 为字符串或 `null`（各家默认）。
- `choices` 每家一项，`argv` 是完整 agent 启动参数数组，`command` 是等价、已作 POSIX shell 引用的命令字符串（引用写法不是固定协议）；不含 corral 外层的名字、cwd、首句或环境参数。标签是 `key=value` 字符串数组，每项各传一次 corral 的 `--label`，空数组不传。
- `cross_review` 是审查格子名或 `null`。非空时 `review` 为 `{name,source,model,effort,choices}`，提供该格的完整启动信息，只展开一层；没有则 `review:null`。要继续读该格的其他关系，用其名字再调 `cell`。
- `rework` 是返工格子名或 `null`，只提供信息，不自动启动或升档；现有默认低档指向同类别中档、中档指向同类别高档，高和碰要害为空。技能要求每档返工一次再不过就按表升档，没有升档格子的返工一次再不过就停下问用户。
- `source` 为 `builtin` / `user`，表示该格定义是否被本机文件提及（含局部覆盖）；`choices[].source` 表示所引用家模板的来源。默认格子引用了本机修改的家时，前者仍为 `builtin`，后者为 `user`。

`ranch dispatch table`：`{ok:true, local:{path,exists}, agents:{<家名>:{source,argv,labels}}, cells:{<格子名>:<与 cell 命令相同的对象>}}`。`agents` 返回未展开的模板，`cells` 返回生效值与启动信息；名字不枚举成固定协议。

R48d：家模板增加可选 `env`（`KEY=VALUE` 字符串数组），在 `table.agents.<家>`、`cell` / `table` 的 `choices[]`（含 `review.choices[]`）原样给出；未定义时字段缺省，旧家的行为和输出不变。每项用第一个 `=` 分隔，键非空，键和值不含 NUL；值可为空、含空格或 `=`，不做模板或 shell 展开，格式错误报 `invalid_data`。调用方每项各传一个 corral `--env` 参数；`argv` / `command` 仍只含 agent 命令，不内嵌环境变量。本机表按字段覆盖，`env` 整个数组替换，`env = []` 清空。`edit` / `reset` 仍只改格子，保留家的 `env`。

内置新增 `agents.kimi` 与 `cells.kimi`，只在用户或任务文件点名 Kimi 时使用：`argv:["kimi","--auto"]`、`env:["KIMI_CODE_TRUST_WORKSPACE=1"]`、`labels:[]`，不指定模型、强度，沿用用户默认。没有默认档位引用 Kimi。启动时逐项传环境变量并照常带 `--prompt`；不写 Kimi 信任记录。对 paddock：兼容新增可选字段和一个格子，设置页可在后续任务显示 `env`，名称仍按返回数据读取。含 `env` 的本机覆盖表需使用 R48d 及以后 ranch 读写，旧版会报未知字段；新版接受没有该字段的旧表。本机表写了 `kimi` 的家或格子（包括 paddock 设置页 `edit` kimi 格写进去的 `[cells.kimi]`）时，退回 R48d 之前的 ranch 会整张表报 `invalid_data`，退回前要先删掉这些段。

本机文件格式（以下只说明覆盖用法，不代表默认分工调整）：

```toml
# 只改一格的模型；其他字段继承默认。
[cells.middle-other]
model = "用户选定的模型"

# 只改一家的命令；未写 labels 则继承该家的默认标签。
[agents.omp]
argv = ["omp", "--approval-mode", "yolo"]

# 也可增加新格子，引用任何已有的家、格子。
[cells."自定义.文档"]
agents = ["pi", "omp"]
cross_review = "review-codex"
rework = "high-other"
```

顶层只允许 `agents`、`cells` 两张表。每个家允许 `argv`（非空字符串数组，首项为程序）、`labels`（字符串数组，新增家省略时为空），R35 新增 `efforts`、`models`（字符串数组，新增家省略时为空），R48d 新增可选 `env`（格式见上）；每格允许 `agents`（字符串数组，R35 起可为空）、`model`、`effort`、`cross_review`、`rework`（可选字符串），R35 新增 `description`（字符串，新增格省略时为空）。家、格子的名字非空且数量不限。按同名条目的字段覆盖，数组整项替换；可选字符串设为 `""` 表示清除。不能删除内置条目。

家模板的 `argv`、`labels` 字符串支持 `${model}`、`${effort}`，每格展开一次，插入的值不再递归展开。未知或未闭合占位符报错。R35 起，缺少模型或强度时省略含该占位符的标签、argv 参数；该参数前一项若是独立选项（以 `-` 开头、无 `=`、不含占位符），一起省略，例如 `-m ${model}`、`-c model_reasoning_effort="${effort}"`。程序项不能因此省略。模板只生成 argv/标签，程序不执行 shell。整个表先合并再校验；TOML 错误、未知键、字段类型错误、缺少必填字段、未知引用以及交叉审查/返工引用构成环（含混合环）都直接报错；即使只读另一个正确格子也不退回默认。

对 paddock：兼容新增，不改现有 corral 和 `ranch dispatch route` 约定，paddock 不用动。首次使用需要部署新版 ranch 和技能；之后本机表随改随读，不编译、不重装技能。旧 ranch 不识别这两个命令，不能单独先更新技能。

### `ranch dispatch edit` / `reset`（R35）

```sh
ranch dispatch edit <名字> [--agents <JSON字符串数组>] [--model <字符串>] [--effort <字符串>] [--cross-review <格子名>] [--rework <格子名>] [--description <字符串>] [--if-version <版本>]
ranch dispatch reset <名字> [--if-version <版本>]
```

- `edit` 至少带一个修改字段，未传字段保持原样；每个参数只能出现一次。`--agents '["pi","omp"]'` 按给定顺序替换可选家，`--agents '[]'` 清空；其余字段传 `""` 清空，模型和强度使用各家默认，审查和返工为 `null`。说明清空后是 `""`。名字必须在当前生效表中存在。
- `reset` 删除整格本机覆盖（含说明），内置格回到默认；纯本机格会消失，若其他格仍引用它则拒绝。无覆盖时成功且不写文件。家模板的 `argv` / `labels` 和可选项不能用此命令修改，只能在本机文件中配置。
- `cell` / `table` 的格子对象新增 `description` 字符串。`table.agents.<家>` 新增 `efforts`、`models` 两个字符串数组，分别为允许强度、建议模型；默认 Claude Code / Codex 强度均为 `high, xhigh`，模型分别为 `opus[1m], sonnet` / `gpt-6-astra`，pi、omp、amp、agy 两数组为空。非空强度必须在每个所选家的 `efforts` 中；空列表表示不接受显式强度。模型不限于建议列表。
- `table.local` 新增 `version` 字符串：不存在为 `missing`，存在为 `sha256:<原始文件字节的摘要>`，空文件与不存在不同，注释变化也改变版本。前端将其作为不透明标识保存，不自行生成。内容改后又完全还原视为同版本。
- 两个写命令成功均退出 0，stdout 一行 JSON，与 `table` 的整张表格式一致，包含写后 `local.version`。写前按读表逻辑校验全表；错误文件不能靠此命令修复。保留其他格、家模板的内容、顺序、注释；同目录临时文件写完并同步后原子替换，保留已有文件权限。缺目录或文件时新建。
- 格式限制：修改前先检查 `toml_edit` 解析再输出的原文是否逐字节一致，不一致则退出 **1**、`error:"format_not_preserved"`、`ok:false`、文件不动。`message` 提示可能的点号键重排等格式问题，请用户手改或改成普通 `[cells.NAME]` 表写法；前端应显示此错误，不自动重新格式化文件。命令新建格子用普通表写法。`cell` / `table` 仍能读取这些合法 TOML。
- 带 `--if-version` 时，开始读取及最后替换前检查版本；不符退出 **3**，`{"ok":false,"error":"conflict","message":"…","expected_version":"…","actual_version":"…"}`，文件不动；即使外部把文件改成无效 TOML，也先报冲突。不带版本不做冲突检查。检查与替换之间仍有很小窗口，不承诺并发事务。
- 数据和校验错误沿用 `invalid_data` / 退出 1，含路径和字段原因；未知格子沿用 `unknown_cell` / 退出 1；写入 I/O 错误新增 `io_error` / 退出 1；缺参数、重复参数、未知参数、`--agents` 不是 JSON 字符串数组为 `usage` / 退出 2。失败均 `ok:false`，写入前失败不修改文件。
- 对 paddock：兼容新增命令和 JSON 字段；现有调用无需修改。设置页只用这些命令和 JSON，遇到 `conflict` 重读再让用户处理新旧改动。须部署 R35 版 ranch；技能不变，改表后下次派活读取立即生效。旧版 ranch 不认识本机新增说明/选项字段，读写本机表的调用方须一起使用新版。

## 被人打断的一轮（R22）

- `corral status NAME` 新增布尔字段 `interrupted`。最近结束的一轮被识别为中断时为 `true`，状态仍是 `idle`，不增加状态值；下一次 `UserPromptSubmit`（新输入开始一轮）清为 `false`。正常 `Stop`、`StopFailure` 为 `false`。没有可靠中断事件时，`false` 不保证没有被人打断。
- `corral wait NAME` 在这一轮返回 `ok: true, result: "interrupted"`（退出码 0），不返回 `idle`；原有暂停、blocked、超时等规则不变。
- `corral reply NAME` 在这一轮返回 `ok: false, error: "no_reply", interrupted: true`（退出码 1），不给出上一轮的回复。下一轮开始后恢复原有回复读取规则。
- `send --after NAME` 不因这次中断结束等待，原提醒继续等下一轮正常结束；原有 `--timeout` 截止时间不延长，到期仍照旧送。收到提醒仍要查状态和回复，不能当成做完。
- 识别范围：Codex、amp 的 Interrupt 和 pi 的 `agent_settled.aborted` 已通过真人打断、真人继续后的完整实测。amp 的中断快捷键是连续两次 Esc。agy 保留 USER_CANCELED → Interrupt 映射，收到时适用上述行为，但本次 Esc 实测没有结束钩子，仍报 working、reply 仍可能返回旧回复；用户同意本轮保留此限制。
- omp（R25）：`agent_end.willContinue` 不为 `true` 时，只看 `messages` 中最后一条 assistant 消息；同时满足 `stopReason: "aborted"`、`errorMessage: "Interrupted by user"`、整数 `errorId` 含 UserInterrupt 位（67108864）才转成 `Interrupt`，适用本节全部规则。`message_end` 的标记本身不结束这一轮；Esc 只取消命令后模型继续正常结束仍为 `Stop`，`"Request was aborted"` 等其他中止、出错、`willContinue` 路径照旧，原 0.25 秒结束延迟和可重试错误的 2.5 秒延迟不变。采集器已原样记录消息，不用更换采集器或重开 omp。调用方和提醒进程须用新版；内部 cursor 缓存 7 → 8，重放旧 reader 已误判为 Stop 的原始事件，新旧 reader 交替读取时各自重建，事件格式仍为 v1/v2。R25 不增加公开字段或状态值，paddock 不用动；旧版调用方和旧提醒仍有原来的误判。
- Claude Code（R24）：新版钩子记录 `transcript_path`。新版命令行对仍未收到明确结束事件的一轮，在状态为 `working` 或被空闲通知变为 `idle` 时，只读该主会话对话记录末尾最多 64 KiB。最近的用户记录须为明确的 `[Request interrupted by user]` 或 `[Request interrupted by user for tool use]`，且其 UTC 毫秒时间戳严格晚于最近一次 `UserPromptSubmit`；显式 `isSidechain: true`、其他会话、后面已有新的用户或 assistant 记录时不认旧标记。识别后按 `Interrupt` 处理并保留到后续输入或结束事件，状态为 `idle`、`interrupted: true`，适用本节的 wait、reply、提醒规则，send 可送入新话。已确认打断后若同轮工具钩子将状态拉回 working，会重新核对标记并恢复 interrupted idle；稳定的已识别 idle 不重复读取，正常 Stop 或 StopFailure 后仍不重新识别。wait 和提醒沿用原轮询，不等待空闲通知或静默超时。新输入钩子已到、对话记录尚未写入时，时间比较排除旧标记。路径缺失、文件读不到、末尾没有完整标记、格式不认识时退回原有行为，不猜打断；使用非阻塞打开并检查实际 FD 为普通文件，FIFO 及指向 FIFO 的链接不阻塞查询。`interrupted: false` 不能保证没有被打断。
- 对 paddock：兼容新增字段和 wait 结果值；现有状态显示不用动，reply 报错时不显示旧预览即可。要显示「被打断」或区分 wait 的新结果值，在 paddock 自己的任务里接；本任务不改 paddock。
- 事件格式仍为 v1/v2，旧版可忽略新增字段。内部 cursor 缓存由 4 升到 5，新旧 reader 交替读取时各自重放事件，不沿用对方的缓存判断；这是可重建缓存，不迁移或删除原事件。旧版命令行和已启动的旧版提醒进程仍不识别中断，不能保证本节行为，使用前需让调用方和提醒进程使用新版。pi 的新采集器补记 `agent_settled.aborted`；已经运行的旧采集器不会因此获得该字段，需重开 pi 才能识别这类打断。
- R24 的 cursor 缓存升到 7，重放旧缓存以恢复主会话的对话记录路径和本轮是否已有明确结束事件。识别结果可缓存，但持久证据仍是对话记录：同版 reader 过期回写或新旧 reader 交替重建导致缓存判断丢失后，即使已收到 idle_prompt，也能重新识别同一轮。正常 Stop、StopFailure、会话结束会关闭该轮，新输入与切换主会话重新限定证据范围，不重新覆盖正常结束；不新增事件文件写入方。旧版命令行忽略事件新增的 `transcript_path` 字段，重建自己的缓存，仍不会识别 Claude Code 的打断。
- R24 部署边界：调用方和提醒进程需使用新版，该 CORRAL_HOME 命名空间的 `.runtime/helper` 也必须指向能记录路径的新版程序。Claude 的钩子调用这个共享 helper；新启动在链接已存在时保留其目标，所以只切 CLI 或停止再重开 Claude 并不足够。用新版执行相应 `corral upgrade` 会切换该命名空间的 helper，后续钩子才可记录路径；空 CORRAL_HOME 首次启动会创建指向启动程序的 helper。本轮不改 helper 选择逻辑，不实际部署。R24 不增加公开字段或状态值，paddock 不用动。

## Claude Code 提问框与框上打断（R43，R48d 增加 Kimi 题目）

`corral status NAME` 兼容新增只读字段 `questions`。仅当前状态为 `blocked`、`last_tool` 为 `AskUserQuestion` 且收到题目时给出；其他情况字段缺省或为 `null`。结构如下（`wait` 返回的状态对象及其超时报错同样带此字段）：

```json
{"questions":{"items":[{"header":"颜色","question":"选哪种颜色？","multi_select":false,"options":[{"label":"红色","description":"暖色"},{"label":"蓝色","description":"冷色"}]}],"truncated":false}}
```

- `items` 按上游顺序列出每道题。`header`、`question`、选项的 `label`、`description` 是字符串；`multi_select` 是布尔值，上游省略时为 `false`。完整保留多道题及各题选项，不从屏幕或用户输入推测。
- 只从 Claude Code 的 `PermissionRequest` 钩子的 `tool_input.questions` 取这些字段；不记录 `answers` 或其他提问参数。R47c 另只采集 Bash 原始命令，见下一节。[官方输入格式](https://code.claude.com/docs/en/hooks#permissionrequest-input) 与 [AskUserQuestion 字段](https://code.claude.com/docs/en/hooks#pretooluse-input)（2026-10-22 核对）。钩子仍只观察，不批准或回答。
- `questions` 的紧凑 JSON（UTF-8，包括结构和转义）最多 **16 KiB**。按题目顺序、每题 header → question → options（每项 label → description）分配剩余空间；字符串只保留完整 Unicode 字符的前缀，放不下的后续题目或选项省略，仍保留已列项的全部字段。任一内容被截断时 `truncated:true`，否则为 `false`；截断后可能没有选项，调用方应明确提示题目不完整。
- 新输入、工具执行/结束、会话结束或切换会清掉旧题目；permission_prompt 通知保留当前题目。旧钩子下一次不带题目的 AskUserQuestion 不会复用上次内容。
- R24 的对话记录识别扩到 `blocked`：三种框（权限、AskUserQuestion、ExitPlanMode）被 Esc，或批准后工具运行中被 Esc，仍以本主会话、非侧链、严格晚于最近输入的明确标记为依据，识别后 `idle`、`interrupted:true`，普通 `send` 可继续。没按 Esc 的框仍是 `blocked`；旧标记不能改判新一轮。已识别打断被晚到钩子拉回 blocked 时同样重新核对证据。正常结束、别家 agent 和原有字段/状态值不变。
- 事件仍为 v1，新增可选 `questions`；旧 reader 忽略它，新 reader 接受无此字段的旧事件。cursor 从 8 升到 9，新旧 reader 交替时各自重放原事件。题目采集需要命名空间的 `.runtime/helper` 使用 R43 程序；打断需要查询方使用 R43 且钩子至少能记 R24 的 transcript_path。共用 helper 的切换规矩仍见上节，单切 CLI 链接不能更新钩子。
- 对 paddock：兼容新增，现有调用无需修改，可以暂不显示题目；Esc 后从误报 blocked 变成 idle，paddock 不用动。pony 如何显示或回答题目由后续任务处理。

R48d：Kimi Code 也提供同样的 `questions`。从本主会话 wire 中属于当前主轮的 `interaction.request(kind=question).request.questions` 取题目，复用以上字段白名单、缺省值、顺序和 16 KiB 截断。仅工具调用或子 agent 的事件不提供题目；`interaction.resolved`（回答或取消）、后续工具活动、整轮结束/打断、新输入会清除。框上 Esc 解除提问后仍为 working，运行中 Esc 使主轮 cancelled 后沿用 interrupted idle。Kimi 的 wire 缓存新增版本 1，旧缓存重读原 wire；全局事件格式和其他家的 cursor 不变，钩子内容不变，读取方需使用 R48d corral。

pony 在 Kimi 提问提醒中显示题目、选项及截断提示，并提示回电脑回答。展示判断与 Claude 的回答判断分开：Kimi 不提供提问/拒绝按钮，线程回复仍排为普通消息，处理完再送，不自动按 Esc、不走 Claude 的答案或 Esc 核对流程；Slack 回答和按钮留到阶段 4。对 paddock：`status.questions` 兼容新增一家会提供，现有调用无需修改；显示题目可沿用同一格式，不据此推断支持 Claude 的按键回答。

## Claude Code Bash 权限框的原始命令（R47c 追加）

`corral status NAME` 兼容新增只读字段 `command`。仅 Claude Code 当前状态为 `blocked`、`last_tool` 为 `Bash` 且收到原始命令时给出；其他情况缺省或 `null`。`wait` 返回的状态对象及超时报错同样带此字段：

```json
{"command":{"text":"ping -c 1 127.0.0.1","truncated":false}}
```

- 只在 `PermissionRequest` 钩子中取 Bash 的 `tool_input.command` 字符串，保留空格、换行及其他字符，不从读屏恢复。缺少参数或参数不是字符串就不记；不记其他 Bash 参数、其他工具参数或计划正文。钩子仍只观察，不批准、不回答。
- `command` 的紧凑 JSON（UTF-8，包含结构和转义）上限为 **16 KiB**，只保留完整 Unicode 字符的前缀；超过上限时 `truncated:true`，否则为 `false`。调用方不能把截断内容当成完整命令。
- 新输入、工具执行／结束、会话结束或切换清除旧命令；permission_prompt 通知保留当前命令。旧 helper 下一次发出没有 `command` 的 Bash 权限事件时清掉上一份，不复用。离开 Bash 的 blocked 状态后不对外提供该字段。
- 事件仍为 v1，兼容新增可选 `command`；旧 reader 忽略，新 reader 接受无该字段的旧事件。cursor 从 9 升到 10，新旧 reader 交替时重放各自不认识的缓存，防止旧 reader 已消费事件但未保留命令。查询方及该命名空间的 `.runtime/helper` 均须使用本次程序；只切 CLI 链接不会更新钩子。共用 helper 的切换仍是单独部署操作。
- 对 paddock：兼容新增只读字段，现有调用无需修改，可以不显示命令。pony 用它核对 Bash 框并展示确认正文；缺少字段、截断或与读屏不符时不给批准按钮。

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
  {"kind":"agy", …},
  {"kind":"kimi", …}
]}
```

- `agents`：corral 支持的每家 agent 一项，按 corral 的登记顺序（现在是 claude、codex、pi、omp、amp、agy、kimi）。以后加一家就多一项；前端照列表画，不要写死名单。
- `kind`：种类，也就是 `corral start NAME -- <程序>` 认的程序名，`status`／`ls` 里的 `kind`。
- `path`：按调用者的 PATH 找到的第一个可执行文件（只看绝对路径的目录），没找到为 `null`。从桌面启动的前端要带上登录 shell 的 PATH 再调。
- `version`：`<path> --version` 输出的第一行非空内容（去掉首尾空白）。各家并行跑，每家最多 3 秒，超时连同它起的子进程一起杀掉。没找到程序、超时、退出码非 0、没有输出时为 `null`，不影响 `ok` 和退出码。内容是各家程序自己的格式，只用来显示。
- `hooks`：corral 怎么给这家接上钩子。`"launch"`：corral 启动 agent 时用启动参数或 agent 状态目录里的文件挂上，不写全局配置，没有要装、要卸的东西。`"global"`（R6 起）：要在这家自己的配置目录里装一个文件、目录或共享文件中的标记块，见下面「要装进全局配置的 agent」。以后可能还有别的值。
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

### Kimi Code 阶段 1（R48c）

`integration status` 兼容新增 `kind:"kimi"`，技能位置沿用 `target:"codex"`（`~/.agents/skills`），不新增技能安装目标：

```json
{"kind":"kimi","path":"/Users/u/.kimi-code/bin/kimi","version":"2.3.1","hooks":"global",
 "install":{"path":"/Users/u/.kimi-code/config.toml","status":"same","version":1,"format":"marked-block"},
 "skills":[{"target":"codex","path":"/Users/u/.agents/skills/corral/SKILL.md","status":"same"}]}
```

- `install.format:"marked-block"` 表示 `install.path` 是共享配置文件，只安装、卸载 corral 的标记块；没有该字段的 amp、agy 沿用原有文件、目录方式。Kimi 的路径是 `$KIMI_CODE_HOME/config.toml`，未设时是 `~/.kimi-code/config.toml`，没有 `install.enabled` 字段。
- 标记是 `# >>> corral kimi integration` / `# <<< corral kimi integration`。块外字节、原文件权限不变；卸载只在块（含前置换行）位于文件开头、块前一字节是换行、或块位于文件末尾时连前置换行一起删除，否则保留该换行以分隔块外内容。文件本身保留（原来不存在且没有新增内容的文件卸载后为空）。`create` 表示未装块，`same` 表示块完整且与本版本相同；半块、重复标记、修改过或无法确认归属的块、配置文件或配置目录是符号链接均为 `foreign`，装卸不写。当前只认版本 1 的原样内容，不把用户改过的块报成 `overwrite`。
- `--dry-run`、`--yes`、确认和结果 JSON 沿用下述命令。即使 `--yes`，写前也重读整个文件，核对内容和文件身份；发生变化报 `changed`、什么都不写。原子改名与最后核对之间仍有不合作的编辑器并发写入的微小窗口，和 R13 信任列表相同。
- 全局钩子经 `$CORRAL_HELPER __hook kimi:<事件>`；检查 corral 环境和实际父进程链，只观察、始终空输出并成功退出，不回批准或拒绝。非 corral 启动、嵌套启动的 Kimi 不记事件；共享 helper 还是旧版时，新 Kimi 直接用启动它的这份 corral。
- 新窗口的可输入编辑器算 `idle`，不必等创建会话，`send` 不需要 `--force`；信任、登录、加载画面不算就绪。`start --prompt` 可用，等编辑器就绪后通过 PTY 送首句并提交，首句以 `-` 开头也支持；收到输入钩子前仍为 `starting`。不改变 Kimi 的模型、权限模式或信任设置。
- 输入和主轮工具为 `working`；主 wire 的批准、提问交互为 `blocked`，当前屏幕的批准／提问导航提示补漏。只在主会话 `agents/main/wire.jsonl` 确认这一轮 `completed` 后转 `idle` 并提供回复：取本轮最后一个有文字的 step，同一 step 的文字片段按顺序连接，排除前面 step 的说明、思考和工具内容；子 agent 的 `Stop`、尚未完成的主 `Stop` 不结束主轮。未完成的新轮 `reply` 报 `no_reply`，不冒充上一轮回复。
- 主轮 `cancelled` 沿用 `idle` 加 `interrupted:true`、`wait.result:"interrupted"`、`reply` 的 `no_reply`；提问框上的 Esc 只解除交互，仍为 `working`。主轮失败沿用 `StopFailure`，没有正常回复。阶段 1 不提供 pony 的结构化题目或批准按钮。
- `stop` 首先 TERM、等 3 秒后 KILL，包含 Kimi 实际派生到其他进程组的命令。父进程先退也等子树清理，但最后一步 KILL 后最多再等 5 秒；还有残留时退出 pen，R48e 起 `corral stop` 的 JSON 输出兼容新增 `remaining_pids`（残留 PID 数组），仍为 `ok:true`、命令退出码为 0，Kimi 本身未退出时 `exit_code` 为 `null`。只取本次停止实例的残留记录；没有残留时不增加该字段，输出逐字不变，其他 agent 的停止输出也不变。`stopped_by` 为 `null` 表示这次没取到退出记录（例如同时运行的 `corral ls` 先清掉了记录），这时不能因为没有 `remaining_pids` 就认为没有残留。Kimi 有残留时停止约需 8 秒（TERM 后等 3 秒 KILL，再最多等 5 秒），调用方传 `--timeout` 时要留够时间。`pen.log` 和 `exit.json` 照旧保留残留记录。每次信号核对 PID 和进程出生标识，其他 agent 的停止行为不变。停止前已经完全脱离且失去父子关系的守护进程不在这份子树内。
- Kimi 要由 R48c 及以后版本启动并操作；旧版不认识这一家、旧进程缺少所需环境，`upgrade` 只换 pen，不能补齐，需重新启动。
- 对 paddock：兼容新增一家与可选 `install.format`，现有状态值和命令不变。列表按返回项显示；安装说明需在 paddock 自己的任务里识别 `marked-block`，说明只修改共享文件的一段、卸载保留配置文件。R48e 的 `stop.remaining_pids` 是兼容新增可选字段，现有调用不用改，可从命令输出获知残留 PID。paddock 不读 Kimi 的内部目录。

### 安装、卸载命令

```
corral integration install   --agent <kind> [--dry-run] [--yes]
corral integration uninstall --agent <kind> [--dry-run] [--yes]
```

- 只对 `hooks` 为 `"global"` 的 agent；对别的种类、不认识的种类、缺 `--agent`、多余参数都报 `usage`，退出码 1。
- 只写或删 `install.path` 那一个文件，或者那个目录和里面 corral 的文件；`install.format:"marked-block"` 时只增删文件中的 corral 块，别的都不动（上一级目录不删）。每个文件的写都是原子的：同目录下先写临时文件再改名。目录里有几个文件时按次序写，最后写让 agent 认出插件的那个（agy 是 `plugin.json`），卸的时候它先删。计划里没有的文件只新建、不覆盖：判断之后那里冒出了别人的文件，就报错、不动它（已经写好的 corral 文件留着，再判断时整个目录算 `foreign`）。卸载删不掉目录时（比如期间有人放进了别的文件），`written` 仍为 `true`，`warnings` 里说明目录留下了。
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

墙内继承 `FENCE_RUN`。PATH 上的 `~/.local/bin/corral` 通常处于 HOME 禁读范围，shell 常报找不到或没权限（通常退出码 127）；corral 程序读得到、能执行时，用户命令才报 `sandbox`、退出码 6，message 为 `running inside fence`，`__hook`、`guide` 仍可用。两种情况都不能再派活，不为保证退出码 6 放开 HOME。

检查与运行之间条件变化、srt 初始化失败时，start 可能已经返回成功；随后 agent 停止，退出码 125。已有运行记录的 fence 执行 runner 失败、runner 捕获的自身或 srt 错误，原因保存在 `fence denials` 的 `exit.message`；agent 退出、状态目录被清理后，`corral read` 读不到终端。创建运行记录前的失败仍只向 stderr 报错。fence 不会回退成无 sandbox 运行。runner 正常结束时透传原程序的退出码或信号；runner 意外先死时 pen 清理剩余进程组。

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

设定或清除在写入前复核 instance；发现目标已停止或重开时，报 `restarted`、退出码 1，不写回旧设置。这次复核用于挡住 fence 检查期间的重开，不提供跨重启的事务保证：复核与写入之间仍有并发窗口。

`status` 和 `ls` 的每个 agent 增加 `child_sandbox`，同样为 `{"profile":"dev"}` 或 `null`。没设、旧记录没有这个文件、记录的 instance 对不上、记录读不了或格式坏时，查询显示 `null`；启动时对坏记录的处理更严格，见下文。

设置保存在 agent 状态目录的 `child-sandbox.json`，原子写入，内容为 `{"instance":"0123abcd4567","child_sandbox":{"profile":"dev"}}`；清除写成同 instance 的 `child_sandbox:null`。只认同一次运行，不向 pen 增加请求；旧 pen 管的 agent 也可设定。文件列入状态目录的清理清单；agent 退出且目录仍有 `lock` 时，由新版 `ls` 随目录清理。旧 CLI 清掉 `lock` 后留下的文件，新版 `ls` 也不会发现和清理；同名用新版 `start` 重开时才清掉，不主动扫描无 lock 的目录。

`corral start` 的优先次序：

1. 同时带 `--sandbox <档案>` 与 `--no-sandbox`，报 `usage`（退出码 1），不起 agent。
2. 显式 `--sandbox <档案>` 使用指定档案；显式 `--no-sandbox` 这次不套墙。两者都会跳过调用者的设置读取。
3. 否则，以调用环境中的 `CORRAL_NAME`、`CORRAL_INSTANCE` 识别调用者，先读取当下的 `child-sandbox.json`，以这次读取为本次 start 的设置快照。没有文件、已清除或记录属于其他 instance，直接走原行为，不查询 pen。有可能生效或损坏的设置时，以本地运行锁和 `meta.json` 的 instance 确认调用者仍是同一次运行，不等待 pen 的 socket 回复。记录 instance 匹配且设了档案，等同 `--sandbox <档案>`，照常再次执行带种类、工作目录的 fence 检查，错误码也相同。
4. 没有这两个环境变量、调用者不在跑、instance 不匹配、没有文件或已清除，都保持原行为，不额外失败。

确认调用者仍是同一次运行后，文件在但读不了、JSON 无法解析、没有有效 instance，或匹配 instance 的设置格式错误，报 `child_sandbox_unreadable`（退出码 1），不起 agent，不当作不套墙。检查运行锁失败，或锁仍被持有而 `meta.json` 读不了、格式坏、缺少有效 instance 等无法确认调用者状态时，也报这个错误，不回退为无墙。锁已释放、锁不存在、metadata 明确属于其他 instance 时，才按调用者已停止或重开处理。能读出不同 instance 的旧设置直接忽略。坏设置可用设定或清除命令覆盖。

`start` 输出和之后 `status` 的 `sandbox` 仍表示这个 agent 实际使用的档案，不另加是否继承的字段。`FENCE_RUN` 防护不变，墙内也不能靠 `--no-sandbox` 再派活。

兼容新增：旧 CLI 不认识 `child-sandbox` 命令和 `--no-sandbox`，也不读取这个文件；用旧 CLI 发起的 `start` 不会继承档案。旧 CLI 的 `ls` 不清理这个文件，并可能删除用于发现目录的 `lock`；此时新版 `ls` 也不会清理残留，同名用新版 `start` 重开时才清掉。instance 不匹配保证不会误用旧设置。要让设定生效，派活所用的 CLI 必须是 R18 或更新版本，无需升级父 agent 的 pen。

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
{"ok":true,"profiles":[{"name":"dev","description":"在工作目录里改代码、编译、测试、提交；HOME 默认不可读。Codex 在 macOS 开启 trustd，会削弱网络隔离","source":"builtin","path":null,"overrides_builtin":false}],"kinds":[{"kind":"claude","source":"builtin","services":["anthropic"]},{"kind":"codex","source":"builtin","services":["openai"]},{"kind":"kimi","source":"builtin","services":["kimi"]},{"kind":"omp","source":"builtin","services":[]},{"kind":"pi","source":"builtin","services":[]}],"services":[{"name":"anthropic","source":"builtin"},{"name":"deepseek","source":"builtin"},{"name":"kimi","source":"builtin"},{"name":"openai","source":"builtin"},{"name":"opencode","source":"builtin"}],"local":{"path":"/Users/u/.fence/local.toml","exists":false}}
```

`source` 为 `builtin` 或 `user`。用户同名文件整份替换内置档案，`path` 为该路径、`overrides_builtin:true`。读不了或数据格式错误仍列出该项，多一个 `error` 对象；整条命令仍为 `ok:true`。

`fence check --profile <档案> [--kind <种类>] [--cwd <目录>]`：只读，不创建运行记录或 mkdir 目录。cwd 默认当前目录；不带 kind 只检查档案和本机补充；带 kind 再合并种类、模型服务与工作区 `.fence.toml`。检查程序、固定 node/srt 和 Linux 的 bwrap/socat/rg。

```json
{"ok":true,"profile":"dev","kind":"codex","platform":"macos","cwd":"/Users/u/w/x","layers":[{"source":"profile","name":"dev","origin":"builtin"},{"source":"agent","name":"codex","origin":"builtin"},{"source":"service","name":"openai","origin":"builtin"},{"source":"repo","path":"/Users/u/w/x/.fence.toml"}],"settings":{"network":{"allowedDomains":["chatgpt.com"],"deniedDomains":[],"allowUnixSockets":["<运行时才有>"]},"filesystem":{"denyRead":["/Users/u"],"allowRead":["/Users/u/w/x"],"allowWrite":["/Users/u/w/x"],"denyWrite":["/Users/u/w/x/.fence.toml","/Users/u/.fence"]},"allowPty":true,"enableWeakerNetworkIsolation":true},"env":{"TMPDIR":"<运行时才有>","TMPPREFIX":"<运行时才有>/zsh","CLAUDE_CODE_TMPDIR":"<运行时才有>"},"args":["-c","projects={\"/Users/u/w/x\"={trust_level=\"trusted\"}}"],"skipped":[],"warnings":["weaker_network_isolation: 削弱网络隔离（com.apple.trustd.agent）"],"runtime":{"node":{"path":"/Users/u/.nvm/versions/node/v24.13.1/bin/node","version":"v24.13.1","ok":true},"srt":{"version":"0.0.80","ok":true}}}
```

示例的路径、域名和参数列表已缩短。`settings` 为实际合并规则；运行时专有的 `${tmp}` 显示为 `<运行时才有>`。`skipped:[{entry,reason}]` 列出缺值或不存在的读路径，`warnings` 列出环境覆盖与平台差异。Codex 默认 trustd 规则会削弱 macOS 网络隔离。

R29：内置 `dev` 新增 `mcp.exa.ai`、`github.com`、`api.github.com`、`raw.githubusercontent.com`、`codeload.github.com`、`objects.githubusercontent.com`、`docs.rs`、`doc.rust-lang.org`、`static.rust-lang.org`。四家种类的 `args` 加入 Exa MCP 临时接入参数，提供免 key、有限频的 `web_search_exa` 和 `web_fetch_exa`；pi 的扩展、omp 的插件资源由 `run` 写到实例 `${tmp}/fence-exa`，不修改各家全局配置。用户同名覆盖仍整份替换。原读写权限不变，GitHub 推送不被专门阻止（用户 10-20 决定），任务书自行约束推送。兼容新增，paddock 不用改。

Codex 使用每次运行独立的 `fence_exa_${run_nonce}` server 名；`${run_nonce}` 是系统随机源产生的 128 位随机值，以 32 位十六进制展开，`check` 显示 `<运行时才有>`。这是为了避免 Codex 递归合并旧同名表的认证字段；fence 不读取 Codex 配置查重。Codex 工具名为 `mcp__fence_exa_<随机值>__web_search_exa`／`mcp__fence_exa_<随机值>__web_fetch_exa`，不能固定完整名称，需使用当次工具目录；其余三家名称不变。此变化不改 corral／paddock 的命令、JSON 或退出码。

R30 替代 R29 的固定资源：`data/mcp/` 提供 HTTP MCP 服务器，`$FENCE_HOME/mcp/` 同名整份替换；档案顶层 `mcp` 数组决定选择，本机 `local.toml` 顶层同名数组整份替换，空数组关闭。内置 `dev` 仍选 Exa，URL 带搜索与读网页两个工具，域名由服务器 URL 自动加入。各家接入改由种类 `[mcp]` 的参数及文件模板生成，临时资源位于 `${tmp}/mcp/<服务器名字>/`。旧自定义种类需自行补模板；不自动改用户配置。

R48f：指定种类没有 `[mcp]` 时，所选 MCP 不生成参数、文件，不额外放行其域名，`layers` 不列其 MCP 来源；选择非空时 `warnings` 给出 `mcp_skipped: <种类> 没有 [mcp] 模板，跳过所选 MCP`。自定义的无模板种类不再自动放行 MCP 域名，需要的在种类 `domains` 里自己写。所有 key 源及链接目标仍强制禁读；不指定种类的 `check` 仍验证、预览档案的 MCP。已有四家内置模板不变。Kimi 按用户决定使用自带服务端搜索、读网页，不走 Exa：2.3.1 的 [commands.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/apps/kimi-code/src/cli/commands.ts) 没有一次性 MCP 配置参数，[configLoader.ts](https://github.com/MoonshotAI/kimi-code/blob/684bae9ea6a437f3bae98bba1dcb797a917d500b/packages/agent-core-v2/src/app/mcpConfig/configLoader.ts) 只加载固定用户／项目路径。不改用户全局配置。

Kimi 可显式 `--sandbox dev`，也继承主控的 `child-sandbox dev`；墙内不能再开 agent。整个 `${home}/.kimi-code` 可写以支持会话、索引及同目录原子写，config.toml、tui.toml、region、server.token、device_id、mcp.json、plugins、AGENTS.md、skills、agents、workspace-trust、bin 禁写。用户 10-24 允许 credentials 写入以自动续期：墙内原本就要读凭证，新增风险是可能改坏登录；Claude Code 经钥匙串也能在墙内续期。其余配置和身份路径仍禁写。原生缓存放本次临时目录，关闭墙内文件 watcher、自动更新和遥测。Kimi 模型服务只新增实测所需的 `api.kimi.com`、`auth.kimi.com`，不额外放行搜索、读网页的目标域名。

不提供会话间隔离：墙内能读写所有 Kimi 会话的历史、批准记录和定时任务，墙外恢复这些会话时会生效；套过墙的会话不要在墙外直接恢复。`${home}/.kimi-code` 下的 server.token、device_id、credentials/mcp/、别的项目的会话、user-history、file-history 墙里都可读；种类数据目前不支持 `deny_read`。

Kimi 全局 hook 在 `FENCE_RUN` 与实例匹配时，父进程校验容许多一层 runner；macOS 用系统进程信息接口核对父 PID，避免墙内不能执行的 setuid `/bin/ps`。嵌套 Kimi 仍不记事件，其他家的 hook 行为不变。新增种类／服务及技能说明保持兼容，命令、JSON 字段和退出码不变，paddock 不用动。

`fence profiles` 兼容新增 `mcp:[{"name":"exa","source":"builtin"}]`（用户项、错误项沿用其他数据组约定）；`check/run` 的 `layers` 新增 `{"source":"mcp","name":"exa","origin":"builtin"}`。`check` 不读取 `$FENCE_HOME/keys/<名字>` 的 key，预览的认证头为空；`run` 才将选中服务器的 key 按 `auth.header`、`auth.prefix` 注入对应配置。Claude 只传 0600 配置文件路径；Codex `env_http_headers` 只引用本次专用环境变量，真实值只通过子进程环境传入，不进入输出的规则 `env`、启动参数、shell 参数或 fence 运行记录。所有 key 源条目及符号链接真实目标均强制禁读，与 MCP 选择无关。agent 仍能看到专用环境或临时配置里的 key。准备失败清理本次新建的实例目录，保留启动前已存在的路径；清理失败在原错误上兼容新增 `cleanup_errors` 路径／原因列表。详见 `crates/fence/README.md`。命令、退出码和现有字段保留，paddock 不用改。

错误码：`usage`、`unknown_profile`、`invalid_data`（附 `problems:[{source,path,message}]`）、`unsupported_kind`（附 `supported:[…]`）、`no_service`、`program_not_found`、`bad_cwd`、`runtime_missing`、`not_found`。amp/agy 本轮无数据，使用 sandbox 时返回 `unsupported_kind`。

`check` 在合并时校验域名和可选端口，错误返回 `invalid_data`，附规则来源和 `domains[索引]`；`run` 复用同一校验，在创建运行记录和启动程序前失败。允许 `*.example.com`、`example.com:443`、`[::1]`、`[2001:db8::1]:443` 等 srt 格式；端口为 1–65535，不带前导零。

`fence denials --agent <名字> [--instance <实例>] [--limit <n>]`：只读，未给实例时取该名字最近一次运行；前端宜传 `corral status` 的 instance，避免重名混淆。limit 默认 200，取最后 n 条并保持时间顺序；`--limit 0` 返回 `denials:[]`，仍给出 `counts`；没有匹配记录返回 `not_found`。

```json
{"ok":true,"agent":"ranch/dev-x","instance":"0123abcd4567","profile":"dev","kind":"codex","started":1760760000.1,"running":false,"exit":{"code":0,"signal":null,"t":1760760200.1},"monitors":{"network":true,"files":true},"counts":{"network":1,"file-read":0,"file-write":0,"other":0},"denials":[{"t":1760760123.4,"type":"network","target":"example.com:443","detail":"deny network-outbound example.com:443 (not on the allow list)"}],"truncated":false}
```

类型为 `network`、`file-read`、`file-write`、`other`；不认识目标时 `target:null`。running 表示 runner 尚在运行；exit 来自 runner 的退出记录，没有则为 null。网络记录来自 srt 代理，macOS 文件记录来自系统日志，Linux 文件记录是 seccomp 对写尝试的诊断提示，路径可能不准且没有读记录。srt 没有监视器健康接口，因此 `files` 只有收到文件事件后才为 true；false 表示尚未证实能记录，文件拒绝还可看 agent 终端。

Linux 的 `deny <syscall> <路径>` 事件归为 `file-write`，`target` 为上游报告的路径，收到后同样置 `monitors.files:true`；不因此把写尝试诊断视为准确的文件访问审计。

R19：`counts` 按 `type` 统计这次运行全部已记录条目，不受 `--limit` 影响；`network`、`file-read`、`file-write`、`other` 四个键总在，没有为 0，未知类型按原值另加键。`truncated:true` 时也只统计已记下的条目。fence/runner 自身或 srt 失败的退出记录可带 `message`，例如 `{"code":125,"signal":null,"t":1760760200.1,"message":"srt initialization failed"}`；只保存运行时自身错误说明，不保存 agent 命令或首句。agent 自己退出（包括退出码 125）不加 `message`。旧记录可能没有这些字段。对 paddock：P5-120 按约定忽略新增字段，不改也能用；卡片若要省掉拉全量，在 paddock 自己的任务里改用 `counts`。

记录在 `$FENCE_HOME/runs/<实例>/`（默认 `~/.fence/runs/`），由墙外的 fence/runner 写：`run.json`、0600 的 `settings.json`、`denials.jsonl`、`monitor.json`、`exit.json`。拒绝日志最多 5 MiB，满了停写并报 `truncated:true`；不保存完整 agent 命令或首句。`--limit` 仅限制返回条数，不改变 truncated。文件事件可能漏记，`files:true` 只证明收到了文件事件，不保证覆盖每次拒绝；`truncated:false` 也不代表上游日志没有遗漏。

临时目录在 `${XDG_CACHE_HOME:-~/.cache}/fence/tmp/<实例>`，与记录分开，起 agent 前创建并按档案放行读写。每次启动清理已结束超过 7 天的记录和它对应的临时目录。整个 `$FENCE_HOME` 及工作区 `.fence.toml` 始终禁止写。

内置 `dev` 档案设 `TMPDIR=${tmp}`、`CLAUDE_CODE_TMPDIR=${tmp}`、`TMPPREFIX=${tmp}/zsh`。srt 0.0.80 会把墙内 `TMPDIR` 改成 `CLAUDE_CODE_TMPDIR`（其次是旧名 `CLAUDE_TMPDIR`，都没有则为 `/tmp/claude`）；R20 起 Claude Code、Codex、pi、omp 使用 `dev` 时，墙内实际 `TMPDIR` 都是各自实例的 `${tmp}`，zsh 的 heredoc 也写在这里。用户整份替换的同名档案需自行补上这些环境项；后续数据层仍可覆盖环境变量。

R20 兼容新增 `unix_sockets`：具体绝对路径的字符串数组，支持现有 `${…}` 占位符、不许 `* ? [` 通配符。档案、种类数据和本机 `local.toml`（含其 `[macos]`、`[linux]` 段）可用，按路径取并集；仓库 `.fence.toml` 不开放此键，写了返回 `invalid_data`。未配置时不放行；内置 `dev` 默认 `unix_sockets = ["${tmp}"]`。

- macOS：合并后映射到 `settings.network.allowUnixSockets`，允许所列路径及其子路径的 unix socket bind 和 connect。路径沿用写保护的逐组件符号链接解析，输出最终真实路径（如 `/tmp` → `/private/tmp`）；尚不存在的后缀保留，断链、循环或其他解析错误报 `invalid_data`。`check` 中 `${tmp}` 仍显示 `<运行时才有>`，不创建目录。
- Linux：srt 不能按路径放行 unix socket，此项不生效；有合并路径时 `check` 的 `warnings` 包含 `unix_sockets: Linux 不能按路径放行 unix socket，已忽略`，实际 `settings.network` 不含 `allowUnixSockets`。不使用 `allowAllUnixSockets`，Linux 实测留 R17c。

`unix_sockets` 不额外放行文件读写、不改变对外网络规则；默认仅放行各实例 `${tmp}` 下的 unix socket，worktree、HOME 的其他位置、`/tmp/claude`、其他临时目录和系统 socket 不因此放开。对 paddock：不用改 `.fence.toml`；合并部署后由 paddock/main 用 Claude Code、Codex 测试 agent 跑 paddock 全量测试复验。

这些强制保护路径以及数据层所有 `deny_write` 同时保护原路径、解析途中每一个符号链接节点和最终真实目标，包含中间目录组件及多跳链接，防止通过替换中间链接更换规则。末级路径尚不存在时，解析已有父目录后保留不存在的后缀；链接自身的目标必须存在，断链、循环或其他解析错误返回 `invalid_data`，拒绝启动。

新 CLI 读旧 metadata 得到 sandbox:null；旧 CLI 忽略新增字段，但不能使用 `--sandbox`。新旧 pen 升级快照原样携带 metadata，runner 不随 pen 升级切换版本；仍有 runner 使用的旧版本目录不能清理。

## `pony status`（R38）

`pony status` 是 ranch 的另一条命令（聊天软件机器人 pony，见 `docs/DESIGN.md` §10），不是 corral 的子命令，也不要求 corral 在跑。它只读本机文件（配置、`run/status.json`、锁），不联网、不调 corral、不碰 agent，几毫秒返回，可以每几秒调一次。输出一行 JSON，不输出 token、用户 ID 和消息内容。

找命令：先 PATH 上的 `pony`，找不到试 `~/.local/bin/pony`（launchd、桌面程序常没有 `~/.local/bin` 在 PATH 里），和找 corral 一样。找到后用 `pony --version` 确认，输出 `{"ok":true,"version":"0.1.0","contract":"1"}`；`contract` 不是 `"1"` 的不按这一节解析。两处都没有就是「未安装」。

成功退出 0，`ok:true`。没配置、没在跑、连不上都是 `ok:true` 里的数据，不是错误；只有参数错报 `usage`、退出 2。

```json
{"ok":true,"contract":"1","version":"0.1.0",
 "config":"ok","config_message":null,
 "service":"installed",
 "running":true,"pid":41234,"started":1792550000.5,"last_exit":null,
 "chats":[
  {"kind":"slack","connection":"connected","since":1792550003.1,"reason":null,"message":null,
   "workspace":"firegnu","bot":"pony","other_connections":0,
   "channels":[
    {"id":"C0AAAAAAAAA","name":"paddock","private":true,"repo":"paddock","problem":null},
    {"id":"C0BBBBBBBBB","name":null,"private":null,"repo":"ranch","problem":"not_member"}]}]}
```

| 字段 | 含义 |
| --- | --- |
| `ok` | 成功时为 `true`；参数错时为 `false`，报 `usage`、退出 2。 |
| `contract` | 这一节的格式版本，现在是 `"1"`。 |
| `version` | pony 的版本，只用来显示。 |
| `config` | `ok`、`missing`（没有配置文件：这台机器没开）、`invalid`。 |
| `config_message` | `config` 不是 `ok` 时的说明。`invalid` 时写文件、位置和原因；凭证文件的错不带原文。在跑的 pony 遇到改坏的配置会继续用旧的，这里照样报 `invalid`。`ok` 时为 `null`。 |
| `service` | `installed`、`not_installed`、`unsupported`（非 macOS）。 |
| `running` | 有 `pony run` 拿着锁。 |
| `pid` | 拿着锁的 `pony run` 的进程号；`running` 为真时才有。 |
| `started` | 那次运行开始的时间，Unix 秒（可带小数）；`running` 为真时才有。 |
| `last_exit` | 没在跑时上一次为什么退出：`{"at":…,"code":"secrets_too_open","message":"…"}`；不知道时为 `null`；在跑时为 `null`。`at` 是 Unix 秒。 |
| `chats` | 配置里每家一项，按 pony 的登记顺序。没配置时是空数组。 |
| `chats[].kind` | 适配器，现在只有 `slack`。前端照列表画，不写死。 |
| `connection` | `connected`、`connecting`（第一次连或正在重连）、`failed`（连不上，等着重试）、`off`（pony 没在跑）。 |
| `since` | 进入当前 `connection` 的时间，Unix 秒（可带小数）；不知道时（没跑过、或上次没留下退出记录）为 `null`。 |
| `reason`、`message` | `failed` 时的原因代码和说明；其他情况为 `null`。代码有：`secrets_missing`、`secrets_too_open`、`secrets_invalid`、`auth_failed`、`socket_mode_off`、`network`、`rate_limited`、`slack_error`；以后可能加，不认识的照 `message` 显示。 |
| `workspace`、`bot` | 连上后由 `auth.test` 得到的工作区名、机器人名；没连上过时为 `null`。 |
| `other_connections` | Slack 报的除自己以外的连接数，大于 0 说明可能有另一台机器也开着。 |
| `channels` | 配置里绑的频道，按配置顺序。 |
| `channels[].id` | 配置里写的 Slack 频道 ID，总有。 |
| `channels[].repo` | 这个频道绑的仓库，总有。 |
| `channels[].name` | 频道名；连上并查过之后才有，否则为 `null`。 |
| `channels[].private` | 是否私有；连上并查过之后才有，否则为 `null`。 |
| `channels[].problem` | `null` 或 `not_member`（机器人没被邀请进去）、`not_found`、`archived`；以后可能加。 |

paddock 怎么显示（P5-147 定样子，这里只说数据怎么对应）：

- `config: missing`：「未配置（只在要用的那台机器上配）」。
- 没在跑：显示 `last_exit.message`；`service: not_installed` 时提示怎么装。
- 连没连上：`connection` 加 `reason`、`message`；`other_connections > 0` 时显示警告。
- 绑了哪些频道：`channels` 每项 `#name`（没有就显示 `id`）→ `repo`，有 `problem` 的标出来。
- 遇到没写的字段要忽略，不认识的取值要能显示，和本文开头的规矩一样。

对 paddock：只是兼容新增，不改 corral 的命令、JSON、`~/.corral`，paddock 现有功能不用动。R38b 合并后就可以照这一节开 P5-147（用户 10-21 定）；R38c 部署之前 PATH 上还没有 `pony`，按「未安装」显示。设置页照 `chats` 列表画，不写死 `slack`。
