# 交接

## 现在在哪（2026-10-05）

- 第 1 步 corral：已迁入（Saddle `a31dea2`），打包工具 `ranch-package`，42 项测试通过。任务和进度记录在 paddock：`../paddock/docs/任务/M2-ranch与corral.md`。
- `~/.local/bin/corral` 已指向 `~/.local/share/ranch/versions/df46247/bin/corral`（用户在场时切换）。Saddle 已剥离 corral、改用 PATH 上的（Saddle `4c86983`）；现有会话 saddle/main、paddock/main 已用 `corral upgrade` 接过来。第 1 步完成。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。

## 下一步

1. 第 2 步 dispatch（不带遥测；任务文件在 paddock 的 `docs/任务/M3-dispatch进ranch.md`，待用户看过）。做完后转回 paddock 开发。遥测、Drover、插件 SDK、插件协议不迁（用户 10-05）。

## 悬着

- `~/.local/share/saddle/versions/` 下旧目录已无会话在用（10-05 升级后），删不删由用户/Saddle 主控定。
- `crates/corral/tests/protocol.rs` 第一次并行跑时有一项卡住，之后 6 次未复现。
