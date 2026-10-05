# ranch 设计

## 1. 是什么

每台机器、每个用户只能有一份的运行时：数据和全局位置只能有一个主人。用户 10-05 定：把它们从 Saddle 里拉出来，独立成 ranch；Saddle（TUI）和 paddock（GPUI）都只是前端（来历见 `../paddock/docs/背景与决策记录.md` §6g–§6j）。

| 部分 | 来源 | 状态 |
| --- | --- | --- |
| corral（agent 运行时，命令 `corral`） | Saddle `crates/corral-core`，提交 `a31dea2` | 第 1 步 |
| dispatch（路由命令 `ranch dispatch route`、`corral-dispatch` 技能，不带遥测） | Saddle `plugins/dispatch` | 第 2 步 |
| 遥测、Drover、插件系统 | Saddle | 不迁；用户 10-05 定砍掉，Saddle 删除代码，数据留在磁盘上 |

## 2. 与前端的关系

- 前端只调用 ranch 装好的命令，读 JSON 输出，不在 Cargo 里引用 ranch 的包，不读内部状态目录。各自的依赖、锁文件、工具链互不进入。
- 全局位置归 ranch：`~/.corral`、`~/.local/bin/corral`、corral 技能（`~/.claude/skills/corral`、`~/.agents/skills/corral`），以后还有 `~/.local/bin/ranch` 和 `corral-dispatch` 技能目录。Saddle、paddock 不再打包、安装这些。
- Saddle 是保底版（用户 10-05）：不再加新功能，只保证和运行时对得上。改 ranch 时按 AGENTS.md 的规矩写明对两个前端的影响。
- 单独分发 paddock 时，打包带上 ranch 的某个版本（paddock DESIGN §7）。

## 3. 安装

- 打包工具（Rust）生成不可变版本目录 `~/.local/share/ranch/versions/<ranch 提交>/`：`bin/corral`、`bin/ranch`、`share/corral/`（随程序的资源和设计文档）、`BUILD.txt`（提交、目标平台、工作区是否干净、校验和）。目录已存在就拒绝；只生成目录，不切链接、不装技能。
- 切换 `~/.local/bin/corral`、`~/.local/bin/ranch` 和安装技能（`corral install-skills`、`ranch dispatch install-skills`）是单独的部署操作，用户在场时做。已在运行的 agent 由启动它的那份 corral 管理；要换过来用 `corral upgrade`（用户 10-05 定：第 1 步切换时不做）。

## 4. 步骤

1. **corral**（任务：`../paddock/docs/任务/M2-ranch与corral.md`）：迁入、打包工具、切换 `~/.local/bin/corral`；Saddle 侧改成用 ranch 的 corral（需求交 Saddle 主控）。做到 Saddle 和 paddock 都能用为止（用户 10-05：“先把corral从saddle中拉出去，直到saddle能够和新的corral一块工作”）。
2. **dispatch**（任务：`../paddock/docs/任务/M3-dispatch进ranch.md`）：路由做成 `ranch dispatch route`，去掉遥测；`corral-dispatch` 技能归 ranch 安装（`ranch dispatch install-skills`），删去遥测部分、命令改成 ranch 的；Saddle 删掉内置 dispatch 插件、放掉技能归属（需求交 Saddle 主控）。

## 5. 待定

- （已定，用户 10-05）只再拆 dispatch（不带遥测），之后转回 paddock 开发；遥测、Drover、整个插件系统砍掉（Saddle 删代码），两个前端只靠 ranch 的 corral 和 dispatch。
- 插件协议的共享方式。
