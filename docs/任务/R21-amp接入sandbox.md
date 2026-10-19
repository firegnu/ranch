# 任务：amp 也能关进 fence 里照常干活

**不做**（用户 10-19：“sandbox不加amp了，这个太特殊了”）。做到一半停下，改动没提交，worktree、分支、编译目录、临时目录都已清掉，agent 已关。

2026-10-19，ranch/main 交给 ranch/dev-r21（Codex，gpt-6-astra / high）。
路由：常规 / 交叉审查要 / 影响面：碰要害（路由：常规（置信 0.14）、要、碰要害）
类型：功能变更
依据：R17b 只给 claude、codex、pi、omp 写了种类数据，amp 带 `--sandbox` 报 `unsupported_kind`。本轮给 amp 补数据，在本机（macOS）实测。agy 不在本轮（用户 10-19 定先做 amp）；Linux 不在本轮（R17c 暂缓）。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做。除了下面「真实 agent 验收」写明的测试 agent，不要再开别的 agent。

## 用户原话

- 10-19：“我有个疑问，现在sandbox是不是只是针对适配的几个起作用？”“amp和agy套上难度大吗？”“加一个draft task，agy和amp的sandbox接入。”
- 10-19 定：先做 amp；验收照主控建议的；同意用真实 amp 登录开测试 agent 实测（花订阅额度）。
- 10-18（关于严格程度）：“不需要严格限制。我只是在要通过做这个来彻底完全的了解一下sandbox。”

## 先读

- `AGENTS.md`「规矩」。
- `docs/CONTRACT.md`「sandbox 与 `fence`（R17b）」一节，以及 R18–R20 的段落。
- `crates/fence/README.md`：数据文件的键、合并次序、占位符、真 srt 测试怎么开。
- `crates/fence/data/agents/` 下现有四家，`crates/fence/data/profiles/dev.toml`，`crates/fence/data/services/`。
- `docs/DESIGN.md` §7「全局安装（R6）」（amp 怎么接 corral），§8 sandbox。
- `docs/调研/R17a-srt小试验.md`：前四家是怎么一家家试出来的。
- `docs/任务/R17b-fence本体.md`「真实 agent 验收」一节：用本分支的程序、隔离 FENCE_HOME 起测试 agent 的做法。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r21-amp-sandbox`，分支 `r21-amp-sandbox`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r21-amp-sandbox`。
- 只动 `crates/fence/`（数据、测试、README）、`docs/CONTRACT.md` 的 fence 一节、本任务文件。

## 主控已知的情况（10-19 查的，没在墙里试过）

- amp 是编译好的本机程序（Mach-O arm64，`/opt/homebrew/bin/amp`），不是 Node 脚本，认不认 `HTTP_PROXY`/`HTTPS_PROXY` 不知道。srt 靠代理管外网，不认代理就连不上网。
- 目录：`~/.config/amp`（corral 全局插件 `plugins/corral.ts` 在这里，用户已装）、`~/.local/share/amp`、`~/.cache/amp`。
- 派活时 amp 不能带 `--prompt`：先起，`corral wait` 到 `idle`，再 `corral send` 第一句。没有免确认参数，卡在确认上时状态是 `blocked`。
- IP Guardian 只包了 codex、claude，碰不到 amp。

## 要做的

1. 照 R17a、R17b 的办法，弄清 amp 在墙里要读、要写哪些目录，要连哪些域名，要不要伪终端，凭证存在文件里还是钥匙串。
2. 写 `crates/fence/data/agents/amp.toml`；需要新的服务数据就加在 `crates/fence/data/services/`，在 `dev` 档案的 `[services]` 里给 amp 接上。照现有四家的写法：凭证和设置文件读得到、写不了（`deny_write`），会话和缓存目录可写。不改现有四家的规则。
3. amp 关在墙里，corral 插件照常把状态报给 pen。
4. 更新 fence README、CONTRACT 里「有数据的种类」的说法。

## 怎么算做完

- `corral start --sandbox dev` 起 amp，在临时合成仓库里做一件小活：改文件、编译或跑个脚本、提交，`corral status`/`reply` 正常。
- 同一个 agent 在墙里读不到 HOME 里别的东西（用假文件试）、写不了 worktree 外面、连不上没放行的网站，`fence denials` 有记录。
- `fence check --profile dev --kind amp` 不报 `unsupported_kind`。
- 验证只做这些：新数据的针对测试（照现有四家的测试样子）加边角用例；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次；下面的真实 agent 验收一遍。觉得不够，在回复里说，不要自己加。

## 真实 agent 验收

- 用本分支编出来的 corral 和 fence（打一个版本目录到临时位置，或用编译目录里的程序），隔离的 `FENCE_HOME`，不部署。
- 测试 agent 名字 `ranch/test-sandbox-amp`，带 `--label role=test`、`--env CAIRN_DISABLE=1`，用 amp 默认模型，活要小。停之前用 `corral read`/`reply` 存好证据，用完 `corral stop`。
- 合成仓库建在 HOME 下的临时目录，用完删掉合成仓库、隔离 FENCE_HOME 里的测试记录、假密钥。
- amp 卡在确认框（`blocked`）或「是否信任目录」上，停下来报告，请主控转给用户去点；不要 attach 上去替它点。

## 不要做

- 不读、不打印、不拷贝真实凭证；验拦截一律用自己放的假文件、假密钥。
- amp 不认代理时，不要靠放开全部网络或关掉网络隔离来凑通，停下来报告。
- 要改 fence、corral 代码（不只是数据和测试）才能通的，停下来报告，等决定。
- 不改用户的 amp 设置，不重装、不停用 corral 全局插件；只读它的目录来弄清布局。不往真实的 `~/.fence/` 写东西。
- 不做 agy，不去 5090，不做 Linux 实测。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不靠删测试、放宽断言、缩短超时来通过。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 不合并到 main，不推送。只在 `r21-amp-sandbox` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 停下来的原因（主控记，10-19）

ranch/dev-r21-1 写了 amp 的种类和服务数据，`fence check --profile dev --kind amp` 能过，fence CLI 测试 17 项过；真实起 amp 时卡住，照任务书停下来问。用户听完决定不做，改动没提交，一并丢掉。查到的：

- **插件不报状态**：corral 给 amp 装的全局插件（`crates/corral/resources/amp_plugin.ts` 的 `startedByCorral`）只认 amp 的父进程是 pen（`CORRAL_PEN_PID`），用来挡住 amp 里再开的 amp。套上墙后进程链是 `pen → fence runner → amp`，插件不报，状态一直 `starting`，派不了活。要通得改这条检查，改了用户要重装插件。agy 走的 `__hook`（`crates/corral/src/hooks.rs` 的 `global`）是同样的检查，以后给 agy 套墙会卡在同一处。
- **配置目录要写**：amp 初始化时会改 `~/.config/amp` 目录本身的权限，整目录只读它就起不来；当时改成目录可写、单独保护设置文件、插件目录、`AGENTS.md` 和凭证。
- amp 是 Bun 编译的本机程序，在墙里的临时目录留下 `.bun-*.node` 文件。

结果：不做
