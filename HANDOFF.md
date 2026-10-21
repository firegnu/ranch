# 交接

2026-10-15 起本文件只记项目级的账：现状、下一步、悬着。每次会话做了什么、用户怎么说的，由 cairn 的接续摘要记，新会话开局自动注入；更早的会话经过在 git 历史里（`git log -- HANDOFF.md`）。

## 现在在哪

- 范围：第 1 步 corral、第 2 步 dispatch 都已完成，范围到此为止（用户 10-05：遥测、Drover、插件系统砍掉，不迁）。之后的活 R1–R16 都已收尾（R11 并入 R12 做完；R16 是改走派活后的第一件，Codex 做、一次过）；R17（sandbox，fence 接 srt，DESIGN §8）进行中：R17a 本机小试验、R17b fence 本体和 `corral start --sandbox` 都已收尾，R17b 已部署（10-19），R17c（5090 上验 Linux）暂缓，任务文件、审查和完成记录在 `docs/任务/`，设计在 `docs/DESIGN.md`（R10 远端安装 §3；R1 暂停 §6；R3 适配器、R6 amp、R7 读屏、R8 agy、R13 agy 派活修整都在 §7），公开格式在 `docs/CONTRACT.md`。
- 安装：
  - 现装 `versions/6e807a0`（10-21 用户部署；R38d、R38d2、R38e、R38f：pony 的共用核心、Slack 适配器和 `pony service`；corral、ranch、fence 和技能自 c3a0b86 起没改，没重装技能。主控核对四个链接都指向 6e807a0，`pony status` 报 `config: missing`、`service: not_installed`，`pony service install` 不带 `--yes` 只出预览、没建 `~/.pony`；本机还没配 Slack，pony 什么都不连）。退回：四个链接切回 `versions/c3a0b86/bin/`。
  - 上一版 `versions/c3a0b86`（10-21 用户部署；R38c：新增 `bin/pony`，多切一个 `~/.local/bin/pony` 链接；corral、ranch、fence 和技能自 e62ee85 起没改，两份技能 dry-run 都是 same，没重装。主控核对四个链接都指向 c3a0b86，`pony --version` 报 contract 1，`pony status` 报 `config: missing`、`service: not_installed`；本机没有 `~/.pony`，pony 什么都不连。`working-tree: modified` 同样是 R17 草稿）。退回：三个链接切回 `versions/e62ee85/bin/`，删掉 `~/.local/bin/pony`。
  - 上一版 `versions/e62ee85`（10-21 用户部署；R34：默认分工表换成低、中、高、碰要害 × 界面和设计、其余八格，另有 `review-codex`、`review-claude` 两格审查，技能跟着改，读表程序不变。主控 10-21 打包，`working-tree: modified` 是因为主仓库有两个未跟踪的 R17 草稿，代码和 e62ee85 一致。三个链接已切，corral-dispatch 技能已重装，两份技能 dry-run 都是 same；本机没有 `~/.ranch/dispatch.toml`，主控核对 `ranch dispatch table` 是新 12 格、可选强度 high/xhigh；不改 pen，共用 helper 仍是 aef2b4a）。退回：三个链接切回 `versions/6907a32/bin/`，重装 corral-dispatch 技能。
  - 上一版 `versions/6907a32`（10-21 用户部署；R36、R37：被委派的 agent 在回复里提问由主控回答，用户已在 agent 窗口里回答时主控不再送；只改技能文字，程序和 c8d57bb 一样。主控 10-21 打包，`working-tree: modified` 是因为主仓库有两个未跟踪的 R17 草稿，代码和 6907a32 一致；同日先打的 `versions/cc5a972` 只有 R36，没部署。三个链接已切，两份技能已重装，主控核对 dry-run 都是 same、三处装好的 corral-dispatch 带新规矩；不改 pen，共用 helper 仍是 aef2b4a）。退回：三个链接切回 `versions/c8d57bb/bin/`，重装两份技能。
  - 上一版 `versions/c8d57bb`（10-20 用户部署；R35：`ranch dispatch edit/reset`、格子说明、可选项、`table.local.version` 和 `--if-version` 防冲突，给 paddock 设置页连线面板用；分工不变，两份技能 dry-run 都是 same，不用重装；不改 pen。三个链接已切，主控核对 `ranch dispatch table` 带说明、可选项和版本）。退回：切回 `versions/7d2ac96/bin/`。
  - 上一版 `versions/7d2ac96`（10-20 用户部署；R31：分工表改成数据表，主控派活调 `ranch dispatch cell <格子>` 读表，本机 `~/.ranch/dispatch.toml` 可只改某格，分工不变。corral-dispatch 技能已重装，corral 技能 same；不改 pen，helper 不用切。部署后在临时仓库开测试主控派三件小活：照方案实现读 `implementation`（astra medium）、碰要害读 `core-code`（astra high，审查 astra xhigh）、杂活读 `chores`（pi），命令和旧规则一致）。退回：三个链接切回 `versions/2f52652/bin/`，重装 corral-dispatch 技能。
  - 上一版 `versions/2f52652`（10-20 用户部署；R30：MCP 服务器做成数据，默认 Exa，可换服务商、支持 API key；只改 fence，技能 dry-run 都是 same，不用重装；不改 pen，helper 不用切。三个链接已切；部署后主控用 corral 套 dev 墙依次开四家，免 key 的 Exa 搜索和读网页都成功）。退回：切回 `versions/276107d/bin/`。
  - 上一版 `versions/276107d`（10-20 用户部署，主控打包时工作区干净；R29：套 dev 墙的四家 agent 都接 Exa 的 MCP（`web_search_exa`、`web_fetch_exa`，免 key 有限频），`dev` 白名单加 `mcp.exa.ai` 和 GitHub 5 个、Rust 文档 3 个域名；推送不挡（用户 10-20）。只改 fence，技能 dry-run 都是 same，不用重装；不改 pen，helper 不用切。三个链接已切；部署后主控用 corral 套墙开四家各试一次：Claude Code、pi、omp 搜索和读网页都成功，Codex 读网页成功、搜索撞上 Exa 免费限频（四家同时开））。退回：三个链接切回 `versions/735dbf3/bin/`。
  - 上一版 `versions/735dbf3`（10-20 用户部署，主控打包时工作区干净；R28：fence 内置加 opencode 服务，pi、omp 的 `dev` 默认放行 DeepSeek 和 opencode；三处技能加了套墙的 agent 停了没写 DONE 时查 `fence denials` 告诉用户。改了 fence 数据和技能文字，不改 pen，helper 不用切。三个链接已切、两份技能已重装，主控核对 dry-run 都是 same；部署后删了本机临时放行，pi、omp 在真墙里都能回复）。退回：三个链接切回 `versions/d748c7c/bin/`，重装两份技能。
  - 上一版 `versions/d748c7c`（10-19 用户部署；R27：技能里对方停了没写 DONE 也再挂提醒，程序和 06e2c27 一样。三个链接切到 d748c7c，两份技能已重装，主控核对 dry-run 都是 same、装好的技能是新说法；共用 helper 仍是 aef2b4a）。退回：三个链接切回 `versions/06e2c27/bin/`，重装两份技能。
  - 上一版 `versions/06e2c27`（10-19 用户部署；R25：omp 被 Esc 打断后认得出。三个链接切到 06e2c27，两份技能已重装，主控核对 dry-run 都是 same；只改 reader，共用 helper 仍是 aef2b4a，不用切，omp 不用重开）。退回：三个链接切回 `versions/aef2b4a/bin/`，重装两份技能。
  - 上一版 `versions/aef2b4a`（10-19 用户部署；R24：Claude Code 被 Esc 打断后读对话记录末尾的标记马上认出来。三个链接、两份技能、共用 `~/.corral/.runtime/helper` 都切到 aef2b4a（helper 原来指 f86d1c2，用 `/bin/cat` 测试 agent 单独 `corral upgrade` 切的，没动别的 agent）。部署后主控核对：在跑的 Claude Code 不用重开，钩子已记 `transcript_path`；真实 Claude Code 写字时按 Esc，约 0.1 秒后 `status` 为 idle、`interrupted: true`，`wait` 返回 `interrupted`，`reply` 报 `no_reply`）。退回：三个链接切回 `versions/86e5c4a/bin/`，重装两份技能，helper 用同样办法切回 `versions/f86d1c2/bin/corral`。
  - 上一版 `versions/86e5c4a`（10-19 用户部署，部署前问过 paddock/main 没在实测；R22：被人打断的一轮 `status.interrupted`、`wait` 返回 `interrupted`、`reply` 报 `no_reply`、`--after` 跳过；两份技能已重装，主控核对三处都是 same，`status` 已带 `interrupted`；不改 pen，不用 `corral upgrade --all`；已开着的 pi 要重开才认得出打断）。退回：三个链接切回 `versions/aa3676b/bin/`，重装两份技能。
  - 上一版 `versions/aa3676b`（10-19 用户部署，部署后主控用 pi 数据在真墙里实测：`TMPDIR` 是实例临时目录，临时目录里 socket 能建能连，工作目录里建 socket 被拦并进了 denials；10-19 打包，R20：`dev` 放行自己 `${tmp}` 下的 unix socket（macOS），四家墙内 `TMPDIR` 都是自己的临时目录；工作区干净；两份技能 dry-run 都是 same，不用重装；不改 pen，不用 `corral upgrade --all`）。退回：三个链接切回 `versions/cb57e07/bin/`。10-19 paddock/main 复验通过：Claude Code、Codex 墙里跑 paddock 全量测试 792 过、0 失败，TMPDIR 都是各自实例目录。
  - 上一版 cb57e07（10-19 用户部署，部署后主控实测：旧记录 `--limit 0` 给出 counts、check 的 env 有 TMPPREFIX、墙里 zsh heredoc 正常；10-19 打包，R19 fence 小修；工作区干净；两份技能 dry-run 都是 same，不用重装；不改 pen，不用 `corral upgrade --all`）。退回：三个链接切回 `versions/eff7a40/bin/`。
  - 上一版 eff7a40（10-19 用户部署，部署后主控用合成程序实测：设 dev 后子 agent 继承、`--no-sandbox` 不套、清除后不套；10-19 打包，R18 `corral child-sandbox`；工作区干净；两份技能 dry-run 都是 same，不用重装；R18 不改 pen 协议，不用 `corral upgrade --all`）。退回：三个链接切回 `versions/7fbb76d/bin/`。
  - 上一版 7fbb76d（10-19 上午 用户部署，R17b：fence 和 `corral start --sandbox`；工作区干净，srt 0.0.80，fence 用的 node 是 `~/.nvm/versions/node/v24.13.1/bin/node`，这个版本不能卸；技能没变，没重装；没跑 `corral upgrade --all`，R17b 的 pen 改动只对新开的 sandbox agent 起作用，`~/.corral/.runtime/helper` 仍是 `f86d1c2`）。部署后起 `ranch/test-sandbox`（claude，`--sandbox dev`）核对过：工作目录能写，写主仓库被拦，访问 example.com 被代理拒，两条都进了 `fence denials`；测完已停。退回：`corral`、`ranch` 切回 `versions/9683ab4/bin/`，删掉 `~/.local/bin/fence`。
  - 上一版 9683ab4（10-17 用户部署，R9：pen 和提醒进程启动时关掉继承来的描述符；只改内部实现，技能没变，没重装；部署后起测试 agent 核对过，新 pen 只开着锁、日志、pty、socket，没有多余管道；没跑 `corral upgrade --all`，部署前起的 pen 还是旧版）。打包时主仓库有两个未跟踪的任务草稿（R14、R15），BUILD.txt 记成 `working-tree: modified`，程序和 9683ab4 的代码一致。退回：两个链接切回 `versions/59705a4/bin/`。
  - 9683ab4 之前是 59705a4（R13：agy 状态兜底、corral 技能里 agy 免确认、pen 临时信任工作目录，部署后真实 agy 验收通过；技能三处都是 `same`）。退回到它之前：两个链接切回 `versions/9b69e4d/bin/`，重跑 `corral install-skills`。再上一版 9b69e4d（R12：corral-dispatch 技能换成分工表和新分档表，`ranch dispatch route` 用 jev-1.14.0；corral 代码和 `f86d1c2` 一样，没跑 `corral upgrade --all`，`~/.corral/.runtime/helper` 仍是 `f86d1c2`）。退回：两个链接切回 `versions/8299e39/bin/`，再跑 `ranch dispatch install-skills`。旧版本目录都保留，不删。
  - corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）都在 `~/.claude/skills/`、`~/.agents/skills/`、`~/.gemini/config/skills/` 下。
  - 用户的 Linux 实测机器（主机名不进仓库）：代码在 `~/Develop/ranch`，在 main 上（远端仓库设了 `receive.denyCurrentBranch updateInstead`，本机 `git push <host>:~/Develop/ranch main` 过去）。装的是 `versions/5de4583/`（R14，10-17 用户让主控经 ssh 部署；程序同 9683ab4）。代码已推到 R16 之后的 main，R16 只改测试，程序没变，没重新打包；那边全套测试过、修好的回显测试 40 遍 0 失败，两份技能都装了，pi、omp 实测过，没有在跑的 agent。退回：链接切回 `versions/53fe6fc/bin/`，重跑两份 `install-skills`。没装 cairn。经非交互 ssh 装技能要先把 `~/.local/bin` 加进 PATH，否则 corral 技能会写成“corral 没装”。那台机器上只用 pi、omp，任何情况下不跑 Claude Code、Codex，pi、omp 不委派活（用户 10-15，暂时的）。主控和委派的 agent 都不能是 Claude Code、Codex，主控（Claude Code）经 ssh 去敲命令也不行（用户 10-19），R10、R14 那种做法不再用；那边的活交给那台机器上的 pi、omp。
  - 全局插件：amp 装在 `~/.config/amp/plugins/corral.ts`（10-14 用户同意后装）；agy 装在 `~/.gemini/config/plugins/corral/`（10-15 用户从 paddock Integrations 装，测过能用）。两个 `integration status` 都是 same。
- corral 和 corral-dispatch 技能开出去的 agent 带 `--env CAIRN_DISABLE=1`，不用 cairn（R2）；cairn 只留给用户直接对话的主控。开出去的 Codex 还带 `--disable memories`，不读跨项目的 Codex 记忆（用户 10-16）。
- 没有进行中的活，没有任务 worktree 和分支；`../ranch-worktrees/` 下只有编译目录 `.target/main`。
- 前端：Saddle（`dedf26a` 起）和 paddock 都只调用 `corral`、`ranch` 命令。paddock 已接完 R1（P5-33）、R4（P5-94）、R6（P5-98）、R8（P5-88），R10 之后接了远程机器上的 agent（P5-90 甲，10-16 合并），进度看 paddock 的 HANDOFF。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。

## 用户交代、一直沿用的

- Saddle 基本不用了，不跟新功能（10-07：“saddle我基本不用了”）。
- agy 很少用，接它主要是为了读屏，它的已知限制不主动追（10-14：“主要是想通过agy加进来读屏幕的能力，agy很少用。”）。
- 部署（切链接、`install-skills`、`corral upgrade --all`、装全局插件）由用户在普通终端里跑，主控事后核对。

## 下一步

1. R18 已部署（10-19）；paddock 正在做 P5-141（主控卡片上的勾，照 CONTRACT「`corral child-sandbox`（R18）」一节，用户 10-19）。P5-120 界面 paddock 已做完（用户 10-19）。10-17 起 ranch 的活由主控派出去（R15，AGENTS.md「开发方式（主控分派）」）。
2. R19 已部署（10-19）：启动失败原因进 `fence denials` 的 `exit.message`，`denials` 加 `counts`（`--limit 0` 只拿条数），`dev` 设 `TMPPREFIX`，CONTRACT／DESIGN 改准墙内调 corral 是 127／6；`count-objects` 的 garbage 查明是 Git 2.56.0 自身误报，不改。paddock 想省掉拉全量，可在自己的任务里改用 `counts`。
3. R17c 暂缓（用户 10-19：“先不做R17c了，先把全部的在macos上跑通”），草稿在 `docs/任务/R17c-Linux上验sandbox.md`；重开前先定那边谁来执行、代码怎么过去、验收。
4. R21（amp 接入 sandbox）不做（用户 10-19：“sandbox不加amp了，这个太特殊了”）：amp 的插件只认父进程是 pen，套墙后不报状态；agy 的钩子同样，见任务文件末尾。已清干净。
5. R22（人按 Esc 打断被委派的 agent 后，主控误以为做完）已合并、10-19 部署到 `versions/86e5c4a`；不改 pen，不用 `corral upgrade --all`，状态由调用方那份命令行现算，切了链接就是新行为，切之前已挂着的提醒仍是旧行为；已开着的 pi 要重开才认得出打断。
6. agy 进 sandbox 不做（用户 10-19：“不接了，改成不做吧”）。R23 摸底已合并：agy 1.4.2 启动要在本机开 TCP 监听，墙里被拒起不来；fence 没接 srt 的 `allowLocalBinding`（一开就放行所有本地地址监听和 localhost 任意端口），钩子父进程检查也要改。都要改代码。报告 `docs/调研/R23-agy进sandbox摸底.md`。
7. R24（Claude Code 被 Esc 打断后马上认出来）已合并、10-19 部署到 `versions/aef2b4a`，部署后实测认得出。以后部署改了钩子的版本，要照这次的办法切共用 helper。
8. R25（omp 被 Esc 打断后认得出）已合并、10-19 部署到 `versions/06e2c27`：omp 生成回复或调用工具时按 Esc，最后一条回复带 `aborted` / `Interrupted by user` / UserInterrupt 位，reader 认作打断；跑命令时 Esc 只停命令、接着正常结束的照旧。开发时真人实测过。
9. R26（agy 被 Esc 打断时报什么）摸底已合并：agy 1.4.2 写字时、命令在跑时按 Esc 都不发 `Stop`/`USER_CANCELED`，对话记录也没有标记，只有 agy 自己的诊断日志记了取消请求；没接，agy 被打断仍认不出。没测到命令同步等待的前 10 秒。报告 `docs/调研/R26-agy被打断时报什么.md`。
10. R27（被委派的 agent 停下来等人、人在它窗口里接着聊，最后写 DONE，主控接不到）已合并、10-19 部署到 `versions/d748c7c`：技能改成停了没写 DONE 也再挂提醒，开完后前台 wait 已停也照收到提醒处理。部署后下一次派活看接不接得到。
11. R29（套墙的 agent 查不了资料）已合并、10-20 部署到 `versions/276107d`。Exa 免 key 有限频，几个 agent 同时搜会被限；要用 key 照 R30 的办法配。Codex 首次可能找不到 Exa 工具（客户端工具发现的限制），派 Codex 查资料时可能要提示它从工具目录里找；Codex 的 Exa 工具名每次运行不同。
12. R30（套墙的 agent 接哪个 MCP 服务器做成数据）已合并、10-20 部署到 `versions/2f52652`，部署后四家依次实测免 key 的 Exa 搜索、读网页都成功：默认仍是 Exa；换服务商只在 `~/.fence/mcp/` 加一份数据、`local.toml` 改 `mcp`；要 key 的放 `~/.fence/keys/<名字>`（只发给那一家，key 进墙后 agent 看得到，不支持含 `$` 的 key）。只改 fence，技能 dry-run 都是 same。用户 10-20 暂时用免费 Exa。带真实 key 的服务商没实测过。
13. R31 已部署（见上）。R34（按低、中、高、碰要害 × 界面和设计、其余分家；交叉审查换家；每档返工一次不过就升档；不用 medium、max、fable、ultra；主控只在明说或 `执行：主控` 时自己做）10-21 已合并、部署到 `versions/e62ee85`。格子名全换（`implementation` 等不再有），已通知 paddock/main：设置页连线面板和 P5-161 的样例要照新表核对。
14. R35 已合并、10-20 部署到 `versions/c8d57bb`；paddock 的设置页连线面板（需求 `docs/需求/paddock-设置里用连线图看和改派活分工.md`）照 R35 的 CONTRACT 接。R35 留的那条建议改（一格可以被清成没有可选家）R34 已定：技能写明读到空的可选家停下告诉用户。
15. R36（被委派的 agent 有问题在回复里问，主控照技能第 8 节自己定、事后告诉用户；隐私、花钱、改批准过的设计、删除、推送、关 agent 或主控拿不准才问用户）已合并；R37（用户已在 agent 窗口里回答时主控不再送决定：送前查 `status` 的 `last_human_input`、`send` 报 `not_idle`/`human_active` 不硬送）已合并。两件一起 10-21 部署到 `versions/6907a32`；下一次派活看 agent 提问时主控接不接得住。「正在打字还没回车、又超过 30 秒没打字」时主控的话仍可能送进去，要改程序，用户 10-21 同意先不堵。
16. R38（从 Slack @ agent 开发，paddock P5-147 的主体；P5-147 只做 Settings 显示，依赖 R38）：R38a 设计 10-21 已合并，设计稿 `docs/调研/R38a-Slack机器人设计.md`，用户定稿七条在其「用户定的（10-21）」一节。机器人叫 pony，单独的包 `crates/pony` 和命令，只调 corral 的 `ls`、`status`、`reply`、`send`，不改 corral（用户底线：「不要动ranch中corral的核心代码」）。10-21 用户又定离线期间的话上线后补收（加说明，超过 12 小时不自动送）。R38b（DESIGN §10、CONTRACT「`pony status`（R38）」、AGENTS.md 加 pony）10-21 已合并，已通知 paddock/main 可开 P5-147；只改文档，没打包。R38c（`crates/pony` 包和命令外壳：`--version`、`status`、`run`，配置和凭证校验、单实例锁、状态文件、日志轮转，打包加 `bin/pony`）10-21 已合并、推送，部署到 `versions/c3a0b86`（PATH 上有 `pony`，没配置）；CONTRACT 同时补了 `since` 不知道时为 `null`（7252a38），已挂给 paddock/main（它那一轮结束后送）。R38c 审查留下、R38e 没包括的：运行中凭证变坏到连接时才发现、`lock::holder` 不能在 run 进程里调、`repo` 没排除 corral 保留名、删掉配置后 run 退出 0 而 launchd 不会再起（见 `docs/任务/R38c-审查.md`）；R38f 只带了最后一条（写进部署说明），凭证每次连接前已重新检查，`lock::holder` 现在只在 `pony status` 里调。R38d（共用核心：路由、线程表、排队和撤回、盯 agent 贴回、提醒、去重、分段、`?`、重启恢复、离线补收；定下核心和适配器的接口，给 R38e 的说明在 `crates/pony/README.md`「适配器接口」）10-21 已合并、推送，没打包部署；返工 2 次（第 2 次经用户同意）。R38d2（补收失败后退避、每次连上后至少再试一次）10-21 已合并、推送，修了 R38d 复核第 2 轮 24、25；返工 1 次。R38e（Slack 适配器：Socket Mode、Web API、事件翻译、限频按 Retry-After 等完重发；顺带修 R38d2 审查 27、28；设计稿 3.3 加收 `thread_broadcast`，用户 10-21 定）10-21 已合并、推送，没打包部署；返工 1 次。复核建议改 13（「分享消息」转发的内容被漏掉）用户 10-21 定放进 R38f。R38f（`pony service install|uninstall` 装卸 launchd 用户服务、`pony status` 报真实 `service`、修 13、README 部署说明和上线实测清单）10-21 已合并、推送，打包到 `versions/6e807a0`，10-21 用户已部署；返工 2 次。10-21 上线：用户建了 Slack 应用（工作区里只有用户一人），私有频道 `#paddock` 绑 paddock 仓库，`~/.pony` 凭证 0600、目录 0700，直连不用代理；`pony service install --yes` 装好 launchd 服务，`pony status` 报 installed、running、connected。实测：验收 1（@ 送到 paddock/main、回复贴回线程、线程里接着聊）、2 的主控新回复提醒和提醒线程回复、3（`@pony ?`）、4（`ranch/main`、`paddock/foo` 被拒）通过；6 主控核对 ranch、paddock 仓库及历史、pony 日志、状态、plist 里都没有 token。实测清单里附件和「分享消息」都提示「附件没送」、只送文字，`is_share` 在第一次推送里就有，R38f 的修法在真 Slack 上管用。没测：5（要第二个账号，用户暂时略过）、2 的 Needs you 提醒（等真碰上）、手机通知（用户先测电脑，手机以后看；不响先查 Slack「电脑活跃时不推手机」设置，再考虑给通知文字补 `<@owner>`）、排队和撤回（Claude Code 把 `sleep` 放后台、agent 没忙起来，没测成；下次让 agent 前台跑 `ping -c 100 127.0.0.1`）、README 实测清单的其余项。小问题：pony 自己发的消息被推回来时，日志记成「忽略了一条别人的消息」，行为对、措辞不准，以后顺手改。R38c 审查留下的 `repo` 没排除 corral 保留名，用户没要，不做。每件任务书先给用户看再派。

## 悬着

- 不接 opencode agent（用户 10-20：「算了，我不打算集成了。反正omp和pi中已经可以加载opencode的go订阅模型了，现在越来越多，越来越难管理」）：R32 已删；要用 opencode 的模型走 omp、pi（R28 已放行 `opencode.ai`）。
- 查资料服务器在 paddock 设置页只做显示（用户 10-20：「按你的建议，只做显示，测试按钮先不做」），需求 `docs/需求/paddock-设置里管理查资料的服务器.md`，ranch 不用改，交用户转 paddock。界面里换服务商、填 key、测试按钮都先不做，等真要换服务商或注册 key 时再议。
- paddock 10-16 提的两件可选的事，都改公开约定，「甲」层不依赖，做不做用户还没定：`corral ls` 带上 status 的字段（或 `status --all`），远端每轮刷新从两次调用减到一次；`corral --version` 带构建号，前端能看出远端比本机旧。
- agy 的已知限制（DESIGN §7「接入 agy」，不主动追）：
  - 子 agent 的会话会不会触发钩子、抢主会话，没实测；
  - 钩子 shell 里 `exec` 失败时不回 `{}`；
  - Antigravity IDE 会不会加载插件，没核对；
  - R13：agy 运行中自己保存设置（如点信任）会不会拿旧内容冲掉 corral 加删的条目，没实测；登录慢时首句抢跑没复现（10 次 0 次），送法没改。
- paddock 的 `docs/DESIGN.md` §3“Saddle 保底”和决策表还写着“只保证 Saddle 和运行时对得上、不兼容时写需求给 Saddle 适配”；ranch 这边已按用户 10-07“保底规矩放松吧，Saddle 不用再写影响了”改了 AGENTS.md 和 DESIGN §2，paddock 那边要不要跟着改，在 paddock 自己的任务里做。
- R17a 记下的 srt 边界（用户 10-18：“不需要严格限制。我只是在要通过做这个来彻底完全的了解一下sandbox。”，不当成 R17b 的门槛）：Codex 要开 `enableWeakerNetworkIsolation`（trustd，削弱网络隔离）；Claude Code 要读 `~/Library/Keychains`、用 `CLAUDE_CODE_TMPDIR`；各家状态目录放得较宽；worktree 提交成功但报 `packed-refs.lock` 被拒，原因没查清；macOS 系统日志的拦截记录没整理成统一取法。
- R17b 留下的：srt 的文件拦截日志会漏条（Codex、pi 写主仓库被拦没记进 `fence denials`，只在终端可见）；全套测试偶有 1 个失败、再跑就过（像原有 hook 协议测试的偶发超时，没认出名字）；corral 对 Codex 在同一轮里会短暂报 idle，派活时提醒会提前到（没查）。`corral start --help` 的用法行没列 `--sandbox`（CONTRACT 有）。
- R18 交叉审查第 2 条（旧运行的慢 `child-sandbox` 设定请求在 fence 检查期间遇到同名重开，可能冲掉新运行的设定）：主控改判建议改，与审查者有分歧，已列给用户；返工加了写入前复核 instance，复核和写入之间还剩很小的窗口，CONTRACT 写明了。用户没说要再修。
- `~/.fence/local.toml`（10-19 用户同意、主控写）：放行 IP Guardian 的 4 个查 IP 网站，允许读 `~/.ip_guardian_bin`、`~/.ip_guardian_allowed`；不写会让墙里的 codex、claude 被 IP Guardian 拦下（退出码 1）。
- R20 交叉审查建议改（R20 之前就有）：`~/.cache` 是指向 HOME 外的符号链接时，`${tmp}` 的环境值和文件规则用链接拼写、socket 规则用真实路径，墙里连临时目录都建不了。用户的 `~/.cache` 不是链接，不受影响；要支持时把三处统一到同一物理路径。
- Linux 上墙里的 unix socket 仍全拦（srt 不能按路径放行），R17c 时再定。
