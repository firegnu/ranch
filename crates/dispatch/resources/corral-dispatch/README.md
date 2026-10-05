# corral-dispatch

给主控用的编排技能：按任务文件拆任务，用 corral 把活派给 Claude Code、Codex 等 agent，再审查、合并、收尾。依赖 corral 技能（`corral install-skills` 安装）；corral 程序本身不引用它。

- `SKILL.md`：技能正文。
- `项目AGENTS模板.md`：项目的 AGENTS.md 里要加的一节，让主控在你说「开始」时自动按本技能分派。
- `ranch dispatch route`：路由。主控拆完每件活调一次，由 TypeSafe 的分类模型建议几档、要不要交叉审查、影响面（定验证预算）；拿不准或调不通时主控自己判断。交给哪家不走路由，由主控照 SKILL.md 第 3 节的分工表定。路由实现随 ranch 交付，不再分发 route.py。

## 路由的 key

在 `~/.zshrc` 里加 `export TYPESAFE_API_KEY=<key>`。corral 开 agent 时从登录 shell 重建环境，之后开的主控都带着它；加 key 之前开的主控要重开。没有 key 时路由不可用，主控照 SKILL.md 第 3 节自己判断，分派照常。

发给 TypeSafe 的只有主控写的三五句任务摘要，不发任务文件。某个项目不想发，在它的 AGENTS.md 里写一句「不用路由」。

## 安装与更新

用 `ranch dispatch install-skills` 安装或更新（`--dry-run` 只看会写什么，`--target claude|codex` 只装一处）。写文件前会列出并请你确认；非交互运行要在你同意后加 `--yes`。

- 文件装到 `~/.claude/skills/corral-dispatch`（设了 `CLAUDE_CONFIG_DIR` 时装到它下面）和 `~/.agents/skills/corral-dispatch`。
- 软链接的目录或文件只报告、不覆盖；目录里不属于本技能的文件留着并提示。项目 AGENTS.md 不自动改动。
- 新开的会话能发现安装后的技能。路由不可用时按原项目规则或本次明确委派要求自行定档继续，不重试、不回退旧 route.py、不追加询问。

不用了就直接删掉这两个目录。

## 在项目里启用

照 `项目AGENTS模板.md` 在项目的 AGENTS.md 里加上「开发方式（主控分派）」一节。想确定这一次一定走本技能时，也可以点名调用：Claude Code 用 `/corral-dispatch`，Codex 用 `$corral-dispatch`。
