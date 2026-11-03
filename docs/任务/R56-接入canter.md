# 总任务：corral、pony、派活分工表接 canter

2026-11-03，canter 主控（canter/main）按用户要求把需求单交给 ranch 主控（ranch/main）。原件：`/tmp/canter-p2-193/ranch-需求单.md`（下面照抄要做的和用户原话）；paddock 那份另交 paddock/main：`/tmp/canter-p2-193/paddock-需求单.md`。

## 背景

canter 是用户自己写的 coding agent（Rust；一台机器一个常驻服务端，终端界面是客户端），仓库在 `~/Developer/personal_projs/canter`。canter 这边为此要做的三件已经做完、合并（main `16fb98c`，用户已装上，`canter --version` 报 `0.1.0 · 16fb98c`）。这里只做 ranch 这边。

## 用户原话（照需求单抄）

- 11-03 看完 canter 的调研报告（`~/Developer/personal_projs/canter/docs/调研/P2-83-接corral和paddock调研.md`）后：「其他我都没有异议，可以按照你的建议走」。
- 问状态订阅放哪：「订阅事件流是进入核心了还是接入canter的接入插件里面（ranch）」；定为放 corral 里 canter 自己的接入文件，corral 公共代码只加一个通用挂点。
- 「也就是说这一版不做pause和fence，其他都会做，包括paddock相关的。」
- 「对，还有pony的（我理解只有作为主控的时候才涉及到pony）。对吧？」
- 受管理模式：「做全的，按你的建议走」。
- 「我的意思是功能不能反向依赖，而不是开发顺序。以后canter要能独立运行的」：canter 不认 corral 的环境变量、不读它的目录；接入只靠 canter 公开的接口。
- 「我的意思是canter这边全部做完，再来做corral和paddock的工作」。
- 11-03：「现在paddock和ranch的主控都开着呢。你可以在他们里面建立任务，然后送话给他们，让他们做」。
- 11-03：「如果其他agent委派出canter，要使用--yolo开出。」

## 要做成什么（需求单原文）

### corral

1. **开**：`corral start <名字> ... -- canter ...` 能开 canter，首句、`--cwd`、`--env`、`--label` 照现有约定生效；名字、instance、labels 照 corral 现有规矩保存。知道这个 agent 对应 canter 的哪个会话（canter 在 `/new`、`/resume`、分叉、交接后会换会话，要跟着换）。
2. **状态**：`corral ls/status/wait` 对 canter 能正确报 starting / working / idle / blocked（等批准、等回答）/ 被打断，`interrupted` 照 CONTRACT 的规矩。状态来源用 canter 的快照和事件流（用户定的路线，不靠钩子）；订阅和翻译放在 corral 里 canter 自己的接入文件，corral 公共代码只加一个通用挂点（某一家可以带一个盯状态的后台线程，pen 负责起停）。
3. **回复、提问、待批准命令**：`corral reply` 给出这一轮最后的回复；被打断的一轮照 CONTRACT 报 no_reply。等回答时给出问题和选项（`questions`），等批准跑命令时给出命令原文（`command`），照现有截断规矩。
4. **送话、按键、attach、read**：照现有方式经终端，行为不变。
5. **停**：`corral stop` 真正停掉 canter 在服务端上的工作（打断在跑的轮、停后台命令、关会话；会话留盘以后能恢复），不能只关掉终端就算停了。canter 已提供 `--close-on-exit`：终端正常退出马上收尾，终端异常消失 10 秒后收尾；可以直接用它，也可以另调关会话的接口，由你们定。
6. **升级**：`corral upgrade` 换 pen 后，canter 的状态订阅接着工作，不丢、不重报旧回复、不误判为停止；canter 服务端自己升级（`canter serve --replace` / `canter upgrade`）时也不误判为用户停止。
7. **不支持的明确报**：暂停 / 继续对 canter 报「不支持」；`--sandbox` 的 fence 档案（显式或从父实例继承的）对 canter 报「不支持」，不悄悄忽略、不退成无墙。canter 自带的沙箱照它自己的配置工作，和 fence 是两回事，说明里分开写。
8. **`corral integration status`** 能列出 canter：程序、版本、接法；技能照现有 target（canter 读 `~/.agents/skills` 和项目根 `.agents/skills`）。

### pony

9. 作为主控时，pony 对 canter 窗口的「拒绝」「打断」按钮能用：给 canter 按一次 Esc；能认出 canter 的终端标题（标题里带 `canter`）。

### 派活分工表

10. dispatch 表加 canter 一家，照 Kimi 的做法：只在用户或任务文件点名时派。别的 agent 委派出来的 canter 一律带 `--yolo` 开（canter 的 `--yolo` 是 Allow all、从不弹批准框，命令仍在 canter 自带沙箱里跑）。注意：`--yolo` 不跳过 canter 第一次进没信任过的仓库时的「是否信任」框，卡在这个框上时 agent 会一直是 starting，照 corral-dispatch 技能现有的「卡在信任框」处理。用到的 corral-dispatch 技能说明一并照改。

## 这一版不做

- 暂停 / 继续、fence 档案（第 7 条明确报不支持）。
- pony 的「允许」按钮；按请求号远程回答和批准。
- corral 列出别的客户端（不是 corral 开的终端）打开的 canter 会话。

## 规矩（需求单）

- canter 不能反向依赖 corral：要 canter 改接口的，写成需求交用户，不在 ranch 里对 canter 写特例的私有读法。
- 只用 canter 公开的接口：`canter --help`；canter `docs/DESIGN.md`「接 corral、paddock、pony（P2-83）」一节；`crates/canter-protocol/README.md`。不读 canter 状态目录里 `server.json` 以外的私有文件。
- 测试不调真实模型；需要真 canter 时用隔离的 `CANTER_HOME` 和假模型服务端（canter 的测试就是这么做的），不碰用户真实的 `~/.canter` 和正在跑的 canter 服务端。
- 不改 canter 仓库。

## 怎么算做完（需求单）

- 用户确认的范围是上面第 1～10 条。
- 最后由用户在场和 canter 主控一起联调（canter 的 P2-193），主路径走一遍：开、看状态、等批准、回复、送话、打断、停、升级 pen。

## 拆法（主控，11-03）

1. **R56a 设计**（先做，不改代码）：定通用挂点、事件翻译、会话跟随、停止、升级交接、「不支持」怎么报、`integration status` 怎么列；给 paddock 的公开约定（能力、错误码、接法）写清；对 canter 接口的疑问列出来；提出实现怎么拆。用户看过、定了再往下。
2. **实现**：按 R56a 定的拆法派（预计 corral 本体一件，碰要害、要交叉审查；pony 和派活表、技能一件，文件不重叠时可并行）。
3. **与 paddock 的次序**：paddock 第 3 条（图标）和新建入口的界面不依赖 corral，可以先做；第 1 条的真正开、第 2 条、第 4 条（按能力显示）、第 5 条依赖 corral 能开 canter、报状态和能力。R56a 定下的公开约定交给 paddock/main 后，paddock 可以照约定先写；corral 实现合并、部署后 paddock 再接真东西验。
4. **联调**：corral 部署后，用户在场和 canter/main、paddock/main 一起走主路径。

## paddock/main 的问题（11-03，审 R56a 时逐条对照，定了照写字段名回给它）

paddock 拆两件：A 图标、New Agent 选 canter（默认 `canter --yolo`）、Agent shell 接管，用假 corral 先做，已开工；B 暂停和沙箱按能力显示、Integrations 的 canter 一行，等接口定了再做。它问：

1. 顺序：哪几条先装上？`corral start -- canter`（第 1 条）、`ls/status`（第 2 条）、`integration status` 列出 canter（第 8 条）大概什么时候能在本机用？
2. 暂停能力：paddock 想在点之前就知道（禁用按钮），最好 `corral ls/status` 每个 agent 带能力字段（例如 `capabilities.pause=false`），或 `integration status` 按种类报。`corral pause` 对 canter 报的错误码和「老 corral 不会暂停」的 `unsupported` 分开（例如 `unsupported_kind`），paddock 现在把 `unsupported` 显示成 needs corral upgrade。
3. fence：`fence profiles` 的 kinds 里不列 canter 就行吗（paddock 按它决定选择器给不给档案）？`corral start --sandbox <档案> -- canter` 报什么错误码？canter 自带沙箱有没有要 corral 报出来给 paddock 显示的字段？
4. `integration status` 的 canter 一行：`hooks` 字段是什么值（现在只有 `launch`、`global`）？「状态来源」有没有字段说明（例如事件／快照订阅），还是 paddock 按 `hooks` 值自己写一句？有没有 `install` 段？
5. `ls` 里 `kind` 是不是就是 `canter`？`--prompt` 对 canter 怎么放（canter 首句是位置参数）？

## 联调（11-03）

- 部署 `5f065af`（R56b + R56c）后，用户在场和 paddock、canter 一起做了冒烟测试，paddock/main 转告全部通过：paddock 里新建 canter（`canter --yolo`）、图标、点信任后 `starting` 转 `idle`、发话后 `working` 再回 `idle`、`reply` 取得到、暂停按钮和菜单里的 Pause 都是灰的、Integrations 显示 Built in、停止时 corral 报 `exiting` 后消失（这次没有剩下的会话，没出 `remaining_sessions` 提示）。
- 没测、留给 ranch 以后自己测（用户定这次不测）：`/fork` 出来的会话 corral 报的状态对不对（R56b 审查第 8 条）；子 agent 的批准框关掉后 corral 能不能回到 idle（R56b 审查第 2 条，已按建议改，没实测）。
