# 任务：调研 agy、amp 怎么接，全局安装和读屏兜底的方案

2026-10-14 起草，ranch 主控（ranch/main）自己做（paddock/main 转达用户需求；用户没指定谁做，按 R3、R4 的做法由 ranch 主控做）。
路由：重 / 交叉审查不要 / 影响面：看得见（路由：几档拿不准（重 0.71），主控定重：面广、要出给以后几件活用的方案；交叉审查拿不准（安全隐私 0.34），主控定不要：只调研不改代码，方案交用户定；影响面看得见 0.96）
类型：调研 + 设计
依据：本轮只查明 agy、amp 能拿到什么、缺什么，给出全局安装和读屏兜底的方案和分步，不写实现代码，不改 DESIGN 的已定内容（方案经用户定了再写进去）。
提示：区分已查证（看了文档、程序或实测）、推测和未知，每条结论写依据；不改用户真实配置。

## 用户要的

- paddock P5-88：“加一个draft 任务，集成agy agent”；P5-98：“再加一个draft task，我还有amp需要集成”。
- R3 时：“先恢复适配器结构，全局配置可以放宽。”（条件在 DESIGN §7：经用户确认才装、钩子不在 corral 启动的 agent 里就什么都不做、只动自己那块、原子写、能卸干净）
- 听 paddock 主控解释读屏兜底后：“我的理解是这样的。读屏幕我也倾向于做一个兜底的策略。你直接发给ranch让他调研吧”。

背景：R4 的 `corral integration status` 已在 paddock P5-94 用上，`hooks: "launch"` 显示 Built in。agy、amp 看起来没法在启动时临时挂钩子，所以要「全局安装」那一档；状态报不全的，用户倾向读屏兜底。需求原文在 paddock 主控发来的消息里，paddock 草稿：`docs/任务/P5-88-集成agy.md`、`P5-98-集成amp.md`（本机未提交）。

## 本机现状（主控已看）
- agy 1.3.5：`~/.local/bin/agy`。本机还有 `../gemini-cli`（是否就是 agy 的上游待核对）。
- amp 0.0.1791950451（10-14 发布）：`/opt/homebrew/bin/amp`。
- herdr 0.9.4：Homebrew 装的，许可 Apache-2.0，源码在 `github.com/herdrdev/herdr` 的 v0.9.4 标签。可以读它的读屏规则参考思路；不复制代码（ranch 是公开仓库、不加许可证，搬 Apache 代码要带声明，这件不搬）。
- 原 Python corral `../corral/docs/ROADMAP.md` R5（2026-09-17）有 amp 的旧结论。

## 要查清的

一、agy
1. 有没有启动时挂钩子的参数或环境变量（`--help`、文档、程序）；没有的话，全局钩子装在哪、怎么登记（herdr 装在 `~/.gemini/config/hooks.json` 的 `herdr` 一块、事件 `PreInvocation`、脚本 `~/.gemini/config/hooks/herdr-agent-state.sh`）。
2. 钩子能报哪些事件：分不分得出工作中、等批准／等回答、空闲，拿不拿得到最后一条回复、会话 ID；报不出的哪几样要靠读屏。
3. 首句能不能从启动参数带进去（`--prompt-interactive`？），怎么让它退出（quit_steps）。

二、amp：按现在的 amp 重新核对旧结论
- 插件从哪加载（个人／工作区设置、项目 `.amp/plugins/`、全局 `~/.config/amp/plugins/`），有没有新的按启动加载的方式（参数、环境变量）。
- 事件 `session.start`、`agent.start`、`tool.call`／`tool.result`、`agent.end`、`awaiting-approval` 还在不在、字段是什么。
- `tool.call` 能不能只观察、不表态（不绕过 amp 自己的权限）。
- 首句带不进去的问题；会话存在 Amp 服务器上对 corral 意味着什么；怎么退出。

三、全局安装那一档（agy、amp 共用）
- `corral integration install/uninstall` 的方案：装什么、装在哪、怎么只动自己那块、原子写、卸干净；装进去的钩子怎么判断「不是 corral 启动的 agent 就什么都不做」（比如看 `CORRAL_EVENTS`）；带版本号看出旧了。
- `integration status` 里怎么表示：要装／已装／旧了／不是我装的（`hooks` 的新值和配套字段）；给 paddock 的 Install／Update／Uninstall 按钮的 JSON；和 R4 公开约定兼容。
- 已装的全局钩子遇到 corral 升级、换版本目录、用户卸 corral 时会怎样（钩子里写死的路径——是否走 `.runtime/helper`）。

四、读屏兜底
- 放在哪：核对 pen 现在怎么托着 PTY、存多少输出（paddock 说 512KB 原始输出、没有屏幕模型），兜底放 pen 里还是命令行读时现算。
- 终端模拟库：候选比较（成熟、活跃维护、许可、依赖、能不能不依赖 ratatui），照 ranch 的依赖规矩。
- 每家一份「屏幕上出现什么算什么状态」的规则怎么表达（进适配器）；钩子优先、读屏只补空缺；状态里要不要标来源。
- 给谁用：agy、不装全局插件时的 amp、corral 不认识的程序。
- 风险和成本：界面一改规则就失效，怎么测（屏幕快照）、怎么发现失效；pen 的 CPU／内存；对在跑的 agent 和 `corral upgrade` 的影响（pen 快照要不要带屏幕状态）。

## 产出
- 调研报告 `docs/调研/R5-agy与amp接入、全局安装和读屏兜底.md`：每条结论标已查证／推测／未知和依据；agy、amp 各自能拿到什么、缺什么；全局安装和读屏兜底的方案、分步、建议先做哪一步、要用户定的事。
- 回报 paddock/main（摘要 + 报告位置）。
- DESIGN 不改；方案用户定了以后在实现那件里写进去。

## 怎么查、不要做
- 只调研，不写实现代码。
- 不改用户的 `~/.gemini`、`~/.config/amp` 等真实配置，不往用户机器装全局钩子或插件；不读 `~/.gemini`、amp 目录里的登录凭据等私人文件（只看文件名、配置结构的公开文档）。
- 实测用临时 HOME；agy、amp 的真实一轮要登录，临时 HOME 下多半跑不到（见文末问题 1）。
- 不碰用户开着的 agent；不按名字批量杀进程。
- herdr 源码只读、放自己的空目录，不运行它的构建脚本。

## 要用户定的
1. **实测到什么程度**：钩子事件的真实样子最好跑一轮看。但临时 HOME 下 agy、amp 没有登录，跑不了一轮；用真实 HOME 就得把钩子装进你的真实配置，这件不允许。主控建议：先只做不用登录就能做的（文档、程序里的字符串、`--help`、临时 HOME 下能走到的部分，以及让钩子脚本只把收到的东西记下来）；要真跑一轮再问你：比如你在临时 HOME 的测试实例里自己登录一次，用完删掉。
2. **herdr 源码**：从 GitHub 下 v0.9.4 源码到临时目录只读，参考读屏规则的思路。主控建议可以。

## 完成记录
