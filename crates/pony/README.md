# pony

ranch 的独立聊天机器人包，不依赖 corral、dispatch 或 fence 的 Rust 包。
公开输出见 `docs/CONTRACT.md` 的 `pony status`，设计见 `docs/DESIGN.md` §10。

R38c 实现 `pony --version`、只读的 `pony status` 和前台 `pony run` 外壳。
配置、凭证和运行文件都在 `PONY_HOME`（默认 `~/.pony`）；没有配置时，
`status` 报 `missing`，`run` 记一行日志后退出 0。

- `chats/mod.rs` 的 `Chat`、`CHATS` 声明适配器种类、凭证键和配置读取。
  `chats/slack.rs` 校验成员与频道 ID；共用核心不写适配器名字。
  连接、事件翻译、收发消息、频道信息留给 R38e。
- `run` 用 POSIX 记录锁持有 `run/lock`，第二份退出 1。`status` 用
  `F_GETLK` 查询持锁进程，不拿锁；旧状态文件不代表仍在运行。
- 凭证从已检查的目录描述符用 `openat` 打开，拒绝符号链接，检查
  当前用户所有权、目录 0700、普通文件 0600，再读内容。解析错误只带
  文件、行号和固定说明，不带原文；本轮不连网络，成功校验后丢弃值。
- `run/status.json` 同目录临时文件写好后原子替换，不存 owner 或凭证。
  `status` 不打开凭证文件；凭证错误来自最近一次 `run` 校验的记录，
  修正凭证后由下一次 `run` 校验更新。未记录过连接变化时 `since` 为 0。
- 每 2 秒重读配置；变坏时保留旧配置并报 `invalid`，恢复有效配置后生效。
  本轮运行中的连接状态为 `connecting`。移除配置后正常退出。
- SIGINT、SIGTERM 正常退出并记录原因；强制结束且未留下退出记录时，
  `last_exit` 为 `null`。日志只记固定事件／错误码，1 MiB 轮转，保留 3 份旧日志。

对 paddock：兼容新增。后续部署会在 PATH 上提供 `pony`，`pony status`
照 CONTRACT 输出，没有配置时报 `missing`；corral 原有约定和现有功能不变。
R38c 不部署，不安装服务；`service` 在 macOS 固定为 `not_installed`，
其他平台为 `unsupported`。

测试用临时 HOME、PONY_HOME、CORRAL_HOME 和假 token，不依赖用户状态或真实 agent。
