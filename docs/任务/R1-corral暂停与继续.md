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

## 完成记录

**做了什么**
- 新增 `src/freeze.rs`：`freeze` 先对 agent 的进程组发 `SIGSTOP`，再沿进程树（macOS 用 `proc_listchildpids`，其他系统读 `/proc`）找子孙，逐个 `SIGSTOP`，直到一轮找不到新的；`thaw` 对冻结时记下的、当前树里的和整个进程组都发 `SIGCONT`（多发无害，少发才有害）。
- pen（`src/pen.rs`）：新增 `pause`／`resume` 两个 op 和 `paused`、`frozen`、`resumed_at` 三个字段（`serde(default)`，旧快照读进来就是没暂停）；`status` 带 `paused`、`paused_at`，capabilities 加 `pause`。暂停中 send／keys 回 `paused`，attach 的键盘输入丢掉（调整大小照常）；stop 先恢复再走原来的步骤；pen 的 `Drop` 和 agent 退出后都先恢复，冻住的东西不会留下。
- 升级（`src/pen/upgrade.rs`、`lib.rs`）：`__pen-probe` 多报 `pause:1`；agent 暂停中时，目标不报这一项就拒绝升级和 Hold 里的 recover（回 `paused`，提示先恢复）。
- 命令行（`src/cli.rs`、`state.rs`、`after.rs`）：`corral pause|resume NAME`；`status`、`ls` 带 `paused`、`paused_at`；退出码 10 `paused`；旧 pen 回 `bad_op` 时报 9 `unsupported`，提示先 `corral upgrade`。`wait` 暂停中不算 idle、不算安静，`--quiet` 从恢复时刻重新计时，超时信息写 `still paused`；`send --after` 被等的一方暂停不算做完，收件方暂停就继续等，到期写 `expired`（`paused`）。
- 文档：`corral guide`、corral 技能（退出码表加 10，写明“不要替人 resume”）、README；改过的迁入文件开头改成“ranch has changed it since”。

**验证了什么**
- 先写测试、确认因为没有 `pause` 命令而失败，再实现。`tests/pause.rs` 7 个：整棵树（含另开会话的子进程）冻住和恢复、重复 pause／resume、`ls`；暂停中 send、keys 退 10，attach 打的字恢复后也没到 agent；wait 不误报 idle／stopped-quiet、恢复后 quiet 重新计时；`--after` 两个方向；暂停中 stop 能停、另开会话的子进程被解冻；暂停中原地升级后仍暂停、能恢复；目标不认暂停时拒绝升级。`freeze.rs` 单元测试 1 个。新测试连跑 5 遍都过。
- 仓库根 `cargo test --all-targets`（corral 原有 37 个和 dispatch 21 个照旧通过）、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`、`git diff --check` 都过。
- 实测：新编的 corral（没切全局链接，单独的 CORRAL_HOME）开 `ranch/test-pause` 跑真的 `claude --model sonnet --effort low`，答完一句后 pause：claude、gitlab／playwright 两个 MCP、uv／python、caffeinate 全部是 `T`；冻 3 分钟期间 send 退 10；resume 后全部回到运行，再问一句正常回答（`wait` 结果 idle）。用完已 stop。

**拿主意的地方**
- 退出码：暂停用新的 10（7、8 的意思不一样）；旧 pen 不认用 9（版本不兼容）。`send --after` 把 10 当成“可以等”的拒绝，和 7、8 一样。
- 暂停前已经收下、还在排队的输入（例如一条 send 的回车）照样写进终端，等恢复后 agent 读到；之后的输入一律拒收或丢掉。
- `--after` 收件方暂停时仍受 `--timeout` 约束，过了期限是 `expired`，不无限等。
- `resumed_at` 只在 pen 的 status 里给 `wait` 用，没加进公开的 `status`。

**没做的事 / 已知边界**
- （审查后改为按会话找，见下）唯一冻不住的：在冻结那一刻正好另开自己的会话、父进程又同时退出的进程（守护进程的两次 fork），以及暂停前就已经这样脱离的守护进程。
- 冻结期间如果有人从外面杀掉树里的进程，系统可能给它那一组发 `SIGHUP`／`SIGCONT`（孤儿进程组规则），那一组会被提前唤醒或挂断。
- 冻结记下的进程号若在暂停期间被外部杀掉又被复用，恢复时会对新进程发一次 `SIGCONT`（对正在运行的进程没有影响）。
- 观察：Claude Code 带着的 `caffeinate` 也会被冻住，但它持有的“不睡眠”断言还在，冻住不等于让 Mac 可以睡。
- Saddle 没跟（兼容新增，暂停的 agent 在 Saddle 里显示原来的状态，send 收到 `paused`），跟不跟交用户定。
- 部署（切 `~/.local/bin/corral`、`corral upgrade --all`、装技能）没做，等用户在场。

### 交叉审查后的修改（R1–R7 都认可，都改了）
- **R1 冻结中漏冻**：`freeze.rs` 改为每轮读一次进程表，冻“agent 的子孙＋这些进程所在会话里的所有进程”，直到没有新的；父进程退出、被 init 收养的孙进程仍在原会话里，照样冻住（暂停前就这样脱离的也冻住）。冻完逐个确认处于停止状态（或已退出），1 秒内确认不了就全部解冻、回 `freeze_failed` 并列出进程号，不再无条件报成功。剩下冻不住的只有在那一刻另开新会话又失去父进程的守护进程，写进 README 和这里的边界。
- **R2 升级和暂停同轮**：升级被接受到完成之间，pause／resume 一律回 `upgrade_busy`，交出去的快照不会在中途变成暂停。
- **R3 Hold 里 agent 被杀**：Hold 循环在 agent 已退出（含未回收的僵尸）且处于暂停时，解冻记下的进程并保存快照。
- **R4 unknown 先于 paused**：`wait` 和 `send --after` 改为先看暂停，再看 unknown；没有 hooks 的程序暂停时也一直等。
- **R5 已收下的消息因暂停确认不到**：普通 send 到期时若 agent 暂停中或在这段时间里恢复过，返回 `ok`、`confirmed:false`、`paused:true`（不重送，不报 3）；`send --after` 的确认窗口在暂停期间顺延，恢复后再给 15 秒。
- **R6 旧客户端混用**：旧版命令行和旧提醒进程不读 `paused`，新版这边改不了它们。写进 `docs/DESIGN.md` §6 和 README：只有换成新 pen 的 agent 才能暂停，部署时先切链接、再 `corral upgrade --all`（pen 和持久提醒一起交给新版），全部 complete 后才用暂停；Saddle 默认按 PATH 找 corral，切链接后即是新版。用户 10-07：“R6 我同意”。
- **R7 技能**：`SKILL.md` 补上 pause／resume 的用法和 `paused` 字段，保留“由用户决定”的边界。
- 新增回归测试 4 个（暂停前父进程已退出的孤儿进程被冻住、无 hooks 程序暂停时 wait／after 不提前、收下后暂停不报丢、Hold 里 agent 被杀后解冻），先在审查时的代码上确认 4 个都失败，再在修改后通过；`tests/pause.rs` 共 11 个，连跑 3 遍都过。R2 的同轮竞态没有写确定性的测试（要靠调度屏障），靠“升级进行中拒绝”的构造保证。全套 test、clippy、fmt 都过。改完后用真的 claude 再实测一次冻结、恢复。

### 复核后（第二轮）
- 复核确认 R2、R3、R4、R5、R7 修好；R1、R6 未闭环。
- R1：冻结那一刻另开新会话、父进程又同时退出的进程会漏掉，按会话找也堵不死（macOS 没有系统级冻结一组进程的手段）。照任务“做不到冻全进程树时停下来报告”交用户，用户 10-07 选“接受”。写进 DESIGN §6 和 README。
- R6：部署条件补上“确认没有旧版 corral 的 wait、无记录提醒还在跑”（切链接前就起的旧 wait 不会被 `upgrade --all` 换掉），用户 10-07 同意。DESIGN §6、README 已改，“Saddle 不跟也能用”改成以这些部署条件为前提。
