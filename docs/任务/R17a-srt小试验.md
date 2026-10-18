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
