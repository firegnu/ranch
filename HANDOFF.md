# 交接

## 现在在哪（2026-10-05）

- 第 1 步 corral、第 2 步 dispatch 都已完成，范围到此为止（用户 10-05：遥测、Drover、插件系统砍掉，不迁）。
- 安装：
  - `~/.local/bin/corral` → `~/.local/share/ranch/versions/df46247/bin/corral`
  - `~/.local/bin/ranch` → `~/.local/share/ranch/versions/d771c10/bin/ranch`（`d771c10` 目录里的 `bin/corral` 与 `df46247` 的校验和相同，以后升级 corral 时可一起切到新目录）
  - corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）都在 `~/.claude/skills/`、`~/.agents/skills/` 下。
- Saddle（`dedf26a` 起）和 paddock 都只调用 `corral`、`ranch` 命令。任务和完成记录在 paddock：`../paddock/docs/任务/M2-ranch与corral.md`、`M3-dispatch进ranch.md`。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。

## 下一步

- 暂无排定的活；用户转回 paddock 开发。改 ranch 时照 AGENTS.md 写明对两个前端的影响。

## 悬着

- `~/.local/share/saddle/versions/` 下旧目录已无会话在用，删不删由用户定。
- `crates/corral/tests/protocol.rs` 第一次并行跑时有一项卡住过，之后未复现。
