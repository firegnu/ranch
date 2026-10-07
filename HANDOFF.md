# 交接

## 现在在哪（2026-10-07）

- R1 corral pause／resume 已合并、推送、收尾、部署（任务和两轮交叉审查：`docs/任务/R1-corral暂停与继续.md`、`R1-审查.md`；设计 `docs/DESIGN.md` §6）。10-07 17:46 用户在场按 §6 部署：打包 `5c5540c`，用户切链接、`corral upgrade --all`（cairn/main、paddock/main、ranch/main 均 complete，两条指向已不在 agent 的旧提醒结束为 not_delivered）、`corral install-skills`；主控用 `ps` 核对没有旧版 wait／`__after` 进程（只剩一个 17:45 起的旧版 `corral attach paddock/main`，只转发不判状态）。暂停可用。

- 第 1 步 corral、第 2 步 dispatch 都已完成，范围到此为止（用户 10-05：遥测、Drover、插件系统砍掉，不迁）。
- 安装：
  - `~/.local/bin/corral` → `~/.local/share/ranch/versions/5c5540c/bin/corral`（10-07 起；旧版 `df46247` 目录保留，不删）
  - `~/.local/bin/ranch` → `~/.local/share/ranch/versions/d771c10/bin/ranch`（`d771c10` 目录里的 `bin/corral` 与 `df46247` 的校验和相同，以后升级 corral 时可一起切到新目录）
  - corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）都在 `~/.claude/skills/`、`~/.agents/skills/` 下。
- Saddle（`dedf26a` 起）和 paddock 都只调用 `corral`、`ranch` 命令。任务和完成记录在 paddock：`../paddock/docs/任务/M2-ranch与corral.md`、`M3-dispatch进ranch.md`。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。

## 下一步

1. paddock 开工 P5-33（界面接 pause／resume）。
2. ~~Saddle 要不要跟暂停~~：不跟。用户 10-07：“saddle我基本不用了”。

## 悬着

- `crates/corral/tests/protocol.rs` 第一次并行跑时有一项卡住过，之后未复现。
- Saddle 基本不用了（用户 10-07），但 AGENTS.md／DESIGN §2 的“Saddle 保底”规矩（改 ranch 写明对 Saddle 的影响、保证它和运行时对得上）要不要放松，已问用户、未答；答之前照原规矩办。
