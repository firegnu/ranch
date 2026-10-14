# 交接

## 现在在哪（2026-10-14）

- R8 接入 agy 已合并、推送、收尾（`docs/任务/R8-接入agy.md`，交叉审查 `R8-审查.md`，设计 DESIGN §7「接入 agy」，格式 CONTRACT）。
  - 做了什么：
    - 全局安装可以装一个目录：agy 的插件 `~/.gemini/config/plugins/corral/`。
    - `__hook agy:<事件>` 把 agy 的钩子翻译成 v1 事件，提示词和回复从 transcript 逐行取。
    - 读屏判等批准（R7 的机制第一次用上）。
    - `integration status` 多报 `install.enabled`。
    - 技能多一个 `gemini` 位置，corral 和 corral-dispatch 两份都装。
    - 技能里 agy 必带 `--prompt`、点名才派。
  - 验证：
    - 现有四家和 amp 不变（R3 快照只多 `all` 里的 gemini）。
    - 真实一轮在用户真实 HOME 跑过：working→idle、回复对、`send` 确认、批准判 blocked 再继续、HUP 停干净。插件用完已卸，空的 `plugins` 目录也删了。
    - 审查后改的插件命令（用 `CORRAL_HELPER`）只在合成测试里跑过，部署后装插件时可以再真跑一轮。
  - 已打包 `versions/f86d1c2/`，**还没部署**，R7 一起带上。部署要做的：
    1. 切两个链接到 `versions/f86d1c2/bin/`。
    2. 重装两份技能：claude、codex 两处是 overwrite，gemini 是新装；dry-run 已核对。
    3. `corral upgrade --all` 可做可不做：agy 的钩子不依赖共享 helper 先切。
    4. agy 插件装不装由用户定（`corral integration install --agent agy`，或以后从 paddock Settings 点）。
  - 退回：切回 `versions/0e5bd19/bin/`。
  - 对 paddock 的影响（兼容新增）写在任务文件完成记录末尾，还没回报 paddock/main。

- R7 读屏兜底已合并、推送、收尾（`docs/任务/R7-读屏兜底.md`，交叉审查 `R7-审查.md`，设计 DESIGN §7「读屏兜底」）：适配器可声明读屏规则，只有声明了的那家 pen 才用 vt100 维护屏幕，命令行在 working／idle 而屏幕命中时报 blocked。这一件没有哪家声明规则，现有四家和 amp 行为不变；公开输出不变，paddock 不用动。审查后加了保护：vt100 panic 只丢掉屏幕，控制字符串不进解析器。已打包 `versions/3b7d113/`，**还没部署**（技能 dry-run 全是 same，不用重装；现有五家不建屏幕，`corral upgrade --all` 可做可不做）。退回：切回 `versions/0e5bd19/bin/`。

- R6 接入 amp 已合并、推送、收尾（`docs/任务/R6-接入amp.md`，交叉审查 `R6-审查.md`，设计 DESIGN §7，格式 CONTRACT）：全局安装框架（`corral integration install|uninstall --agent amp`）、amp 适配器和插件（`~/.config/amp/plugins/corral.ts`）、终端标题判等批准、技能里 amp 点名才派。已部署（10-14 19:01 用户在普通终端里跑）：两个链接指向 `versions/0e5bd19/bin/`，两条 `install-skills` 已跑，`corral upgrade --all` 已跑（paddock/main、ranch/main complete，pen 原地换成 `0e5bd19`；`~/.corral/.runtime/helper` 也是 `0e5bd19`；已不在的 agent 只有旧提醒记录，结为 complete）。主控核对：`integration status` 五家技能都是 same、amp 插件 same；两条 install-skills 的 dry-run 全是 same；进程表里的 corral 只有 `0e5bd19`；开 `ranch/test-claude` 问一句拿到回复、stop 干净。插件已经在用户同意下装进真实的 `~/.config/amp/plugins/corral.ts`，就是最终内容（`integration status` 为 same），留不留由用户定。退回：切回 `versions/653c5b2/bin/`。已回报 paddock/main（10-14，用户要它接着做 paddock 这边的集成）。

- R5 调研已合并、推送、收尾（`docs/任务/R5-agy与amp接入调研.md`，报告 `docs/调研/R5-agy与amp接入、全局安装和读屏兜底.md`）：只出报告，没改代码、没改 DESIGN，不用部署。结论：agy、amp 都只能走全局安装（agy 插件目录 `~/.gemini/config/plugins/corral/`，amp 系统插件 `~/.config/amp/plugins/corral.ts`）；amp 能报全，agy 缺「等批准」要读屏补；读屏建议放 pen 里用 vt100。建议次序 ① 全局安装框架 + agy → ② amp → ③ 读屏。用户 10-14 已定第六节，见任务文件末尾；等用户来讨论后再立任务。

- R4 已合并、推送、收尾（`docs/任务/R4-corral-integration-status.md`）：新增只读命令 `corral integration status`（给 paddock P5-94 Settings 用，格式在新建的 `docs/CONTRACT.md`）；技能位置改成按位置登记（pi 读 `~/.agents`，omp 两个都读），`install-skills` 行为不变。已部署（10-14 用户在普通终端里跑）：`~/.local/bin/corral`、`~/.local/bin/ranch` 都指向 `versions/653c5b2/bin/`；技能内容没变（dry-run 全是 same），pen、钩子、启动命令没变，没跑 `install-skills`、没 `corral upgrade --all`（在跑的 agent 的 pen 仍是 `7b761e7`，和 `653c5b2` 的 pen 代码相同）。主控部署后跑 `corral integration status`，四家路径、版本、技能状态都对。退回：切回 `versions/7b761e7/bin/`。已回报 paddock/main（合并提交 `b0e3b6f`、格式见 `docs/CONTRACT.md`、没有 `--json`）。

- R3 已合并、推送、收尾（`docs/任务/R3-corral恢复适配器结构.md`，设计 `docs/DESIGN.md` §7）：corral 恢复一家一个适配器（`crates/corral/src/agents/`），行为不变，靠 `crates/corral/tests/snapshots/` 里在旧代码上钉的快照保证；四家真实 agent 实测过。已部署（10-14 用户在普通终端里跑）：`~/.local/bin/corral`、`~/.local/bin/ranch` 都指向 `versions/7b761e7/bin/`；技能内容与已装的相同（dry-run 全是 same），没跑 `install-skills`；之后用户又跑了 `corral upgrade --all`（10-14，所有 agent 都空闲时）：6 个在跑的 agent（cairn、client-demo、global-mesh、jb-generator、paddock、ranch 的 main）全部 complete，pen 原地换成 `7b761e7`，agent 进程没重启；`.runtime/helper` 切到 `7b761e7`；已不在的 agent（jb-finetune/codex-1、owlet/main、ranch/review-pause-1、saddle/main）只有旧提醒记录，结为 complete（其中两条 not_delivered）。主控核对：各 agent `exe` 都是 `7b761e7`、升级状态 none，进程表里没有别的版本的 corral 进程。部署后主控用新版读了现有 agent 的状态（正常），开 `ranch/test-claude` 问两句、拿到回复、`stop` 由 SIGHUP 停干净。退回：链接切回 `versions/d55defb/bin/`。用户 10-14 定的「全局配置可以放宽」已记进 DESIGN §7，代码留给以后装全局钩子那件。

- R2 已合并、推送、收尾（`docs/任务/R2-派活agent不用cairn.md`）：corral-dispatch、corral 技能开出去的 agent 带 `--env CAIRN_DISABLE=1`，不用 cairn（用户拿 owlet 试点 cairn）。只改技能文字，corral 程序行为与 `5c5540c` 相同。已部署（10-08 16:27 用户在普通终端里跑）：`~/.local/bin/corral`、`~/.local/bin/ranch` 都指向 `versions/d55defb/bin/`，两条 `install-skills` 已跑，主控核对 `~/.claude`、`~/.agents` 下的技能与仓库逐字节一致；没做 `corral upgrade --all`（程序代码与 `5c5540c` 相同）。正在跑的主控重开后才读到新技能。

- R1 corral pause／resume 已合并、推送、收尾、部署（任务和两轮交叉审查：`docs/任务/R1-corral暂停与继续.md`、`R1-审查.md`；设计 `docs/DESIGN.md` §6）。10-07 17:46 用户在场按 §6 部署：打包 `5c5540c`，用户切链接、`corral upgrade --all`（cairn/main、paddock/main、ranch/main 均 complete，两条指向已不在 agent 的旧提醒结束为 not_delivered）、`corral install-skills`；主控用 `ps` 核对没有旧版 wait／`__after` 进程（只剩一个 17:45 起的旧版 `corral attach paddock/main`，只转发不判状态）。暂停可用。

- 第 1 步 corral、第 2 步 dispatch 都已完成，范围到此为止（用户 10-05：遥测、Drover、插件系统砍掉，不迁）。
- 安装：
  - `~/.local/bin/corral`、`~/.local/bin/ranch` → `~/.local/share/ranch/versions/0e5bd19/bin/`（10-14 19:01 起；旧版 `653c5b2`、`7b761e7`、`d55defb`、`5c5540c`、`d771c10`、`df46247` 目录保留，不删；10-14 19:01 `corral upgrade --all` 后，在跑的 agent 的 pen 和 `~/.corral/.runtime/helper` 都是 `0e5bd19`）
  - corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）都在 `~/.claude/skills/`、`~/.agents/skills/` 下。
- Saddle（`dedf26a` 起）和 paddock 都只调用 `corral`、`ranch` 命令。任务和完成记录在 paddock：`../paddock/docs/任务/M2-ranch与corral.md`、`M3-dispatch进ranch.md`。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。

## 下一步

1. 部署 R7＋R8（用户在场，见上）。之后经用户同意回报 paddock/main：agy 的接入（见 R8 完成记录「对 paddock 的影响」）。
2. paddock 接 R6：Settings 里装／卸 amp 插件、New Agent 支持 amp（在 paddock 自己的任务里做）。
3. 修 macOS 上 pen 继承多余描述符的问题（见「悬着」第一条），单独一件小活，先写任务文件给用户看。
4. paddock 接 P5-94 的 Settings 页（按 `docs/CONTRACT.md` 读 `corral integration status`）。
5. paddock 开工 P5-33（界面接 pause／resume）。
6. ~~Saddle 要不要跟暂停~~：不跟。用户 10-07：“saddle我基本不用了”。

## 悬着

- macOS 上多个线程同时起 `corral start` 时，某一次的输出管道可能漏进另一个常驻的 pen（Rust 标准库在 macOS 上建管道和设 close-on-exec 分两步），调用方等不到输出结束，卡住不返回。
  - `protocol.rs` 的 `concurrent_unique_starts_and_listing_preserve_live_locks` 偶尔卡住就是这个（R8 期间抓到现场：一个 pen 开着测试进程的管道）。
  - paddock 同时起好几个 agent 时理论上也会碰上。
  - 修法：pen 启动时（`pen.rs` 起 pen 的 `pre_exec`）关掉 0、1、2 以外继承来的描述符。
  - 不在 R8 里改，等用户定。
- agy 的已知限制（DESIGN §7「接入 agy」）：
  - 子 agent 的会话会不会触发钩子、抢主会话，没实测；
  - 钩子 shell 里 `exec` 失败时不回 `{}`；
  - Antigravity IDE 会不会加载插件，没核对。
- R8 摸底留在用户 `~/.gemini` 里、还没清的东西，等用户定：
  - agy 信任列表里的 `/private/tmp/r8agy.Vg4v/work`（目录已删）；
  - `~/.gemini/antigravity-cli/brain/` 下的两份测试对话：摸底的 `dc7a4646-…`、真实一轮的 `42d1db0b-…`。
- paddock 的 `docs/DESIGN.md` §3“Saddle 保底”和决策表还写着“只保证 Saddle 和运行时对得上、不兼容时写需求给 Saddle 适配”；ranch 这边已按用户 10-07“保底规矩放松吧，Saddle 不用再写影响了”改了 AGENTS.md 和 DESIGN §2，paddock 那边要不要跟着改，在 paddock 自己的任务里做。
