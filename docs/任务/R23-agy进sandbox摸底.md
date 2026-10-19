# 任务：摸底——agy 关进 fence 里能不能照常干活，要放行什么，还有什么挡路

2026-10-19，ranch/main 交给 ranch/dev-r23（Codex，gpt-6-astra / high）。
路由：重 / 交叉审查不要 / 影响面：看得见（路由：重（置信 0.0）、要、碰要害；推翻：本轮是不改代码的调研，按 corral-dispatch 第 3 节一律算看得见、不交叉审查）
类型：调研
依据：用户 10-19 要先探一下 agy 好不好接进 sandbox。本轮只摸底，不改 ranch 的代码和内置数据；接不接，看了结果由用户定。
提示：区分已查证事实、推测和未知，给出直接依据；结论限定在本轮调查范围。
你是被委派的 agent：照本文件做。除了下面写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-19：“agy的sandbox能否探测一下。看看好不好集成进来？”“可以，写任务文件派出去吧”（同意用真实 agy 登录实测，花 agy 的额度）。
- 10-19（amp 那件）：“sandbox不加amp了，这个太特殊了”。
- 10-14：“主要是想通过agy加进来读屏幕的能力，agy很少用。”

## 先读

- `AGENTS.md`「规矩」。
- `docs/CONTRACT.md`「sandbox 与 `fence`（R17b）」一节，以及 R18–R20 的段落。
- `crates/fence/README.md`：数据文件的键、合并次序、占位符；`$FENCE_HOME/agents/` 下放的新种类文件 fence 也认。
- `crates/fence/data/agents/` 下现有四家（照它们的写法），`crates/fence/data/profiles/dev.toml`。
- `docs/DESIGN.md` §7「接入 agy（R8）」「agy 派活修整（R13）」。
- `docs/任务/R21-amp接入sandbox.md` 末尾「停下来的原因」：amp 卡住的地方。

## 已知会卡住的地方（主控 10-19 读代码，没实测）

- agy 的钩子是一行 shell，exec `$CORRAL_HELPER __hook agy:<事件>`；真正的判断在 `crates/corral/src/hooks.rs` 的 `global`：只认 agy 的父进程是 pen（`CORRAL_PEN_PID`）。套上墙后，agy 的父进程是 fence 的 runner，钩子不报，`SessionStart` 进不来，状态会一直 `starting`。
- 这一条不要绕，也不要改代码去接通；只核实它是不是确实这样（进程链、事件文件里有没有东西）。状态停在 `starting` 时，用 `corral read` 读屏幕判断 agy 本身在不在干活。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r23-agy-probe`，分支 `r23-agy-probe`（已从 main 建好）。只新写 `docs/调研/R23-agy进sandbox摸底.md` 和本任务文件的完成记录。
- 用已经装好的 `corral`、`fence`（`~/.local/bin/`，`versions/aa3676b`），不用编译。agy 的规则写在一个临时的、隔离的 `FENCE_HOME` 下的 `agents/agy.toml`（需要的话再放一份 `profiles/` 或 `services/`），不进仓库。

## 要弄清的

1. agy 在墙里要读、要写 `~/.gemini` 下（和别处）的哪些目录、文件；凭证存在文件里还是钥匙串。
2. 要连哪些域名（登录、模型、更新检查之类）；agy 认不认 srt 给的代理（`HTTP_PROXY`/`HTTPS_PROXY`）。
3. 要不要伪终端（`allow_pty`）。
4. 在墙里能不能做完一件小活：在 HOME 下临时合成仓库里改文件、跑个脚本、提交。
5. 上面那条父进程检查是不是唯一挡住 corral 拿到状态的；还有没有别的（比如 `CORRAL_HELPER` 指向的程序在墙里能不能读、能不能运行，corral 插件目录读不读得到）。
6. 同一个 agent 在墙里读不到 HOME 里别的东西（用假文件试）、写不了 worktree 外面、连不上没放行的网站，`fence denials` 有记录。

## 怎么算做完

- `docs/调研/R23-agy进sandbox摸底.md` 写清上面 6 条的结论，每条标明是实测、读代码还是推测；附一份试出来能用的 `agy.toml`（写在文档里，不进 `crates/`）；最后列「要接进来还差什么」：哪些要改代码、哪些只是数据，各自大概多大。
- 验证只做这些：照上面 6 条实测一遍，`git diff --check`。觉得不够，在回复里说，不要自己加。

## 测试 agent

- 名字 `ranch/test-sandbox-agy`，带 `--label role=test`、`--env CAIRN_DISABLE=1`、`--sandbox dev`，`--env FENCE_HOME=<隔离目录>`（或别的让 fence 用上隔离数据的办法，照 CONTRACT），按 corral 技能的 agy 写法起：必须带 `--prompt`，最后是 `-- agy --dangerously-skip-permissions`。活要小。停之前用 `corral read`/`reply` 存好证据，用完 `corral stop`。
- 合成仓库建在 HOME 下的临时目录。用完删掉合成仓库、隔离 FENCE_HOME、假密钥，以及测试 agent 在 `~/.cache/fence/tmp/` 下留的那个实例目录（只删你自己那次的）。
- pen 会替 corral 开的 agy 往 `~/.gemini/antigravity-cli/settings.json` 的 `trustedWorkspaces` 补信任目录（R13，用户同意过）。不要手改这个文件；在完成记录里写明加进去了哪些条目，由主控和用户定要不要清。
- agy 卡在登录框、信任框或别的对话框上，停下来报告，请主控转给用户去点；不要 attach 上去替它点。

## 不要做

- 不改 ranch 的代码和 `crates/fence/data/` 下的内置数据，不改 corral 插件。
- 不读、不打印、不拷贝真实凭证；验拦截一律用自己放的假文件、假密钥。
- agy 不认代理时，不要靠放开全部网络或关掉网络隔离来凑通，记下来就行。
- 不改用户的 agy 设置，不重装、不停用 corral 全局插件；只读它的目录来弄清布局。不往真实的 `~/.fence/` 写东西。
- 不做 amp，不去 5090，不做 Linux 实测。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 不合并到 main，不推送。只在 `r23-agy-probe` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-19，ranch/dev-r23-1，分支 `r23-agy-probe`。

- **做了什么：** 新写 `docs/调研/R23-agy进sandbox摸底.md`，逐项区分实测、代码／二进制线索和未知；记录失败配置、直接证据及后续工作。用现装 aa3676b、隔离 FENCE_HOME、HOME 下合成仓库和假文件，按指定参数三次启动 `ranch/test-sandbox-agy`（前一次退出后重开），没有开其他 agent。只改本报告和本任务记录，paddock 不用动。
- **验证了什么：** agy 1.4.2 的 language server 在 `listen tcp 127.0.0.1:0` 时被拒，三次均退出码 1；`allow_pty=false/true` 都不能启动。抓到 pen → Node runner、0 字节 events、短暂 starting／no_reply，未抓到 agy → hook 的存活进程链。临时规则的 `fence check` 通过，检查了各实例的 denials；`git diff --check` 验证文档。没有把尚未执行的小任务、代理和隔离探针算作通过。
- **拿主意的地方：** 本地监听比已知的父进程检查更早阻塞；fence 没有暴露 srt `allowLocalBinding`。本轮只读代码核对，不改 runner、不绕父进程检查、不放宽网络；没有冒充「试出来能用」的配置，报告给出失败候选和原因。开启 PTY 后复核一次，再沿用配置抓短命进程和事件，不扩大验证范围。
- **没做的事：** 未完成模型代理、钥匙串墙内可用性、PTY 必要性、小活提交、同一个 agy 的三项隔离、helper 实际执行及钩子拒绝路径的验证，因为 agy 在执行首句前退出。没有改代码、内置数据、插件或用户设置，没有编译、部署、合并、推送，也未做 Linux、amp 或读屏能力测试。
- **踩过的坑：** 只在种类数据填 domains 会报 `no_service`，补临时服务后 check 才过；日志更新还要写 `cli.log.tmp`，补放行后仍卡在 TCP bind。start 返回成功不代表 agy 启动成功，退出太快时 read/reply 已经 not_found；TCP bind 错误后两轮没进 denials，必须对照 agy 启动日志，不能拿零条记录当通过。清理时工具拒绝带强制选项的删除命令，改成不带强制选项、逐个写明本次目录的 `rm -r` 后完成。
- **收尾：** 三次测试 agent 均自行退出，最终 stop 返回 not_found。已删合成仓库、隔离 FENCE_HOME、假文件和临时证据，以及本次三个 fence 缓存实例 `8d4260342c25`、`e9251290e40b`、`2a18546f8466`。pen 临时加入的 trustedWorkspaces 条目是 `$HOME/.r23-agy-probe.v385Gu/repo`，第三轮运行时确认存在、退出后确认已撤回，最终无此残留；没有手改设置。agy 自己生成的常规日志保留，未提交原始日志或用户数据。
- **要主控／用户决定：** 是否继续接入；若继续，是否接受 srt local binding 实际允许所有本地地址 bind/inbound 和 localhost 任意端口 outbound 的范围。需要先另定设计，再派 fence 配置能力和 corral 顶层身份校验的代码任务；不是仅补一份 agy 数据就能完成。

## 主控审查

- 10-19 ranch/main 看了 diff：只新增调研文档和完成记录，没改代码、内置数据、插件和用户设置，「不要做」里的一件没做。看得见那档，不重跑测试。
- 挑最关键的一条核对：已装 srt 0.0.80 的 `macos-sandbox-utils.js` 在 `allowLocalBinding` 为 true 时放行 `network-bind`/`network-inbound (local ip "*:*")` 和 `network-outbound (remote ip "localhost:*")`；fence 的 `rules.rs` 没有接这个开关。和报告一致。
- 结论照报告：agy 1.4.2 启动时要在本机开 TCP 监听（`listen tcp 127.0.0.1:0`），墙里被拒，起不来；后面还有钩子父进程检查。都要改代码，不是只加数据。接不接、接受不接受那个开关的放行范围，交用户定。
- 6 条里 4、6 两条和第 2 条的域名没测成，原因是 agy 起不来，报告如实标了未知。测试 agent 已退出，信任条目已自动撤回，临时目录已清。
- 结果：一次过

用户 10-19 定：agy 不接进 sandbox（“不接了，改成不做吧”）。
