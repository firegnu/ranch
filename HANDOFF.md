# 交接

## 现在在哪（2026-10-05）

- 第 1 步 corral：已迁入（Saddle `a31dea2`），打包工具 `ranch-package`，42 项测试通过。任务和进度记录在 paddock：`../paddock/docs/任务/M2-ranch与corral.md`。
- `~/.local/bin/corral` 已指向 `~/.local/share/ranch/versions/df46247/bin/corral`（用户在场时切换）；原指向 Saddle `711ab18`，退回就改回去。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。

## 下一步

1. 等 Saddle 主控把 Saddle 改成用 ranch 的 corral（需求由用户转交），再用测试 agent 核对 Saddle 和 paddock 都能用；之后在 main 补收尾提交。
2. 之后转回 paddock 开发；遥测、插件协议与 dispatch 何时进 ranch，到时由用户定。

## 悬着

- 还在用 Saddle 版本目录里 corral 的会话（paddock/main、Saddle 主控）结束前，那些目录不能删。
- `crates/corral/tests/protocol.rs` 第一次并行跑时有一项卡住，之后 6 次未复现。
