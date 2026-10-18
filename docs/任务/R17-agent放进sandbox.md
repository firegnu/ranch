# 任务：agent 放进 sandbox——corral 开 agent 时可选套上 srt 围栏

依据：用户 10-16 在 paddock 提的草稿 P5-120：“委派的agent放到sandbox中，要设计一套sandbox”。10-17 在 paddock 主控那里讨论定下：目的不是修问题，“我主要是项把agent这套sandbox搞清楚它的玩法”，“不单单是调研，而是要在我的这套harness上玩起来”；墙放在 ranch（corral 开 agent 时套上），用户：“对，搬到ranch。这边只留界面的东西”。paddock 那边只留界面（P5-120：New Agent 选档案、卡片小锁、被拦记录面板、Settings 看档案），要等这件做出公开命令再做。10-18 在 ranch 主控这里定了方式（见下）。验收还没定。

## 用户 10-18 定的
- 用户原话：“可以，就按这样分，先接 srt（但是要支持linux）。 另外默认委派出去的不要家srt围栏（默认保持现在）。还有架构设计上我希望还是类似插件式的（以后可以很方便的把sandbox剥离出来成为像cairn这样独立的服务）。这是我的几个要求”
- 走围栏式，接 srt（Anthropic sandbox-runtime，开源 `github.com/anthropics/sandbox-runtime`，npm `@anthropic-ai/sandbox-runtime`）。“按这样分”指：corral 里接 srt；容器式（Docker Sandboxes 一类）用户自己先手动试，不进 ranch，以后要不要再说。
  - 讨论经过：用户原先以为 sandbox 是 Docker 那种独立隔离的环境；主控讲了围栏式和容器式的区别（容器编不了 paddock 这种 macOS 程序，登录、编译缓存、接 corral 都要另做），用户：“我总感觉本地隔离的容器处于一个很尴尬的地步”，“要么本地应该是像claude code或者codex这种srt，或者是cloud based的sandbox那种”。
- 要支持 Linux。
- 默认不带：派出去的 agent 默认保持现在的样子，不套墙；要套时手动加。原先设想的“派活技能默认带 `--sandbox`”一件不做。
- 插件式：sandbox 和 corral 之间接口要窄，以后能整块剥离成像 cairn 那样的独立服务。

## 10-17 已定、仍然有效的
- 管什么：读、写、上网三样都管，一律默认不许、按需放行。
- 起步：先小试验，再改 corral——先验证 agent 关在墙里还能正常干完一件活（编译、测试、提交），过了再做后面的。
- 哪几家：第一版 Claude Code + Codex（10-18 改为 Claude Code、Codex、pi、omp 四家，见拆法）。
- 做成选项：“提供sandbox的选项，加了sandbox就是sandbox里面跑，不加就是现在的形式”。
- 墙本身全局一套，在 corral 开 agent 时套上，和哪个仓库无关；前端只显示和选择。规则全局档案为主，用占位符（这个 agent 的工作目录、编译目录、HOME），开 agent 时填真实路径；仓库特有的几条由仓库里一个小文件补充，只写额外放行。
- 放在 ranch，不单开仓库；以后要独立时整块搬走。
- 公开约定（`--sandbox`、`corral status` 里的档案字段、sandbox 的命令和 JSON）都是兼容的新增，照 AGENTS.md 写说明，paddock 在 P5-120 里接。

## 放在哪、边界怎么划（用户 10-18 定：“同意B，名字就叫fence”）
主控比较了三种：A 做进 corral（剥离要从 corral 里拆，不合要求）；B 放在 ranch 仓库、按插件边界做；C 现在就单开仓库像 cairn（新仓库、安装、跨版本兼容，对一个还在试验、srt 还是 beta 的东西太早）。用户选 B。10-17 草稿里把各家特有的规则放进 `crates/corral/src/agents/` 各家文件的做法作废，改成：
- sandbox 叫 **fence**，是单独的一块（`crates/fence`），出自己的命令 `fence`；corral 不在 Cargo 里依赖它，只按一条窄约定调它的命令，像 corral 对 cairn 只认一个环境变量那样。命令从第一天就叫 `fence`，不挂在 `ranch` 下面，剥离后前端不用改。
- corral 只做一件事：带了 `--sandbox <档案>` 时，把 agent 命令换成「fence + 档案、agent 种类、工作目录、这个 agent 的 corral 状态目录 + `--` + 原命令」再起。fence 和 corral 打进同一个版本目录，corral 调同一目录里的那份。找不到 fence 或档案有错就启动失败，不悄悄不套墙。
- 各家要放行的目录和域名（Claude 的 `~/.claude`、Anthropic 的域名；Codex 的 `~/.codex`、OpenAI 的域名）按 agent 种类写在 fence 自己那块里，剥离时一起带走。
- 列档案、检查档案、看被拦记录由 `fence` 自己的命令提供，前端只调它。
- corral 那边要处理的不随位置变：agent 外面多了一层 srt 进程，corral 记的 agent 进程号、`stop` 发信号、暂停冻结子进程、`corral ls` 判断还在不在都要照顾到。
- srt 的版本由 ranch 固定，升级作为单独任务。
- 该拆出去的时候：不经 corral 也想用、比 corral 还大、要单独发布。

## 和 Claude Code、Codex 自带的 sandbox 的关系（主控 10-18）
- Claude Code 的 `/sandbox` 底层就是 srt，但它包的是自己跑的每条 shell 命令，Claude Code 本身和它改文件的工具不在墙里（那部分靠权限设置）。fence 用 srt 包的是整个 agent 进程，agent 自己、它开的工具、MCP、corral 的 hook 全在墙里，各家一样。
- Codex 自带的读不限、上网只有开关两档；pi、omp、agy 没有自带的。
- 套了 fence 的 agent 不能再开自己那层（macOS 不许嵌套 Seatbelt，Linux 墙里不许建用户命名空间）。用户现在的配置里 Claude Code、Codex 自己的 sandbox 都没开；用户 10-18：“这个先不要做实验了。这种可能性比较低。我一般不会开带sandbox的claude code或者codex。”不试，也不做替它关掉。

## 以后要接 pi、omp 等（用户 10-18：“我委派出去的不一定只有claude code或者codex，还有pi，omp等……即使现在不做也要考虑，不然将来突然要使用了，完全不支持就麻烦了”）
第一版只做 Claude Code、Codex，但结构要让以后加一家只是加一份数据，不改 corral、不改 fence 的代码：
- 各家一份数据文件，按 agent 种类（corral 的 `claude`、`codex`、`pi`、`omp`、`agy`、`amp`）命名，写这一家要读、要写的目录（如 `~/.pi/agent`、`~/.omp/agent`）、它自己要连的域名。还没有数据文件的种类带 `--sandbox` 时直接报不支持，不拿半套规则硬跑。
- 模型服务和 agent 分开：Claude Code、Codex 连的服务基本固定，pi、omp 由用户自己配（本机 pi 现在用的是 deepseek）。所以「连哪个模型服务」单独成一块，档案里写用哪些；Claude Code、Codex 的数据文件带上各自默认的那个。fence 不去读各家内部的配置猜。
- pi、omp 多出来、R17a 里要顺带记下的（只看目录和文档，不实跑）：
  - 它们要写自己目录下的东西：sqlite 数据库（omp 的 `agent.db` 等）、会话、pi 的 `npm` 目录；
  - 用 nvm 下的 node 或 omp 自带的运行时；
  - corral 用 `--extension` 给它们加载的 hook 文件，在 corral 状态目录里，要能读；
  - 5090 上如果连本机跑的模型，要放行本机地址（srt 默认拦本机地址，要写明 IP 和端口）；
  - omp 带浏览器（puppeteer），Linux 墙里 Chromium 的沙箱起不来，可能要关这个功能或接受它不能用。
- 5090 上只用 pi、omp，所以 Linux 验证和接 pi、omp 天然是同一件事。
- srt 本身不认识任何 agent，只包一个进程（主控 10-18）；各家能不能在里面跑，卡在两处，要逐家实测：
  - 上网只能走 srt 的代理（它设 `HTTP_PROXY`、`HTTPS_PROXY`），agent 的网络库不认这两个变量就连不上模型。Node 自带的 fetch 默认不认（要 `NODE_USE_ENV_PROXY=1` 一类的开关），pi 用 Node，最要查；Codex（Rust）、omp 的运行时也要确认。
  - agent 的 shell 工具要是自己开伪终端（Codex 有这种跑法），macOS 上要开 srt 的 `allowPty`。

## srt 要点（主控 10-18 读 README，未实测）
- 包一层用：`srt --settings 规则.json -- <命令>`，对整棵进程树生效。写默认全不许、按 `allowWrite` 放行；读默认全许、按 `denyRead` 圈掉再用 `allowRead` 放回；上网默认全不许，经它在本机起的代理按域名放行。Node 写的，0.0.80，自称 Beta Research Preview，配置格式可能变。
- macOS 用 `sandbox-exec`（Seatbelt），要 ripgrep。Linux 用 bubblewrap，网络整个拿掉、只经 socat 接到外面的代理，要装 bubblewrap、socat、ripgrep；Ubuntu 24.04 起要 sudo 改 `kernel.apparmor_restrict_unprivileged_userns` 或加 AppArmor 配置。
- Linux 比 macOS 弱、影响设计的：写规则只认具体路径、不认通配符；读规则的通配符只在套上墙那一刻展开；本地 socket 只能全禁或全开；墙里不能再建用户命名空间（套不了第二层）。

## 拆法
用户 10-18：“先在我本机完全跑通了，再测试5090那台”。
1. R17a 小试验：调研加实测，不改 ranch 代码。在本机用 srt 关住 Claude Code、Codex、pi、omp 各干一件小活，并逐项记下在 Linux 上做不做得到；产出能跑通的规则和 R17b 的依据。用户 10-18 确认：“是的，四家都在本机先跑通，验收加那三条。”
2. R17b fence 本体加 `corral start --sandbox`，按上面定的边界，在本机跑通上面几家。
3. R17c 本机全通过后，再到 5090 上验 Linux：只用 pi、omp（用户规矩），先用合成脚本验墙本身。

## 还没定（继续和用户讨论）
- 5090 是什么系统、要不要 sudo 改设置，到 R17c 前再查。
- worktree 提交要写主仓库 `.git` 里这个 worktree 的那几处和对象库，怎么放行（Linux 写规则不认通配符，`refs/heads/<分支>.lock` 写不出来）。
- 档案先内置哪几种；全局档案放哪；仓库补充文件的名字和格式。
- 被拦记录从哪来（srt 的记录、macOS 系统日志），命令怎么给出来。
- srt 装在哪、怎么固定版本（装前问用户）。
- 谁来做：照本仓库 AGENTS.md，主控拆、派。

## 怎么算做完
- 用户原话：“委派的agent放到sandbox中，要设计一套sandbox”；“要在我的这套harness上玩起来”；“先接 srt（但是要支持linux）”；“默认委派出去的不要家srt围栏（默认保持现在）”；“架构设计上我希望还是类似插件式的（以后可以很方便的把sandbox剥离出来成为像cairn这样独立的服务）”。（各件的验收定了再补。）

## 不做
- 容器式、云端那两类（用户自己先手动试容器，不进 ranch）。
- 派出去的 agent 默认带墙。
- paddock 的界面（在 paddock 的 P5-120 里做）。
