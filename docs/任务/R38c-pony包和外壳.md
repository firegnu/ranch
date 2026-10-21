# 任务：R38c 建 pony 的包和命令外壳

2026-10-21，ranch/main 交给 ranch/dev-r38c（Codex，gpt-6-astra / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.62）、要、碰要害；凭证权限和单实例锁算安全和并发；按第 1 条读 `critical-other`，审查读 `review-claude`：Claude Code opus[1m] xhigh）
类型：功能变更
依据：R38 的实现第二件。照 `docs/DESIGN.md` §10 和设计稿，建 `crates/pony`：命令 `--version`、`status`、`run` 的外壳，配置和凭证的读取、校验、权限检查，单实例锁、状态文件、日志轮转，`Chat` 接口和登记表，打包工具把 `bin/pony` 放进版本目录。本轮不连 Slack、不调 corral、不做路由和提醒（R38d、R38e），不做 `pony service`（R38f）。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：「写 R38c 任务书吧」
- 底线：「不要动ranch中corral的核心代码。」
- R38 验收第 6 条：「Slack 的凭证只放在本机，不进任何仓库。」
- 其余用户定的各条见 `docs/DESIGN.md` §10「用户 10-21 定的」。

## 先读

- `AGENTS.md`「规矩」：语言与依赖、测试不依赖真实 agent 和用户状态、验证、独立编译目录、安装。
- `docs/DESIGN.md` §10 全文，§3（版本目录和打包），§8 里 fence 作为独立一块的写法。
- `docs/CONTRACT.md` 末尾「`pony status`（R38）」一节：输出必须照它。
- `docs/调研/R38a-Slack机器人设计.md` 第 2 节（插件式结构、适配器接口）、3.1（命令）、3.2（配置）、3.4（锁、日志）、第 4 节（`pony status`）、第 6 节（凭证）。
- `crates/fence/`：独立一块怎么组织（Cargo.toml、`FENCE_HOME`、测试写法），可照着做。
- `tools/package/src/main.rs`：打包怎么把 `bin/fence` 放进版本目录。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38c-pony-shell`，分支 `r38c-pony-shell`（已从 main 建好）。只动新建的 `crates/pony/`、根 `Cargo.toml` 的 `members`、`Cargo.lock`、`tools/package/`，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r38c-pony-shell`。

## 要做的

1. **包**：`crates/pony`，命令 `pony`，Cargo 里不依赖 `corral-core`、`ranch-dispatch`、`fence`。第三方依赖优先用工作区已在用的（如 `serde_json`、`toml`、`libc`）；要加新的库先在回复里问主控。
2. **`Chat` 接口和登记表 `CHATS`**（设计稿第 2 节）：本轮定好接口，登记 `slack` 一家，只实现 `kind`、凭证的键（`app_token`、`bot_token`）、读这一家配置（校验 `owner`、频道 ID 的写法）。连接、收发消息、频道信息留给 R38e，本轮不连任何网络。共用核心的代码不写任何一家的名字。
3. **`pony --version`**：照 CONTRACT。
4. **配置**（设计稿 3.2）：`PONY_HOME`（默认 `~/.pony`）下的 `config.toml`，按设计稿的规矩校验，错了报出文件、位置和原因；没有这个文件就是 `missing`。
5. **凭证**（设计稿第 6 节）：`secrets/<kind>.toml`，按适配器声明的键读；目录 0700、文件 0600、属于当前用户、是普通文件，不是就报 `secrets_too_open`，拒绝符号链接；解析出错只报行号，不带原文。凭证的值不出现在任何输出和日志里。
6. **`pony run`**（外壳）：拿 `run/lock`，拿不到退出 1；没有配置文件记一行日志、退出 0；有配置就校验配置和凭证，把状态写进 `run/status.json`，运行中配置改了重读（改坏了继续用旧的、状态报 `invalid`）。本轮各家的连接还没实现，`connection` 报 `connecting`，`reason`、`message` 为 `null`。日志 `logs/pony.log`，满 1 MiB 轮转留 3 份，不记任何凭证。
7. **`pony status`**：照 CONTRACT 那一节输出；只读配置、`run/status.json`、锁（不拿锁），不联网、不调 corral。`service` 本轮固定报 `not_installed`（macOS）或 `unsupported`（其他平台），R38f 再接。
8. **打包**：`tools/package` 把 `bin/pony` 放进版本目录，`BUILD.txt` 的校验和带上它。不切链接、不部署。
9. 写清对 paddock 的影响：部署后 PATH 上有 `pony`，`pony status` 照 CONTRACT 输出；没配置时报 `missing`。

## 怎么算做完

- 在隔离的 `PONY_HOME` 里，`pony status` 对这几种情况的输出都照 CONTRACT：没配置、配置错、凭证太宽（含符号链接）、在跑、没在跑。
- `pony run` 没配置时退出 0；同一个 `PONY_HOME` 第二份拿不到锁退出 1。
- 用户原话：「Slack 的凭证只放在本机，不进任何仓库。」：凭证只从 `PONY_HOME` 读，测试用假 token（写成 `xoxb-test` 这种不像真 token 的样子），输出、日志、报错里都没有凭证的值。
- 用户原话：「不要动ranch中corral的核心代码。」：`crates/corral/` 没有改动。
- 验证只做这些：上面几条各有测试；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 `crates/corral/`、`crates/dispatch/`、`crates/fence/`、`crates/ranch/`，不改 `docs/CONTRACT.md`、`docs/DESIGN.md`。CONTRACT 和设计稿有说不清或对不上的地方，问主控，不自己改约定。
- 不连 Slack、不发任何网络请求，不调 corral，不做路由、排队、提醒、分段（R38d、R38e）。
- 不做 `pony service`，不写 `~/Library/LaunchAgents`（R38f）。
- 不建、不读真实的 `~/.pony`；测试一律用临时目录做 `PONY_HOME` 和 `HOME`。
- 不装技能，不动 `~/.claude`、`~/.agents`、`~/.gemini`、`~/.ranch`、`~/.corral`。
- 不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch`、`pkill -f pony` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停测试里起的 `pony run` 用记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r38c-pony-shell` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-21，ranch/dev-r38c。

- 做了什么：新增独立 `crates/pony`，实现 `--version`、只读 `status`、前台 `run`；`Chat` 和 `CHATS` 登记配置与凭证声明，当前适配器校验成员和频道 ID，共用核心没有适配器名字。实现配置校验、2 秒重读、坏配置保留旧值，凭证目录／文件的权限、所有者、普通文件和符号链接检查，POSIX 单实例锁、原子状态文件、信号退出记录、1 MiB 日志轮转并留 3 份。打包工具的构建、复制和 BUILD.txt 校验和都加入 `bin/pony`。
- 验证了什么：6 个隔离命令测试，逐步确认目标行为 RED 后实现并转 GREEN；覆盖无配置、配置错误、凭证缺失／错误／过宽／符号链接、输出不带假 token 或 owner、运行与未运行、第二份锁冲突、热重读和旧状态文件。所有测试程序的 HOME、PONY_HOME、CORRAL_HOME 都指向临时目录，测试起的进程按自身 PID 停并等待回收。`cargo test --all-targets` 跑一次，通过（203 passed、11 ignored，忽略项是原有测试）；`cargo fmt --check` 跑一次，通过。`cargo clippy --all-targets -- -D warnings` 跑一次，报两处 `byte_char_slices`，已按提示把字节数组等价改成 `b"UW"`、`b"CG"`；遵守「各一次」限制，未补跑，不能记为修正后已通过。三项命令都使用指定的独立编译目录；Cargo 离线运行，没有下载或升级第三方依赖。
- 拿主意的地方：`status` 不打开凭证文件，凭证错误由 `run` 检查后记录；PONY_HOME 本身不安全时在读取配置前直接拒绝。连接尚未实现，正常运行时为 `connecting`，未知的历史切换时间为 0；坏配置保留运行中的频道，删掉配置则正常退出。正常信号退出记录 `stopped`，强制结束且没有退出记录时 `last_exit` 为 null。没有新增依赖或改设计／CONTRACT 的决定；建议主控审查时复核一次 clippy，修正后的通过记录尚缺。
- 对 paddock：只是兼容新增，后续部署会在 PATH 上提供 `pony`，`pony status` 按 CONTRACT 输出，没配置时报 `missing`；现有 corral 约定和 paddock 功能不变。包内 README 写明了本轮边界和凭证状态来自最近一次运行检查。
- 没做的事：没有连接 Slack、发网络请求、调用 corral、实现路由／提醒／service；没有运行打包产出、部署、切链接、装技能、合并或推送。没有读写真实 `~/.pony` 或操作用户现有 agent。未修改 corral、dispatch、fence、ranch 的代码、DESIGN、CONTRACT 或别的仓库；工作区依赖锁文件只新增 pony 的登记。没有额外扩大测试范围。
- 踩过的坑：`libc::F_UNLCK as _` 用在比较中无法推断类型，改成显式 `libc::c_short`。自查补测发现 PONY_HOME 不安全时 run 无法写状态，status 起初会误报配置有效，已把目录检查加入配置读取并验证通过。clippy 的两处等价字节字符串写法按上面记录修正，没有用重复全量检查绕过次数限制。

## 返工记录

2026-10-21，第 1 次，按 `R38c-审查.md` 末尾主控决定修改。

- 改了什么：不稳定测试在重启后等 `run/status.json` 的 pid 等于新进程，再发 SIGKILL；首次启动也等有效的连接时间，避免拿尚未初始化完的快照做比较。PONY_HOME 去掉尾部斜杠和点并转绝对路径，空值回退 HOME，HOME 未设时报错。配置校验顶层、适配器段和频道项的未知键；适配器可声明专有键；裸键错误带键位置，带引号的键只报解析器记录的行号，不回显键和值。信号处理提前到拿锁前，读完配置就在校验凭证前写本轮快照。`since` 未知时改为 null，README 同步；增加按 CHATS 登记逐项检查核心源码和 main.rs 不出现适配器名字的测试。没有新增依赖，没有修改 CONTRACT，按主控 main 提交 7252a38 的约定实现；审查第 6–12 条没有改。
- 验证了什么：`cargo test -p pony` 一次通过（1 个核心边界测试、8 个命令测试），其中增加了尾部 `/`、`/.` 的符号链接、空 PONY_HOME／缺失 HOME、未知键定位与不泄露、未知 `since: null` 的检查。不稳定那条另单独连续跑 5 次，全部通过。`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次通过；没有重跑工作区全套测试。所有命令前台运行完成，使用指定独立编译目录，Cargo 离线；测试进程只操作隔离目录并按 PID 回收。
- 要主控决定的事：没有。本轮只修改 pony 和本任务记录，未部署、合并或推送。
