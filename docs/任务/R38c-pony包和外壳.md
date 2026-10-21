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
