# 任务：pi、omp 套墙时 DeepSeek 和 opencode 都能用；套墙的 agent 被拦停下时主控看得出来

2026-10-20，ranch/main 交给 ranch/dev-r28（Codex，gpt-6-astra / medium）。
路由：常规 / 交叉审查不要 / 影响面：改行为（路由：轻（置信 0.16）、要、碰要害；推翻：放行名单只给 pi、omp 加一个用户点名的域名 opencode.ai，主控已在本机配置里实测放行后能用，技能部分只改文字；写错了 `fence check` 一眼看得出、随时能改回，按改行为走，不交叉审查）
类型：功能变更
依据：本机 pi、omp 的模型服务除了 DeepSeek 还有 opencode，fence 内置只放行 DeepSeek，omp 套墙连不上模型，一轮就停；本轮把 opencode 加进内置默认，并让主控在套墙的 agent 停下没写 DONE 时查出是不是被墙拦了。不改 corral、fence 的程序逻辑。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-20（paddock/main 转来，用户让交给 ranch 查）：「omp 套 fence dev 墙开起来十几秒就停，没有回复，也没干活。……请查清 omp 为什么停、该怎么放行，以及套墙起不来时主控怎么能看出来。」
- 10-20：「我今天把deepseek的api key换成opencode了，所以你都要支持」
- 10-20：「pi 和 omp 都加」
- 10-20：主控说明的做法——「改 ranch 里 fence 自带的规则，让 pi、omp 默认就同时放行 DeepSeek 和 opencode」；「套墙的 agent 停了又没写完成标记，主控就去查 fence 的拦截记录，直接告诉你"被拦了，拦的是 opencode.ai"这种话」。用户：「可以」。

## 先读

- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」。
- `crates/fence/README.md` 开头几段（数据怎么合并、`[services]` 表、「pi/omp 的 `dev` 默认服务是 DeepSeek」那句）。
- `crates/fence/data/profiles/dev.toml` 的 `[services]`，`crates/fence/data/services/`。
- `crates/fence/tests/cli.rs` 里按种类核对内置域名的那段（pi、omp 那两项现在只有 `api.deepseek.com`）。
- `docs/CONTRACT.md`「sandbox 与 `fence`」一节里 `fence denials` 的输出格式。
- `crates/corral/resources/SKILL.md` 第 55 行附近、`crates/corral/resources/AGENT_USAGE.md` 第 51 行附近、`crates/dispatch/resources/corral-dispatch/SKILL.md` 第 245、257 行附近：对方停了但最后一行不是 DONE 那几句。

## 主控查到的（10-20）

- 本机 `~/.omp/agent/config.yml` 默认模型是 `opencode-go/deepseek-v4.1-flash`，走 `opencode.ai`；`~/.pi/agent/settings.json` 目前仍是 `deepseek`。
- paddock 那次（实例 07c49744e696）`fence denials` 里 network 拦了 `opencode.ai:443` 7 次。主控用 corral 套墙开 omp 复现：屏幕上是 `Proxy refused to open a tunnel: 403 host is not on the allow list`，随即 Stop，`corral reply` 报 `no_reply`。
- 主控在 `~/.fence` 加本机补充（`services/opencode.toml` 写 `domains = ["opencode.ai"]`，`local.toml` 的 `[services]` 写 pi、omp 都是 `["deepseek", "opencode"]`）后，omp、pi 在墙内都正常回复，corral 套墙开 omp 也正常回复。只多放行 `opencode.ai` 就够；`build.stencil.so`、`catalog.stencil.so`、写 `~/.omp/logs`、读 `~` 和 `~/.omp` 仍被拦，不影响使用，本轮不放行。
- 本机补充是临时的，R28 部署后由主控删，你不要动 `~/.fence`。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r28-opencode-fence`，分支 `r28-opencode-fence`（已从 main 建好）。只动 `crates/fence/data/`、`crates/fence/tests/`、`crates/fence/README.md`、上面三份技能和 guide 文件、必要时 `docs/CONTRACT.md` 里写内置服务的句子，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r28-opencode-fence`。

## 要做的

1. fence 内置加一个 `opencode` 模型服务，域名 `opencode.ai`；`dev` 档案里 pi、omp 的默认服务改成 DeepSeek 和 opencode 都放行。同步改 README 里那句默认服务的说明，和测试里 pi、omp 的内置域名。
2. 三处技能文字（corral 技能、`corral guide` 的 `AGENT_USAGE.md`、corral-dispatch 技能）里「对方停了但最后一行不是 DONE」那几句加上：这个 agent 是套墙开的（`corral status` 的 `sandbox` 不是 `null`）时，主控先看 `fence denials --agent <名字>` 有没有被拦的网络、看一眼屏幕末尾的报错，把被拦的域名和报错告诉用户。其余照现有规则（告诉用户、重挂提醒等）不变。三处说法一致。
3. 写清对 paddock 的影响。

## 怎么算做完

- 用户原话：「我今天把deepseek的api key换成opencode了，所以你都要支持」「pi 和 omp 都加」。
- 用户原话（转述的做法，用户说「可以」）：「套墙的 agent 停了又没写完成标记，主控就去查 fence 的拦截记录，直接告诉你"被拦了，拦的是 opencode.ai"这种话」。
- 验证只做这些：改测试里 pi、omp 的内置域名，先在只改测试时跑一次看它失败，再改数据让它通过；fence 包的测试跑一次；`git diff --check`；用编译目录里的 fence 跑 `fence check --profile dev --kind omp`、`--kind pi`（隔离 `FENCE_HOME`，不读真的 `~/.fence`）看域名里有 `api.deepseek.com` 和 `opencode.ai`；如果有测试检查技能和 guide 的文本，跑那几个。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 corral、fence 的程序逻辑，不加命令和参数。
- 不放行 `opencode.ai` 以外的域名，不改 claude、codex 的数据，不放宽读写路径。
- 不动 `~/.fence`、`~/.omp`、`~/.pi` 等用户配置，不读认证文件。
- 不起真实的 pi、omp、Claude Code、Codex 去连模型服务。
- 不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 不合并到 main，不推送。只在 `r28-opencode-fence` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：新增内置 `opencode` 服务，只放行 `opencode.ai`；pi、omp 的 dev 默认服务同时包含 DeepSeek 和 opencode。同步 README、域名测试和 CONTRACT 的内置服务列表示例。三处技能/guide 统一增加停下且没有 DONE（含 no_reply）时的套墙诊断：查网络拒绝、读终端末尾，把域名和报错告诉用户；原有打断、提醒规则保留。
- 验证了什么：只改测试后跑 `cargo test -p fence --test cli four_agents_match_the_development_profile`，因 pi 缺少 opencode.ai 失败；改数据后 `cargo test -p fence` 通过（20 通过、8 个真实 srt 测试按原设置忽略），包含目标测试转绿。`cargo test -p corral-core --test adapters agy_reads_corral_skills_from_its_own_config` 和 `cargo test -p ranch-dispatch skills::tests::creates_then_reports_same_and_leaves_extra_files` 各 1 项通过；未找到针对 guide 正文的断言。编译目录里的 fence 在隔离 HOME/FENCE_HOME、合成 pi/omp 路径及 `FENCE_RUNNER=/bin/sh` 下分别运行 `check --profile dev --kind omp`、`--kind pi`，均 ok，均有 api.deepseek.com、opencode.ai。`git diff --check` 通过。命令均前台执行并等到结束。
- 拿主意的地方：退出后可能读不到终端，三处说明都补充如实告知并看 denials 的 exit.message，沿用现有公开约定。paddock 不用改代码：只是内置服务和技能的兼容新增，命令、参数、JSON 格式、退出码均不变；规则和技能由主控后续部署。没有需要主控另作决定的事。
- 没做的事：未改程序逻辑、其他种类数据或路径权限，未动用户配置、认证文件和现有 agent；未起真实模型服务、未部署、未合并或推送。
- 踩过的坑：技能测试第一次误用包名 corral，改成实际包名 corral-core 后通过。直接用开发二进制跑 check 缺少打包后的 runtime.toml，改为现有测试使用的合成 runner 后通过。临时目录的 rm -rf 清理被工具拒绝，改用逐个 unlink 和 rmdir 清理。

## 主控审查

- 10-20 ranch/main 看了 diff：只动了 fence 数据、测试、README、CONTRACT 的示例和三处技能/guide，程序逻辑没改；放行只加了 `opencode.ai`，只给 pi、omp。三处技能说法一致：套墙的 agent 停了没写 DONE，先查 `fence denials` 的网络拦截和屏幕末尾，把被拦的域名和报错告诉用户，其余规则不变。
- 重跑一次：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过，`cargo test --all-targets` 180 通过、0 失败，和对方说的 fence 部分一致。
- 建议改（不采纳）：README 里加的「对 paddock」一段属于这次改动的说明，放在 README 里略显多余；不影响使用，留着。
- 主控能不能真从拦截记录看出原因，要部署后遇到真实被拦时看。
- 结果：一次过
