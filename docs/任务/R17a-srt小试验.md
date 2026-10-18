# 任务：srt 小试验——在本机把 Claude Code、Codex、pi、omp 关进 srt，各干完一件活

10-18，ranch/main 交给 ranch/dev-r17a（Codex，gpt-6-astra / medium）。
路由：轻 / 交叉审查不要 / 影响面：看得见（路由：轻（置信 0.74）、要、碰要害；推翻：本轮不改代码的调研，照派活规则一律算看得见、不交叉审查）
类型：调研
依据：本轮只在本机实测 srt 能不能关住四家 agent 正常干活、该拦的拦不拦得住，写成报告给 R17b（fence 本体）用；不改 ranch 代码，不做 fence。
提示：区分已查证事实、推测和未知，给出直接依据；结论限定在本轮调查范围。
你是被委派的 agent：照本文件做。除了下面「在哪里干活」里写明的测试 agent，不要再开别的 agent。

## 先读
- `docs/任务/R17-agent放进sandbox.md`：全部（为什么做、用户定了什么、fence 以后的样子、srt 要点、pi/omp 要注意的）。
- `docs/DESIGN.md` §8。
- `CLAUDE.md`「规矩」一节：不干扰用户的 agent、测试 agent 的命名、不按名字杀进程。
- srt 的 README：`github.com/anthropics/sandbox-runtime`（或装好后包里的 `README.md`）。

## 在哪里干活
- worktree：`../ranch-worktrees/r17a-srt-trial`，分支 `r17a-srt-trial`（已从 main 建好）。只在这里写报告和完成记录：`docs/调研/R17a-srt小试验.md`、本文件末尾。不改 `crates/` 下任何东西。
- srt：装 `@anthropic-ai/sandbox-runtime@0.0.80`，用 `npm install --prefix <目录>` 装到你自己的临时目录里，不装全局（不用 `-g`），不改 `~/.srt-settings.json`，规则文件都用 `--settings` 指定。本机已有 `sandbox-exec`、`rg`。
- 试验用的仓库：自己在临时目录里建一个小 Rust 项目（git 仓库，带一个 worktree），agent 在 worktree 里干活。不要拿 ranch、paddock 或别的真实仓库当试验品。
- 测试 agent：用 corral 开，名字 `ranch/test-sandbox-<家>`（`claude`、`codex`、`pi`、`omp`），带 `--label role=test`、`--env CAIRN_DISABLE=1`，Codex 加 `--disable memories`；用完 `corral stop`。四家照派活时的免确认参数起：`claude --dangerously-skip-permissions`、`codex --yolo`、`pi`、`omp --approval-mode yolo`。模型、强度用各家默认即可，活要小。
- 已知：corral 按命令第一个词的文件名认 agent 种类（`crates/corral/src/agents/mod.rs` 的 `argv`），直接 `corral start ... -- srt ... -- claude` 会被当成不认识的程序，不挂 hook。不改代码的话，可以在临时目录放一个和 agent 同名的包装脚本（比如叫 `claude`，里面 `exec srt --settings ... -- <真正的 claude> "$@"`），用它的路径起。用别的办法也行，在报告里写清楚。

## 要做的
1. 四家各在 srt 里干完一件小活：在 worktree 里改代码、`cargo build`、`cargo test`、`git commit`。规则照 10-17 定的：读、写、上网一律默认不许，按需放行（`denyRead` 圈掉 `~` 再放回需要的，写只放行需要的，网络只放行需要的域名）。撞墙了就补规则，记下每次补的是什么、为什么。
2. 每家都试这几件该被拦的事，看是不是被拦住：读自己放的假密钥文件（放在规则没放行的目录里，模拟 `~/.ssh`）、写主仓库的工作区文件、`git push` 或连一个没放行的网站。
3. 找被拦的记录：从哪里能看到（srt 的输出、macOS 系统日志或别的），怎么按某一个 agent 取出来。
4. 关在墙里时，`corral ls`、`corral status` 里这个 agent 的状态是不是照常更新（starting → working → idle 这类）。
5. 顺带记下（不当成要过的关）：
   - 套了墙以后 `corral stop` 能不能正常停掉；`corral pause`/恢复对它有什么影响；
   - 每家的网络库认不认 srt 设的代理变量（pi 是 Node，`NODE_USE_ENV_PROXY` 一类要不要设），伪终端要不要开 `allowPty`；
   - worktree 提交要放行主仓库 `.git` 里的哪几处，能不能不放行主仓库的 HEAD、index、config 和别的分支。
6. 报告 `docs/调研/R17a-srt小试验.md`，给 R17b 用：
   - 四家最后能跑通的规则文件（全文），以及哪些是公共的、哪些是每家特有的（目录、域名）、哪些是模型服务的（如 Anthropic、OpenAI、deepseek 的域名）、哪些是这个仓库特有的（编译目录、cargo）；
   - 第 2–5 项的结果；
   - 每一项在 Linux（bubblewrap）上做不做得到：照 srt README 判断，标明是推测、没在 Linux 上试（Linux 写规则不认通配符、本地 socket 只有全开全关这类限制要对上）；
   - 撞墙记录：卡在哪、怎么补的；
   - 还没弄清的。

## 怎么算做完
- 用户原话：“先验证 agent 关在墙里还能正常干完一件活（编译、测试、提交）”；四家都在本机：“是的，四家都在本机先跑通，验收加那三条。”
- 该拦的确实拦住（读假密钥、写主仓库、push 或连没放行的网站）。
- 被拦的记录找得到。
- 关在墙里时 `corral ls` 的状态照常更新。
- 验证只做这些：`git diff --check`；报告里每家附上关键命令和输出摘录（编译、测试、提交成功，被拦的报错）。觉得不够，在回复里说，不要自己加。

## 不要做
- 不改 `crates/` 下的代码，不做 fence，不改 corral。
- 不全局装 srt，不改 `~/.srt-settings.json`、各家 agent 的全局配置、`~/.claude/settings.json`、`~/.codex/config.toml`。
- 不让测试 agent 读真实的 `~/.ssh`、各家凭证文件或别的私人数据，拦截一律用自己放的假文件试；报告里不贴任何真实凭证、token、私人路径以外的私人内容。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。只操作你自己开的 `ranch/test-sandbox-*`。
- 不在 ranch、paddock 或别的真实仓库里试。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 遇到要 sudo、要改系统设置、要付费开通、或某家在墙里怎么补规则都起不来，停下来报告，等决定，不要自己换别的办法（比如换成容器、关掉墙）。
- 不合并到 main，不推送。只在 `r17a-srt-trial` 上提交。

## 做完
在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录（阶段：只完成 Codex 小活，R17a 总验收未完成）

- 做了什么：按用户后续要求，先停四家，再只继续 Codex。保留 srt 0.0.80 和 IP Guardian 入口，定位 HTTP TLS 阻塞，取得 Codex 编译、测试、提交与三项拒绝的完整回复；规则全文和证据写入 `docs/调研/R17a-srt小试验.md`。
- 验证了什么：最终 build/test/commit 均退出 0，提交 `915d7bc`；读假密钥、写合成主仓库、访问 example.com 分别退出 1/1/56。status 有 working/idle；ls 可见但该版本没有 state 字段。空闲 pause/resume 正常，stop 返回 0，测试 agent 全部停止。报告 `git diff --check` 通过。
- 拿主意的地方：只为本次进程传完整 projects 表完成用户授权的信任，不放开全局 config.toml 写入；补编译目录读取权限。为验证 TLS，单独试了 Keychains 只读（无效）、trustd 放行（有效）；最终带 `enableWeakerNetworkIsolation:true`，必须由主控决定是否接受其隔离代价，不能算严格网络隔离总验收通过。
- 没做的事：未继续 Claude、pi、omp，未完成四家总验收；未实跑 Linux，未实现 fence、改 ranch 代码、全局安装、改全局配置、合并或推送。目录和域名尚未证明最小化。
- 踩过的坑：PTY 未放行导致启动失败；只补 ab.chatgpt.com 不能修复 TLS；点 Trust 会写全局 config 被拒，带点路径的单键 -c 未生效，改传完整 projects 表有效；编译目录只放写不放读会失败；commit 退出 0 仍报 packed-refs.lock 拒绝，关闭 maintenance.auto 也未消除，具体原因未查清；agent 停止后 corral reply/read 不可取，必须先存证据。

## 完成记录（阶段补充：pi、omp 分别复测）

- 做了什么：用户先要求 pi，pi 停止后再要求 omp；两家严格顺序运行。补齐各自规则全文、启动形式、前次撞墙原因和完整结果；没有重启 Codex 或 Claude。
- 验证了什么：pi、omp 均 build/test/commit 退出 0，各 2 个测试通过，提交分别 `cd4a1fe`、`8a571b2`；两家的读假密钥/写合成主仓库/连未放行网站均退出 1/1/56。status 的 working/idle、ls 暂停标记、空闲 pause/resume/stop 正常；最终无测试 agent。`git diff --check` 通过。
- 拿主意的地方：根据已查证的编译目录读权限遗漏，补各自独立目录 allowRead，沿用已有 allowWrite。保持 srt 默认网络隔离，不开 Codex 的 trustd 开关，也不为更新检查和其他 provider 探测扩大域名白名单。
- 没做的事：Claude 尚未完成，四家总验收未完成；不做 fence，不改全局配置，不改其他仓库，不跑 Linux，不合并或推送。应用状态目录权限尚未最小化。
- 踩过的坑：pi 早前 setRawMode EPERM 和配置/认证 lock 目录创建失败，omp 早前 native addon utime 和 run/daemons mkdir 被拒，报告已保留；本轮两家都仍有 packed-refs.lock 报错但提交成功。pi 的 NODE_USE_ENV_PROXY 未设置仍能连接当前 DeepSeek；不要泛化为所有 Node/provider 都如此。

## 完成记录（Claude 补齐与本轮调研收尾）

- 做了什么：最后只开 Claude Code，补齐登录、Bash 工具临时路径规则，完成剩余一家的小活和拒绝试验。报告已有四家规则全文、公共/特有/模型服务/仓库路径区分、启动与结果摘录、撞墙记录、Linux 文档对照和未知项。
- 验证了什么：Claude 最终 build/test/commit 退出 0，提交 `a7daeae`；三项拒绝退出 1/1/56，Bash 自身没有 cwd 收尾错误。status 有 working/idle，空闲 pause/resume 正常，stop 成功；四个测试 agent 全部关闭。`git diff --check` 通过。
- 拿主意的地方：复用用户此前对同一临时 worktree 的信任授权，始终禁止全局配置写入。Keychains 只给正常客户端认证使用，不导出或读取其条目内容；用官方 CLAUDE_CODE_TMPDIR 将内部临时文件移到本次已授权目录，撤回先试的旧 tmp 路径放行，不扩大整个 /tmp。
- 没做的事：不做 fence、不改程序、不改全局配置、不合并推送部署；不测 amp/agy、不实跑 Linux。四家操作证据已齐，但严格隔离总验收不能直接宣告通过：Codex trustd 弱化开关、Claude Keychains 和各家较宽应用状态目录的边界仍交主控决定。未证明 packed refs 等任意 Git 布局可用。
- 踩过的坑：Claude 未登录源于 Keychains 被读限制挡住；只设置 TMPDIR 不能覆盖其全部 Bash 临时文件，先在 mkdir 阶段失败，再在 cwd 收尾阶段失败，改用 CLAUDE_CODE_TMPDIR 后正常。前两轮明确区分了工具内部失败与目标命令退出码，没有拿内部错误当作拦截验收。全局 cairn hook 被拒但 corral 状态事件照常更新，未为消除提示扩权。
