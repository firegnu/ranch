# 任务：corral-dispatch、corral 技能开出去的 agent 一律不用 cairn

2026-10-08 起草，paddock/main 自己做（ranch 由 paddock 主控兼管、自己实现，AGENTS.md「开发方式」）。
路由：轻 / 交叉审查不要 / 影响面：看得见（只改技能文字）
类型：文档／技能文字
依据：用户打算拿 owlet 试点 cairn（`../cairn`，工作接续记忆）。owlet 由主控 `owlet/main` 用 corral-dispatch 派活；cairn 的 `adopt` 对整个仓库（含所有 worktree）生效，派出去的 agent 也会被挂上。讨论记录在 paddock 主控会话（10-08），结论见下。
提示：只改技能文字，沿用现有写法。

## 用户要的

主控说明：cairn 每轮结束会让 agent 多续跑一轮去保存；续跑提示要求“原样再给出上一条最终回答”，cairn 第一阶段实测 G 里 DONE、JSON 都保留了，但 corral 在第一次 Stop 就标 idle、记下 reply（`crates/corral/src/events.rs` 的 `Stop` 分支），主控会在 agent 还要再跑一小轮时被叫醒；派出去的 agent 各在自己的 worktree 一条工作线，worktree 删后只剩历史，进度本来就在任务文件里，存了用处不大，每轮还多花一轮。建议主控用 cairn、派出去的 agent 一律 `CAIRN_DISABLE=1`，做法是改 corral-dispatch 技能。

用户 10-08：“我理解你的建议是要修改dispatch的skill？”；问“但是如果用户没有安装cairn呢？”，主控答：只是一个环境变量，没装 cairn 时没有程序读它，行为和不加一样，技能里配一句说明。用户看过最终方案（ranch 改技能 → 用户装 cairn、在 owlet adopt → cairn 定 JSON 约定 → paddock 做只读的 Recap 面板），说“开始吧”。

主控看任务文件时问：corral 技能（开一个 agent 问个问题）开的 agent 要不要也关？用户：“你的建议呢？”主控建议也关：那类 agent 按示例开在仓库目录（`--cwd <仓库目录>`），和主控同一条 cairn 工作线，它存的停点会盖过主控的；回答完就没人再用。用户：“同意，并进 R2 开始做吧”。

## 在哪里干活
- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r2-dispatch-no-cairn`，分支 `r2-dispatch-no-cairn`（开工时从 main 建）。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r2-dispatch-no-cairn`。

## 要做的

1. `crates/dispatch/resources/corral-dispatch/SKILL.md` 第 4 节「派出去」：
- 示例 `corral start` 命令加上 `--env CAIRN_DISABLE=1`。
- 加一条：本技能开的 agent（干活的、交叉审查的）一律带 `--env CAIRN_DISABLE=1`，不用 cairn；它们的进度写在任务文件里，cairn 留给用户直接对话的主控。没装 cairn 时这个变量不起作用，照写即可，不要删。

2. corral 技能 `crates/corral/resources/SKILL.md` 和 `corral guide` 输出的 `crates/corral/resources/AGENT_USAGE.md`：示例里开 agent 的 `corral start` 都加 `--env CAIRN_DISABLE=1`，配同样一句说明；另说明：要开一个以后自己长期对话的 agent（实际是主控），不走本技能，直接 `corral start … -- claude`，带着 cairn。

不改的：`项目AGENTS模板.md`（开主控那行照旧，主控要用 cairn；且有测试固定字节）；corral 技能自检里“请先用 `corral start` 启动我”那句（那是开主控）；corral 程序的行为。

对 paddock 的影响：技能内容是公开约定，这是兼容的新增；paddock 不用动（paddock New Agent 的 cairn 开关是 paddock 自己的另一件活）。

## 怎么算做完
- 用户原话：“我理解你的建议是要修改dispatch的skill？”“但是如果用户没有安装cairn呢？”“开始吧”“同意，并进 R2 开始做吧”。
- `git diff --check`；`cargo test --all-targets`（含技能编进程序、装技能的测试）、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 都过。
- 部署（用户在场，照 AGENTS.md「安装」）：主控打包到 `~/.local/share/ranch/versions/<提交>/`，用户在终端里切 `~/.local/bin/ranch`、`~/.local/bin/corral`，跑 `ranch dispatch install-skills`、`corral install-skills`（只换程序和技能，不 `corral upgrade --all`）；主控核对装上的技能与仓库一致。正在跑的主控（如 `owlet/main`）重开后才读到新技能。

## 完成记录

- 做了什么：`corral-dispatch/SKILL.md` 第 4 节示例加 `--env CAIRN_DISABLE=1`，并加一条“本技能开的 agent（干活的、交叉审查的）一律带它”及理由、“没装 cairn 时不起作用，照写，不要删”。corral 技能 `SKILL.md` 两处示例、`AGENT_USAGE.md`（`corral guide`）两处示例同样加上，各配一句说明：临时委派的 agent 开在同一目录，会把停点存到调用方的工作线上；要开以后自己长期对话的 agent，直接 `corral start … -- claude`，不带这个变量。
- 验证了什么：`git diff --check`；`cargo test --workspace --all-targets` 全过（含 lifecycle 里装技能的测试）；`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 过。另用本分支 debug 版在临时 HOME 里跑 `ranch dispatch install-skills --target claude --yes`、`corral install-skills --yes`，装出的两份技能都带新内容；`corral guide` 输出里有 3 处 `CAIRN_DISABLE`（两处示例、一句说明）。临时目录已删。
- 拿主意的地方：说明句放在示例下面第一条（dispatch）／`start` 那一步（corral 技能）；交叉审查的 agent 也算在内（它也是本技能开的一次性 agent）。没改 `项目AGENTS模板.md` 和 corral 技能自检里开主控的那句。
- 没做的事：部署（切 `~/.local/bin/ranch`、`~/.local/bin/corral`，跑两条 `install-skills`）要用户在场、在普通终端里跑；cairn 本身的安装和 owlet 的 `adopt` 不在本任务。
