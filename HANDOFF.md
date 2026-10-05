# 交接

## 现在在哪（2026-10-05）

- 仓库刚建：规矩（AGENTS.md）、设计（docs/DESIGN.md）。由 paddock 主控兼管。
- 正在做第 1 步 corral，任务文件在 paddock：`../paddock/docs/任务/M2-ranch与corral.md`。

## 下一步

1. 迁入 corral（Saddle `a31dea2`），写打包工具，检查通过后合并。
2. 用户在场时切换 `~/.local/bin/corral`。
3. Saddle 主控改成用 ranch 的 corral 后，用测试 agent 核对 Saddle 和 paddock 都能用。

## 悬着

- 还在用 Saddle 版本目录里 corral 的会话（paddock/main、Saddle 主控）结束前，那些目录不能删。
