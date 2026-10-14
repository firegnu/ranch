# 任务：corral integration status（只读，列出各家 agent 的接入情况）

2026-10-14 起草，ranch 主控（ranch/main）自己做（paddock/main 转达用户需求；用户这次没指定谁做，按 R3 的做法由 ranch 主控做）。
路由：常规 / 交叉审查不要 / 影响面：改行为（路由：几档拿不准（常规 0.69），主控定常规；交叉审查拿不准（安全隐私 0.35、核心规则 0.22），主控定不要：只读、不碰现有命令；影响面拿不准，主控定改行为：新增公开命令）
类型：功能变更
依据：本轮只加一条只读命令和它的公开约定，顺带把「技能装到哪」从按家改成按位置（R3 的 `skill_base`），现有命令行为不变；不做 install/uninstall、全局钩子、新 agent。
提示：只读；外部程序（`--version`）要有超时，卡住或失败不能拖累整条命令。

## 用户要的

用户 10-14 和 paddock 主控讨论 paddock P5-94「Settings 里一键配置 agent」时说：
- “其实我的最终目的是要想herdr的settings里面的一个agent适配的功能一样。”
- “那么我们是不是也可以这么做？（限于omp，pi，claude code以及codex这种）。”
- 听了主控对这条命令的解释后：“按你的建议，先给ranch提这条命令”。

paddock 要在 Settings 里加一页，每家 agent 一行。「corral 支持哪几家、每家怎么接」只有 corral 的适配器登记表知道，paddock 不想写死名单，所以请 corral 出一条只读命令，把登记表和本机现状打出来（需求原文见 paddock 仓库 `docs/任务/P5-94-Settings里一键配置agent.md`，草稿）。

## 主控核对过的事实（技能目录）
- Claude Code：`$CLAUDE_CONFIG_DIR` 或 `~/.claude` 下的 `skills/`，项目里 `.claude/skills/`。
- Codex：`~/.agents/skills/`（corral 现在 `--target codex` 装的位置）。
- pi 1.2.0：自己的 `~/.pi/agent/skills/`、项目 `.pi/skills/`；另外读 `~/.agents/skills/` 和项目往上找的 `.agents/skills/`（pi 自带 `docs/skills.md`、`dist/core/package-manager.js`）。
- omp 18.10.4：Claude 的用户技能目录和项目往上找的 `.claude/skills/`；`.agents/skills/`、`.agent/skills/`（项目往上找 + 主目录）；`~/.codex/skills/`；还有 opencode、`.github/skills/`（omp 程序里的说明字符串和 Claude 技能加载代码）。
- 所以 corral 能装的两个位置：`~/.claude/skills`（`--target claude`）Claude Code 和 omp 读；`~/.agents/skills`（`--target codex`）Codex、pi、omp 读。R3 把位置挂在 claude、codex 两个适配器上，pi、omp 报 None，不符合事实。

## 在哪里干活
- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r4-integration-status`，分支 `r4-integration-status`（开工时从 main 建）。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r4-integration-status`。

## 要做的

1. **技能位置改成按位置登记**：`skills.rs` 里一张位置表，两项，名字就是现有的 `--target` 值：`claude`（`$CLAUDE_CONFIG_DIR` 或 `~/.claude`，`--project` 时 `<项目>/.claude`）、`codex`（`~/.agents`，`--project` 时 `<项目>/.agents`）。适配器改成声明自己读哪几个位置：claude → `claude`；codex → `codex`；pi → `codex`；omp → `claude`、`codex`。`install-skills` 认的 target、顺序、路径、输出都不变（R3 的 `adapters_skills.json` 快照不改，照旧通过）。
   - `--target codex` 这个名字容易误会（实际是几家共用的 `~/.agents`），但改名是改公开用法，这件不改；要改另立一件（比如加 `agents` 作别名）。
2. **`corral integration status`**：只读，不写任何文件、不碰在跑的 agent。和 corral 其他命令一样总是输出一行 JSON（不加 `--json` 参数；corral 没有非 JSON 输出）。输出（字段名以此为准，写进公开约定）：
   ```json
   {"ok":true,"agents":[
     {"kind":"claude",
      "path":"/Users/u/.local/bin/claude",
      "version":"2.1.299 (Claude Code)",
      "hooks":"launch",
      "skills":[{"target":"claude","path":"/Users/u/.claude/skills/corral/SKILL.md","status":"same"}]},
     …]}
   ```
   - `agents`：登记表里每家一项，按登记顺序（claude、codex、pi、omp）。
   - `kind`：种类，就是 `corral start … -- <程序>` 认的程序名。
   - `path`：按调用者的 PATH 找到的第一个可执行文件，没找到为 `null`。
   - `version`：找到时跑 `<path> --version`（stdin 空、不接终端、各家并行、每家最多等 3 秒，超时整组进程杀掉），取输出第一行去掉首尾空白；超时、失败、没输出或没找到都为 `null`，不影响 `ok` 和退出码。
   - `hooks`：钩子怎么接。现在四家都是 `"launch"`：corral 启动 agent 时用启动参数或状态目录里的文件挂上，不写全局配置。以后要装进全局配置的会是别的值（那时再定），前端遇到不认识的值要能显示、不报错。
   - `skills`：这家会读到的、corral 能装的技能位置，按位置表顺序；`target` 是 `install-skills --target` 要传的值，`path` 是 corral 技能文件的位置，`status` 和 `install-skills --dry-run` 同一套判断：`create`（没装）、`same`（已是最新）、`overwrite`（装了旧的或别的内容）、`foreign`（是符号链接等不归 corral 管的，不会动它）。只报用户级位置（不看项目目录）；agent 自己专有的目录（如 pi 的 `~/.pi/agent/skills`）corral 不装，不报。
   - 出错（比如读技能文件时权限错误）按 corral 现有规矩报 `ok:false`。
3. **公开约定**：ranch 还没有自己的 `docs/CONTRACT.md`（corral 的基础约定在原 corral 仓库 `6923da1` 的 `docs/CONTRACT.md`，`corral guide` 也这么指）。新建 ranch `docs/CONTRACT.md`，写明「ranch 起在原约定之上的新增」，这件只写 `integration status` 一节（暂停等以前的新增仍在 DESIGN §6 和 README，不在这件补）。`corral` 的用法行、crate README 加上这条命令。`corral guide`（给 agent 看的）不加：这条是给前端用的。
4. `docs/DESIGN.md` §7 补一句：技能位置按位置登记，各家声明读哪些位置；`integration status` 由登记表生成。

## 对 paddock 的影响（写进完成记录）
- 公开约定兼容新增：一条新命令。paddock 照 `docs/CONTRACT.md` 写解析和测试；命令是 `corral integration status`，没有 `--json`。
- 页面上的 Install／Update 照旧调 `corral install-skills --target claude|codex --yes`（用户确认后）。一个位置几家共用，装一次几家都有。

## 怎么算做完
- 用户原话：“其实我的最终目的是要想herdr的settings里面的一个agent适配的功能一样。”“按你的建议，先给ranch提这条命令”。
- 测试（隔离的 HOME、PATH，合成程序，不用真实 agent）：四家都按登记顺序出现；找不到的为 `null`；`--version` 正常、失败、没输出、卡住（3 秒左右返回、卡住的进程被杀掉）；技能四种 status；`CLAUDE_CONFIG_DIR`；命令不写任何文件（跑前跑后 HOME 下的文件一样）。R3 的快照不改、照旧通过。
- `git diff --check`；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。
- 在本机跑一次（只读）新编的 `corral integration status`，结果和实际一致。

## 不要做
- `integration install/uninstall`、全局钩子、agy、amp、读屏兜底（以后另提）。
- 不改 `install-skills` 的 target 名和行为；不加 `--json`。
- 不切 `~/.local/bin/corral`；部署照规矩，用户在场、在普通终端里跑。

## 部署（用户在场）
- 主控打包、核对；用户切 `~/.local/bin/corral`、`~/.local/bin/ranch`。技能内容不变，不跑 `install-skills`。只加了命令行这边的新命令，pen 和钩子没变，不需要 `corral upgrade --all`。

## 完成记录

- 做了什么：
  - 技能位置按位置登记：`skills.rs` 的 `TARGETS`（`claude`、`codex`）和 `base`、`plan`（某个位置的技能文件和装／删会怎样）；`install-skills` 改用它们。适配器的 `skill_base` 换成 `skill_targets`：claude → `claude`；codex、pi → `codex`；omp → `claude`、`codex`。
  - `corral integration status`（`src/integration.rs`，`cli.rs` 接入、用法行和 `--help`）：按登记顺序列出每家的 `kind`、`path`、`version`、`hooks`、`skills`，格式同任务文件第 2 条。
  - 新建 `docs/CONTRACT.md`（ranch 在原 corral 约定之上的新增，这件只写 `integration status`）；根 README、`crates/corral/README.md`、`docs/DESIGN.md` §7 补上。
- 验证了什么：
  - `crates/corral/tests/integration.rs`（隔离的 HOME、PATH，合成程序）：四家按顺序、全没装时 path／version 为 null、技能全是 create 且路径对；PATH 里不可执行的文件和相对目录不算；`--version` 正常（取第一行非空、去空白）、退出码非 0、没输出、卡住（3 秒左右返回，卡住的程序和它留下的子进程都被杀掉）；`CLAUDE_CONFIG_DIR` 下 same、`~/.agents` 下 overwrite、符号链接 foreign；跑前跑后 HOME 下的文件逐字节一样；`integration`、`integration install`、多余参数、`--json` 都报 usage。
  - 反向检查：把杀进程组改成只杀主进程，测试抓到留下的子进程（改回后通过；那次留下的两个 `sleep 60` 核对是测试进程组的，按 PID 停掉）。
  - R3 的快照（含 `install-skills` 的 `adapters_skills.json`）未改、照旧通过；`git diff --check`；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 都过。
  - 本机只读跑新编的 `corral integration status`：四家路径和版本（claude 2.1.299、codex-cli 0.162.1、pi 1.2.0、omp/18.10.4）都对，技能都是 same，用时约 1.3 秒。
- 拿主意的地方：
  - 不加 `--json`（corral 只输出 JSON）；`--target codex` 不改名；只报 corral 能装的用户级位置。
  - `version` 只要退出码 0 的标准输出；标准错误不看。
  - `hooks` 写成字符串，以后的全局接法再配字段；约定里写明前端要容忍不认识的值。
  - 约定里加了一句别放进每秒的刷新（要等各家 `--version`）。
- 没做的事：install/uninstall、全局钩子、新 agent；`--target` 改名；部署（要用户在场）。
- 对 paddock：兼容新增一条命令 `corral integration status`（没有 `--json`），格式见 `docs/CONTRACT.md`；Install／Update 照旧调 `corral install-skills --target claude|codex --yes`。
