# 任务（草稿）：agy 派活修整：状态兜底、免确认、信任目录

2026-10-17 起草，ranch 主控（ranch/main）自己做。第 1 部分（实测）已做完；第 2–4 部分用户 10-17 看过，开工。
执行：主控
路由：常规 / 交叉审查要 / 影响面：碰要害（路由不可用：调用被 Claude Code 自动权限检查拦下，没有拿到结果。第 4 部分要改写用户的 agy 配置，多个 pen 可能同时写，算碰要害，交叉审查照规矩给 Codex astra / xhigh；第 2、3 部分单看是「改行为」）
类型：第 1 部分调研实测（已完成）；第 2 部分改技能内容；第 3、4 部分功能变更

## 用户要的

- 10-17：“现在我会委派一些活到agy，查一下委派agy整条路径有没有不合适的地方”
- 10-17：“先测一下，写任务文件”
- 10-17：“另外还有一个问题，委派出去的agy的permission我觉得应该是类似claude或者codex这种是bypass的”
- 10-17：“5次，Claude 不动，第2部分等实测出来之后，和以后的修复一起部署。”
- 10-17：“在同一个目录里重测5次，那5行也删掉”
- 10-17（信任框）：“关于3， 现在我看codex或者claude code并不会出现这个问题”；“但是这样的话，agy会增长很快吧？”；主控答「开 agy 时加、它退出时删、下次加时清掉目录已不在的」后：“好，写进任务文件”

## 现场（10-17 主控排查所得）

- paddock 主控 16:23:30 派出 `paddock/dev-card-details-1`（agy 1.4.0，带 `--dangerously-skip-permissions --prompt-interactive=…`）。agy 一直在干活，但：
  - 前 6 分钟 corral 一条钩子事件都没收到；16:29:51 之后才有 `UserPromptSubmit`、`PostToolUse`，后来也有了 `Stop`；
  - `SessionStart` 始终没来，事件都积在 `pending`，状态一直是 `starting`（paddock 显示叹号）。`corral wait` 等不到 `idle`，DONE 提醒不会来，`corral reply` 拿不到回复。
- agy 日志（`~/.gemini/antigravity-cli/log/cli-20261017_162330.log`）：首句 16:23:40.28 就送进了会话，登录到 16:23:41.06 才完成；那次组装提示词时 `skills`、`mcp_servers`、`user_rules` 三段都是空的。16:29:51 重新组装后，钩子才开始工作。
- agy 10-17 自己升到了 1.4.0（R8 时是 1.3.5）。
- 免确认：派活技能里 agy 已经是 `agy --dangerously-skip-permissions`，上面那个 agy 的日志也写着 `auto-approving all tool permissions`。corral 技能（`crates/corral/resources/SKILL.md`、`AGENT_USAGE.md`）里开 agy 只写 `-- agy`，不免确认；同一处 Codex 是 `--yolo`、omp 是 `--approval-mode yolo`、Claude Code 是 `claude`（也不免确认，用户定不动）。
- 信任框：三家各有一份信任记录（Claude Code `~/.claude.json` 的 `hasTrustDialogAccepted`，Codex `~/.codex/config.toml` 的 `trust_level`，agy `~/.gemini/antigravity-cli/settings.json` 的 `trustedWorkspaces`），区别在怎么查：
  - Claude Code 往上找上级目录，Codex 归到所属主仓库，所以派活开的 worktree 不弹框，记录也不随 worktree 增长（Claude Code 记录里 worktree 只有 5 条、Codex 1 条，都不是 ranch、paddock 派活的）；
  - agy 只认目录本身。列表里有 `/Users/firegnu`、`~/Developer/personal_projs`，新目录照样弹。agy 的命令行参数和程序里都没找到跳过信任框或认上级目录的选项。

## 要做的

### 1. 实测抢跑（已完成，见文末「实测记录」）

10 次 0 次复现；推测是 agy 登录慢时不等登录就送首句。暂不改首句的送法。

### 2. corral 技能里 agy 改免确认

- `crates/corral/resources/SKILL.md`、`AGENT_USAGE.md` 里开 agy 的命令改成 `-- agy --dangerously-skip-permissions`，和派活技能一致。Claude Code 不动。

### 3. agy 的状态兜底（缺了「会话开始」也能跟上）

- 现在：没收到 `SessionStart` 就没有主会话，之后的事件全积在 `pending`，状态停在 `starting`。
- 改成：**只对 agy**（适配器声明，共用代码不写 agy 的名字），还没有主会话时，收到 agy 的事件就把它的 `session_id` 当主会话，积压的事件照常算上。已有主会话时照旧，不改别家。
- 管不到：钩子完全没挂上的那一段，状态仍是 `starting`；那一轮 agy 没加载技能和 MCP，是 agy 自己的问题。
- 已在跑、积压了事件的 agy（如 `paddock/dev-card-details-1`）部署新版后会不会直接恢复，实测确认，结果写进完成记录。

### 4. 派 agy 时替它补上「上级目录信任过就算信任」

- **加**：corral 开 agy 时（pen 启动 agy 之前），这个工作目录不在 agy 的 `trustedWorkspaces` 里、而它的某个上级目录在里面，就把工作目录加进去，并在 corral 自己的记录里记下「这条是 corral 加的」。上级目录都没被信任的，不加，照旧弹框由人点。
- **删**：这个 agy 退出时（`corral stop` 或它自己退出），pen 删掉自己加的那条。
- **兜底**：pen 被强杀没删成的，下次 corral 再加条目时，顺手清掉 corral 以前加的、目录已经不存在的那些。
- **只动自己加的**：用户自己点过信任的条目一概不删不改；文件里别的设置原样保留。
- 写法照全局安装的规矩：原子写（临时文件加改名），保留文件权限；多个 pen 同时写要互斥，不能互相覆盖；文件读不了、不是合法 JSON、或结构认不出时，什么都不写，照旧弹框。
- 要用真实 agy 实测：agy 运行时自己也会写这个文件（比如用户点信任时），要确认它不会拿内存里的旧内容把 corral 加的或删的那条覆盖掉；会的话停下来找用户商量。
- 动手前先在 `docs/DESIGN.md` §7 写明：corral 会为它开的 agy 改写 agy 的信任列表，范围和条件同上。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r13-agy`，分支 `r13-agy`。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r13-agy`。
- 真实 agy 实测：自己开 `ranch/test-agy-<n>`（`--label role=test`），**固定用同一个测试目录**，用完 `corral stop`、删目录。第 4 部分的实测会往用户的 agy 信任列表里加删条目，开测前告诉用户。

## 不做

- 不碰正在跑的 `paddock/dev-card-details-1` 和其他现有 agent，只读 `status`。
- 不改首句的送法（`--prompt-interactive`）。
- 不改 Claude Code、Codex 的信任记录，也不改它们在 corral 技能里的启动命令。
- 不改派活技能。

## 怎么算做完

- 用户 10-17：“验收按你的建议”。即：
  1. 第 2 部分：corral 技能里 agy 的命令带 `--dangerously-skip-permissions`；
  2. 第 3 部分：一条测试，造「没有 `SessionStart`、只有后续事件」的 agy 事件流，修之前停在 `starting`、修之后到 `idle` 并拿到回复；别家缺 `SessionStart` 时行为不变；
  3. 第 4 部分：测试覆盖加、退出时删、清掉残留、不动用户的条目、上级目录未信任时不加、文件坏了不写、并发写不丢；真实 agy 在一个新目录里开起来不弹信任框，退出后那条已删；
  4. `cargo test --all-targets`、clippy、fmt 全过；交叉审查到「可以合并」；新版本目录打好。

## 部署

- 第 2–4 部分一起打一个版本目录，部署（切链接、`corral install-skills`）由用户在普通终端里做（用户 10-17：第 2 部分和以后的修复一起部署）。

## 用户定的（10-17，第 2–4 部分）

- “第3部分也做，验收按你的建议，开工吧。”

## 对 paddock 的影响

- 第 2 部分：技能内容有兼容的改动，paddock 不用动。
- 第 3 部分：只改内部实现，paddock 不用动；agy 的状态不再一直是 `starting`。
- 第 4 部分：corral 开 agy 时多了改写 agy 信任列表这件事，属于兼容的新增行为，paddock 不用动。paddock 用旧版 corral 开的 agy 照旧弹框。
- 在修好之前：paddock 派给 agy 的活，主控可能一直等不到完成，要人去看；每次派 agy 都要点一次信任。

## 实测记录（10-17，主控）

测试 agy 都是 1.4.0，命令照派活技能（`--dangerously-skip-permissions`、`--prompt`），首句「只回复一句：你好。最后一行写 DONE。」，用完都已 `corral stop`，临时目录已删。

| 组 | 目录 | 信任框 | 次数 | 首句在登录之后 | `SessionStart` 到了 | 拿到 DONE |
|---|---|---|---|---|---|---|
| 一 | 每次新建（`.agy-test-1`…`5`） | 每次都弹，用户在 paddock 里点掉 | 5 | 5（晚 0.8–2.1 秒） | 5 | 5 |
| 二 | 同一个已信任目录（`.agy-test-1`） | 不弹 | 5 | 5（晚 1.4–2.9 秒） | 5 | 5 |

- **抢跑没有复现**（10 次 0 次）。
- 对照启动日志的顺序：正常时「新建对话 → 约 1 秒后登录完成 → 再 1–3 秒送首句」；paddock 那次登录用了 11 秒（正常 5–6 秒），首句在新建对话后 5.4 秒、登录完成之前就送了。推测：agy 等登录有上限，登录慢时不等就送，这一轮便没有钩子和技能。10 次里登录都不慢，所以这条没法证实，也说不清出现频率。目前观察到的是 11 次里出 1 次。
- **agy 的信任只认目录本身，不继承上级目录**；点过一次后同一目录不再弹。派活每件活一个新 worktree，所以每次派给 agy 都要人点一次信任。
- 第一组 5 次每次新建目录，是主控测试设计的失误，用户因此点了 5 次信任。agy 把这 5 个目录记进了 `trustedWorkspaces`。用户要求删掉，主控去改时被 Claude Code 自动权限检查拦下，给了用户一条命令自己删。
