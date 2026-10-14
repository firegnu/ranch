# 任务：corral 恢复适配器结构（一家一个适配器）

2026-10-14 起草，ranch 主控自己做，不委派（用户 10-14，经 paddock/main 转达）。
路由：重 / 交叉审查不要 / 影响面：碰要害（路由：重 1.0；交叉审查拿不准（核心规则 0.38、并发 0.30），用户 10-14 定不要；影响面拿不准，主控定碰要害：部署后所有新开的 agent 都走新代码）
类型：重构
依据：本轮只把 corral 里按 agent 种类写的代码收成「一家一个适配器 + 一张登记表」，行为、公开约定、内部文件一律不变；不加新 agent、不加新命令、不动全局配置。
提示：先在旧代码上钉住快照，再搬；快照文件提交后不再改，改完逐字节一样。

## 用户要的

用户 10-14 和 paddock 主控讨论 paddock P5-94「Settings 里一键适配 agent」时说：
- “我记得我之前就说过，各个agent是按照这插件的方式接入到ranch的（代码级别的插件类型）。”
- “先恢复适配器结构，全局配置可以放宽。”
- “好，那就先在 ranch 立任务，重构适配器结构。你把需求发给他，让他主控做，不再委派，这是一个有难度的活，让他小心点。”

原来 Python 版 corral（`../corral/src/corral/agents/`）就是这样：`base.py` 加 claude、codex、pi、omp 各一个模块，在 `__init__.py` 的 `ADAPTERS` 里登记，每家写明种类、钩子文件、启动时怎么接（`build`）、怎么退出（`quit_steps`）。迁到 Rust（Saddle `corral-core` → ranch）后这层被摊平了。目标：以后加一家只加一个文件、登记一行。

## 先读
- `../corral/src/corral/agents/`（结构和每家的注释，比如 codex 为什么必须 `--dangerously-bypass-hook-trust`、为什么停止要等 60 秒）。
- 现在散落的地方（主控已核对）：
  - `crates/corral/src/hooks.rs`：`known`（:50）、`argv` 里 claude／codex／pi|omp 各怎么接（:88、:110、:125）、`quit_steps`（:152）。`run`（钩子写事件）、`helper`（`.runtime/helper`）是各家共用的，留在这里。
  - `crates/corral/src/events/raw.rs`：只认 pi／omp（:32）、omp 没界面忽略（:39）、omp 的 `ask` 算等回答（:82）、pi 退出原因（:145）。
  - `crates/corral/src/skills.rs`：`--target` 认 `claude`／`codex`（:30）、各装到哪（:55-69）。
  - `crates/corral/src/state.rs:20-25`：内部文件清单里的 `hook.py`、`hook_pi.ts`、`hook_omp.ts`；`resources/hook_pi.ts`、`hook_omp.ts`。
  - 调用点：`pen.rs:751`（argv）、`pen.rs:826`（种类＝程序名）、`cli.rs:132`、`:512-516`（known、种类）、`cli.rs:649`（quit_steps）、`events.rs:204-214`（raw）、`cli.rs:431`（skills）、`upgrade.rs:9`（helper）、`lib.rs:68`（`__hook`）。
  - 共用状态机 `events.rs:78` 认 codex 的 `Interrupt`：这是 v1 事件格式里的事件名（钩子写进事件文件的），属于共用的事件词表，不搬进 codex 适配器，只在 codex 适配器里注明它会发这个事件。

## 在哪里干活
- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r3-adapters`，分支 `r3-adapters`（开工时从 main 建）。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r3-adapters`。

## 要做的

1. **先钉快照（第一个提交，只加测试和快照文件，不动产品代码）**，在旧代码上跑通：
   - 四家和一个不认识的程序（如 `cat`）各自拼出的启动命令：带／不带 `--prompt`、带用户自己的参数；helper 路径、栏位目录换成占位符后比对。pi／omp 写进栏位目录的钩子文件内容。
   - 四家和默认的停止步骤。
   - 认不认得、能不能带 `--prompt`（不认识的报 usage）。
   - pi／omp 原始事件翻译：每个分家特例（omp 没界面、omp `ask`、pi 非 quit 退出、pi 的 `agent_settled`、omp 的 `agent_end` 延时）和共有路径，喂一串事件，比对翻译后的状态；不认识的 adapter 报 `incompatible`。
   - 技能装到哪：`--target all|claude|codex`、`--project`、`CLAUDE_CONFIG_DIR` 下的路径；非法 target 报错。
   - 内部文件清单 `state::FILES` 原样。
   快照存成仓库里的数据文件；这个提交之后快照文件不再改，合并前用 `git diff <第一个提交> -- <快照文件>` 证明没动过。
2. **适配器结构**（`crates/corral/src/agents/`）：
   - `mod.rs`：适配器接口（trait）、登记表、按程序名找适配器、不认识时的默认停止步骤。
   - `claude.rs`、`codex.rs`、`pi.rs`、`omp.rs`：每家写明种类、钩子文件（如有）、启动时怎么接、停止步骤、技能装到哪（如有）、原始事件里的分家规则（如有）。把 Python 版里有用的理由注释带回来。
   - pi、omp 共用的原始事件翻译留在 `events/raw.rs`，分家的那几处改成问适配器。
   - 共用的地方（`pen.rs`、`cli.rs`、`skills.rs`、`events`）只查登记表，不再写某一家的名字。
3. 帮助、README 只在有提到这层结构的地方跟着改。`docs/DESIGN.md` 加 §7「agent 适配器」：结构、加一家要做什么；并记下用户 10-14「全局配置可以放宽」及条件（经用户确认才装、钩子不在 corral 启动的 agent 里就什么都不做、只动自己那块、原子写、能卸干净），注明这件不改代码、留给以后装全局钩子那件（用户 10-14 同意现在记）。

## 不变的（公开约定和内部格式）
- 公开命令、参数、JSON 输出、退出码；`install-skills --target` 认的值。
- 事件文件格式（v1、v2）、钩子写出的事件、`hook_pi.ts`／`hook_omp.ts` 内容、`.runtime/helper` 的用法、cursor 格式、内部文件清单（含旧的 `hook.py`）。
- 现有 protocol、compat、lifecycle、pause、upgrade 测试全过，不改断言。

## 对 paddock 的影响
- 只改内部实现：paddock 不用动。
- 已开着的 agent：它们的钩子走 `~/.corral/.runtime/helper`，新版 `ls`／`status` 读它们写的事件，格式不变；`stop` 由新版命令行按新代码算停止步骤，快照保证一样。

## 怎么算做完
- 用户原话：“先恢复适配器结构，全局配置可以放宽。”“这是一个有难度的活，让他小心点。”
- paddock 主控转达的要求：行为完全不变，先写改前快照在旧代码上跑通，再重构，改完逐字节一样；现有 protocol、compat 测试全过；公开命令的 JSON、事件文件格式、钩子写出的事件、`.runtime/helper` 的用法、内部文件清单都不变。
- 验证：`git diff --check`；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`；快照文件自第一个提交起未改。
- 真实 agent 实测（用户 10-14 同意）：用新编的 corral（隔离的 CORRAL_HOME，不切 `~/.local/bin/corral`）开 `ranch/test-claude`、`test-codex`、`test-pi`、`test-omp`（`--label role=test`），各问一句、等到 idle、读到回复、`stop` 停干净。

## 不要做
- 不加新 agent（agy 等）；不加 `corral integration …` 之类的新命令（paddock P5-94 以后另提只读的 `corral integration status --json`）。
- 不装、不改全局钩子和任何全局配置（放宽的决定以后那件用）。
- `crates/dispatch/src/skills.rs` 也有 `claude`／`codex` 两个装技能的位置，它是另一个包、不引用 corral-core，这件不动。
- 不切 `~/.local/bin/corral`、不 `corral upgrade` 任何在运行的 agent；`corral ls` 里的 agent 都是用户的，不碰。实测只用自己开的 `ranch/test-*`。
- 不按名字批量杀进程。

## 部署（用户在场）
- 主控打包到 `~/.local/share/ranch/versions/<提交>/`、核对；用户在普通终端里切 `~/.local/bin/corral`、`~/.local/bin/ranch`。技能内容没变，不跑 `install-skills`；行为没变，不需要 `corral upgrade --all`。
- 退回：把 `~/.local/bin/corral` 链接切回旧版本目录（现为 `versions/d55defb/bin/corral`）。
- 爆炸半径：部署后所有新开的 agent 走新代码（拼错命令＝钩子没接上、状态一直空；停止步骤错＝停不干净）。部署后主控开一个 `ranch/test-*` 走一遍再告诉用户可以照常用。

## 用户 10-14 定的
- 交叉审查：不要，全部主控自己做。
- 真实 agent 实测：四家都测。
- 设计文档：现在就记（结构 + 放宽全局配置的决定）。

## 完成记录

- 做了什么：
  - `0cf7e2d` 先在旧代码上钉快照（只加测试和快照文件）：`crates/corral/tests/adapters.rs` 走公开命令，四家和不认识的程序（`other`、`claude-code`）各自的启动命令（不带参数、带用户参数和 `--`、带 `--prompt`、空 `--prompt`、按 PATH 找的裸名字）、写进状态目录的钩子文件及权限、`status` 的 kind／state、`stop` 的退出码和停在哪一步，以及 `install-skills` 各 target、`--project`、`CLAUDE_CONFIG_DIR`、`--remove`、非法 target、不存在的项目目录；`src/adapter_snapshot.rs` 钉认不认得、停止步骤、`state::FILES`；`src/events/raw.rs` 的测试把 pi／omp 各特例和共有路径的事件串逐条喂给翻译，记下每步后的完整状态。快照在 `crates/corral/tests/snapshots/`。
  - `8f6eb04` 重构：`crates/corral/src/agents/`（`mod.rs` 接口 `Adapter`、登记表 `ADAPTERS`、`find`／`kind`／`known`／`quit_steps`／`argv`；`claude.rs`、`codex.rs`、`pi.rs`、`omp.rs`），带回 Python 版的理由注释。`hooks.rs` 只留 `run`、`helper`；`pen.rs`、`cli.rs`、`skills.rs`、`events/raw.rs` 改查登记表。
  - `docs/DESIGN.md` §7：结构、加一家要做什么、用户 10-14 放宽全局配置的决定和条件（这件不改代码）。
- 验证了什么：
  - 快照文件自 `0cf7e2d` 起未改（`git diff 0cf7e2d -- crates/corral/tests/snapshots` 为空），重构后全部快照照旧通过。
  - 反向检查快照有没有用：逐个故意改坏（pi 退出原因、omp `ask`、omp 没界面、codex Interrupt 超时、claude 停止等待、默认停止步骤、codex 技能目录、claude `--project` 目录、首句前的 `--`、按程序名找适配器、pi 不收原始事件、codex 停止步骤），每一处都有快照报错，改回后通过。
  - `git diff --check`；`cargo test --all-targets`（protocol、compat、lifecycle、pause、upgrade 等原有测试不改断言，全过）、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 都过。
  - 真实 agent：用本分支编的 corral 副本、隔离的 CORRAL_HOME（不切 `~/.local/bin/corral`，不碰用户的 agent），依次开 `ranch/test-claude`、`test-codex`、`test-pi`、`test-omp`（`--label role=test`，`--env CAIRN_DISABLE=1`）：`--prompt` 问一句、`wait` 到 idle、`reply` 拿到 `pong`；`send` 第二句确认送达、`wait` 到 idle、`reply` 拿到 `ping`；`stop` 停干净（claude、omp 由 SIGHUP 停，pi 由 SIGHUP 停、退出码 0，codex 由两次 Ctrl-C 停、退出码 0）。之后查过没有残留进程，临时目录已删。
- 拿主意的地方：
  - 接口按「问适配器」写，而不是把整段翻译搬进每家：pi、omp 原始事件翻译大部分相同，留在 `events/raw.rs`，分家的四处（收不收原始事件、omp 没界面的忽略、omp `ask` 算等回答、pi 只有 quit 才算结束）改成适配器方法，默认值就是不分家的行为。
  - 技能位置进适配器（claude、codex 有，pi、omp 没有），`--target` 认的值由登记表来，仍是 `all|claude|codex`。
  - `state::FILES` 不由登记表拼：它也管 agent 名字合不合法，是公开行为；保持原样，单元测试查每家写的文件都在里面。
  - 种类（程序文件名）三处算法统一成 `agents::kind`。`pen.rs` 原来对没有文件名的路径会 panic，现在得到空字符串；这种路径（以 `..` 结尾或 `/`）是目录，启动程序那一步就已失败，到不了这里，行为不变。
  - 实测要用短路径：scratchpad 路径太长，Unix socket 放不下，改在 `/tmp/r3.XXXX`（和测试一样），用完删除。
- 没做的事：交叉审查（用户 10-14 定不要）；`crates/dispatch/src/skills.rs` 里同样的两个技能位置（另一个包，不引用 corral-core）；部署（要用户在场）。
