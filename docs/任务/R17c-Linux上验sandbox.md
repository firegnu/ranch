# 任务：到 5090 上验 Linux 的 sandbox——pi、omp 关在 fence 里照常干活

**暂缓**（用户 10-19：“先不做R17c了，先把全部的在macos上跑通”）。重开前要定：那边由 pi 还是 omp 执行、谁去开；代码怎么过去、结果怎么回来；验收用不用下面建议的 8 条；本地模型验不验。

2026-10-19，ranch/main 起草，待用户看过、定验收。
执行：5090 上的 pi 或 omp（怎么开、代码怎么过去待用户定）。用户 10-19：“5090上不管是主控还是委派出去的agent都不能是claude code或者codex。”ranch/main 是 Claude Code，只写本文件、看交回的结果，不 ssh 上去操作；R10、R14 由主控经 ssh 做的办法不再用。执行的 pi、omp 不再往下派活。
类型：其他
依据：R17b 已在 macOS 上让四家关在 srt 里照常干活（10-19 部署到本机 `7fbb76d`）；fence 的 Linux 规则（`[linux]` 段、bubblewrap、seccomp 记录）只有自动化测试，没在真 Linux 上跑过。本轮在 5090 上实测，不改代码；发现要改代码的，另开一件在本机派出去修，修好再回来复验。

## 用户原话

- 10-18：“先在我本机完全跑通了，再测试5090那台”。
- 10-18：“可以，就按这样分，先接 srt（但是要支持linux）。”
- 10-18（关于严格程度）：“不需要严格限制。我只是在要通过做这个来彻底完全的了解一下sandbox。”
- 10-15（那台机器的规矩，用户说明是暂时的）：“那上面有pi……你可以拿pi和omp做实验。但是也不要让他们委派任何活……不要在任何情况下运行那台机器上的claude code和codex”。

## 现状（主控 10-19 经 ssh 只读查的；那时还没领会上面那条规矩，以后不再这样查）

- 机器：R10、R14 那台（ssh 别名和地址不进仓库，写 `<host>`），Ubuntu 24.04.4，内核 7.0，x86_64。
- 依赖已经都在：`/usr/bin/bwrap`、`socat`、`rg`。node 在 nvm 里（v24.15.0，满足 ≥ 22.12），不在非交互 ssh 的 PATH 里。
- AppArmor：`kernel.apparmor_restrict_unprivileged_userns = 1`，但系统自带 `/etc/apparmor.d/bwrap-userns-restrict`；试过 `bwrap --unshare-net --unshare-pid` 能起，墙里再 `--unshare-user` 也能起。看起来不用 sudo 改设置，以 srt 实跑为准。
- ranch：代码 `~/Develop/ranch` 在 `e6fcb67`（R16），装的是 `versions/5de4583/`（R14），没有 `fence`，没有在跑的 agent。
- 模型：pi、omp 都用 deepseek（`api.deepseek.com`，fence 的 `dev` 档案已放行）。机器上 11434 端口有个 ollama，但 pi、omp 都没配它。

## 要做的

1. 本机 `git push <host>:~/Develop/ranch main`，那边到 `7fbb76d` 之后的 main。
2. 那边跑 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`（用的是临时目录里的假 `claude`、`codex` 脚本，不调真程序）。真 srt 那组 `#[ignore]` 测试（`crates/fence/tests/srt.rs`）借用 `--kind claude` 的规则，这台机器上不跑，由第 5 步代替。
3. 那边 `cargo run -p ranch-package` 打包，核对 `BUILD.txt`：目标是 Linux、工作区干净、srt 0.0.80、记下的 node 路径是 nvm 里那份。
4. 部署：`corral`、`ranch` 两个链接切到新版本目录，新增 `~/.local/bin/fence`。技能没变，不重装；没有在跑的 agent，不 `corral upgrade --all`。
5. 先用合成脚本验墙本身（`fence check`、`fence run --kind pi`／`--kind omp -- <脚本>`），确认 srt 在这台机器上起得来、读写网三样拦得住、`fence denials` 有记录。
6. pi、omp 各开一个 `ranch/test-sandbox-<种类>`（`--sandbox dev`、`--label role=test`、`--env CAIRN_DISABLE=1`；`pi`、`omp --approval-mode yolo`），在 HOME 下的临时合成仓库里各干一件小活，照 R17b 的验收走一遍（见下）。一家做完、存好证据（`corral read`/`reply`），再开下一家，用完 `corral stop`。
7. R17b 方案留给 R17c 的几项，在第 5、6 步里顺带看：
   - `[linux]` 段的两处整目录放行（worktree 的 ref、锁目录）够不够用；
   - HOME 盖了可写 tmpfs 后，「写主仓库」按主仓库内容没变来判，不按有没有报错；
   - bwrap 下 `pause`/`resume`、`stop` 能不能把整组进程冻住、杀干净；
   - seccomp 的写记录能不能进 `fence denials`。
8. 结果写进本文件「完成记录」，更新 HANDOFF 里远端装的版本，DESIGN §8 补上 Linux 实测的结论；收尾提交。

## 不做

- 那台机器上任何情况下都不运行 Claude Code、Codex；pi、omp 不委派活。测试时也不碰它们（用户 10-19：“我的那台5090上我不想测试的时候碰codex和claude code。”）：不读、不写 `~/.claude`、`~/.codex`，`fence` 不用 `--kind claude`／`--kind codex`，不跑借用 Claude 规则的真 srt 测试。
- 主机名、地址、用户名不进仓库。
- 不动那台机器上别的东西：不 `pkill`，不改 shell 配置，不往真实的 `~/.fence/` 写 `local.toml`。要装系统包、要 sudo、要改 AppArmor 或内核设置的，停下来问用户。
- 不读真实密钥：拦截一律用自己放的假密钥文件试。
- 不在那台机器上改代码。要改 fence、corral 的，写成新任务，在本机派出去修，合并后再回来复验。
- 不做 paddock 界面（P5-120），不接 amp、agy。

## 怎么算做完（待用户定）

主控建议照 R17b 用户定的验收，换成 Linux 上的 pi、omp：

1. 那台机器上标准检查（test、clippy、fmt）全过，第 5 步的合成脚本结果对。
2. 新版本装好，`fence --version` 能跑。
3. pi、omp 用 `corral start --sandbox` 关在墙里，各干完一件活（改代码、编译、测试、提交）。
4. 该拦的确实拦住了（读没放行的假密钥文件、写主仓库、连没放行的网站）。
5. 被拦的记录找得到（`fence denials` 按 agent 取）。
6. 关在墙里时状态照常更新（`corral status`）。
7. `corral stop`、暂停和恢复照常。
8. 测试 agent 都停掉，合成仓库、假密钥、`~/.fence/runs` 里的测试记录清掉。

## 对 paddock 的影响

只验证、不改公开约定，paddock 不用动。验完 paddock 的 P5-120 可以按 Linux 也支持来接（远端先 `command -v fence`，再试 `~/.local/bin/fence`，见 CONTRACT）。
