# 任务：fence 本体，加 `corral start --sandbox`——四家在本机关进墙里照常干活

10-18，ranch/main 交给 ranch/dev-r17b（Codex，gpt-6-astra / high）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.94）、要、碰要害）
类型：功能变更
依据：照已定方案 `docs/调研/R17b-fence方案.md` 实现 fence 和 corral 的接入，本机四家跑通；Linux 实测留给 R17c。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做。除了下面「真实 agent 验收」写明的测试 agent，不要再开别的 agent。

## 先读
- `docs/调研/R17b-fence方案.md`：全部。这是要照着做的方案；开头 5 条用户 10-18 都同意了推荐项（“五条都同意”）。
- `docs/DESIGN.md` §3（安装）、§8（sandbox，已按方案改好）。
- `docs/调研/R17a-srt小试验.md`：「给 R17b 的跨平台结论边界」和四家的撞墙记录，要用时再查。
- `docs/CONTRACT.md` 开头（公开约定怎么写）。
- `CLAUDE.md`「规矩」全部。

## 在哪里干活
- worktree：`../ranch-worktrees/r17b-fence`，分支 `r17b-fence`（已从 main 建好）。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r17b-fence`。
- 会动：新建 `crates/fence/`；`crates/corral/`（`--sandbox`、pen 里套一层、`sandbox` 字段、`FENCE_RUN` 防护）；`tools/package/`；`Cargo.toml` 的 workspace 成员；ranch 仓库顶层的 `.fence.toml`；`docs/CONTRACT.md`（方案 §5 那段）。DESIGN 主控已改，有要改的在完成记录里写，不要自己改。

## 要做的（用户要的结果）
- `corral start ... --sandbox <档案> -- <agent 命令>`：agent 关在 srt 的墙里跑；不加 `--sandbox` 就是现在的样子（用户：“提供sandbox的选项，加了sandbox就是sandbox里面跑，不加就是现在的形式”）。派出去的 agent 默认不加，corral-dispatch 技能不改。
- fence 是单独的一块：`crates/fence`，出自己的命令 `fence`；corral 不在 Cargo 里依赖它，只按方案 §1 的约定调它的命令（用户：“类似插件式的（以后可以很方便的把sandbox剥离出来成为像cairn这样独立的服务）”）。fence 和 corral 打进同一个版本目录。
- 用户 10-18 派活时又强调：“这是插件的架构方式来写这个fence”。照方案落到代码上就是：corral 的 `Cargo.toml` 不依赖 fence，corral 的代码里只出现方案 §1 那两条命令、`CORRAL_FENCE` 和同目录查找，不解析 fence 的档案和数据；fence 不读 corral 的状态目录、不认 corral 的目录结构，corral 要放行的路径只经 `--read`/`--write` 传进来；各家要放行的东西只在 fence 的数据文件里，不写进 corral 的适配器。
- 规则、数据、占位符、合并、四家数据、模型服务、仓库补充、本机补充：照方案 §3、§4。用户 10-18：“不需要严格限制”，不要求收窄到最小。以后加一家只加数据，不改 corral、fence 的代码；还没有数据的种类带 `--sandbox` 报 `unsupported_kind`。
- 要支持 Linux：规则和数据要能在 Linux（bubblewrap）上用（写规则只用具体路径，本地 socket 不靠按路径放行，`[linux]` 段照方案）。Linux 实测不在本轮。
- 给前端的 `fence` 命令（`--version`、`profiles`、`check`、`denials`）和 corral 的 `sandbox` 字段、新错误码：照方案 §5，写进 `docs/CONTRACT.md`。都是兼容的新增，paddock 在 P5-120 里接。
- corral 管套了墙的 agent：照方案 §2，`stop`、`pause`/恢复、`status`/`ls` 判断还在不在、hook 报状态都照常。
- srt 固定 0.0.80，打包时装进版本目录，用打包时记下的 node：照方案 §6。
- Codex 的数据默认开 `enableWeakerNetworkIsolation`（trustd），档案说明里写明它削弱网络隔离（用户 10-18 同意）。
- 次序照方案 §8 的 1–6。

## 怎么算做完
- 用户 10-18：“按你的建议来，四条都同意”。
- 四家都在本机先跑通（用户：“是的，四家都在本机先跑通”）：Claude Code、Codex、pi、omp 用 `corral start --sandbox` 关在墙里，各干完一件活（改代码、编译、测试、提交）。
- 该拦的确实拦住了（读没放行的密钥文件、写主仓库、连没放行的网站）。
- 被拦的记录找得到（用 `fence` 的命令按 agent 取）。
- 关在墙里时状态照常更新（`corral status`）。
- `corral stop`、暂停和恢复照常。
- 验证只做这些：
  - 方案 §8 第 1–4 项写的自动化测试（隔离 HOME、CORRAL_HOME、FENCE_HOME，合成程序，不依赖真实 agent）；真 srt 那组 `#[ignore]` 测试在本机跑一次。
  - 合并前：`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。
  - 真实 agent 验收：照方案 §8 第 6 项那张表和「另外要实测的」各一遍。用本分支编出来的 corral 和 fence（打一个版本目录到临时位置，或用编译目录里的程序），不部署。
  - 觉得不够，在回复里说，不要自己加。

## 真实 agent 验收
- 测试 agent 名字 `ranch/test-sandbox-<种类>`，带 `--label role=test`、`--env CAIRN_DISABLE=1`，Codex 加 `--disable memories`；四家照免确认参数起（`claude --dangerously-skip-permissions`、`codex --yolo`、`pi`、`omp --approval-mode yolo`），模型用各家默认，活要小。一家做完、记下证据（停之前用 `corral read`/`reply` 存好），再开下一家。用完 `corral stop`。
- 合成仓库建在 HOME 下的临时目录（方案要求，和真实用法一样），带一个小的 crates.io 依赖。用完删掉合成仓库、`~/.fence/runs` 里的测试记录、假密钥。
- 拦截一律用自己放的假密钥文件试，不读真实的 `~/.ssh`、各家凭证。

## 不要做
- 不改 corral-dispatch 技能，不让派出去的默认带墙。
- 不做容器式、云端。不做 paddock 界面。不接 amp、agy 的数据。
- 不全局装 srt，不改各家 agent 的全局配置（`~/.claude/settings.json`、`~/.codex/config.toml` 等），不往真实的 `~/.fence/` 写 `local.toml` 或替换文件（测试用隔离的 `FENCE_HOME`）。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不靠删测试、放宽断言、缩短超时来通过。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 遇到要 sudo、要改系统设置、方案里的做法在实测中走不通（比如 srt 库接口和方案说的不一样、某家在墙里怎么补都起不来），停下来报告，等决定，不要自己换别的办法。
- 不合并到 main，不推送。只在 `r17b-fence` 上提交。

## 做完
在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

### 做了什么

- 新增独立 `crates/fence`：TOML 内置/用户数据、合并、占位符、平台取舍、规则校验、`profiles/check/run/denials/--version`；增加种类只加数据。corral 的 Cargo 不依赖 fence，接入只走两条命令及读写路径参数，各家适配器没有加入 sandbox 数据。
- corral 增加 `start --sandbox`、启动前检查、原命令外层包装、公开 `sandbox` 字段、`FENCE_RUN` 防护及 runner 先死时的进程组清理。不带选项保持原行为；派活技能未改。
- srt 固定 0.0.80，锁文件入库，Node runner 记录拒绝、收尾代理并透传退出状态。打包包含 fence、runner、srt 及固定 node 路径/版本；临时版本包内 `fence check --profile dev --kind claude` 成功，srt 及依赖约 16 MiB，整个版本目录约 24 MiB。
- 新增仓库 `.fence.toml`、fence README，补齐 CONTRACT。对 paddock 全是兼容新增，现有用法不用动；P5-120 可按 CONTRACT 接档案、锁标记、拒绝记录和规则展示。

### 验证了什么

- 自动化：隔离 HOME/CORRAL_HOME/FENCE_HOME；四家规则定值、用户覆盖和错误码、worktree/主仓库/游离 HEAD/斜杠分支、平台取舍、合成 fence/runner、钩子/首句/种类、旧 metadata、暂停恢复、退出码、runner 先死清组、7 天记录与缓存联动清理。损坏的覆盖文件（包括断链）会失败，不回退内置规则。
- 真 srt 的三项 `#[ignore]` 测试本机运行通过：文件/网络拦截和规则目录保护；普通退出与 HUP（正常退出码 23、信号 SIGHUP）透传；packed refs 下斜杠分支提交及 amend。Linux 没有实跑。
- `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过。
- 真实四家顺序运行，均用本分支临时版本包、`--sandbox dev`、测试标签、禁用 cairn；Codex 另禁用 memories。合成仓库在 HOME 下，使用 itoa 1.0.14（验收前本机缓存没有该版本，Claude 首轮成功下载）。编译目录由继承的 `CARGO_TARGET_DIR` 指定；提交署名与真实 Git 配置一致，没有修改全局配置。

| 种类 | build/test/commit/amend 退出码 | 读假密钥/写主仓库/curl 退出码 | 提交证据 | stop |
| --- | --- | --- | --- | --- |
| Claude Code | 0/0/0/0 | 1/1/56（CONNECT 403） | `6d6f29b`；状态补测再提交 `698897c` | 129 / SIGHUP |
| Codex | 0/0/0/0 | 1/1/56（CONNECT 403） | `1be04c5` | 0 / keys |
| pi | 0/0/0/0 | 1/1/56（CONNECT 403） | `069dd61` | 0 / SIGHUP |
| omp | 0/0/0/0 | 1/1/56（CONNECT 403） | `5a09072` | 129 / SIGHUP |

- 四家均记录到 starting/working/idle，status/ls 有 `sandbox.profile=dev`；空闲 pause/resume 成功。每家停止前保存 read/reply，停止后 ls 无测试 agent，按各自 PGID 查 ps 无 runner、代理、log stream 残留。主仓库 protected 文件保持原内容。
- 合成仓库、假密钥、隔离的 pen/fence 记录、六个实例的缓存临时目录及临时版本包已删除；原始验收证据留在本任务编译目录的 `acceptance-evidence/`，不入库。
- Codex 仅信任 `${repo}` 时未弹信任框。packed-refs.lock 仍报拒绝，但四家提交/amend 都为 0。Claude 不放开 `.claude.json` 写入仍能干活。
- `fence denials --agent` 四家都取得网络拒绝和文件事件（`monitors.files=true`）。Claude/omp 取得读假密钥、写主仓库和网络三项；Codex/pi 的主仓库写拒绝只在 agent 终端有明确证据，srt 日志未收录该条，按任务约定记下交主控。未伪造或从终端补写为 srt 事件，文档说明文件日志不保证逐条完整。

### 拿主意的地方

- **主控决定（用户 10-18 授权主控）**：`${tmp}` 改到 `${XDG_CACHE_HOME:-~/.cache}/fence/tmp/<实例>`，由 fence 创建、按档案放行，与运行记录按 7 天规则一起清理。原因是 srt 的禁止写压过放行写；原方案把 tmp 放在被整目录禁止写的 FENCE_HOME 内，实测不可写。移出后保留整个 FENCE_HOME 禁止写，Linux 上整目录禁止最稳。DESIGN 未自行修改；主控后续同步这条决定。
- 斜杠分支被 pack-refs 后，其 loose-ref 父目录会消失。沿用方案已有的 `mkdir` 数据机制，启动前准备当前分支 ref/reflog 的父目录；不增加墙内对其他分支的写权限。真实 srt 回归先复现失败，再通过。
- srt 没有文件监视健康接口，`files` 仅在实际收到文件事件后置 true。源码每批日志只解析首条拒绝，且本机实测存在漏条；不声称日志完整，不改固定的第三方运行时。

### 没做的事

- 没有部署、切链接、装技能、upgrade、合并或推送，没有改别的仓库或用户的全局 agent 配置。没有接 amp/agy，没有改 corral-dispatch。
- Linux 实测、本地模型服务验证留 R17c。omp 关闭 allow_pty 的可选试验未做。
- 文件日志漏条保留为上游记录能力的限制，已交代；没有扩大监视或追加任务书以外的验收。

### 踩过的坑

- 最初 `/bin/true` 合成夹具在本机不存在，改用存在的 `/bin/sh`；不是用语法错误或坏夹具当 RED。
- macOS 默认测试临时路径太长，srt 的 Unix socket 初始化 EINVAL；合成测试改用 `/tmp` 下的短路径。
- 原 tmp 与 FENCE_HOME 禁止写冲突，按主控决定修正。
- packed refs 的斜杠分支父目录缺失导致首次 Claude commit/amend 128，补 `mkdir` 数据后恢复。
- Claude 的 zsh heredoc 尝试写 `/private/tmp/zsh…` 被拒，随后用自身 Write 工具完成编辑；没有为此放宽 `/tmp`。用户全局 cairn hook 即使禁用也有入口不可读提示，corral 自己的 hook 和任务仍正常。
- Claude 一轮 Stop 后自行调用 ScheduleWakeup，额外 PreToolUse 使状态回到 working；补测明确不调定时工具，观察到 idle 并完成暂停/恢复。没有改 corral 的既有状态机去掩盖该事件。
- 最终全套测试曾在既有 hook 协议测试等待合成 agent 参数文件时超时；单项复跑和随后完整全套均通过，未改测试断言或超时。最终 clippy、fmt 和真 srt 三项回归也通过。
