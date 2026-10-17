# 任务：接入自己的cairn dispatch实现自举

2026-10-17，用户在看板上新建。
执行：主控

## 用户原话

- 10-17：“你这边是不是没有接入dispatch。总是你自己写，然后轻codex审核”
- 10-17：“R15吧，告诉我具体的步骤”
- 10-17：“等一下，paddock已经是这种方式了。理论上不应该要改claude或者codex的设置啊”
- 10-17：“其实我觉得就是按照dispatch（也就是你的repo中有一个agents模版.md里面的内容改就好了）”

## 要做的

- ranch 以后的活和 paddock 一样由主控派出去，用的是 ranch 自己装的 corral-dispatch 技能和 `ranch dispatch route`。
- 照 `crates/dispatch/resources/corral-dispatch/项目AGENTS模板.md` 第 1 节，把 AGENTS.md 的「开发方式」换成「开发方式（主控分派）」，尖括号部分填 ranch 的：
  - 名字前缀 `ranch/dev-`；
  - 任务文件 `docs/任务/`；
  - worktree `../ranch-worktrees/`；
  - 合并后推送到 origin。
- ranch 原有、模板里没有的几条留着：完成记录、推送前用 gitleaks 和 trufflehog 查、清编译子目录、更新 HANDOFF。
- 不改 Claude Code、Codex 的设置。R13 那次路由被拦，是自动模式冲着那次摘要的内容拦的（摘要里写了开免确认的 agy），不是路由命令本身要放行；paddock 主控一直在正常调路由。HANDOFF 里原来写的“要用户放行”是错的，一并改正。

## 不做

- 不改 corral-dispatch 技能、模板和路由。

## 怎么算做完

- 用户 10-17：“按照dispatch（也就是你的repo中有一个agents模版.md里面的内容改就好了）”

## 完成记录（10-17，主控）

- AGENTS.md「开发方式」按模板换掉。原来那句“由 paddock 主控兼管、自己实现”删掉，主控写成 `ranch/main`。
- HANDOFF「悬着」里“要不要改走派活”一条删掉。
- 只改文档，没有要跑的测试；`git diff --check` 通过。
