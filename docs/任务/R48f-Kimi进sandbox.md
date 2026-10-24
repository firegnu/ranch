# 任务：R48f Kimi Code 进 fence 的 sandbox（Kimi 阶段 3）

2026-10-24，ranch/main 起草（草稿，未定、未派）。

## 用户原话

- 10-24：「1: 我觉得要做到现在已经接入的agent的所有功能（但是可以分阶段做，有几个阶段你要给我列一下）……」。主控列的阶段：0 补测 → 1 基本接入 → 2 派活 → 3 sandbox → 4 Slack 按钮（可选）。0、1、2 已做完（R48b、R48c、R48d，R48e 补了阶段 1 的两条建议）。
- 10-24：「阶段3和阶段4新建两个draft task」。

## 现状

- fence 已支持 Claude Code、Codex、pi、omp（`crates/fence/data/agents/*.toml`，`profiles/dev.toml`），设计见 `docs/DESIGN.md` §8。Kimi 还没有 fence 数据，`corral start --sandbox` 开不了 Kimi；派活表里 Kimi 只能点名、不套墙。
- 摸底在 `docs/调研/R48-接入Kimi-Code.md` §6（只看了路径和网络，没真进墙跑）：要连的域名（managed API、`/search` `/fetch`、发行 CDN、遥测）、要读的路径（可执行文件、config.toml、tui.toml、credentials、oauth/region、技能、MCP/插件配置）都是推测；Kimi 没有 `--mcp-config` 参数，查资料 MCP 怎么注入未知；`~/.kimi-code/server/`、`server.token` 是什么待查。

## 要做的（派之前再定）

- 用真 Kimi 在墙里实测：普通 TUI、用户自己的 provider、关掉自动更新和遥测，确认登录身份刷新、配置写入、文件读写、子进程、网络哪些会被拦，定出最小放行。
- 写 Kimi 的 fence 数据，`corral start --sandbox` 能开 Kimi；查资料怎么给（沿用 Exa MCP 还是 Kimi 自带搜索）。
- 派活表、技能里 Kimi 能不能套墙派；paddock sandbox 界面要不要跟。

## 待定

- 验收、派给谁、实测额度，派之前问用户。
- 只做 macOS；Linux（5090）沿用 R17c 暂缓。
