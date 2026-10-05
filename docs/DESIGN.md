# ranch 设计

## 1. 是什么

每台机器、每个用户只能有一份的运行时：数据和全局位置只能有一个主人。用户 10-05 定：把它们从 Saddle 里拉出来，独立成 ranch；Saddle（TUI）和 paddock（GPUI）都只是前端（来历见 `../paddock/docs/背景与决策记录.md` §6g–§6j）。

| 部分 | 来源 | 状态 |
| --- | --- | --- |
| corral（agent 运行时，命令 `corral`） | Saddle `crates/corral-core`，提交 `a31dea2` | 第 1 步 |
| 遥测的存储与命令 | Saddle `src/telemetry/` | 第 2 步 |
| 插件协议、dispatch 插件 | Saddle `crates/plugin-protocol`、`plugins/dispatch` | 第 3 步 |
| Drover | Saddle `plugins/drover` | 未定（paddock DESIGN §3 第 8 步） |

## 2. 与前端的关系

- 前端只调用 ranch 装好的命令，读 JSON 输出，不在 Cargo 里引用 ranch 的包，不读内部状态目录。各自的依赖、锁文件、工具链互不进入。
- 全局位置归 ranch：`~/.corral`、`~/.local/bin/corral`、corral 技能（`~/.claude/skills/corral`、`~/.codex/skills/corral`），以后还有遥测数据库和 dispatch 技能目录。Saddle、paddock 不再打包、安装这些。
- Saddle 是保底版（用户 10-05）：不再加新功能，只保证和运行时对得上。改 ranch 时按 AGENTS.md 的规矩写明对两个前端的影响。
- 单独分发 paddock 时，打包带上 ranch 的某个版本（paddock DESIGN §7）。

## 3. 安装

- 打包工具（Rust）生成不可变版本目录 `~/.local/share/ranch/versions/<ranch 提交>/`：`bin/corral`、`share/corral/`（随程序的资源和设计文档）、`BUILD.txt`（提交、目标平台、工作区是否干净、校验和）。目录已存在就拒绝；只生成目录，不切链接、不装技能。
- 切换 `~/.local/bin/corral` 是单独的部署操作，用户在场时做。已在运行的 agent 由启动它的那份 corral 管理；要换过来用 `corral upgrade`（用户 10-05 定：第 1 步切换时不做）。

## 4. 步骤

1. **corral**（任务：`../paddock/docs/任务/M2-ranch与corral.md`）：迁入、打包工具、切换 `~/.local/bin/corral`；Saddle 侧改成用 ranch 的 corral（需求交 Saddle 主控）。做到 Saddle 和 paddock 都能用为止（用户 10-05：“先把corral从saddle中拉出去，直到saddle能够和新的corral一块工作”）。
2. **遥测**：存储与命令迁入，数据库格式不变；提供与 `saddle telemetry`、`saddle agent` 用法相同的命令（名字到时定），Saddle 保留旧命令转发。
3. **插件协议与 dispatch**：协议的类型怎么让两个前端共享到时定；dispatch 作为两个前端都能加载的插件，技能归 ranch 安装。

## 5. 待定

- 第 2、3 步什么时候做（第 1 步完成后转回 paddock 开发，用户到时定）。
- 插件协议的共享方式。
