# 任务：R55 主控开免确认 agent 偶尔被自动权限检查拦下

**草稿**：用户 10-28 说「暂时先这样吧，记一个task，以后找时间再修」。派之前补路由、交给谁、几档和验证预算，先给用户看过。

2026-10-28，ranch/main 起草。
类型：技能内容变更（可能）
依据：见下面「查到的」。
优先：高

## 用户原话

- 10-28：「调查一下，claude code做主控的时候偶尔会触发」（贴的是 canter 主控派 P2-113 时，开 `canter/dev-compact-left` 被自动权限检查拦下，只好请用户用 `!` 自己跑）
- 10-28：「应该怎么解决？」
- 10-28：「爆炸半径大吗？以及如何部署？我现在还有几个主控在跑」
- 10-28：「可以，先验证一下」
- 10-28：「暂时先这样吧，记一个task，以后找时间再修」

## 查到的

- 拦下的理由：`Permission for this action was denied by the Claude Code auto mode classifier. Reason: [Create Unsafe Agents]`。
- `~/.claude/settings.json` 已有放行规则 `Bash(corral start:*)`。
- canter 主控（会话 `37fe690d-2381-4947-8edf-9571e12c4f1d`）五次开 agent，每次都把 `corral start` 和别的命令串在一起：前面接 `git worktree add … &&`、`git commit … &&`，后面接 `&& corral wait …`，有一次接 `| python3 …`。串起来的命令对不上前缀规则，交给自动权限检查判断；检查结果不固定，四次放行，一次（P2-113）拦下。
- corral-dispatch 技能（`crates/dispatch/resources/corral-dispatch/SKILL.md` 第 232 行起）只给了 `corral start` 的写法，没说要单独一条跑。
- `~/.claude/settings.json` 的 `autoMode.environment` 写的可信仓库是 owlet，没提 ranch、canter，也没说派活开免确认 agent 是用户认可的做法。

## 没查清的

- 自动模式下，单独一条 `corral start … -- claude --dangerously-skip-permissions` 是否稳定命中 `Bash(corral start:*)`、不再交给检查。这条规则能开任意程序，可能被当成太宽的规则，在自动模式下不生效。本机 `~/.claude/debug` 没有日志，看不出每次是规则放行还是检查放行。先查 Claude Code 文档里自动模式和放行规则的关系，再决定怎么实测。
- 其他项目里同样理由被拦过几次，没统计（用户打断了跨项目搜索）。

## 主控建议的修法（待用户定）

1. 先验证上面「没查清的」第一条。实测只开 `ranch/test-<名字>`（`--label role=test`），用完 `corral stop`，不碰用户的 agent。
2. 验证通过：在 SKILL.md 写明 `corral start` 单独一条跑，前后不加 `&&`、`;`、`|`；建 worktree、`corral wait` 各自另起一条。技能编进程序，要打新版本、`ranch deploy` 重装技能。
3. 可选：更新 `autoMode.environment`，补上 ranch、canter 和派活约定（用户的全局设置，用户同意再改）。
4. 不做：把 `--dangerously-skip-permissions` 藏进包装脚本躲开检查。

## 对 paddock 的影响

技能内容是公开约定，这次是兼容的补充，paddock 不用动。

## 部署要点（10-28 和用户说过的）

- 只改技能文字，程序逻辑不变；部署只切链接，在跑的主控、开发 agent、pony 不受影响。
- 在跑的主控已读进旧技能，要重开或再调技能才生效；之前可以直接告诉它们「`corral start` 单独一条跑」。
