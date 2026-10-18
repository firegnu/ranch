# 任务：出 fence 的方案——corral 怎么调它、档案和数据长什么样、给前端的命令，交用户定

10-18，ranch/main 交给 ranch/dev-r17b-design（Claude Code，opus[1m] / high）。
路由：重 / 交叉审查不要 / 影响面：看得见（路由：重（置信 1.0）、拿不准、看得见；本轮不改代码的设计，照派活规则不交叉审查）
类型：设计
依据：本轮只出 R17b（fence 本体加 `corral start --sandbox`）的方案，交用户定；不写代码，实现另派。
提示：围绕已知约束给出推荐及关键取舍；有实质备选时再比较，不凑方案数量。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 先读
- `docs/任务/R17b-fence本体.md`：全部。这是用户要的结果和验收，方案要能做到它。
- `docs/任务/R17-agent放进sandbox.md`：「用户 10-18 定的」「放在哪、边界怎么划」「以后要接 pi、omp 等」「srt 要点」几节。
- `docs/调研/R17a-srt小试验.md`：全部。四家实测通过的规则全文、撞墙记录、「给 R17b 的跨平台结论边界」。
- `docs/DESIGN.md` §3（安装、版本目录）、§7（agent 适配器）、§8。
- `docs/CONTRACT.md` 开头（公开约定怎么写）。
- `CLAUDE.md`「规矩」：兼容规矩、安装规矩、测试不依赖真实 agent。
- 代码按需看：`crates/corral/src/agents/mod.rs`（`argv`、按命令第一个词认种类）、`crates/corral/src/pen.rs`（起 agent、进程号）、`crates/corral/src/freeze.rs`（暂停）、stop 的信号步骤、`tools/package`（打包）。

## 在哪里干活
- worktree：`../ranch-worktrees/r17b-fence-design`，分支 `r17b-fence-design`（已从 main 建好）。只写方案文件 `docs/调研/R17b-fence方案.md` 和本文件末尾的完成记录，不改代码。

## 要做的
出一份方案，用户看完能逐项拍板。每项给推荐和关键取舍，只在有实质备选时才比较。至少要定下这些：
1. **corral 和 fence 之间的约定**：corral 怎么找到 fence（同一版本目录）、传什么（档案、agent 种类、工作目录、这个 agent 的状态目录、原命令……）、fence 怎么起 srt 和 agent、出错时怎样（找不到 fence、档案有错、种类不支持：启动失败，不悄悄不套墙）、退出码。约定要窄到以后 fence 剥离成独立服务时，corral 这边不用改或只改怎么找 fence。
2. **corral 这边要改的**：套了墙以后命令第一个词不再是 agent（R17a 用了同名包装脚本），种类、hook、首句参数怎么照旧；agent 外面多了 fence/srt 进程后，记的进程号、`stop` 的信号、`pause`/恢复冻结的进程树、`status`/`ls` 判断还在不在怎么办；`corral status` 里档案字段叫什么。
3. **档案和数据**：全局档案放哪、什么格式、内置哪几种；占位符有哪些；仓库补充文件叫什么、放哪、能写什么（只写额外放行）；各家数据（目录、域名、环境变量、启动参数，如 Claude 的 `CLAUDE_CODE_TMPDIR`、Codex 的本次信任表）和模型服务怎么分、怎么合并成一份 srt 规则；还没有数据的种类怎么报。照 R17a 写出四家的数据草稿。要能用在 Linux 上（写规则只用具体路径，本地 socket 不靠按路径放行），哪些是 macOS 专有项（`allowPty`、trustd）怎么处理。
4. **worktree 提交**：R17a 放行的几处（对象库、这个 worktree 的管理目录、当前分支的 ref 和 reflog）怎么由占位符算出来；`packed-refs.lock` 报错怎么看待（用户说不严格限制，不必消除，但要说清影响）。
5. **给前端的 `fence` 命令**：列档案、检查档案、看某个 agent 被拦了什么，各自的用法和 JSON 输出；被拦记录从哪来（R17a 能取到的是每家 srt 的输出，系统日志没取到）、存在哪、谁写谁读。写成能直接放进 `docs/CONTRACT.md` 的样子。
6. **srt 怎么打进版本目录**：`tools/package` 怎么装固定版本、装在哪、运行时用哪个 node（srt 是 Node 写的；本机 node 在 nvm 下）、打包机器没有 npm/node 时怎样；Linux 上同样做法行不行。
7. **兼容**：对 paddock 的影响（照 CLAUDE.md 写）；旧版 corral 和新版同时在时会怎样。
8. **实现怎么拆、怎么验**：拆成几件、先后次序、哪些能并行；自动化测试怎么做到不依赖真实 agent（隔离 HOME、CORRAL_HOME、合成程序），哪些只能用真实 agent 验（对应 `R17b-fence本体.md` 的验收）。

## 怎么算做完
- 方案文件 `docs/调研/R17b-fence方案.md` 覆盖上面 8 项，每项有推荐；有要用户拍板的，在文件开头列成问题（最多五个，按重要程度排，每个给选项和推荐）。
- 验证只做这些：`git diff --check`。觉得不够，在回复里说，不要自己加。

## 不要做
- 不改代码，不建 `crates/fence`，不改 `tools/package`。
- 不装 srt、不开 agent、不实跑（R17a 已经实测过；方案里拿不准、要实测才能定的，写成实现时的验证项）。
- 不改 DESIGN.md、CONTRACT.md（方案定了再由主控改）。
- 不要按项目名或路径批量杀进程（`pkill -f <项目名>` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。
- 不合并到 main，不推送。只在 `r17b-fence-design` 上提交。

## 做完
在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
