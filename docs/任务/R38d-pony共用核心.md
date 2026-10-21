# 任务：R38d 做 pony 的共用核心

2026-10-21，ranch/main 交给 ranch/dev-r38d（Codex，gpt-6-astra / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 1.0）、要、碰要害；只认一个用户和没绑频道的仓库不发算安全和隐私，排队、去重和 state.json 算并发和核心规则；按第 1 条读 `critical-other`，审查读 `review-claude`：Claude Code opus[1m] xhigh）
类型：功能变更
依据：R38 的实现第三件。照设计稿做 pony 的共用核心：路由、线程表、送话和排队、盯 agent 和提醒、去重、分段、`?` 看状态、重启恢复、离线补收；定下核心和适配器之间的代码接口，并把核心接进 `pony run`。本轮不连 Slack（R38e 照本轮定的接口填 `chats/slack.rs`），不做 `pony service`（R38f）。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：「写 R38d 任务书吧」
- 底线：「不要动ranch中corral的核心代码。」
- R38 验收（10-21：「验收照这七条。」）第 1–5 条见下面「怎么算做完」。
- 离线补收（10-21）：「按你的推荐，选第 2 种，加说明，12 小时上限」。
- 每轮都提醒（10-21）：「先这么定，如果以后嫌吵，咱们再改。」
- 其余用户定的各条见 `docs/DESIGN.md` §10「用户 10-21 定的」和设计稿「用户定的（10-21）」。

## 先读

- `AGENTS.md`「规矩」：语言与依赖、测试不依赖真实 agent 和用户状态、验证、独立编译目录。
- `docs/DESIGN.md` §10。
- `docs/调研/R38a-Slack机器人设计.md`：「用户定的（10-21）」「和已定的各条对不上的地方」，第 1 节（找 corral），第 2 节（适配器接口、共用核心管什么、接第二家），3.5–3.11 全部，第 10 节（第一期不做的）。
- `docs/CONTRACT.md`：corral 的 `ls`、`status`、`reply`、`send`（字段、退出码）和末尾「`pony status`（R38）」。
- `crates/pony/` 现有代码和 README；`docs/任务/R38c-审查.md` 末尾「主控对审查意见的决定」里留给 R38e 的几条（`lock::holder` 不能在 run 进程里调、`repo` 没排除 corral 保留名）。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38d-pony-core`，分支 `r38d-pony-core`（已从 main 建好）。只动 `crates/pony/`、`Cargo.lock`（只有确实要加依赖时），以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r38d-pony-core`。

## 要做的

1. **核心和适配器的接口**：在 `chats/mod.rs` 的 `Chat` 上补齐设计稿第 2 节「适配器接口」表里还没有的几项：连接（起连接、报连上／断开和原因）、收到的消息翻成共用格式（频道、消息 id、所在线程、发的人、有没有 @ 机器人、去掉 @ 和转义后的正文、有没有附件；「某条消息被删了」）、上限（一条最多多少字、有没有线程）、发消息（Markdown 发到频道或线程、返回消息 id、核心标了「要提醒」的由适配器负责让用户手机响）、频道信息（名字、是否私有、机器人在不在里面）、离线补收要的「读频道和线程里比某条新的消息」。接口要让 R38e 只改 `chats/slack.rs` 就能接上。`slack.rs` 本轮只给出占位实现，不连网络：配置了 slack 时 `pony run` 照 R38c 那样报 `connecting`，不发任何请求。
2. **调 corral 的那层**（设计稿第 1 节）：找 corral（配置里的 `corral`，没有就 PATH，再 `~/.local/bin/corral`），起来时 `corral --version` 看 `contract` 是 `"1"`；只用 `ls`、`status`、`reply`、`send`，送话用 `corral send <全名> -- <正文>`，不加 `--force`。照 CONTRACT 解析 JSON 和退出码。
3. **共用核心**，照设计稿逐节做：
   - 3.5 一句话进来之后：只认 `owner`；各种情况表；路由（`?`、带斜杠的全名、短名、默认 `<仓库>/main`）；先回 `→ <全名>`（重开过的注明）；没绑仓库的频道、没接入的仓库、没有这个 agent、主控没在跑、正文为空各自怎么回。
   - 3.6 agent 正在干活时：排队；各状态和 `corral send` 各结果怎么做、线程里怎么说；一轮一句按顺序送；删掉排队的消息就是撤回；agent 退出了排着的都不送。
   - 3.7 一轮结束：每 2 秒 `ls`、`status`，只看绑了频道的仓库（没绑的连 `status` 都不调）；判一轮结束的条件；贴回线程（有回复、被打断、退出了），然后送下一句。不用 `corral wait`。
   - 3.8 主控新回复和 Needs you：判断、发在哪、提醒的线程对应这个 agent、不重复的记号、第一次看到的 agent 只当起点、pony 停着时结束的轮补发最近一条。
   - 3.9 长回复分段：不截、不删、不改字；断点顺序、代码块补收尾和重开、表格、`(i/N)`、发到哪、空回复、发送失败对半再切和报错。上限由适配器给。
   - 3.10 `?` 看状态：只列绑了频道的仓库，本频道的排最前，状态说法、排着几句。
   - 3.11 重启后：`state.json`（0600，只有拿锁的那份写，临时文件写好再改名）存线程表（30 天没用的清掉并照设计稿回话）、在等回复的对话、排队的原文、去重记号、最近处理过的消息 id；离线补收（每次连上后补读、按先后照常路由排队、按消息 id 去重、送给 agent 时开头加说明、`→` 行注明离线期间发的、超过 12 小时不自动送并照设计稿回话、第一次起来不补收）。
4. **接进 `pony run`**：核心跑在 `run` 里，和 R38c 已有的配置重读、状态文件、日志共用；`pony status` 的 `chats[]` 照核心和适配器报的连接状态、频道信息出（字段照 CONTRACT，本轮 Slack 不连，仍是 `connecting`）。日志不记消息正文、不记凭证；别人的消息只记「忽略了一条别人的消息」。
5. 写清对 paddock 的影响：`pony status` 输出格式不变。

## 怎么算做完

用户原话（R38 验收第 1–5 条）。本轮不连 Slack，下面几条在核心这一层用测试里登记的内存假适配器和假 corral 跑通；真 Slack 上的实测在 R38f：

1. 「在 `#paddock` 里 @bot 说一句，`paddock/main` 能收到，这一轮的回复贴回同一个线程；在线程里接着回，就是接着和它聊。」
2. 「某个 agent 变成 Needs you，或者主控有新回复时，对应频道里出现提醒；在提醒的线程里回复，能送到这个 agent。」（Needs you 时照用户 10-21 定的：先排着，对话框处理完再送。）
3. 「在聊天里能看哪些 agent 在干活、哪些在等。」
4. 「没绑频道的仓库，提醒、回复、状态都不出现在 Slack；跨仓库 @ 它会被拒绝。」（包括 pony 对没绑频道的仓库的 agent 连 `status` 都不调。）
5. 「别人在频道里 @bot，机器人不理。」

另外：

- 离线补收：「按你的推荐，选第 2 种，加说明，12 小时上限」：离线期间发的话上线后照常路由、排队，送给 agent 时开头加说明；超过 12 小时的不自动送；第一次起来不补收。
- 长回复分段后拼回来和原文一字不差（除了补的代码块收尾／开头和 `(i/N)`）。
- 重启后线程表、排队、在等回复的对话、去重记号都还在，同一条消息不会送两次。
- 「不要动ranch中corral的核心代码。」：`crates/corral/` 没有改动。
- 验证只做这些：上面各条和设计稿 3.5–3.11 各表里的情况各有测试（核心层，内存假适配器、假 corral）；调 corral 的那层对 CONTRACT 里的输出和退出码有测试；R38c 已有的「核心源码不出现适配器名」测试继续通过；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 `crates/corral/`、`crates/dispatch/`、`crates/fence/`、`crates/ranch/`、`tools/`，不改 `docs/CONTRACT.md`、`docs/DESIGN.md`、设计稿。设计稿和 CONTRACT 有说不清、对不上或做不到的地方，问主控，不自己改约定。
- 不连 Slack、不发任何网络请求；`chats/slack.rs` 只做占位，Socket Mode、Web API 留给 R38e。
- 测试和你自己试的时候**绝不调真实的 corral**：不调 PATH 上或 `~/.local/bin` 的 `corral`，不读写 `~/.corral`。测试用内存假 corral；要起程序时 `HOME`、`PONY_HOME`、`CORRAL_HOME` 都指向临时目录，配置里的 `corral` 指向测试里的假程序。
- 不建、不读真实的 `~/.pony`。
- 不做 `pony service`，不写 `~/Library/LaunchAgents`（R38f）。不做第二期（开、停 agent）和设计稿第 10 节列的不做的事。
- 加新的第三方库先在回复里问主控。
- 不装技能，不动 `~/.claude`、`~/.agents`、`~/.gemini`、`~/.ranch`、`~/.corral`。不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch`、`pkill -f pony` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停测试里起的进程用记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r38d-pony-core` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话；另写一段「给 R38e 的接口说明」：`Chat` 上新加的每一项是什么、Slack 那边要填什么。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-21，ranch/dev-r38d，分支 `r38d-pony-core`。

### 做了什么

- 新增 corral 命令边界，只调用 `--version`、`ls`、`status`、`reply`、`send`；找程序的顺序、contract 1 校验、JSON 和退出码按约定处理，正文放在 `--` 后，不加 force。
- 实现共用核心：owner 和频道隔离、全名／短名／默认主控路由、线程续聊、重开提示、排队和撤回、各发送结果、每轮回复、主控新回复、Needs you、去重、`?`、30 天线程过期、重启恢复和 12 小时离线补收。未绑定仓库不调用 status，不输出其信息。
- `state.json` 用 0600 临时文件同步后改名并同步目录。适配器接收入口在回执前持久保存事件；核心等待 corral／发送时仍可接收。陌生人的正文在保存前丢弃，只留下固定的忽略事件。
- Markdown 按字符和行切分，正文不改，代码围栏和段号单独添加；表格尽量整块。拒绝过长／格式错误时只重切失败段，最小 1,000 字；最终失败回线程并记日志和 status。已经成功的段不重发，重切部分有自己的 `(i/N)`。
- 核心接进 `pony run`，沿用配置重读、锁、状态和日志。Slack 仍是纯占位，无网络请求，报 connecting；README 写明 R38e 接口和 paddock 影响。`pony status` 输出格式不变，paddock 不用动。

### 验证了什么

- 按任务限定的核心和命令边界做测试：路由、隐私、队列、撤回、状态说法、全部发送结果、稳定轮询、提醒与线程、分段与失败、重启去重和发送结果不明、补收顺序与 12 小时上限、首次不补收。关键行为先跑到目标断言失败再实现；队首 human_active 退避和发送确认失败也先复现再修。
- pony 最后一次 `cargo test -p pony`：32 项通过。测试只用内存适配器／内存 corral，以及 Rust 编译的假命令；实际启动假命令或 pony 的 HOME、PONY_HOME、CORRAL_HOME 都是临时目录。核心源码不出现适配器名的原测试继续通过。
- `cargo test --all-targets` 跑一次通过（228 通过、11 项按原标记忽略：3 项外部兼容基线、8 项真实 srt 环境）。之后自查补了发送确认失败的回归与持久恢复，并重跑上述 pony 全部测试通过；没有再跑全仓测试。
- `cargo clippy --all-targets -- -D warnings` 首次发现新分段代码一处 collapsible_if；合并该条件后复跑通过。这一次必要的修正复核比原定的一次多跑了一次，未增加其他验证项目。`cargo fmt --check` 一次通过；`git diff --check` 通过。
- 全部命令前台等到结束，编译目录均为 `.target/r38d-pony-core`。未改 `crates/corral/`、其他包、Cargo.lock 或公开约定文档。

### 拿主意的地方

- 主控明确决定采用方案 1：发送前记 Sending；重启遇到发送结果不明的，不自动重送，在原线程回「上次发送结果不明，没重送，请确认后重发」，日志记 send_uncertain；其余队列照常恢复。按主控要求只记在这里，不改设计稿。
- 线程过期后移除 agent／实例对应，保留过期线程 ID，保证再次回复时能明确提示，而不是误当成陌生线程或猜路由。
- 接收与回执通过持久 Events 入口分开；适配器工作线程先保存，再回执，避免 corral send 等待期间耽误入站回执。发送确认文字也随队列保存，确认未发成功前不会发送正文。
- 没有新的待主控决定事项，也没有新增第三方依赖。

### 没做的事

不连 Slack，不做 service，不部署，不装技能，不读写真正的 `~/.pony`／`~/.corral`，不操作用户 agent，不改 paddock，不合并 main，不推送。真实平台的 Markdown 转换、意外 @ 转义、网络限频／重连和上线实测留给 R38e／R38f。R38c 留给 R38e 的 corral 保留名配置校验未顺带修改。

### 踩过的坑

直接调用 rustc 编译假程序时默认 edition 不随 Cargo 包走，2024 的 let-chain 被拒；给假程序编译显式加 `--edition=2024` 后通过。自查的回归还发现：队首等 human_active 时后面的消息可能越过它，以及发送确认失败后正文可能先送；分别用严格队首检查、持久保存待发送确认修好。没有通过删测试、放宽断言或缩短超时绕过。

### 给 R38e 的接口说明

`Chat` 原有 kind、secret_keys、config_keys、read_config 保留，新增 `connect(Connect) -> Result<Box<dyn Connection>>`。Connect 含本家的 ChatConfig、proxy、credentials 回调和 Events；credentials 必须每次连接／重连调用，重新校验权限和读取凭证；connect 起工作线程后返回，网络／认证失败通过事件报告，由适配器退避重试。工作线程在 close 和 Drop 时停止并等待退出。

- `Events::emit(Event)`：成功意味着已落盘，可以 ACK；失败不能 ACK。Connection 事件给 connection、公开 reason／message、workspace、bot、other_connections。先报 connected，再交付这次连接的消息。不能把 ticket URL、token、原始 HTTP 错误放进公开说明。
- `Event::Message`：channel、id、thread、sender、mentioned、去 @／解转义后的 Markdown text、attachments、Unix 秒 at。适配器排除机器人消息与编辑。`Event::Deleted` 给原消息 channel／id。核心统一 owner 检查、去重、路由和队列。
- `Connection::name`：用户提示里的平台名，Slack 填 Slack；`limits` 填 characters=10_000、threads=true。无线程平台的降级逻辑已经在核心。
- `Connection::send(Outgoing)`：channel、可选 thread、markdown、notify，成功返回实际消息 ID。Slack 实现 markdown 块、通知文案和 @ owner、转义正文中的意外 @、关闭链接预览、每频道限频与重试。SendError.code 是公开错误码，resize=true 只用于确定未发出、可以缩小重试的格式／长度错误。
- `Connection::channel(id)`：返回 name、private、problem；None 表示机器人在可用频道内，其他值使用 not_member／not_found／archived 等公开代码。
- `Connection::history(channel, &HistoryAfter)`（返工 1 更新）：读完所有分页，返回频道里比游标新的统一 Message，包含新旧线程里的回复。HistoryAfter.id 为可选的不透明消息 ID，at 为 Unix 秒下界（不含）；没有 ID 时按 at 读。首次没有历史起点不调用，之后无频道游标时用上次在线时间。由适配器决定如何省请求，Slack 可按父消息 latest_reply 查有新回复的线程，也要覆盖旧父消息下的新回复；核心不再逐个线程调用。核心检查 owner、频道、时间与去重，跳过收件箱已有的在线消息，合并按时间排序补收。
- `Connection::exists(channel, thread, id)`（返工 1 新增）：核对原消息是否仍在；顶层 thread=None，回复传父消息 ID。仅确定删除才返回 false，无权限／网络故障／无法判断返回错误。每次连接后核心核对全部 Queued，false 按撤回处理；错误记日志、暂缓该条、每轮重试，其他 agent 照常。
- attachments 由核心处理：路由成功的 `→` 之后说「附件没送」；只有附件没有字时说「附件没送，只收文字」。发送失败只记日志和线程，`pony status` 的 connection 只由适配器 Connection 事件决定。
- `Connection::close`：停止并等待自有网络线程。`chats/slack.rs` 的 Placeholder 已把本轮全部接口留齐；R38e 只改这个文件即可接入，共用核心不需要引用任何一家名字。详细说明也在 `crates/pony/README.md`。

## 返工记录

2026-10-21，第 1 次返工，按 R38d 审查文件末尾「主控对审查意见的决定」。

### 改了什么

- 必须改 1、建议改 5、11：history 改为整频道新消息（含新旧线程回复），核心不再逐线程读取；无频道游标时仅首次不补收，后续按上次在线时间补收。历史结果跳过收件箱已有的在线消息，其他消息合并排序后路由。README 和上面的「给 R38e 的接口说明」同步更新。
- 必须改 2：在等回复时检查 `reply.at >= status.turn_started`；旧回复不贴回，改回「X 这一轮结束了，没有新回复」，再送下一句。
- 必须改 3：路由错误回 error／message 后出队；盯 agent 错误只跳过该 agent 当轮，补收错误只跳过该频道，连接事件正常出队；错误码写日志。确认发不出去只拦住该条队列。回复／提醒先保存完成及去重标记，再发送分段，部分失败和重启都不重发成功段；已发出的提醒根消息仍能续聊。io／state_invalid 等致命错误继续退出。
- 建议改 4：state 和 status 内容不变时不改写文件。seen 保留 24 小时（12 小时窗口加 12 小时余量）；不再创建逐线程游标，旧线程游标随线程清理；expired 保留 90 天。在线时间每分钟保存，给无游标频道留下最多一分钟重叠；旧版无时间的 seen／expired 仍能读取。
- 建议改 6、7：发送失败只进日志和线程，connection 只取适配器事件；不改 status JSON 或 CONTRACT。这取代首次完成记录中「发送失败进 status」的说法。带附件路由后说「附件没送」，只有附件时说「附件没送，只收文字」。
- 建议改 8：新增 `exists(channel, thread, id)`，每次连接后核对 Queued 原消息，确定删除则撤回并在线程说「撤回了」；核对出错暂缓该条并继续重试，其他 agent 照常。接口说明和 README 已写清 false 与错误的区别。
- 建议改 9、10：同一句 human_active／not_idle 退回只提示一次，记号跨重启保存；同批消息给空闲 agent 时，排队计数不含马上发送的队首。corral_unavailable 明确说「没送」，不再说「结果不明」。
- 12–17 未改；没有新增依赖，没有改设计稿、CONTRACT、corral 或其他包。paddock 不用动。未部署、合并或推送。

### 验证了什么

- 审查标「已复现」的场景分别补了测试：离线新线程回复、StopFailure 后旧回复、盯 agent 错误、路由错误、频道 channel／history 错误、发送失败后连接状态、重复拒绝提示、同批队列位置。另覆盖分段部分失败与重启、无游标提醒频道、在线与历史重叠、附件、离线撤回和核对失败、文件不改写与清理、旧状态读取、corral_unavailable。
- 严格只执行一次 `cargo test -p pony`：lib 2 项、CLI 9 项通过，核心 32 项通过、1 项新增测试失败；Cargo 因该失败未继续运行后面的 corral／split 测试。失败在 `partial_reply_or_notice_failure_does_not_repost_successful_chunks_after_restart` 的夹具：提醒标题先单独成段，夹具只允许标题通过，却断言已有正文段发出。已把故障阈值改为允许标题（或已有确认）加一段正文通过，断言不变；遵照次数限制没有重跑，因此修正后的测试尚未执行确认。
- `cargo clippy --all-targets -- -D warnings` 一次通过；`cargo fmt --check` 一次通过。没有重跑全仓测试或另跑定向测试。全部命令在前台等到结束，编译目录为 `.target/r38d-pony-core`；测试使用临时目录、内存适配器／corral 和隔离的假程序。
- 主控已定的 1–11 均按决定实现，没有新增设计决策待确认。验证上请主控决定是否允许补跑修正后的测试；未擅自突破一次限制。
