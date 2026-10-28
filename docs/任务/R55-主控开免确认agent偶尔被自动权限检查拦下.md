# 任务：R55 主控开免确认 agent 偶尔被自动权限检查拦下

**已做**：用户 10-28 说「你直接改吧」，由 ranch/main 直接改，没派 agent。

2026-10-28，ranch/main 起草。
类型：技能内容变更
依据：见下面「查到的」。
优先：高

## 用户原话

- 10-28：「调查一下，claude code做主控的时候偶尔会触发」（贴的是 canter 主控派 P2-113 时，开 `canter/dev-compact-left` 被自动权限检查拦下，只好请用户用 `!` 自己跑）
- 10-28：「应该怎么解决？」
- 10-28：「爆炸半径大吗？以及如何部署？我现在还有几个主控在跑」
- 10-28：「可以，先验证一下」
- 10-28：「暂时先这样吧，记一个task，以后找时间再修」
- 10-28：「R55要解决了，现在频繁的会出现这个问题，先给我讲一下原因」
- 10-28：「你查一下，这几次canter主控都被拦了」
- 10-28：「部署起来麻烦吗？」「可以，开始吧」

## 查到的

- 拦下的理由：`Permission for this action was denied by the Claude Code auto mode classifier. Reason: [Create Unsafe Agents]`。
- `~/.claude/settings.json` 已有放行规则 `Bash(corral start:*)`。它只对单独一条、以 `corral start` 开头的命令生效；和别的命令串起来就对不上，交给自动权限检查判断，检查结果不固定。
- canter 主控 10-24 到 10-28 最近 12 个会话里，用 Bash 开 agent 共 131 次：

  | 命令形式 | 次数 | 被拦 |
  |---|---|---|
  | 单独一条 `corral start …` | 38（其中 27 次开带 `--dangerously-skip-permissions` 的 Claude Code） | 0 |
  | 和别的命令串着（`&&`、`\|`、`;`、多行） | 93 | 11（9 次含 Claude Code，2 次只有带 `--yolo` 的 Codex） |

  串法常见三种：前面接 `git worktree add`、`git commit`、`git merge --ff-only`；后面接 `&& corral wait …`；接 `| jq …`、`| python3 …` 取结果；也有一次运行里多行开几个 agent。
- 单独一条从没被拦，说明自动模式下放行规则对它生效（若也交给检查，按串联约 12% 的被拦比例，38 次全过的可能不到 1%）。
- corral-dispatch 技能（`crates/dispatch/resources/corral-dispatch/SKILL.md` 第 4 节）只给了 `corral start` 的写法，没说要单独一条跑。
- canter 的 HANDOFF 里写着「再被拦就把整条命令给用户用 `!` 跑」，是当时的临时办法。
- `~/.claude/settings.json` 的 `autoMode.environment` 写的可信仓库是 owlet，没提 ranch、canter。

## 要做的

1. 在 SKILL.md 第 4 节写明：`corral start` 单独一条运行，前后不接别的命令，建 worktree、提交、`corral wait`、取结果都另起一条；单独一条仍被拦，把命令交给用户用 `!` 运行。
2. 打新版本，用户 `ranch deploy` 部署（会重装两份技能）。

## 不做

- 不把 `--dangerously-skip-permissions` 藏进包装脚本躲开检查。
- 不改 `autoMode.environment`（用户的全局设置；要改另问用户）。
- 不改 canter 仓库：canter HANDOFF 那条临时办法，写成需求交用户转给 canter 主控。

## 对 paddock 的影响

技能内容是公开约定，这次是兼容的补充，paddock 不用动。

## 部署要点（10-28 和用户说过的）

- 只改技能文字，程序逻辑不变；部署只切链接，在跑的主控、开发 agent、pony 不受影响。
- 在跑的主控已读进旧技能，要重开或再调技能才生效；之前可以直接告诉它们「`corral start` 单独一条跑」。

## 完成记录

- 10-28 ranch/main 直接改（用户：「你直接改吧。改完告诉我如何部署即可」）。
- 改了：SKILL.md 第 4 节 `corral start` 示例后加一段，照上面给用户看过的文字。
- 验证：`cargo test -p ranch-dispatch -p ranch` 全过。实际效果要部署后看主控开 agent 还被不被拦。
- 没做：见「不做」。canter HANDOFF 里的临时办法，部署后请用户转告 canter 主控删掉。
