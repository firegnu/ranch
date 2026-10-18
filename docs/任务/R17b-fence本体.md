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
