# 任务：agent 放进 sandbox——corral 开 agent 时可选套上一道墙

依据：用户 10-16 在 paddock 提的草稿 P5-120：“委派的agent放到sandbox中，要设计一套sandbox”。10-17 在 paddock 主控那里讨论定下：目的不是修问题，“我主要是项把agent这套sandbox搞清楚它的玩法”，“不单单是调研，而是要在我的这套harness上玩起来”；墙放在 ranch（corral 开 agent 时套上），用户：“对，搬到ranch。这边只留界面的东西”。paddock 那边只留界面（P5-120：New Agent 选档案、卡片小锁、被拦记录面板、Settings 看档案），要等这件做出公开命令再做。讨论还没完，范围、验收没定。

## 现在是怎样（paddock 主控 10-16、10-17 记的）
- 派活时开发 agent 一律免确认启动：Claude Code `--dangerously-skip-permissions`、Codex `--yolo`、omp `--approval-mode yolo`、agy `--dangerously-skip-permissions`；pi 本来就没有权限框；amp 看用户自己的设置。能约束它们的只有任务文件里「不要做」那几条（corral-dispatch 技能第 4 节原话：“免确认也就没有安全网”）。
- worktree 只把各 agent 的代码改动分开，不圈住 agent：它能写整个 `$HOME`、主仓库和别的 worktree（worktree 和主仓库共用一个 `.git`），能读 `~/.ssh`、各家凭证，能联网、`git push`、按名字杀进程。
- 本机现有：`/usr/bin/sandbox-exec`（macOS Seatbelt）、`codex sandbox`（Codex 自带的，能包别的命令）、Apple `container`、`docker`。没装 Anthropic 的 `srt`。

## 背景：现在叫“sandbox”的开源东西分三类（主控 10-17 讲给用户的，凭已有了解，没上网核对）
1. **围栏式**：agent 还在本机，操作系统按规则拦它，开销几乎为零，照常用本机的工具链。
   - 底层零件：macOS Seatbelt（`sandbox-exec`）；Linux Landlock、bubblewrap、seccomp。要自己写规则。
   - Anthropic sandbox-runtime（`srt`，开源）：Claude Code `/sandbox` 用的就是它；macOS 上包 Seatbelt、Linux 上用 bubblewrap；管读、写，上网经它自己的代理按域名放行；能包任意命令（`srt -- codex`），规则写在一个 JSON 里。
   - Codex 自带 sandbox（开源）：只读、只能写工作目录、全放开三档；能单独包别的命令（`codex sandbox`）。
   - Gemini CLI：同样用 Seatbelt，也可选 Docker。
2. **容器式**：给 agent 一个单独的 Linux 环境，看不到本机文件。Docker 一类（Dagger 的 container-use、Docker 官方给 agent 的 sandbox）、Apple `container`、devcontainer。隔离最彻底，但里面是 Linux：编不了 macOS 程序（如 paddock），用不了本机钥匙串和各家登录。
3. **云端／微型虚拟机**：E2B、Daytona、Microsandbox、Modal、Cloudflare、Vercel 等，多用 Firecracker 一类。主要给 AI 产品执行模型生成的代码用，不是圈住本机的编程 agent。
- 用户问“这个不是sandbox吗？为什么会出现哪些不能读？”：主控答围栏式看得见真实硬盘，靠规则管；可以做成默认什么都读不到、写不了、连不上，只放行必须的（系统目录、Rust/cargo 工具链、自己的 worktree 和编译目录、自家 agent 的配置；联网只放模型服务和下载依赖的站）。在 Mac 上给编 macOS 程序的 agent 加墙只能用围栏式。

## 用户 10-17 已定的
- 管什么：读、写、上网三样都管（主控补：一律默认不许、按需放行）。
- 起步：先小试验，再改 corral——先验证 agent 关在墙里还能正常干完一件活（编译、测试、提交），过了再做后面的。
- 哪几家：第一版 Claude Code + Codex。
- 做成选项：用户“提供sandbox的选项，加了sandbox就是sandbox里面跑，不加就是现在的形式”。主控建议：先默认关，手动开着试；用顺了再让派出去的（免确认启动的）默认开；用户自己开的和主控先一直不开。理由：对单人本机、自己仓库，风险主要是手滑的命令落到主仓库（reset、push、删分支）和读到网页、依赖里藏的指令后读密钥外发；墙漏放一处 agent 就会撞墙卡住、来回试，这比乱写更常见。不开时零代价。
- 独立于仓库，规则分两层（用户问“是和cairn记忆系统一样的独立的一套还是和各个repo绑定的？”）：
  - 墙本身全局一套，在 corral 开 agent 时套上，和哪个仓库无关；前端只显示和选择。不像 cairn 要每个仓库 adopt，开不开在开 agent 时定。
  - 规则全局档案为主，用占位符（这个 agent 的 worktree、它的编译目录、HOME），开 agent 时 corral 填真实路径；仓库特有的几条（如 paddock 的 `paddock-worktrees/.target/`、网页项目的 `node_modules` 缓存和 npm）由仓库里一个小文件补充，例如 `.corral/sandbox.json`，只写额外放行。
- 放在 ranch，不单开仓库（用户问“是不是和cairn一样是不是有一个独立的repo”；主控答墙只能在开 agent 那一刻套上，入口只有 corral，最难的拦截 `srt` 已做好；用户：“可以，补进草稿”）。照 ranch 现有两种写法拆开放：
  1. 主体照 `crates/dispatch`：新开 `crates/sandbox` 一块库——读档案、合并仓库补充、填占位符、调 `srt` 包命令、整理被拦记录；对外在 `ranch` 下加命令（列档案、检查档案、看某个 agent 被拦了什么），前端只调这些公开命令。
  2. 各家特有的照插件：放进 `crates/corral/src/agents/` 下各家的文件（`claude.rs` 要 `~/.claude`、连 Anthropic；`codex.rs` 要 `~/.codex`、连 OpenAI），以后加 pi、omp 在它们自己的文件里补。
  3. 套上墙在 `corral start --sandbox <档案>`：corral 调 sandbox 库包住 agent 命令再起；`corral status` 写明用的哪个档案。
  4. 派活默认带 sandbox（以后）：corral-dispatch 技能（`crates/dispatch/resources/corral-dispatch/`）里派出去的 agent 加上 `--sandbox`，任务文件写明用哪个档案。
  - 以后要拆成独立仓库（不经 corral 也想用、比 corral 还大、要单独发布）时整块 `crates/sandbox` 搬走。
- 公开约定：`--sandbox`、`corral status` 里的档案字段、`ranch` 下的 sandbox 命令和它们的 JSON 都是兼容的新增，照 AGENTS.md 写说明，paddock 在 P5-120 里接。

## 还没定（继续和用户讨论）
- 小试验用什么包：`srt`（要先装，装前问用户）、`codex sandbox`、还是自己写 Seatbelt 规则；主控倾向 `srt`。
- worktree 提交要写主仓库 `.git` 里这个 worktree 的那几处和对象库，怎么放行。
- 档案先内置哪几种；全局档案放哪个目录；仓库补充文件的名字和格式。
- 套了墙之后，Claude Code、Codex 自己的 sandbox 还能不能再套一层（macOS 对嵌套有限制，主控记不准）；派出去的带免确认参数、用不到自己的 sandbox，大概没事，小试验里确认。
- 被拦记录从哪来（`srt` 自己的日志还是系统日志），`ranch` 命令怎么给出来。
- 派出去的什么时候改成默认开（主控建议小试验和手动试顺了以后）。
- 拆成几件：小试验一件（调研加实测，不改代码），之后 `crates/sandbox` 加 `corral start --sandbox` 一件，派活技能默认开一件。
- 谁来做：照本仓库 AGENTS.md，主控拆、派；小试验派给谁。
- Linux（5090）以后再做；5090 上照用户规矩暂时只用 pi、omp。

## 怎么算做完
- 用户原话：“委派的agent放到sandbox中，要设计一套sandbox”；“要在我的这套harness上玩起来”。（范围定了再补。）

## 不做
- 容器式、云端那两类（编不了 macOS 程序；不是这个用途）。
- paddock 的界面（在 paddock 的 P5-120 里做）。
