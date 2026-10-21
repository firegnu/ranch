# pony

ranch 的独立聊天机器人包，不依赖 corral、dispatch 或 fence 的 Rust 包。
公开输出见 `docs/CONTRACT.md` 的 `pony status`，设计见 `docs/DESIGN.md` §10。

R38d 在 R38c 的命令外壳上接入共用核心。`pony run` 前台运行；
配置、凭证、状态、日志都在 `PONY_HOME`（默认 `~/.pony`）。没有配置时
`status` 报 `missing`，`run` 记一行日志后退出 0。

- `corral.rs` 只调用公开命令。按配置、PATH、`~/.local/bin/corral` 找程序，
  启动检查 `--version` 的 contract 1；只用 ls、status、reply、send。
  发送参数是 `send NAME -- 正文`，不加 force，也不读 corral 的状态文件。
- `core/engine.rs` 统一处理 owner、频道绑定、路由、线程、队列、每轮回复、
  主控新回复和 Needs you 提醒、`?`、离线补收；每 2 秒轮询，空闲须连续
  两轮稳定。未接入仓库不调用 status，也不输出其存在与状态。
- `core/state.rs` 把线程、待回复、队列原文、消息去重、补收游标和接收箱
  写入 0600 的 `state.json`：同目录临时文件同步后改名，再同步目录。
  只有拿运行锁的进程创建 Store。适配器工作线程调用 Events::emit，成功
  后才回执；落盘不依赖核心是否正在等 corral 或发消息。别人的消息在
  落盘前换成不带正文、用户 ID 的 Ignored 事件。
- 发送前持久化 Sending。重启遇到它时不重送，在线程回
  「上次发送结果不明，没重送，请确认后重发」，并记录 send_uncertain。
  这是主控决定的崩溃恢复边界；其余队列按顺序恢复。
- 每次连上读取已记录游标之后的频道和已知线程历史，按时间顺序去重、
  路由、排队；补收正文带发送时间和离线说明。超过 12 小时提示重发，
  没有游标的频道不补收。线程 30 天未用后清除 agent 对应，保留过期
  线程 ID 以便用户再回复时得到明确提示。
- `core/split.rs` 保留原文字节，按空行、行尾、代码行尾、字符边界分段，
  尽量不拆表格；代码围栏和段号另存。投递因格式或长度被拒时，仅对
  失败段对半再切，最小到 1,000 字；重切段增加自己的 `(i/N)`，已经
  成功的段不重发。最终失败会在线程、日志和 status 报告。
- `run` 的 POSIX 记录锁、配置重读、凭证权限检查和日志轮转沿用 R38c。
  不在持锁进程调用 `lock::holder` 或 `status::read`。日志只记事件、
  路由元数据和错误码，不记消息正文、回复正文或凭证。配置删掉正常退出。

## 适配器接口（给 R38e）

`Chat` 静态登记在 `CHATS`。已有的 kind、凭证键、配置解析不变；新增
`connect(Connect) -> Box<dyn Connection>`。每个连接对象负责自己的网络
工作线程，close 和 Drop 时停止并等待它们退出。同步的发送和历史接口
可以在内部使用独立工作线程和限频队列。

`Connect` 提供本家的配置、proxy、凭证读取回调和 Events：

- 每次连接／重连前调用 credentials，重新检查文件权限并读取凭证。
  网络与认证失败用 Connection 事件报告并由适配器退避重试；启动连接
  本身不应等待网络成功。
- 先 emit Connection（connecting / connected / failed，以及公开的原因、
  workspace、bot、其他连接数），再交付该连接收到的消息。不要传 token、
  ticket URL、原始 HTTP 错误或原始 payload 作为公开说明。
- Message 提供 channel、id、thread、sender、mentioned、解码后的 Markdown
  text、attachments、Unix 秒 at。只交付新消息，不交付机器人消息和编辑。
  Deleted 提供原消息的 channel、id；emit 成功才 ACK。核心去重与排队。
- Connection::name 是用户提示中的平台名称；limits 给字数上限和线程能力。
  Slack 填 Slack、10,000、有线程；无线程平台要求每句 @ 并点名 agent。
- send(Outgoing) 接收频道、可选线程、Markdown、notify，返回实际消息 ID。
  notify=true 让适配器提醒 owner；平台转义、屏蔽正文中的意外 @、通知
  文案、关闭链接预览、限频与网络重试由适配器处理。SendError 的 resize
  只用于确定未发送且可以缩小重试的长度／格式错误；code 必须是公开错误码。
- channel(id) 返回名字、是否私有、problem（无问题为 None；否则 not_member、
  not_found、archived）。history(channel, thread, after) 读完所有分页，返回
  比不透明消息 ID 更新的 Message；None 线程表示频道历史。两者的错误说明
  也不得包含凭证。核心会再检查 owner、时间、绑定及去重。

`chats/slack.rs` 本轮仅校验配置／凭证并返回占位连接，不发网络请求，
没有 connected 事件。R38e 只需在这个文件实现上述接口；核心和 status
格式无需再改。运行中的连接仍报 connecting。

对 paddock：`pony status` 的 JSON 格式不变；corral 的公开约定不变，
paddock 不用动。本轮不部署、不安装服务。service 在 macOS 仍为
not_installed，其他平台为 unsupported。

测试只用内存适配器、内存 corral 和用 Rust 编译的假命令；子进程 HOME、
PONY_HOME、CORRAL_HOME 全部隔离，不依赖用户状态和真实 agent。
