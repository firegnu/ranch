# 交接

2026-10-15 起本文件只记项目级的账：现状、下一步、悬着。每次会话做了什么、用户怎么说的，由 cairn 的接续摘要记，新会话开局自动注入；更早的会话经过在 git 历史里（`git log -- HANDOFF.md`）。

## 现在在哪

- 范围：第 1 步 corral、第 2 步 dispatch 都已完成，范围到此为止（用户 10-05：遥测、Drover、插件系统砍掉，不迁）。之后的活 R1–R8、R10 都已合并、推送、收尾，任务文件、审查和完成记录在 `docs/任务/`，设计在 `docs/DESIGN.md`（R10 远端安装 §3；R1 暂停 §6；R3 适配器、R6 amp、R7 读屏、R8 agy 都在 §7），公开格式在 `docs/CONTRACT.md`。
- 安装：
  - `~/.local/bin/corral`、`~/.local/bin/ranch` → `~/.local/share/ranch/versions/8299e39/bin/`（10-16 起：两个技能开出去的 Codex 都加 `--disable memories`；corral-dispatch 的完成记录和回复多写「踩过的坑」。corral 代码和 `f86d1c2` 一样，没跑 `corral upgrade --all`，`~/.corral/.runtime/helper` 仍是 `f86d1c2`）。退回：切回 `versions/14fb8d9/bin/`（没有「踩过的坑」）或 `versions/f86d1c2/bin/`。旧版本目录都保留，不删。
  - corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）都在 `~/.claude/skills/`、`~/.agents/skills/`、`~/.gemini/config/skills/` 下。
  - 用户的 Linux 实测机器（主机名不进仓库）：代码在 `~/Develop/ranch`（远端仓库设了 `receive.denyCurrentBranch updateInstead`，本机 `git push` 过去），装的是 `versions/53fe6fc/`（R10 分支上的提交，10-16 用户部署），两份技能都装了，没有在跑的 agent。那台机器上只用 pi、omp，任何情况下不跑 Claude Code、Codex，pi、omp 不委派活（用户 10-15，暂时的）。
  - 全局插件：amp 装在 `~/.config/amp/plugins/corral.ts`（10-14 用户同意后装）；agy 装在 `~/.gemini/config/plugins/corral/`（10-15 用户从 paddock Integrations 装，测过能用）。两个 `integration status` 都是 same。
- corral 和 corral-dispatch 技能开出去的 agent 带 `--env CAIRN_DISABLE=1`，不用 cairn（R2）；cairn 只留给用户直接对话的主控。开出去的 Codex 还带 `--disable memories`，不读跨项目的 Codex 记忆（用户 10-16）。
- 没有进行中的活，没有任务 worktree 和分支；`../ranch-worktrees/` 下只有编译目录 `.target/main`。
- 前端：Saddle（`dedf26a` 起）和 paddock 都只调用 `corral`、`ranch` 命令。paddock 已接完 R1（P5-33）、R4（P5-94）、R6（P5-98）、R8（P5-88），进度看 paddock 的 HANDOFF。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。

## 用户交代、一直沿用的

- Saddle 基本不用了，不跟新功能（10-07：“saddle我基本不用了”）。
- agy 很少用，接它主要是为了读屏，它的已知限制不主动追（10-14：“主要是想通过agy加进来读屏幕的能力，agy很少用。”）。
- 部署（切链接、`install-skills`、`corral upgrade --all`、装全局插件）由用户在普通终端里跑，主控事后核对。

## 下一步

1. paddock 接着做 P5-90（远程 agent 的「甲」层）：R10 装好了，由 paddock/main 请用户跑它的实测清单 4–5。ranch 这边没有要做的。
2. 草稿任务 `docs/任务/R11-路由模型升到jev-1.14.0.md`（用户 10-16）：先查 1.14.0 改了什么，动手前给用户看、定验收。
3. 草稿任务 `docs/任务/R9-常驻进程不带走调用方的描述符.md`（用户 10-14：“记为一个draft task。找时间再做”）：修「悬着」第一条。动手前再给用户看、定验收。

## 悬着

- macOS 上多个线程同时起 `corral start` 时，某一次的输出管道可能漏进另一个常驻的 pen（Rust 标准库在 macOS 上建管道和设 close-on-exec 分两步），调用方等不到输出结束，卡住不返回。
  - `protocol.rs` 的 `concurrent_unique_starts_and_listing_preserve_live_locks` 偶尔卡住就是这个（R8 期间抓到现场：一个 pen 开着测试进程的管道）。
  - paddock 同时起好几个 agent 时理论上也会碰上。
  - 修法：pen 启动时（`pen.rs` 起 pen 的 `pre_exec`）关掉 0、1、2 以外继承来的描述符。即草稿任务 R9。
- paddock 10-16 提的两件可选的事，都改公开约定，「甲」层不依赖，做不做用户还没定：`corral ls` 带上 status 的字段（或 `status --all`），远端每轮刷新从两次调用减到一次；`corral --version` 带构建号，前端能看出远端比本机旧。
- agy 的已知限制（DESIGN §7「接入 agy」，不主动追）：
  - 子 agent 的会话会不会触发钩子、抢主会话，没实测；
  - 钩子 shell 里 `exec` 失败时不回 `{}`；
  - Antigravity IDE 会不会加载插件，没核对。
- paddock 的 `docs/DESIGN.md` §3“Saddle 保底”和决策表还写着“只保证 Saddle 和运行时对得上、不兼容时写需求给 Saddle 适配”；ranch 这边已按用户 10-07“保底规矩放松吧，Saddle 不用再写影响了”改了 AGENTS.md 和 DESIGN §2，paddock 那边要不要跟着改，在 paddock 自己的任务里做。
