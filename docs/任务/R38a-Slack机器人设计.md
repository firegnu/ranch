# 任务：R38 先出设计——从 Slack @ agent 的机器人怎么做

2026-10-21，ranch/main 交给 ranch/dev-r38a（Claude Code，opus[1m] / xhigh）。
路由：高 / 交叉审查不要 / 影响面：看得见（路由：重（置信 0.8）、要、碰要害；推翻：本轮只出设计、不改代码，技能规定调研和设计一律算看得见、不交叉审查；档位按第 2 条「本轮只出设计」定为高，类别界面和设计，读 `high-design`）
类型：设计
依据：R38（`docs/任务/R38-从Slack等聊天软件at着agent开发.md`）开工的第一步。本轮只出设计稿给用户定，不写代码、不改 DESIGN、不建配置、不注册 Slack 应用。
提示：围绕已知约束给出推荐及关键取舍；有实质备选时再比较，不凑方案数量。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：「R38 可以开始了，先出设计」
- 10-21：主控提议把插件式写实：聊天软件一层照 corral 接 agent 的做法做适配器；机器人是单独的包和命令，不放进 corral。用户：「可以，补完就派吧。底线还是一条，不要动ranch中corral的核心代码。」
- R38 其余原话和已定的各条，见 R38 任务书「用户原话」「用户已定的」。

## 先读

- `AGENTS.md`「规矩」，特别是语言与依赖、前端只用公开约定、改动要写明对 paddock 的影响、安装、不要干扰用户正在用的 agent。
- `docs/任务/R38-从Slack等聊天软件at着agent开发.md` 全文：已定的、交给 ranch 定的、开工前要定、验收 1–7。
- `docs/DESIGN.md` 第 1–4 节（范围、与前端的关系、安装），第 7 节（agent 适配器：这次「每家聊天软件一个适配器」要照这套做法），第 8 节（fence 插件式的做法，可作参照）。
- `docs/CONTRACT.md` 里 corral 的 `send`、`wait`、`reply`、`status`、`ls`、`send --after` 的约定，以及 `not_idle`、`human_active` 等错误。
- `crates/corral/src/agents/` 下适配器怎么登记（只看结构，不用全读）。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38a-chat-design`，分支 `r38a-chat-design`（已从 main 建好）。只新建 `docs/调研/R38a-Slack机器人设计.md`，以及在本任务文件末尾写完成记录。
- 可以上网查 Slack 的 API 文档和 Rust 库（crates.io、GitHub、docs.rs）。

## 要做的

写一份设计稿 `docs/调研/R38a-Slack机器人设计.md`，给用户定。用户已定的各条（R38「用户已定的」）照用，不重新讨论；有和它们冲突的地方，单独列出来。设计稿要回答：

1. **R38「交给 ranch 定的」那几项**：命令名；配置文件的位置和格式；Slack 怎么接、要哪些权限；后台怎么常驻；长回复怎么分段（不能悄悄截掉）；agent 正在干活时又来了话怎么办（要在线程里告诉用户）；怎么判断「主控新回复」和 Needs you、怎么不重复提醒；机器人重启后线程和 agent 的对应还在不在。
2. **插件式结构**：
   - 聊天软件这层照 corral 接 agent 的做法（DESIGN 第 7 节）：适配器接口有哪几项，登记表长什么样，共用核心各管什么、不写任何一家的名字；写明接第二家（比如飞书）只加一个适配器文件、在登记表加一行，共用核心不改。
   - 机器人是单独的包和命令，不放进 corral：和 corral 只通过 PATH 上 `corral` 的公开命令来往，不在 Cargo 里依赖 corral 的包，以后能整块拆出去（参照 DESIGN 第 8 节 fence 的做法）。
   - 设计不需要改 corral 的代码。如果某个需求非改 corral 不可，单独列出来，写清为什么、有没有不改的办法，放进「要用户定的问题」。
3. **给 paddock 的只读命令**：命令名和 JSON 输出的字段（在不在跑、连没连上和原因、绑了哪些频道各对应哪个仓库）。写到 paddock 能照着先做界面的程度。
4. **Slack 的 Rust 库**：slack-morphism 等候选，照 AGENTS.md「只用成熟、活跃维护的库」给结论和依据（最近发版、维护情况、下载量、依赖规模）；都不够格时，自己用已有的 HTTP 库加 WebSocket 库接 Socket Mode 要多少活。
5. **凭证**：放哪、权限怎么设，怎么保证不进仓库、不进给 paddock 的输出、不进日志。
6. **对 `docs/DESIGN.md` 的改动建议**：范围从「到 dispatch 为止」扩到这个常驻程序，写成要加的节和要点，不直接改 DESIGN。
7. **实现怎么拆**：拆成几件，每件做什么、按什么顺序；给 paddock 的命令和 JSON 能不能先定下来，让 paddock 先做界面。
8. **要用户定的问题**：最多五个，按重要程度排，每个给选项和你的建议。

## 怎么算做完

- 设计稿回答了上面 1–8，推荐的办法都写了理由，用户已定的各条没有被改动。
- 用户原话：「不要动ranch中corral的核心代码」：设计里机器人是单独的包和命令，实现不改 corral 的代码；非改不可的地方单独列出、交用户定。
- 验证只做这些：`git diff --check`；设计稿里引用的 Slack 能力和 Rust 库信息写明出处链接和查的日期。觉得不够，在回复里说，不要自己加。

## 不要做

- 不写代码、不加依赖、不改 `Cargo.toml`，不改 `docs/DESIGN.md`、`docs/CONTRACT.md`。
- 设计里不安排改 corral 的代码（`crates/corral/`）。
- 不注册 Slack 应用、不申请 token、不连 Slack、不往任何聊天软件发消息。
- 不重新讨论 R38「用户已定的」各条；觉得哪条有问题，写进「要用户定的问题」。
- 不装技能，不动 `~/.claude`、`~/.agents`、`~/.gemini`、`~/.ranch`、`~/.corral`。
- 不改 paddock、corral 程序、fence。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。只读的 `corral ls`、`status` 可以用来对照设计。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r38a-chat-design` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

ranch/dev-r38a，2026-10-21。

- **做了什么**：写了设计稿 `docs/调研/R38a-Slack机器人设计.md`。
  - 机器人暂名 pony，是单独的包 `crates/pony` 和命令 `pony`，只调 PATH 上 corral 的 `ls`、`status`、`reply`、`send`，Cargo 不依赖 corral，不改 corral 的代码。
  - 插件式：适配器接口各项、登记表 `CHATS`、共用核心各管什么，接第二家只加一个文件和一行。
  - 回答了交给 ranch 定的各项：命令、配置、Slack 的接法和权限、常驻、路由、忙时、分段、提醒和去重、重启后。
  - 给 paddock 的 `pony status` 和它的 JSON 字段；Slack 库对比；凭证；DESIGN 改动建议；拆成 R38b–R38f。
  - 开头列了要用户定的五个问题，单独列了一处和已定的对不上的地方：Needs you 时 agent 在等对话框，`corral send` 送不进去。
- **验证了什么**：
  - `git diff --check` 通过。
  - 引用的 Slack 能力和 Rust 库信息都在文末写了出处链接，都是 2026-10-21 查的。
  - 库的发版、下载量用 crates.io 接口取，维护情况用 GitHub 接口取。
  - 依赖个数：在草稿目录各建一个空工程，用 `cargo tree` 和 ranch 现有的依赖比，没碰仓库的 `Cargo.toml`。cargo 因此把 slack-morphism 等包下进了 `~/.cargo/registry` 缓存。
  - slack-morphism 的 Socket Mode 直连、错误日志带 wss 地址，是读它 2.31.0 的源码看到的。
  - 对照设计只读过 `corral ls`、`corral status`（自己和 ranch/main），没碰别的 agent。
- **拿主意的地方**：
  - 名字 pony。
  - 不用 `wait`，一个每 2 秒的循环同时管贴回线程和提醒。
  - 只订 `message.channels`、`message.groups`，不订 `app_mention`。
  - 主控按名字 `<仓库>/main` 认。
  - 看状态用 `@pony ?`。
  - Needs you 一律发在频道顶层。
  - 要提醒的消息开头 @ owner：Slack 默认只推 @ 和私信。
  - 分段上限 10,000 字。
  - 凭证放 0600 文件，不用钥匙串。
  - 配置随改随生效。
  - 不补收 pony 不在线时发的话。
  - 推荐自己接 Slack，不用 slack-morphism。它够格，但要新增 101 个包；本机系统设置开着 HTTP 代理，它的 Socket Mode 不走代理。这一条放进了要用户定的问题 2。
- **没做的事**：
  - 没改 DESIGN、CONTRACT，没写代码，没连 Slack。所以没试这台 Mac 不经代理能不能连上 Slack。
  - 两处文档没写清的留到实现时实测：markdown 块里 `<!channel>`、`<@U…>` 会不会真 @ 到人；`conversations.info` 带不带 `is_member`。
  - 飞书只核对了「长连接接收事件」一项，用来检验接口，其余没细查。
- **踩过的坑**：
  - docs.rs 上 `tungstenite/client/fn.client_tls_with_config.html` 是 404，这个函数在库的根下；改读本地 crate 源码 `src/tls.rs` 核对了签名。
  - crates.io 的网页对命令行请求返回 404，改用它的 JSON 接口。
  - Slack 的 Socket Mode 页没写回执时限和重发次数，改引 Events API 页的 3 秒、最多 3 次。

## 用户定稿

用户 10-21 定，已写进设计稿「用户定的（10-21）」一节，正文各处改成定稿说法：

1. 主控新回复和提醒：每一轮都发到 Slack。提醒（主控新回复、Needs you）和每一轮回复的第一段 @ 用户。pony 不判断用户在不在电脑前，手机吵不吵靠 Slack 自己的通知设置。用户原话：「先这么定，如果以后嫌吵，咱们再改。」R38f 上线实测时，加一条核对 Slack 有没有「电脑上不活跃时才推到手机」这类设置。
2. 常驻：A，launchd 用户服务。
3. Needs you：A，第一期只发提醒，线程里回的话排着，对话框在电脑上处理完再送。
4. Slack 库：A，自己用 ureq + tungstenite 接。
5. agent 忙时来话：A，排队，删掉还在排队的 Slack 消息就是撤回。
6. 名字就叫 pony。
7. R38b 把 `pony status` 写进 CONTRACT 之后，paddock 的 P5-147 就可以开工。

「要用户定的」那一节保留当时提的问题和选项，没删。

## 主控审查

- 看了 diff：只新建设计稿和本文件的完成记录；没改代码、DESIGN、CONTRACT，设计里不改 corral 的代码（用户底线），用户已定的各条没动。要做的 1–8 都答到了；对不上的一处（Needs you 时 `send` 送不进 `blocked` 的 agent）已列出，用户选了第一期只提醒、回话排着。
- 主控审查时加了一问：设计稿把「人在电脑前时每轮也提醒」写成已定，用户原话只有「主控有新回复时提醒」。主控先建议按 macOS 键鼠空闲时间判断，用户指出会增加复杂度，主控收回；用户定每轮都发并 @，吵不吵靠 Slack 的通知设置，「先这么定，如果以后嫌吵，咱们再改」。
- 用户定稿七条已写进设计稿「用户定的（10-21）」；`git diff --check` 通过。设计不做交叉审查。

结果：一次过
