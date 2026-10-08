# 交接

## 现在在哪（2026-10-08）

- R2 已合并、推送、收尾（`docs/任务/R2-派活agent不用cairn.md`）：corral-dispatch、corral 技能开出去的 agent 带 `--env CAIRN_DISABLE=1`，不用 cairn（用户拿 owlet 试点 cairn）。只改技能文字，corral 程序行为与 `5c5540c` 相同。已部署（10-08 16:27 用户在普通终端里跑）：`~/.local/bin/corral`、`~/.local/bin/ranch` 都指向 `versions/d55defb/bin/`，两条 `install-skills` 已跑，主控核对 `~/.claude`、`~/.agents` 下的技能与仓库逐字节一致；没做 `corral upgrade --all`（程序代码与 `5c5540c` 相同）。正在跑的主控重开后才读到新技能。

- R1 corral pause／resume 已合并、推送、收尾、部署（任务和两轮交叉审查：`docs/任务/R1-corral暂停与继续.md`、`R1-审查.md`；设计 `docs/DESIGN.md` §6）。10-07 17:46 用户在场按 §6 部署：打包 `5c5540c`，用户切链接、`corral upgrade --all`（cairn/main、paddock/main、ranch/main 均 complete，两条指向已不在 agent 的旧提醒结束为 not_delivered）、`corral install-skills`；主控用 `ps` 核对没有旧版 wait／`__after` 进程（只剩一个 17:45 起的旧版 `corral attach paddock/main`，只转发不判状态）。暂停可用。

- 第 1 步 corral、第 2 步 dispatch 都已完成，范围到此为止（用户 10-05：遥测、Drover、插件系统砍掉，不迁）。
- 安装：
  - `~/.local/bin/corral`、`~/.local/bin/ranch` → `~/.local/share/ranch/versions/d55defb/bin/`（10-08 起；旧版 `5c5540c`、`d771c10`、`df46247` 目录保留，不删；运行中的 agent 仍由启动它的 `5c5540c` 管）
  - corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）都在 `~/.claude/skills/`、`~/.agents/skills/` 下。
- Saddle（`dedf26a` 起）和 paddock 都只调用 `corral`、`ranch` 命令。任务和完成记录在 paddock：`../paddock/docs/任务/M2-ranch与corral.md`、`M3-dispatch进ranch.md`。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。

## 下一步

1. paddock 开工 P5-33（界面接 pause／resume）。
2. ~~Saddle 要不要跟暂停~~：不跟。用户 10-07：“saddle我基本不用了”。

## 悬着

- `crates/corral/tests/protocol.rs` 第一次并行跑时有一项卡住过，之后未复现。
- paddock 的 `docs/DESIGN.md` §3“Saddle 保底”和决策表还写着“只保证 Saddle 和运行时对得上、不兼容时写需求给 Saddle 适配”；ranch 这边已按用户 10-07“保底规矩放松吧，Saddle 不用再写影响了”改了 AGENTS.md 和 DESIGN §2，paddock 那边要不要跟着改，在 paddock 自己的任务里做。
