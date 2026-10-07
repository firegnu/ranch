# 任务：corral 加 pause／resume（冻结 agent，原地继续）

2026-10-07 起草，paddock/main 自己做（ranch 由 paddock 主控兼管、自己实现，AGENTS.md「开发方式」）。交叉审查交给 Codex（重：gpt-6-astra / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重 1.0，交叉审查要（并发 0.81、核心规则 0.76），影响面碰要害）
类型：功能变更
依据：本轮只在 corral 里加暂停和继续，以及暂停对现有命令的影响；不改 agent 程序本身，前端（paddock）的界面另起任务 `../paddock/docs/任务/P5-33-agent暂停.md`。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。

## 用户要的

用户 10-07：“agent能否有一个pause的功能。pause就是暂停而不是停止……不让其对外有连接或者之类的行为”；主控给了“打断”和“冻结”两种，用户：“要冻结那种”。用法（用户 10-07）：“一般的暂停，我都会等全部任务都完成之后，打一个结了之后才会暂停……有的时候网络不好的时候或者晚上我要下班了不再开发了，我不想一个个agent关掉（我担心不关掉只idle的话回应为网络问题，agent出问题）”。用户担心碰到 Claude Code、Codex 的底层，主控说明原理（系统信号 `SIGSTOP`／`SIGCONT`，和 Ctrl+Z、合上笔记本同一类，不改 agent 程序、配置和文件，对 claude、codex、pi、omp 和任何程序都一样）后，用户：“明白了，按你说的做”。

## 先读
- `docs/DESIGN.md` §2、§3；`AGENTS.md`「改动要写明对两个前端的影响」「安装」。
- 代码：`crates/corral/src/pen.rs`（`request` 里的 op、`stop_step`、`Drop`、agent 用 `setsid` 起成自己的进程组）、`pen/upgrade.rs`（快照，原地升级和 recover）、`cli.rs`（`status`、`turn_end`、`send`、`stop`、帮助和参数表）、`after.rs`、`state.rs`（`ls`）。

## 在哪里干活
- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r1-pause`，分支 `r1-pause`（开工时从 main 建）。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r1-pause`。

## 要做的

1. **`corral pause NAME`**：pen 先对 agent 的进程组发 `SIGSTOP`，再沿进程树找出另开了进程组的子孙进程一并发 `SIGSTOP`，直到找不到新的为止。已经暂停的再 pause 返回成功（幂等）。
2. **`corral resume NAME`**：对冻结时记下的进程和 agent 的进程组发 `SIGCONT`。没暂停的 resume 返回成功。
3. **状态**：`status` 和 `ls` 的每个 agent 多 `paused`（true／false）和 `paused_at`（时间戳或 null）。原有的 `state`（idle、working…）不变：不认这两个字段的旧前端照常工作。
4. **暂停期间别的命令**：
   - `send`、`keys` 拒绝，错误码 `paused`，不排进输入队列。
   - attach 进来的键盘输入丢掉，不在终端里攒着、醒来时一起涌进去。画面照常能看（就是冻结那一刻的样子）。
   - `wait`：暂停不算 idle 也不算安静，一直等到超时；`--quiet` 的计时从恢复那一刻重新算。
   - `--after`：被等的 agent 暂停时不算做完；要送达的 agent 暂停时等它恢复再送，不算失败。
   - `stop`：先解冻再按原来的步骤停（冻住的进程不处理 `HUP`／`TERM`）。
   - `upgrade`、`recover`：pen 原地换新后仍是暂停，记下的进程不丢。
5. **旧的 pen 不认 pause**：在运行中的 agent 由旧版 pen 管时，`corral pause` 报 `unsupported`，提示先 `corral upgrade NAME`；不退回到在命令行这边直接发信号。
6. 帮助、`guide`、README、corral 技能里写上这两条命令和 `paused` 字段。

## 对两个前端的影响（写进完成记录）
- 公开约定兼容新增：两条命令、`paused`／`paused_at` 字段、错误码 `paused`／`unsupported`。
- Saddle：不跟也能用；暂停的 agent 在 Saddle 里看起来是原来的状态，往里 send 会收到 `paused` 错误。要不要跟，交用户决定。
- paddock：界面在 P5-33 做。

## 怎么算做完
- 上面 1–6 条达到。
- 验证：
  - 测试（隔离的 HOME、CORRAL_HOME，合成程序，不用真实 agent）：pause 后 agent 和它另开进程组的子进程都处于停止状态，resume 后都恢复；重复 pause、resume；暂停中 send、keys 被拒、attach 输入被丢；wait 不返回、`--quiet` 不误报；`--after` 两个方向；暂停中 stop 能停掉；暂停中原地升级后仍暂停、resume 能恢复。
  - 在仓库根跑 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。
  - 实测一次：用新编的 corral（不切 `~/.local/bin/corral`，指定程序路径和隔离的 CORRAL_HOME）开 `ranch/test-pause`（`--label role=test`）跑真的 `claude`，问一句、等它 idle、pause 几分钟、resume、再问一句能正常回答；用完 stop。
- 交叉审查（Codex，重档，只读，自己的 detached worktree）：重点看漏冻（另开进程组、冻结过程中新起的进程）、暂停中 stop／pen 退出时进程会不会一直冻着、wait／after 的判断、升级快照、和旧版 corral 同时操作同一批 agent 时的兼容。

## 不要做
- 不改 claude、codex、pi、omp 的程序、配置、hooks 和会话文件；不往终端里送任何按键来实现暂停。
- 不切 `~/.local/bin/corral`，不 `corral upgrade` 任何在运行的 agent：打包、切换、升级是单独的部署操作，用户在场时做（AGENTS.md「安装」）。
- `corral ls` 里的 agent 都是用户的，不对它们 pause、stop、send、keys、upgrade。实测只用自己开的 `ranch/test-pause`。
- 不改 Saddle、paddock 仓库。
- 不按名字批量杀进程；停自己起的进程用记下的 PID。
- 做不到“冻全进程树”时停下来报告，不退回到只冻进程组。

## 做完之后（主控，用户在场）
1. 合并、推送、打包新版本目录。
2. 切 `~/.local/bin/corral`。
3. `corral upgrade --all`：给现在开着的 agent（包括 `cairn/main`、`ranch/main`、`paddock/main`）原地换新 pen，不重启 agent；之后它们才能被暂停（用户 10-07 同意按主控的建议做）。
4. 装技能（`corral install-skills`）。
