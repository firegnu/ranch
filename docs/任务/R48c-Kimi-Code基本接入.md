# 任务：R48c Kimi Code 基本接入（阶段 1）

2026-10-24，ranch/main 交给 ranch/dev-r48c（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.9）、要、碰要害）
类型：功能变更
依据：给 corral 加 Kimi Code 这一家，做到能开、能看状态、能送话、能取回复、能停干净，钩子装在 Kimi 自己的 `config.toml` 里 corral 的一段标记块里；派活分工表、pony、sandbox 不在本轮。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做。除了下面写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-24：「我觉得要做到现在已经接入的agent的所有功能（但是可以分阶段做，有几个阶段你要给我列一下）」；「3: 继续使用我现在的账号」。
- 10-24，主控讲了 Kimi 的钩子只能写进它自己的 `config.toml`、照 herdr 的办法插一段带标记的块之后，用户问「你说的config.toml是kimi自己的config文件吧」，主控答是 `~/.kimi-code/config.toml`、只加 corral 自己带标记的一段、卸载只删这一段；用户：「好的，起草吧」。
- 10-24：「模型我是自己改的。你不用管。」（corral 不管用户的默认模型。）

## 先读

- `docs/DESIGN.md` §7：开头的适配器结构、全局安装（R6）、读屏兜底（R7）、接入 agy（R8）、被人打断（R22）几条，和最后一条「接入 Kimi Code（R48）」。
- `docs/调研/R48b-Kimi-Code补测.md`：「结论先说」、§1–§7、「对阶段 1 的建议」；附录是钩子实收原文，要哪种事件的字段去那里查。
- `docs/调研/R48-接入Kimi-Code.md` §2、§3（钩子位置、回复在 wire 里的格式）。
- `docs/CONTRACT.md`「`corral integration status`」「`corral integration install|uninstall`」。
- 代码：`crates/corral/src/agents/mod.rs`（`Adapter`、`Global`、`env`）、`agents/agy.rs`（全局钩子经 `$CORRAL_HELPER __hook <种类>:<事件>` 的样子）、`agents/amp.rs`（`prompt()` 为 false 的做法）、`integration.rs`、`pen.rs` 的 `stop_step`。

## 主控查到的（10-24）

- 本机 Kimi Code 2.3.1，程序 `~/.kimi-code/bin/kimi`，程序文件名 `kimi`（corral 的种类就叫 `kimi`）。用户自己开的 Kimi 也读同一份 `~/.kimi-code/config.toml`。
- herdr 0.9.5（`/opt/homebrew/bin/herdr`）的 Kimi 集成是同一路子：脚本放 `~/.kimi-code/hooks/`，在 `config.toml` 插 `# >>> herdr kimi integration` … `# <<< herdr kimi integration` 一段；它找配置目录时认 `KIMI_CODE_HOME`。用户机器上没装它的 Kimi 集成。可以 `strings` 它的程序对照，不要运行 `herdr integration install|uninstall`。
- pen 停 agent 时对 agent 的进程组发信号（`kill(-pid)`）；R48b 实测 Kimi 跑的命令在另一个进程组，TERM、HUP 后都留下来。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r48c`，分支 `r48c`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r48c`。
- 只动 `crates/corral/`、`docs/DESIGN.md`（§7 Kimi 那条，照实现补细节）、`docs/CONTRACT.md`、本任务文件末尾。

## 要做的

1. 加 `agents/kimi.rs`，在 `ADAPTERS` 登记；corral 和 corral-dispatch 技能从 `~/.agents/skills` 读，照现有位置声明，不新增技能位置。
2. **钩子装法：共用文件里的一段标记块。** 给全局安装加这种装法（DESIGN §7 R48 那条的条件照做）：`corral integration install|uninstall --agent kimi` 只在 Kimi 的 `config.toml`（`$KIMI_CODE_HOME/config.toml`，没设就 `~/.kimi-code/config.toml`）里加或删 corral 那一段，块外字节不动；`--dry-run`、`--yes`、确认、`foreign` 的规矩和现有一样；`integration status` 列出 kimi 一项。块里的钩子经 `$CORRAL_HELPER __hook kimi:<事件>` 记事件，不是 corral 开的这个 Kimi（照现有全局钩子的环境和父进程检查）就立刻退出、不输出；对 Kimi 一律空输出、成功退出，不回批准或拒绝。amp、agy 的装法和行为不变。
3. **状态和回复。** 照 R48b「对阶段 1 的建议」：
   - Kimi 编辑器能输入、还没会话时，算能收话（不用 `--force` 就能 `send`）；
   - 收到输入、调工具算 `working`；批准框、提问框算 `blocked`（钩子加读屏，画面见 R48b §2）；
   - 这一轮在主会话 `agents/main/wire.jsonl` 里结束了才算 `idle`，`reply` 取这一轮主会话的文字；
   - 运行中 Esc 认出 `interrupted`；提问框上 Esc 不算打断；
   - 子 agent 的事件不改主会话的状态。
4. **首句。** `corral start … --prompt "<句>" -- kimi` 能把首句送进去，首句以 `-` 开头也行。
5. **停干净。** `corral stop` 停 Kimi 时，连它正在跑的命令（另一个进程组的子进程）一起停掉。只对 Kimi 生效，别家的停止步骤和行为不变；不能误杀不是这个 Kimi 起的进程。
6. 改了公开格式的（`integration status` 多一家、新装法在 `install.*` 里怎么报等），写进 CONTRACT，并写明对 paddock 的影响。

## 怎么算做完

- 用户要的结果（10-24）：Kimi Code 做到现有几家的全部功能，分阶段；本轮是阶段 1：corral 能开 Kimi、看状态（能收话、干活、等批准或回答、空闲、被打断）、送话、取回复、`--prompt` 带首句、停干净，钩子用 `corral integration install --agent kimi` 装进 Kimi 自己的 `config.toml` 里 corral 的一段、卸载只删这一段。
- 验证只做这些：每项新行为一条针对的自动化测试（停止和状态的修 bug 式改动先写测试、确认它先失败）；边角用例：标记块缺一半、重复、块里内容被改过、配置文件是符号链接、判断之后写之前文件被改了，都不写；卸载只删块、块外字节不变；停止不碰不属于这个 Kimi 的进程；子 agent 的 `Stop` 不让主会话变 `idle`；项目标准检查跑一次：`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`；真实 Kimi 照下面的边界把主路径走一遍（装块、开、首句、调一次要批准的工具、提问框、运行中 Esc、正常结束取回复、带着在跑的命令 stop、卸块）。觉得不够，在回复里说，不要自己加。

## 真实 Kimi 实测的边界

- 用用户已登录的账号，全部不超过 15 轮很短的输入，工作目录用临时目录。
- 往真实 `~/.kimi-code/config.toml` 装 corral 的块之前先备份；用本分支编出来的 corral 装、测、卸；测完块必须卸掉，卸完和备份比对，块外有差别写进完成记录（Kimi 自己的正常改动留着，不拿备份整份覆盖）。
- 测试 agent：名字 `ranch/test-r48c-<几个字>`，带 `--label role=test`、`--env CAIRN_DISABLE=1`，用本分支编出来的 corral 开，用完 `corral stop`。最多同时开一个。
- 自动化测试不碰真实 HOME 和真实 Kimi：用隔离的 HOME、CORRAL_HOME、`KIMI_CODE_HOME` 和合成程序。

## 不要做

- 不做派活分工表、corral/corral-dispatch 技能里的 Kimi 写法、pony、fence（阶段 2、3）。
- 不改 PATH 上装好的 `corral`、`ranch`，不打包、不部署，不切共用 helper（`~/.corral/.runtime/helper`）。
- 不运行 `herdr integration install|uninstall`，不改 herdr 的配置；不改用户的 Kimi 模型和其他设置；不读、不往仓库里抄凭证、token、`device_id`、`server.token` 的内容。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。只操作自己开的 `ranch/test-r48c-*`。
- 不按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f kimi` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉；用户自己可能也开着 kimi。停自己起的进程用记下的 PID。
- 不放宽现有测试的断言、不缩短超时、不删测试来过。
- 碰到要拿主意的事（包括要改公开格式里已有字段的含义、要动别家的行为、额度不够、真实 `config.toml` 的块装不上或卸不干净），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r48c` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、真跑了几轮、拿主意的地方、没做的事、对 paddock 的影响、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-24，ranch/dev-r48c。

- **做了什么**：登记 `kimi` 适配器，技能声明复用 `codex` 位置。增加共享配置标记块安装方式，Kimi 的 `config.toml` 只增删 corral 块，保留块外字节和权限；钩子经过现有环境、父进程检查，空输出、成功退出。接通无会话就绪、普通 send 的输入确认、PTY 首句、主 wire 状态和回复、批准／提问读屏兜底、运行中打断，以及带出生标识核对的 Kimi 子树停止。DESIGN §7 Kimi 条与 CONTRACT 已补齐，amp、agy 沿用原安装方式与停止步骤。
- **验证了什么**：就绪、主 wire 状态、首句和停止均先跑到目标失败，再实现通过；停止的 RED 是独立进程组中的命令活过了 `corral stop`。新增 7 条生命周期测试、2 条安装测试和 1 条写前竞争测试，覆盖所列标记损坏／重复／修改、符号链接、判断后文件变化、块外保留、主子会话、半行、两种取消和无关进程保护；合成命令入口虽标为 ignored，停止测试会作为子进程实际调用。`cargo test --all-targets` 完整跑一次通过；Clippy 首次指出比较时的多余字符串分配，改为解析数字轮次后，Kimi 测试重跑通过，`cargo clippy --all-targets -- -D warnings` 和 `cargo fmt --check` 均通过。未启用项目原有需要外部基线或真实 srt 的 ignored 测试，未删测试、放宽断言或缩短超时。
- **真跑了几轮**：真实 Kimi Code 2.3.1 共 **5 次短输入**，另有 2 次 Enter 批准、2 次 Esc，合计 9 次输入操作；两个不带首句的启动用于检查就绪（首个发现规则过严后停止重开）。只使用 `ranch/test-r48c-ready`、`ranch/test-r48c-main`，始终最多一个，带 `role=test`、`CAIRN_DISABLE=1`；工作目录和 `CORRAL_HOME` 都是临时目录，复用真实账号，未切默认模型。无会话普通 send 与 `--prompt` 都成功收到前导 `-` 的首句；Bash 真正写入并回读 marker，`reply` 分别返回指定文本；提问框 Esc 后为 working，随后运行中 Esc 为 interrupted 且无旧回复。最后确认 Kimi → Bash → sleep 确实在运行，Bash/sleep 的 PGID 与 Kimi 不同，stop 后三个 PID 均消失，测试命名空间为空。
- **真实配置收尾**：安装前备份真实 `config.toml`，用本分支 corral 安装、实测、卸载；卸载后 `cmp` 与备份逐字节相同，块外差异为零，没有拿备份整份覆盖。测试块已卸掉，共享 helper、PATH 上安装的程序和用户原有 agent 均未改动；Kimi 自己正常写入的会话、缓存保留。未读取凭证、token、device_id 或 server.token。
- **拿主意的地方**：无身份区分的工具、权限和结束钩子只记录观察，由主 wire 判状态，避免子 agent 抢状态或 Stop 提前报完成；每次状态查询逐行读主 wire，只保留本轮文字和最新状态，没有新增常驻监视器。标记块当前只认版本 1 原样内容，修改过的块为 foreign；新增 `install.format:"marked-block"`，不改变已有家的字段含义。新文件用硬链接只新建，已有共享文件写前比较全部内容、设备号、inode 和权限，原子替换；保留设计已有的最后核对与改名之间的并发窗口。没有需要主控决定的事项。
- **没做的事**：没有做派活分工表、两份技能内容、pony 题目／按钮、fence、打包或部署，没有合并或推送。验证严格限于任务场景，没有额外扩展真实子 agent、后台轮、运行中追发／steer 或 Linux 真机测试；子 agent Stop 的隔离由合成主／子 wire 测试覆盖。
- **对 paddock 的影响**：兼容增加 kimi 一项与可选 `install.format`，状态值和命令保持不变。paddock 自己的任务可补充“只改共享配置中的 corral 块、卸载保留配置文件”的安装说明；不需要读取 Kimi 内部目录，也不增加技能位置。
- **踩过的坑**：源码页脚说明提到权限模式，但真实默认模式并不显示它，初版就绪规则导致 starting；改为识别空编辑器和实际 context 状态行后重跑通过。旧的 `finds_programs_and_reads_versions_without_hanging` 在一次并发针对性回归里没有读到合成 Claude 版本，单独重跑和随后的完整标准测试都通过，未改其断言或超时。首个 send 合成程序只回显、不报输入钩子，确认阶段正确报 not_delivered，补成真正调用全局 hook 的合成程序后通过。

### 返工 1

2026-10-24，按用户指定处理主仓库 `docs/任务/R48c-审查.md` 对 `154c2fb` 的第 1–6 条；审查文件只读，第 7、8、9 条不改。

- **卸载换行（1）**：仅在含前置换行的块位于文件开头、块前一字节为换行、或块位于文件末尾时连前置换行一起删除；其他情况留下行间分隔。新增无末尾换行后紧接 `[x]`、删除块前空行后的等价输入、herdr 标记及文件首尾对照。原有“块后有空行”的断言按新规则精确保留前置换行，CONTRACT 和 DESIGN 同步。
- **目录 key（2）**：每段连续的不允许字符替换成一个 `-`，不再依据前一个输出字符判断；trim、截长、hash 顺序不变。用 6 组写死的目录与完整 key 对照，包括 `foo - bar`、`a-(b)`、`my-项目x`、大小写、全中文和连续标点。
- **wire 增量读取（3）**：cursor 保存路径、dev、ino、文件长度、完整行 offset、原始 `turn_started` 及本轮 turn／step／文字／工具／交互／最新状态；只读追加的完整行，逐行筛选改为按字节查找。路径或身份变化、文件变短或输入时间变化时重读；重复查询和迟到的观察钩子仍使用已累积的结果。测试覆盖不重读旧字节、追加、半行、同长度替换 inode、原 inode 截短、同一毫秒内输入时间变化和新轮清除旧回复。
- **停止上限（4）**：最后一步 KILL 后最多再等子树 5 秒，残留 PID 追加到 `pen.log` 并写入 `exit.json.remaining_pids`；Kimi 自身未退出时保持 `code:null`，超时后的 Drop 用非阻塞 waitpid，避免退出清理重新卡住。CONTRACT 和 DESIGN 补明上限。测试从最后 KILL 后的快照开始，用仍存活的合成进程模拟信号未能终止目标，分别验证父进程已退／未退两种情况，实等 5 秒并核对日志、残留 PID 和退出码；无需 root，也没有制造真正不可杀进程。
- **写前核对（5）**：Plan 保存配置目录，apply 沿用该目录做符号链接检查，不重新读取 HOME。竞争测试的临时目录 canonicalize 后分别断言内容变化、同内容替换 inode 的 `configuration changed; nothing written`，以及符号链接变化的专用错误；另测计划目录边界和装卸保持 0640 权限。
- **最终回复（6）**：出现新 step 的文字时清空前一步文字，同一 step 的片段继续连接。公开生命周期测试先写工具前说明，再写工具后的两段最终文字，确认 `reply` 只返回后者；CONTRACT 和 DESIGN 写清取最后一个有文字的 step。
- **验证**：上述六项各先获得目标行为的失败证据，再修复通过；旧竞争测试在收紧错误断言后也实际通过内容核对分支。只跑直接相关的 19 项唯一测试：`--lib integration::block::tests`（2）、`--lib workdir_keys_match_kimi_23`（1）、`--lib final_kill_wait_is_bounded`（1）、`--test kimi`（8）、`--test integration kimi_`（3）、`--test integration installs_and_removes_only_`（与前者重复 1 项，另有 amp／agy 2 项）、`--test protocol stop_timeout_does_not_cancel_pen_owned_shutdown`（1）、`--test protocol agent_exit_drains_last_output_to_attached_client`（1）；全部通过。命令均为 `cargo test -p corral-core` 加所列筛选，均使用 r48c 独立编译目录。Kimi 测试中的 ignored 项只是由停止测试实际调用的合成子进程入口。`cargo clippy -p corral-core --lib --test integration --test kimi -- -D warnings`、`cargo fmt --check`、`git diff --check` 均通过。没有跑全套，没有再真跑 Kimi，没有放宽断言或缩短等待时限。
- **范围与决定**：只按这次明确选择实现，不需主控另作决定；未修改第 7–9 条、主仓库审查文件、真实配置或其他仓库，没有打包、部署、合并或推送。paddock 无需代码适配，现有命令与状态字段不变；残留 PID 只在异常停止时兼容增加到内部退出记录。停止测试的快照用于覆盖无法安全制造的失败信号场景，普通 Kimi 子树停止及无关进程保护仍由公开命令测试覆盖。

## 主控审查

2026-10-24，ranch/main。
- 实际开的是 `ranch/dev-r48c-1`（Codex gpt-6-astra / xhigh），交叉审查 `ranch/review-r48c-1`（Claude Code opus[1m] / xhigh），审查意见和复核在 `docs/任务/R48c-审查.md`。
- 第一次提交 `154c2fb`：只动了任务允许的目录；文档和实现对得上；现场核对真实 `~/.kimi-code/config.toml` 里没有 corral 的块，没有残留的 kimi、sleep 进程，测试 agent 已关。主控跑全套：第一次 `protocol` 的 `injected_hooks_use_the_namespace_entry_and_adapter_arguments_are_preserved` 失败一次（原有的测试竞态，审查第 9 条确认和本件无关），单独复跑 3 次和随后全套都过（454 通过、14 跳过）；clippy、fmt 通过。主控记一条「建议改」：每次查状态从头读 wire。
- 交叉审查：必须改 2 条（卸载可能把块前后两行粘成一行、写坏 config.toml；`workdir_key` 和 Kimi 算法不一致，部分目录名下状态一直 working），建议改 4 条（wire 从头读、停止没上限、单测没测到比对分支、回复拼接了整轮所有 step），可以不改 3 条。主控同意全部意见，6 条一起交回返工；7–9 条不改。
- 返工 `063622c`：主控核对两条必须改的修法，跑了相关测试、clippy、fmt；因为改了共用的 pen.rs，又跑一次全套：459 通过、0 失败、14 跳过。复核 1 结论可以合并。
- 复核新提的建议改，不挡合并，没有改，留给用户定要不要另开一件：
  1. 每轮第一次从头读 wire 比原来慢（50 MB 合成数据 37 ms → 129 ms），之后的查询是微秒级；改法是每行只做一次 UTF-8 转换再 `contains`。
  2. 停止到 5 秒上限仍有残留进程时，PID 只记在 pen.log 和 `exit.json`，`corral stop` 照样返回成功，前端和派活看不出没停干净；改法是 `stop` 输出带上 `remaining_pids`（兼容新增）。
- 对 paddock 的影响：`integration status` 兼容新增 kimi 一项和 `install.format:"marked-block"`，现有用法不用改；安装说明（只改共享文件的一段、卸载保留配置文件）在 paddock 自己的任务里补。
结果：返工 1 次
