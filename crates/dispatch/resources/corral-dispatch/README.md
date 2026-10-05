# corral-dispatch

给主控用的编排技能：按任务文件拆任务，用 corral 把活派给 Claude Code、Codex 等 agent，再审查、合并、收尾。依赖 corral 技能（`corral install-skills` 安装）；corral 程序本身不引用它。

- `SKILL.md`：技能正文。
- `遥测操作.md`：随同版本安装的可选遥测全流程，离开开发 checkout 仍可查阅。
- `项目AGENTS模板.md`：项目的 AGENTS.md 里要加的一节，让主控在你说「开始」时自动按本技能分派。
- `saddle plugin run dispatch route`：路由。主控拆完每件活调一次，由 TypeSafe 的分类模型建议几档、要不要交叉审查、影响面（定验证预算）；拿不准或调不通时主控自己判断。交给哪家不走路由，由主控照 SKILL.md 第 3 节的分工表定。路由实现随 Saddle 交付，不再分发 route.py。

## 路由的 key

在 `~/.zshrc` 里加 `export TYPESAFE_API_KEY=<key>`。corral 开 agent 时从登录 shell 重建环境，之后开的主控都带着它；加 key 之前开的主控要重开。没有 key 时路由不可用，主控照 SKILL.md 第 3 节自己判断，分派照常。

发给 TypeSafe 的只有主控写的三五句任务摘要，不发任务文件。某个项目不想发，在它的 AGENTS.md 里写一句「不用路由」。

## 安装与更新

随 Saddle 启用 dispatch 插件自动安装。在 Plugins 管理页启用 Dispatch；同页可看各目标状态、接入说明和模板路径，也可用 `saddle plugin status dispatch` 只读查看。源码接入不表示日常版已安装或旧消费者已切换。

- 文件安装到已有的 `~/.claude/skills/corral-dispatch` 和 `~/.agents/skills/corral-dispatch`；缺少 agent 主目录时跳过。启用或 Sync resources 可安装缺失资源，启动 Saddle 只升级自有且未修改的旧版。
- 旧 Corral 软链接、用户修改及外来同名目录只报告冲突，不覆盖；旧链接切换留给迁移阶段。项目 AGENTS.md 不自动改动。
- 新开的会话能发现安装后的技能。停用保留技能和项目规则；路由不可用时按原项目规则或本次明确委派要求自行定档继续，不自行启用、不重试、不回退旧 route.py、不追加询问。

## 卸载资源

先在 Plugins 管理页停用 Dispatch，再选择 Remove resources，只删除本插件拥有且未修改的资源。用户文件、外来目录、旧软链接及项目 AGENTS.md 都保留。

## 可选遥测

stdin 只传任务摘要；需要记录时使用 `saddle plugin run dispatch route --record-context /绝对路径/context.json [--brief-file /绝对路径/任务书.md]`。任务书由宿主保存，不发给 TypeSafe。遥测初始关闭，记录上下文显式传入；采集关闭或失败不改变路由结果，不补跑请求。需求来源、身份与 parent、任务书快照、回复/审查/实际收尾、首尾 call_id 回执与查询见同目录 [遥测操作.md](遥测操作.md)。仅显式选择链路才采集；准备失败可在业务尝试前走原路径，已尝试后不因遥测失败、125/127 或缺回执改直调重发。旧 dlog 不再是本技能的操作依赖，不导入旧记录；历史保留供离线查看。

## 在项目里启用

照 `项目AGENTS模板.md` 在项目的 AGENTS.md 里加上「开发方式（主控分派）」一节。想确定这一次一定走本技能时，也可以点名调用：Claude Code 用 `/corral-dispatch`，Codex 用 `$corral-dispatch`。
