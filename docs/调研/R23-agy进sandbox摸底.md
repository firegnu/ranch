# R23：agy 进 sandbox 摸底

2026-10-19，ranch/dev-r23-1。只调查本机 macOS，未改 ranch 代码、内置数据或插件，未手改用户设置。测试使用现装 `versions/aa3676b` 的 corral/fence、srt 0.0.80、Node v24.13.1，以及 **agy 1.4.2**（早先 R8 调查的是 1.3.5）。

**结论：当前版本不能只加 `agy.toml` 就接通。实测先卡在 agy 内置 language server 的 TCP 本地监听，尚未走到钩子、模型请求和执行任务。** `allow_pty=false/true` 都以相同错误退出。已知的钩子父进程检查仍是后续代码障碍，但本轮没有运行到足以直接验证钩子拒绝的阶段，不能把空事件文件归因于那条检查，更不能说它是唯一障碍。

本轮没有试出能完成工作的规则；下文保留实际试过的候选配置，**不是可用配方**。六项中未能执行的部分明确记为未知，未以墙外或合成程序的结果代替同一个真实 agy 的验收。

## 实测入口与证据

临时根目录为 `$HOME/.r23-agy-probe.v385Gu`：`repo/` 是普通合成 Git 仓库，`outside/` 放假密钥和预期被拒的写入目标，`fence/` 是隔离 FENCE_HOME。未读取真实 `~/.fence/local.toml`，未往真实 FENCE_HOME 写入规则。

三次均使用同一个测试名字，前一次自动退出后才重开：

```sh
corral start ranch/test-sandbox-agy \
  --cwd "$PROBE_ROOT/repo" --label role=test \
  --env CAIRN_DISABLE=1 --env FENCE_HOME="$PROBE_ROOT/fence" \
  --env R23_OUTSIDE="$PROBE_ROOT/outside" --sandbox dev \
  --prompt '<运行合成仓库 probe.sh、保存结果、提交；不读真实凭证、不改设置、不绕过权限错误>' \
  -- agy --dangerously-skip-permissions
```

`PROBE_ROOT` 是上面的临时根目录。真实首句包含具体操作和 DONE 约定，没有传模型参数。所有观测、启动和收尾命令均在前台等待完成，没有挂后台提醒。

| 次序／实例 | 配置和目的 | 观测 |
| --- | --- | --- |
| 1／`8d4260342c25` | `allow_pty=false`；初始候选规则 | start 成功，随后退出码 1。denials：file-write 2、other 12、network 0、file-read 0；两条写拒绝都是 `antigravity-cli/cli.log.tmp`。|
| 2／`e9251290e40b` | 改 `allow_pty=true`，补 `cli.log.tmp` 写权限 | start 成功，短暂 `starting`、`last_event:null`、reply 为 `no_reply`，随后退出码 1。denials：other 3，其余 0。|
| 3／`2a18546f8466` | 沿用第二次配置，抓启动瞬间的进程、事件和信任条目 | 仍退出码 1。抓到 pen PID 49572 → Node runner PID 50290，后者当时有一个 git 子进程；未抓到稍后短命的 agy。事件文件大小 0，status 为 `starting`、`last_event:null`；信任条目存在，退出后自动消失。denials 同第二次。|

三次各自的 agy 日志均出现：

```text
server.go:671] Language server will attempt to listen on host localhost
main.go:905] CLI failed to start - listen tcp 127.0.0.1:0: bind: operation not permitted
```

直接来源是本轮新建的 `~/.gemini/antigravity-cli/log/cli-20261019_163217.log`、`cli-20261019_163319.log`、`cli-20261019_163421.log`。仅提取上述启动错误，未拷贝完整日志或凭证。fence 记录的运行时间约 0.16–0.23 秒；因此退出后 `corral read/reply/status` 返回 `not_found`。第二、三次在仍运行时调用 `read` 得到空输出，`reply` 得到 `no_reply`，没有登录／信任／其他对话框可供操作。

第一轮 denials 中还有一条 `deny(1) network-outbound`，归类为 `other`，无目标地址；不能据此推导出所需域名。第二、三轮没有记录那条网络拒绝，仍有相同的 bind 错误，说明不能用「denials 没有条目」判断操作未被阻止。

## 六项结论

### 1. 文件访问和凭证

**实测：** agy 启动会创建 `antigravity-cli/log/cli-<时间>.log`，并尝试创建 `antigravity-cli/cli.log.tmp`。第一轮的临时日志链接写入被拒；补放行后第二轮不再记录这项拒绝，程序仍因 TCP bind 退出。这个变化不足以证明其余文件权限已齐备。

**本机目录观察：** `~/.gemini/config/plugins/corral/` 含已安装的 `plugin.json`、`hooks.json`；`config/` 还有技能、规则、MCP 配置等。`antigravity-cli/` 有配置、日志、会话、缓存、summary 数据库、历史、brain、annotations、presence 等。存在这些目录不等于本轮证明必须放行全部目录。候选配置对 config 整目录只读，对 CLI 配置和状态文件加写保护，对可能的运行状态路径逐项放行写；没有笼统开放整个 `~/.gemini`。

**既有调查＋当前二进制线索：** [R8 任务的登录核对](../任务/R8-接入agy.md) 已记录 agy 使用 macOS 系统钥匙串、临时 HOME 隔不开登录。本轮 1.4.2 二进制仍含 `cli/backend/auth/keyring.go`、`KeyringTokenStorage.LoadStoredToken/SaveToken`、go-keyring 的 macOSXKeychain 符号。它支持「有钥匙串凭证路径」的判断，**不能证明每一种凭证只存在钥匙串**。`~/.gemini` 也确实有名称像 OAuth 凭证的文件；只看文件名和元数据，未读内容，不能把别的 Gemini/IDE 文件直接归为 agy 1.4.2 的登录源。

**未知：** 已登录 agy 在墙内是否能读钥匙串、是否需要 `~/Library/Keychains` 放行、刷新凭证写哪里、运行一轮完整会话的最小文件集。候选规则给了 Keychains 只读权限，但失败发生得太早，不能算验证过。

### 2. 域名与 srt 代理

**二进制线索，非已验证域名清单：** 1.4.2 含 `HTTP_PROXY`、`HTTPS_PROXY`、`net/http.ProxyFromEnvironment`，也含 `cloudcode-pa.googleapis.com`、`oauth2.googleapis.com`、`accounts.google.com`、`antigravity-unleash.goog`、`antigravity-cli-auto-updater-974169037036.us-central1.run.app`。本轮把这五个候选域名加入临时服务数据；用途推测依次是模型后端、OAuth、登录、功能开关、更新检查。二进制还有许多其他 Google 服务地址，不能全部当成必需白名单。

**实测：** 没有到达可验证的登录、模型或更新请求。没有看到 srt 代理记录带域名的拒绝。

**未知：** agy 的实际模型调用、登录刷新、更新检查是否均走 srt 代理；完整必需域名和是否需要 trustd。安装的 srt 源码将 `com.apple.trustd.agent` 放行注释为 Go TLS 证书验证所需的较弱隔离选项，这是后续要观察的风险点，不是 agy 已触发的事实。本轮未开启该选项，未放开全部网络，也未关网络隔离。

### 3. 要不要 `allow_pty`

**实测：** 关闭和开启都在 TCP bind 处退出。关闭时有 `/dev/ttys031` 的 file-ioctl 拒绝，开启后未再记录它；这与终端相关放行生效相符，但日志可能漏记，不能据此证明完整 agy 工具执行是否必须开启。

**读代码：** corral 的 pen 在墙外先创建终端；`allow_pty` 管墙内额外的终端能力，不能从「corral 用终端启动」直接推导其值。

**未知：** agy 的正常交互和 shell 工具是否需要墙内新建 PTY。后续候选先保留 `true`，须在能启动后验证。

### 4. 改文件、跑脚本、提交

**实测未完成：** 合成仓库初始提交是 `c098a1f probe initial`，`message.txt` 内容仍是 `before`，只有测试准备阶段放入的 `probe.sh` 未跟踪；没有 `probe-result.txt`，没有 agy 创建的提交。

首句要求脚本把 message 改成 after、检查内容、保存探针输出，再用合成身份和 `commit.gpgsign=false` 提交。agy 在执行首句前已经退出；不能把准备脚本和初始提交算作 agy 在墙内工作成功。

### 5. 状态、父进程、helper 和插件

**实测：** 抓到 `pen → Node runner`，短暂 `starting`、空读屏、`no_reply`、0 字节 events。没有抓到存活的 agy 或钩子进程；不能声称已经实测完整 `pen → runner → agy → hook` 链，也不能把 startup 时 events 为空当作父进程检查被执行的证据。

**读代码：**

- [sandbox.rs](../../crates/corral/src/sandbox.rs) 把命令交给 fence，并把 corral `.runtime`、版本目录父目录、该 agent 状态目录传为 `--read`，events 传为 `--write`。
- [runner.mjs](../../crates/fence/resources/runner.mjs) 用 Node `spawn` 启动 sandbox 包装的原命令；agy 不会成为 pen 的直接子进程。
- [hooks.rs 的 global](../../crates/corral/src/hooks.rs) 用钩子父进程的 PPID 和 `CORRAL_PEN_PID` 比较，不相等就返回；这一逻辑不能识别现有 fence 包装链。
- [agy 适配器](../../crates/corral/src/agents/agy.rs) 与已安装插件的命令相符：测试 `CORRAL_HELPER` 可执行后 `exec ... __hook agy:<事件>`，插件没有为了本轮被修改。

**推测／未知：** helper 及事件文件的路径放行已有通道，不像 amp 需要重新安装修改后的插件；修正父进程校验大概率可放在 corral 的 helper 实现。但墙内 helper 能否执行、`ps` 能否取得所需进程信息、插件是否实际加载、transcript 是否完整可读，都未到实测阶段，不能宣称除了父进程检查再无问题。

**已查证的另一阻碍：** TCP 本地监听先于上述流程失败。安装的 srt `dist/sandbox/sandbox-config.js` 有 `network.allowLocalBinding`，默认 false；`macos-sandbox-utils.js` 在它为 true 时允许：

```text
(allow network-bind (local ip "*:*"))
(allow network-inbound (local ip "*:*"))
(allow network-outbound (remote ip "localhost:*"))
```

而 fence 的 [rules.rs](../../crates/fence/src/rules.rs) 只接出 `allow_pty`、`weaker_network_isolation` 等已有键，没有 local binding 数据键或转换逻辑。`domains` 是外部代理目的地规则，R20 的 `unix_sockets` 是 Unix socket 路径规则，都不能允许 `listen tcp 127.0.0.1:0`。仅在临时 TOML 写一个新键不具备解决这个障碍的能力。

### 6. 同一个 agy 的隔离探针与 denials

**未执行，不能验收：** 准备的脚本会尝试读 `outside/fake-secret`（内容为明显的假数据，读入 `/dev/null`）、写 `outside/write-attempt`、经默认代理 curl `https://example.com/`。还会检查 helper 可执行、插件可读和事件大小。由于 agy 启动失败，这些探针均未运行；假文件存在不构成拒绝读取的证据。

**实测的有限结论：** fence 确实拒绝了 agy 启动阶段的一些操作，并记录到 denials；第一轮 `monitors.files:true`，后两轮 `files:false`、`network:true`。前者只证明收到过文件事件，后者不能证明无文件访问失败。日志中的 TCP bind 错误后两轮并未出现在 denials。因此目前不能给出「同一个 agy 三种隔离通过且都有记录」的结论。

## 试过的候选数据（失败配置，供后续复现）

第一轮直接在种类数据写 domains、没有服务时，`fence check` 报 `no_service`，尚未启动 agent。随后加临时服务，check 通过。以下是第二、三轮实际种类配置；保留试验时重复的 domains，fence 按并集合并。读写范围仍待缩减和验证。

`$FENCE_HOME/agents/agy.toml`：

```toml
description = "R23 temporary agy probe"
services = ["agy-probe"]
read = ["${home}/.local/bin/agy", "${home}/.gemini/config", "${home}/.gemini/antigravity-cli"]
write = [
  "${home}/.gemini/antigravity-cli/log",
  "${home}/.gemini/antigravity-cli/cli.log",
  "${home}/.gemini/antigravity-cli/cli.log.tmp",
  "${home}/.gemini/antigravity-cli/cache",
  "${home}/.gemini/antigravity-cli/crashes",
  "${home}/.gemini/antigravity-cli/conversations",
  "${home}/.gemini/antigravity-cli/conversation_summaries.db",
  "${home}/.gemini/antigravity-cli/conversation_summaries.db-shm",
  "${home}/.gemini/antigravity-cli/conversation_summaries.db-wal",
  "${home}/.gemini/antigravity-cli/history.jsonl",
  "${home}/.gemini/antigravity-cli/annotations",
  "${home}/.gemini/antigravity-cli/brain",
  "${home}/.gemini/antigravity-cli/implicit",
  "${home}/.gemini/antigravity-cli/presence",
  "${home}/.gemini/antigravity-cli/plugin_data",
  "${home}/.gemini/antigravity-cli/scratch"
]
deny_write = ["${home}/.gemini/config", "${home}/.gemini/antigravity-cli/settings.json", "${home}/.gemini/antigravity-cli/keybindings.json", "${home}/.gemini/antigravity-cli/jetski_state.pbtxt", "${home}/.local/bin/agy"]
domains = ["cloudcode-pa.googleapis.com", "oauth2.googleapis.com", "accounts.google.com", "antigravity-unleash.goog", "antigravity-cli-auto-updater-974169037036.us-central1.run.app"]
[macos]
allow_pty = true
read = ["${home}/Library/Keychains"]
```

`$FENCE_HOME/services/agy-probe.toml`：

```toml
description = "R23 agy service candidates; not all are verified requirements"
domains = ["cloudcode-pa.googleapis.com", "oauth2.googleapis.com", "accounts.google.com", "antigravity-unleash.goog", "antigravity-cli-auto-updater-974169037036.us-central1.run.app"]
```

档案使用现装的内置 dev，没有本机／仓库补充，没有 `FENCE_RUNNER` 替换，也没有 `weaker_network_isolation`。

## 要接进来还差什么

以下是据本轮证据给出的工作量判断，不是已经通过的实现方案；不会据此直接开发。

| 项目 | 数据或代码 | 大概大小及未决点 |
| --- | --- | --- |
| 允许 agy 的本地 TCP 服务启动 | **代码＋设计决定** | fence 接出 srt 的 local binding 配置，包含校验、转换、约定和针对验证，约小到中等改动。上游开关实际允许所有本地地址 bind/inbound，以及 localhost 任意端口 outbound，并非只放行 agy 一个回环端口；须由用户决定是否接受。若要更窄，不能只透传这个开关。|
| 在 fence 包装下仍识别顶层 agy 钩子 | **代码** | corral 的 global hook 校验及启动身份传递，约中等。不能粗暴允许任意后代，否则原来防嵌套 agy 污染事件的约束会失效；还须核对墙内进程查询能力。|
| 路径、域名、PTY、凭证保护 | **主要是数据，尚未测完** | 一份 agy 种类和服务数据本身很小；启动障碍消除后再逐项实测。钥匙串、Go TLS、MCP 启动、运行中写入是否再需要代码支持，目前无法估计。|
| 恢复任务和隔离验证 | **真实验收工作** | 要重新跑本报告六项剩余部分，不能沿用这次的未知结果作为通过。|

主控／用户需要先决定是否继续接入，以及是否接受上游 local binding 的实际放行范围。若继续，应先改 DESIGN 再另派功能任务；本轮没有授权或实施放宽。

**对 paddock：** 本轮只新增调研文档，paddock 不用动。以后新增 fence 数据键需按公开约定说明；是否改前端由后续任务决定。

## 收尾与验证边界

- 三次 agent 均自行退出；最终 `corral stop ranch/test-sandbox-agy` 返回 `not_found`，没有遗留运行中的测试 agent。
- pen 为合成仓库补入的信任路径是 `$HOME/.r23-agy-probe.v385Gu/repo`。第三轮在运行时读该字段确认存在，退出后确认不存在；第一轮退出后也确认不存在，第二轮未单独抓取退出后的字段。最终没有残留，没有手改 settings，主控无需清理这条信任记录。
- 已删除合成仓库、隔离 FENCE_HOME、假文件及临时证据目录；删除的 fence 缓存只限本次三个实例 `8d4260342c25`、`e9251290e40b`、`2a18546f8466`。
- agy 自己生成的上述三份日志留在它的常规日志目录，未主动清理或改写全局日志链接。没有把原始日志、屏幕、凭证或用户会话放进 Git。
- 文档改动用 `git diff --check` 验证；未编译、未跑额外测试、未部署、未合并、未推送。未实现成功配置、未完成真实工作和三项隔离验收，是启动障碍导致的明确限制。
