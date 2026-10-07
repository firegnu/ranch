# 交接

## 现在在哪（2026-10-07）

- R1 corral pause／resume 已合并、推送、收尾（任务和两轮交叉审查：`docs/任务/R1-corral暂停与继续.md`、`R1-审查.md`；设计 `docs/DESIGN.md` §6）。**还没部署**，下面的安装仍是旧版。

- 第 1 步 corral、第 2 步 dispatch 都已完成，范围到此为止（用户 10-05：遥测、Drover、插件系统砍掉，不迁）。
- 安装：
  - `~/.local/bin/corral` → `~/.local/share/ranch/versions/df46247/bin/corral`
  - `~/.local/bin/ranch` → `~/.local/share/ranch/versions/d771c10/bin/ranch`（`d771c10` 目录里的 `bin/corral` 与 `df46247` 的校验和相同，以后升级 corral 时可一起切到新目录）
  - corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）都在 `~/.claude/skills/`、`~/.agents/skills/` 下。
- Saddle（`dedf26a` 起）和 paddock 都只调用 `corral`、`ranch` 命令。任务和完成记录在 paddock：`../paddock/docs/任务/M2-ranch与corral.md`、`M3-dispatch进ranch.md`。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。

## 下一步

1. **部署 R1（用户在场时做）**，次序照 DESIGN §6：`cargo run -p ranch-package` 打包新版本目录；切 `~/.local/bin/corral`（`ranch` 是否一起切另定）；`corral upgrade --all`，逐项核对 complete（含 `legacy_after` 提示）；`ps` 查旧版本目录下的 `corral wait`、`__after` 等进程，等它们结束或换新版重开；`corral install-skills`。全部满足后才开始用暂停。
   - `~/.corral/ranch/review-pause-1/.after/` 里留着一条主控误挂、worker 已停的提醒记录（agent 已关）；`upgrade --all` 会为它起一个 worker，因 agent 不在而结束为 not_delivered，无害。
2. 部署后 paddock 开工 P5-33（界面接 pause／resume）。
3. Saddle 要不要跟暂停（兼容新增，满足部署条件后不跟也能用），交用户定。

## 悬着

- `crates/corral/tests/protocol.rs` 第一次并行跑时有一项卡住过，之后未复现。
