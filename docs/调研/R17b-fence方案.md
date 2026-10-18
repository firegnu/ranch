# R17b：fence 方案

2026-10-18，ranch/dev-r17b-design。只出方案，不写代码、不实跑。依据：`docs/任务/R17b-fence本体.md`（要的结果和验收）、`docs/任务/R17-agent放进sandbox.md`、`docs/调研/R17a-srt小试验.md`、DESIGN §3/§7/§8、corral 现有代码（`agents/mod.rs`、`pen.rs`、`freeze.rs`、`cli.rs`、`state.rs`）、`tools/package`。另外从本机 npm 缓存里解出 srt 0.0.80 的包（没有安装）读了源码，下文标「srt 源码」的都出自那里。

## 要用户拍板的（按重要程度）

1. **fence 怎么起 srt**
   - A（推荐）：fence 带一个约 80 行的 Node 启动脚本 `runner.mjs`，用 srt 的库接口（`SandboxManager.initialize`、`wrapWithSandbox`、违规记录 `getSandboxViolationStore()`）起 agent。被拦记录单独写进 fence 的文件，agent 自己的 stderr 照常在终端里。还能打开 srt 自带的文件拦截监视：macOS 读系统日志，Linux 用 seccomp 观察写操作。srt 命令行不开这个（`cli.js` 调 `initialize` 时没传 `enableLogMonitor`）。代价是仓库里多一个 JS 文件，和「工具用 Rust 或纯数据文件」的规矩有出入；它不是工具，是跑在 srt 自己运行时里的启动代码，性质和 `hook_pi.ts` 一样。
   - B：照 R17a 直接调 `srt --debug --settings … -- 命令`。srt 只有一个 stderr，调试输出和 agent 的 stderr 混在一起（`srt 源码`：`logForDebugging` 一律写 stderr，子进程 stdio 是 `inherit`）。要么整个 stderr 导进文件（R17a 就这样做，agent 往 stderr 写的东西不再出现在终端），要么不留记录。能取到的只有网络拦截，取不到读写文件被拦的记录。
   - 选 B 时，验收「被拦的记录找得到」里读密钥和写主仓库两项只能靠 agent 的屏幕（`corral read`），`fence` 命令只能给出网络拦截。
2. **用户自己能不能改档案和数据**
   - 推荐：内置一套，编进 `fence`；用户在 `~/.fence/` 下放同名文件就整份替换内置的（`fence profiles` 标明来源）；`~/.fence/local.toml` 写本机特有的补充，每次都加上（比如本机 IP Guardian 包装器要读的目录和查 IP 的域名，5090 上本地模型的地址）。
   - 备选：只认内置的，改一条就得出新版本。用户说过想「彻底完全的了解一下 sandbox」，改起来方便更合适。
3. **srt 用哪个 node**
   - 推荐：打包时找 PATH 上的 node（要 ≥ 22.12，这是 srt 0.0.80 的要求），把绝对路径和版本记进版本目录；运行时就用这一个，不在了就启动失败（`runtime_missing`），提示重新打包。本机是 `~/.nvm/versions/node/v24.13.1/bin/node`。
   - 备选：把 node 拷进版本目录。完全自带、不怕 nvm 删版本，但每个版本目录多约 100 MB。
   - 不推荐运行时按 PATH 找：corral 起 agent 用的是登录 shell 的环境，换了 nvm 默认版本，srt 就悄悄跟着变。
4. **仓库补充文件**
   - 推荐：工作区顶层的 `.fence.toml`。worktree 里读的就是它自己那份，跟着分支走。fence 每次都把这个文件加进禁止写，墙里的 agent 改不了它，下一次也就没法借它给自己多放行。
   - 备选：只认主仓库根目录那份，worktree 里的不算。主仓库在墙里本来就写不了，但分支里改了补充文件，要合并之后才生效。
5. **内置几个档案；pi、omp 默认用哪个模型服务**
   - 推荐：只内置 `dev` 一个（在工作目录里改代码、编译、测试、提交）。`dev` 里给 pi、omp 默认配 `deepseek`，这是本机现在用的；5090 上用本地模型，在那台的 `local.toml` 里改。
   - 备选：pi、omp 内置不写服务，没配就报 `no_service`；或者再加一个只读的 `review` 档案给审查用。

下面每项都给了推荐；上面五条以外的，主控审查时有异议再提。

## 1. corral 和 fence 之间的约定

### 找 fence
- corral 的 CLI 先看环境变量 `CORRAL_FENCE`（绝对路径），没有就用自己程序所在目录里的 `fence`（`<版本目录>/bin/fence`，`executable()` 是规范化过的真实路径，不会落到 `~/.local/bin`）。
- 以后 fence 剥离出去，corral 只改这一处：默认路径改成 PATH 上的 `fence`，或者部署时设 `CORRAL_FENCE`。
- 找到的绝对路径由 CLI 写进给 pen 的配置，pen 不再找。

### corral 调的两条命令
1. 启动前检查（在 CLI 里，起 pen 之前）：
   ```
   <fence> check --profile <档案> --kind <种类> --cwd <工作目录>
   ```
   退出码 0、`ok:true` 才往下走。档案不存在或有错、种类没有数据、没有模型服务、程序找不到、node/srt 不在、Linux 缺 bwrap/socat/rg，都在这里报出来，`corral start` 失败，什么都不起。
2. 起 agent（在 pen 里，代替原来直接 exec agent）：
   ```
   <fence> run --profile <档案> --kind <种类> --agent <名字> --instance <实例>
               [--read <路径>]… [--write <路径>]…
               -- <原命令：程序、钩子参数、用户参数、首句>
   ```
   - 工作目录就是进程的当前目录（pen 照旧 `current_dir(cwd)`），环境就是进程的环境（pen 照旧用登录 shell 重建的那份加上 `--env`、`CORRAL_*`）。不单传，少一个可能不一致的地方。
   - `--read`、`--write`：调用方自己要的放行。corral 传它自己的管道：`$CORRAL_HOME/.runtime`（helper 链接）、版本目录的上一级 `~/.local/share/ranch/versions`（helper 指向的程序；只放自己这一版的话，`corral upgrade` 换了 helper 之后钩子就读不到新程序了）、这个 agent 的状态目录（pi/omp 的 `--extension` 钩子文件在这里）可读，`<状态目录>/events` 可写。fence 不知道 corral 的目录长什么样，以后换别的调用方也是这几个参数。任务里写的「传这个 agent 的状态目录」就落在这两个参数上。
   - `--agent`、`--instance` 只当记录的标签，用来按 agent 查被拦记录，fence 不解析。
   - `--kind` 用 corral 的种类名（原命令第一个词的文件名，`claude`、`codex`、`pi`、`omp`）。
3. 不调别的。列档案、看被拦记录由前端直接调 `fence`（见 §5）。

### fence run 做什么
1. 按 §3 合并出这次的规则，展开占位符；按 PATH 把原命令第一个词解析成绝对路径，这个文件和它的真实路径自动放行读（Claude 的 `~/.local/bin/claude` 是指向 `~/.local/share/claude/versions/…` 的链接）。
2. 建运行目录 `~/.fence/runs/<实例>/`：`run.json`（agent、实例、档案、种类、cwd、开始时间、进程号、合并来源）、`settings.json`（给 srt 的规则，0600）、`tmp/`。
3. 设环境：数据里的 `env`（如 `TMPDIR`、`CLAUDE_CODE_TMPDIR`），再加 `FENCE_RUN=<实例>`。
4. 把数据里的启动参数插在程序后面（如 Codex 的本次信任表）。
5. `exec <node> <版本目录>/share/fence/runner.mjs --settings … --denials … --exit … -- <命令>`。fence 自己不留进程，pen 记下的进程号就是 runner。
6. runner：`initialize(settings, 无询问回调, 开监视)` → `wrapWithSandbox` → 起子进程（stdio 继承终端）。拦截事件一条一行写进 `denials.jsonl`。HUP、INT、TERM 都不让 runner 自己退出：pen 发的是进程组信号，agent 自己会收到。子进程退出后 runner 收尾（`reset()` 停掉代理和监视），写 `exit.json`，然后用和子进程一样的方式结束：子进程正常退出就用同一个退出码，被信号杀掉就恢复默认处理、对自己发同一个信号。这样 `corral stop` 看到的 `exit_code`、`stopped_by` 和不套墙时一样。R17a 用 srt 命令行时 HUP 先杀了 srt，所以是 `-1`。

### 出错
- 一律启动失败，不悄悄不套墙。
- corral 这边：
  - 找不到 fence 程序：`sandbox_unavailable`。
  - `fence check` 不过：`sandbox_failed`，原样带上 fence 的 JSON（`"fence":{…}`）。
  - 两种退出码都是 1，pen 不起。
- fence 的错误码（`ok:false` 的 `error`）：
  - `usage`
  - `unknown_profile`
  - `invalid_data`：带 `problems:[{source,path,message}]`。
  - `unsupported_kind`：带 `supported:[…]`。
  - `no_service`
  - `program_not_found`
  - `bad_cwd`
  - `runtime_missing`：node、srt，或 Linux 上的 bwrap/socat/rg 没有。
  - `not_found`：`denials` 找不到运行记录。
- `fence run` 的退出码：
  - fence 或 srt 没能把 agent 起来（检查之后条件变了，或 srt 初始化失败）：125，原因写在 stderr。stderr 就是 agent 的终端，`corral read` 看得到。125 沿用 `env`、`docker run` 的惯例，126/127 留给 shell 的不可执行、找不到。
  - 别的情况就是 agent 自己的退出状态。
- 已知的空隙：检查和起 agent 之间条件变了，或 srt 初始化失败，这时 `corral start` 已经返回 ok，agent 随即以 125 退出，状态成 stopped。要堵上，得让 pen 等 runner 报「墙已建好」（多一个继承的管道），这次不做。

## 2. corral 这边要改的

- **参数**：`corral start NAME [--sandbox <档案>] [其他照旧] -- <命令>`。不带就和现在一模一样。派活技能不改。
- **种类、钩子、首句照旧**：
  - pen 现在按 `argv[0]` 认种类、加钩子参数和首句（`agents::argv`）。顺序不变，先按原命令把这些做完，**最后**再在前面套上 `fence run … --`。
  - 要改的是 pen 里其余用 `argv[0]` 的地方：`agents::kind(program)`（写进 `meta.json` 的 `kind`、`trust::add`）、`agents::env(&raw[0])`。它们都要用原命令的第一个词，不能用 fence 的路径。实现时逐处核对，加测试：套墙后 `kind` 仍是原种类。
  - 不再需要 R17a 那种同名包装脚本。
- **`--prompt` 的检查**：CLI 照旧按原命令认种类，不受影响。
- **进程号、stop、pause、还在不在**：
  - 进程链：pen →（exec）fence →（exec）node runner【`agent_pid`】→ `sh -c` → `sandbox-exec`/`bwrap` → `bash -c` → agent。另外 runner 还会起 `log stream`（macOS）或 socat（Linux）。
  - 全在 pen 用 `setsid` 建的那个会话、那个进程组里：srt 的 `spawn` 不带 `detached`（srt 源码）。
  - `stop`：pen 向进程组发信号（`kill(-agent_pid, …)`），agent 直接收到，和现在一样。Codex 的按键步骤走终端，也一样。runner 不先退（见上节），步骤最后的 KILL 一次清掉全组。
  - `pause`/`resume`：`freeze` 冻进程组、全部子孙和它们所在的会话，runner、srt 的代理、`log stream`、agent 都在里面。Linux 上 bwrap 可能 `--new-session`，但还是子孙，照样能冻上。R17c 要实测。
  - `status`/`ls` 判断还在不在：看 pen 的 socket，pen 跟着 `agent_pid`（runner）走。runner 只在 agent 退出后才退，所以判断照旧。
  - 要补的一处：runner 意外先死、agent 还在时，pen 看到 `agent_pid` 退了，组里剩下的进程就失去了代理和监管。pen 对套墙的 agent 在 `agent_pid` 退出时向进程组补一个 SIGKILL。实现时先看现有 pen 对「agent 退了、组里还剩进程」怎么处理，再定是不是只对套墙的这样做。
  - `corral upgrade` 只换 pen，不碰 agent 进程。runner 和 srt 一直用起它的那个版本目录里的文件，清旧版本时要算上（和 agy 用旧 helper 一样，`ps` 能看到 `node <版本目录>/share/fence/runner.mjs`）。
- **钩子报状态**：钩子在墙里跑 `$CORRAL_HOME/.runtime/helper __hook …`，写 `<状态目录>/events`。靠上面 corral 自己传的 `--read`、`--write` 放行，R17a 四家都验过。
- **记录和输出**：
  - `meta.json` 加 `"sandbox":{"profile":"dev","fence":"<绝对路径>"}`。`argv` 仍记原命令，不含 fence 那层。
  - `status`、`ls`、`where`、`start` 的返回加 `"sandbox":{"profile":"dev"}`，不套墙时是 `null`。用一个对象而不是只给字符串，以后要加字段（比如实例的记录位置）不用再加顶层字段。
  - paddock 的卡片小锁靠 `ls`，所以 `ls` 也要有。
  - 旧 pen 写的 `meta.json` 没有这个字段，读出来就是 `null`。
- **墙里再调 corral**：
  - 墙里 HOME 不可读，Unix socket 也没放行，agent 在墙里调 `corral ls/start/send` 只会报一串看不懂的权限错误。
  - 推荐 corral 照 `CODEX_SANDBOX` 的做法：看到环境变量 `FENCE_RUN`，用户命令报 `sandbox`（退出码 6）「running inside fence」。
  - `__hook` 和 `guide` 不在此列，钩子要照常写事件。
  - 后果：套了墙的 agent 不能再派活。这是这个阶段的限制，以后要支持再说。

## 3. 档案和数据

### 位置和格式
- **格式**：TOML（能写注释，人改起来方便）。fence 合并后生成 srt 要的 JSON。新依赖 `toml` crate，成熟、活跃。
- **内置**：源文件放 `crates/fence/data/{profiles,agents,services}/*.toml`，用 `include_str!` 编进 `fence`，和 corral 的资源一样。
- **用户的**（待拍板第 2 条）：
  - `$FENCE_HOME`（默认 `~/.fence`）下的 `profiles/`、`agents/`、`services/`，同名就整份替换内置的。
  - `local.toml`：本机补充，每次都加上。
  - `runs/`：运行记录。
  - 和 corral 的 `~/.corral` 同一个习惯。
- **仓库补充**（待拍板第 4 条）：`<工作区顶层>/.fence.toml`。

### 一块数据长什么样
档案、agent 数据、模型服务、本机补充、仓库补充都用同一种块，能写的键按来源有限制：

| 键 | 意思 | 谁能写 |
| --- | --- | --- |
| `description` | 一句话说明 | 档案、agent、服务 |
| `deny_read` | 圈掉不许读的（srt `denyRead`） | 档案 |
| `read` | 放行读（`allowRead`） | 全部 |
| `write` | 放行写（`allowWrite`）；写的同时自动放行读 | 全部 |
| `deny_write` | 放行范围里再禁止写（`denyWrite`） | 档案、agent、本机 |
| `mkdir` | 起 agent 前先建好的目录（srt 不会替 agent 建放行目录的上级） | 档案、本机、仓库 |
| `domains` | 放行的域名，可带端口、可写 IP（本地模型） | 全部 |
| `env` | 给 agent 加的环境变量 | 档案、agent、本机 |
| `args` | 插在程序后面的启动参数 | agent |
| `services` | agent：默认用哪些模型服务；档案/本机：`[services]` 表，按种类指定 | agent、档案、本机 |
| `[macos]` / `[linux]` | 只在这个平台生效的同名键，外加 `allow_pty`、`weaker_network_isolation`（只在 `[macos]` 里） | 档案、agent、本机 |

- 不支持 Unix socket 放行：Linux 上 srt 不能按路径放行 socket，只能全开或全关。四家不放行 socket 也干完了活（R17a），v1 不给这个键，免得靠它。
- 路径里不许出现 `* ? [`：Linux 写规则不认通配符，读规则的通配符只在套墙那一刻展开。展开后必须是绝对路径；`..` 按字面规整。
- 仓库补充只许 `read`、`write`、`mkdir`、`domains`，也就是只写额外放行；写了别的键就报 `invalid_data`。

### 占位符
写成 `${名字}`。不用 `{名字}`，因为 Codex 的信任表参数本身是 TOML 内联表，到处是花括号。`${名字:json}` 展开成 JSON 字符串字面量，它同时也是合法的 TOML 字符串。

| 占位符 | 值 |
| --- | --- |
| `${home}` | agent 环境里的 `HOME`（`--env HOME=` 也算） |
| `${cwd}` | 工作目录（规范化后） |
| `${worktree}` | `git rev-parse --show-toplevel`；不在仓库里就是 `${cwd}` |
| `${repo}` | 主仓库的工作区根：`--git-common-dir` 的上一级；不在仓库里就是 `${worktree}` |
| `${git_dir}` | `git rev-parse --absolute-git-dir`（worktree 是 `主仓库/.git/worktrees/<名字>`） |
| `${git_common}` | `git rev-parse --path-format=absolute --git-common-dir` |
| `${branch}` | `git symbolic-ref --short -q HEAD`；游离 HEAD 时没有值 |
| `${tmp}` | `~/.fence/runs/<实例>/tmp`，fence 建好 |
| `${target}` | agent 环境里的 `CARGO_TARGET_DIR`（绝对路径时）；没设就没有值 |

- 路径里用到没有值的占位符，这一条就跳过，`fence check` 在 `skipped` 里列出来。比如不在仓库里时 git 那几条全跳过；游离 HEAD 时分支 ref 那几条跳过。
- 参数里用到没有值的占位符，报 `invalid_data`。
- 读放行里不存在的路径也跳过：Linux 绑不了不存在的路径。写放行不跳过，分支的 `.lock` 本来就要新建。

### 合并
1. 层次，后面的往上加：档案 → 种类数据 → 模型服务 → 仓库补充 → 本机补充 → 调用方（corral 的 `--read/--write`）。
2. 列表取并集、去重。
3. `env` 同一个键后面的覆盖前面的，`fence check` 给出警告。
4. `args` 只来自种类数据。
5. **模型服务**：
   - 先看种类数据的 `services`（Claude 是 `anthropic`，Codex 是 `openai`，pi/omp 没有）。
   - 档案或本机补充的 `[services]` 里写了这个种类的，就整份替换，本机的优先。
   - 最后一份都没有就报 `no_service`。
   - fence 不读各家内部的配置去猜。
6. fence 自己总会加上的：
   - `deny_write`：`${worktree}/.fence.toml`、`$FENCE_HOME`。墙里改不了下一次的规则。
   - 写放行同时放行读。HOME 被 `deny_read` 圈掉了，R17a 里编译目录只放行写、没放行读，结果读失败。
7. 平台段：只用当前平台那段。`[macos]` 的 `allow_pty`、`weaker_network_isolation` 在 Linux 上丢掉，`fence check` 注明「此平台不适用」。srt 在 Linux 上其实也收这两个键、只是忽略（srt 源码），但 fence 不往里写，生成的规则更好读。
8. 生成 srt 规则：
   - `filesystem.denyRead` = `deny_read`
   - `allowRead` = `read` ∪ `write`
   - `allowWrite` = `write`
   - `denyWrite` = `deny_write`
   - `network.allowedDomains` = `domains`
   - macOS 再加 `allowPty`、`enableWeakerNetworkIsolation`
   - 不写 `allowUnixSockets`、`allowAllUnixSockets`、`allowLocalBinding`
9. **还没有数据的种类**：`unsupported_kind`，带上有数据的种类列表。amp、agy 这次不写数据，带 `--sandbox` 就报这个。

### 数据草稿（照 R17a）
内置档案 `dev`：
```toml
description = "在工作目录里改代码、编译、测试、提交；HOME 默认不可读"
deny_read = ["${home}"]
read = [
  "${home}/.gitconfig", "${home}/.config/git",
  "${home}/.cargo/bin", "${home}/.cargo/config.toml", "${home}/.rustup",
  "${home}/.nvm/versions/node",
  "${git_common}",
]
write = [
  "${worktree}", "${tmp}", "${target}",
  "${home}/.cargo/registry", "${home}/.cargo/git",
  "${git_common}/objects", "${git_dir}",
  "${git_common}/refs/heads/${branch}",
  "${git_common}/refs/heads/${branch}.lock",
  "${git_common}/logs/refs/heads/${branch}",
]
mkdir = ["${tmp}", "${target}"]
domains = ["crates.io", "index.crates.io", "static.crates.io"]
env = { TMPDIR = "${tmp}" }

[services]
pi = ["deepseek"]
omp = ["deepseek"]

[linux]
# bwrap 只能绑定已有的文件，ref 改名替换绑定的文件会失败；Linux 上放行整个目录，R17c 验
write = ["${git_common}/refs/heads", "${git_common}/logs/refs/heads"]
```
和 R17a 不同的地方：
- 不设 `GIT_CONFIG_GLOBAL=/dev/null`，改为放行读 `~/.gitconfig`、`~/.config/git`。真实提交要用用户的署名；R17a 的 `~/.config/git/ignore` 读拒警告也就没了。
- 不用隔离的 `CARGO_HOME`，改为共用 `~/.cargo` 的下载缓存，放行 crates.io。R17a 没有第三方 crate，真实仓库有。
- `${git_common}` 整个可读：真实仓库在 HOME 下，R17a 的合成仓库在 `/private/tmp`，按 srt 默认本来就可读，所以 R17a 没暴露这一点。
- 主仓库的工作区不放行，读写都不许。

`agents/claude.toml`：
```toml
description = "Claude Code"
services = ["anthropic"]
read = ["${home}/.local/share/claude", "${home}/.claude.json"]
write = ["${home}/.claude"]
deny_write = ["${home}/.claude/settings.json"]
env = { CLAUDE_CODE_TMPDIR = "${tmp}" }
[macos]
allow_pty = true
read = ["${home}/Library/Keychains"]   # 登录凭据在钥匙串里；读整个目录，范围偏宽
```

`agents/codex.toml`：
```toml
description = "Codex"
services = ["openai"]
write = ["${home}/.codex"]
deny_write = ["${home}/.codex/config.toml", "${home}/.codex/auth.json"]
# 只对这次进程信任主仓库，不写全局 config.toml。整个表一起传：带点的路径用键路径写法不生效（R17a）
args = ["-c", 'projects={${repo:json}={trust_level="trusted"}}']
[macos]
allow_pty = true
# 削弱网络隔离：重新开放 com.apple.trustd.agent，墙里的程序可能经它往外传数据。
# 本机 Codex 没有它 HTTPS 校验证书失败（R17a）；用户 10-18 同意默认开
weaker_network_isolation = true
```
- 只信任 `${repo}`：DESIGN §7（R13）记着 Codex 把 worktree 归到所属主仓库，所以不再像 R17a 那样两个都传。两个路径相同时 TOML 内联表重复键会报错，也是一个原因。实现时验：worktree 里不弹信任框。

`agents/pi.toml`：
```toml
description = "pi"
read = ["${home}/.bun/bin", "${home}/node_modules", "${home}/.pi/agent"]
write = [
  "${home}/.pi/agent/sessions",
  "${home}/.pi/agent/auth.json.lock", "${home}/.pi/agent/settings.json.lock",
]
deny_write = ["${home}/.pi/agent/settings.json", "${home}/.pi/agent/auth.json", "${home}/.pi/agent/models.json"]
[macos]
allow_pty = true
[linux]
# 锁目录要在父目录里新建，Linux 绑不了不存在的路径；放行整个目录，靠 deny_write 护住配置，R17c 验
write = ["${home}/.pi/agent"]
```

`agents/omp.toml`：
```toml
description = "omp (Oh My Pi)"
write = ["${home}/.omp/agent", "${home}/.omp/natives", "${home}/.omp/run"]
deny_write = [
  "${home}/.omp/agent/config.yml", "${home}/.omp/agent/settings.json",
  "${home}/.omp/agent/auth.json", "${home}/.omp/agent/models.json",
]
[macos]
allow_pty = true
```
- R17a 只放行了 `natives/18.12.1`，这里改成整个 `natives`，不跟 omp 版本号绑死。

模型服务：
```toml
# services/anthropic.toml
domains = ["api.anthropic.com", "claude.ai", "platform.claude.com"]
# services/openai.toml
domains = ["chatgpt.com", "ab.chatgpt.com", "api.openai.com", "auth.openai.com"]
# services/deepseek.toml
domains = ["api.deepseek.com"]
```

本机 `~/.fence/local.toml` 示例（不进仓库，用户自己写）：
```toml
# IP Guardian 包装了 PATH 上的 codex（~/.ip_guardian_bin/codex → /usr/local/bin/codex）
read = ["${home}/.ip_guardian_bin", "${home}/.ip_guardian_allowed"]
domains = ["ifconfig.me", "api.ipify.org", "ipv4.icanhazip.com", "checkip.amazonaws.com"]
```

5090 上用本地模型时：
```toml
# 5090 的 ~/.fence/services/local.toml
domains = ["192.168.1.20:8000"]   # srt 默认拦本机和内网地址，要写明 IP 和端口
# 5090 的 ~/.fence/local.toml
[services]
pi = ["local"]
omp = ["local"]
```

ranch 仓库自己的 `.fence.toml`：
```toml
# 编译目录约定见 CLAUDE.md：主仓库和 worktree 共用 ../ranch-worktrees/.target/<子目录>
write = ["${repo}/../ranch-worktrees/.target"]
```

### Linux 和 macOS 专有项
- 只用具体路径，不用通配符，fence 校验时就拒掉。
- 不靠按路径放行本地 socket（v1 根本没有这个键）。钩子靠写文件报状态，不用 socket。
- `allow_pty`、`weaker_network_isolation`、钥匙串只写在 `[macos]` 段里。Linux 上 Claude 的认证在哪要 R17c 另查，但 5090 只跑 pi、omp。
- 需要整个目录放行的写在 `[linux]` 段里：ref 目录、pi 的锁。
- Linux 上 HOME 被 `deny_read` 时，srt 在 HOME 上盖一层**可写**的 tmpfs（srt 源码注释）。墙里写没放行的 HOME 路径不会报错，只是写进临时层、出墙就没了。「写主仓库」那条拦截在 Linux 上可能不是 EPERM，而是「写成功但主仓库没变」。R17c 验收要按「主仓库内容没变」来判。

## 4. worktree 提交

- 放行这几处：
  - `${git_common}/objects`：共享对象库。
  - `${git_dir}`：这个 worktree 的管理目录，`HEAD`、`index`、`logs/HEAD` 等。
  - `${git_common}/refs/heads/${branch}` 和 `${branch}.lock`、`${git_common}/logs/refs/heads/${branch}`：当前分支的 ref 和 reflog。
  - 读 `${git_common}`：config、`packed-refs`、别的 ref。
- 都由 `git rev-parse` 算出来，不靠目录名猜。分支名带斜杠（`feature/x`）照样是具体路径。
- 在主仓库目录里直接干活时，`${git_dir}` 就是整个 `.git`，本来就在 `${worktree}` 底下。srt 自己总会禁止写 `.git/config` 和 `.git/hooks`。
- 不在仓库里、或游离 HEAD 时，对应的条目跳过。
- Linux 上放行整个 `refs/heads`、`logs/refs/heads` 目录，所以 Linux 上能写别的分支的 loose ref。用户说过不严格限制，这里接受。

**`packed-refs.lock` 报错**：
- 四家提交时 git 都试图建 `packed-refs.lock` 被拒，但提交退出码 0、提交确实写进去了（R17a）。
- 原因没查到，关掉自动维护也照样出现。
- 不放行它：`packed-refs` 是所有分支共用的一个文件，能写它就等于能改任何分支。
- 影响：
  - 改代码、提交、`commit --amend` 不受影响；分支只在 `packed-refs` 里时更新它也是写 loose ref，不受影响。
  - 凡是要重写 `packed-refs` 的操作会失败：删除或改名已经打包的分支或标签、`git pack-refs`、`git gc`/维护里的打包、`git fetch --prune`。
  - 墙里也建不了新分支或新 worktree，`refs/heads/<新名>`、`.git/worktrees/<新名>` 都没放行。
  - 这些都不在「改代码、编译、测试、提交」里，失败时 git 会说清楚。
- 实现时补一个验证：用本机 git 版本，在已有 `packed-refs` 的仓库里提交、`amend`，确认退出码仍是 0。

## 5. 给前端的 `fence` 命令（可以直接放进 CONTRACT）

### 写法
- 和 corral 一样：每条命令只输出一行 JSON，`ok:true` 时退出码 0，出错时 `ok:false`、`error`、`message`，退出码 1。
- 遇到没写的字段要忽略，遇到不认识的取值要能显示、不报错。
- `fence run` 是给 corral 的，不在前端约定里。

### `fence --version`
```json
{"ok":true,"version":"0.1.0","contract":"1","srt":"0.0.80","node":"v24.13.1"}
```
前端看 `contract` 是不是 `"1"`。

### `fence profiles`
只读。列出能用的档案和有数据的种类，给 New Agent 选档案、Settings 看档案用。
```json
{"ok":true,
 "profiles":[{"name":"dev","description":"在工作目录里改代码、编译、测试、提交；HOME 默认不可读",
              "source":"builtin","path":null,"overrides_builtin":false}],
 "kinds":[{"kind":"claude","source":"builtin","services":["anthropic"]},
          {"kind":"codex","source":"builtin","services":["openai"]},
          {"kind":"pi","source":"builtin","services":[]},
          {"kind":"omp","source":"builtin","services":[]}],
 "services":[{"name":"anthropic","source":"builtin"},{"name":"deepseek","source":"builtin"},{"name":"openai","source":"builtin"}],
 "local":{"path":"/Users/u/.fence/local.toml","exists":true}}
```
- `source`：`builtin` 或 `user`。用户文件和内置同名时 `source` 是 `user`、`overrides_builtin` 是 `true`。
- 用户文件读不了或格式不对，这一项照列，多一个 `error`。整条命令仍然 `ok:true`，检查交给 `fence check`。

### `fence check --profile <档案> [--kind <种类>] [--cwd <目录>]`
- 只读。
- 不带 `--kind`：只查档案本身和本机补充。
- 带 `--kind`：再合并种类数据、模型服务、仓库补充，展开占位符，给出这次会用的规则。`--cwd` 默认当前目录。
- 检查 node、srt；Linux 上还查 bwrap、socat、rg（srt 0.0.80 在 macOS 上不再要 rg，srt 源码）。
```json
{"ok":true,"profile":"dev","kind":"codex","platform":"macos","cwd":"/Users/u/w/x",
 "layers":[{"source":"profile","name":"dev","origin":"builtin"},
           {"source":"agent","name":"codex","origin":"builtin"},
           {"source":"service","name":"openai","origin":"builtin"},
           {"source":"repo","path":"/Users/u/w/x/.fence.toml"},
           {"source":"local","path":"/Users/u/.fence/local.toml"}],
 "settings":{"network":{"allowedDomains":["…"],"deniedDomains":[]},
             "filesystem":{"denyRead":["…"],"allowRead":["…"],"allowWrite":["…"],"denyWrite":["…"]},
             "allowPty":true,"enableWeakerNetworkIsolation":true},
 "env":{"TMPDIR":"<运行时才有>"},"args":["-c","projects={…}"],
 "skipped":[{"entry":"${target}","reason":"CARGO_TARGET_DIR not set"}],
 "warnings":["weaker_network_isolation: 削弱网络隔离（com.apple.trustd.agent）"],
 "runtime":{"node":{"path":"/Users/u/.nvm/versions/node/v24.13.1/bin/node","version":"v24.13.1","ok":true},
            "srt":{"version":"0.0.80","ok":true}}}
```
- `settings` 就是交给 srt 的规则，Settings 页可以原样显示。
- `${tmp}` 这类运行时才有的值显示成 `<运行时才有>`。
- 档案有错时 `ok:false`，`error` 是 `invalid_data`、`unknown_profile`、`unsupported_kind`、`no_service`、`runtime_missing` 之一，`problems` 列出每处错在哪个文件哪一项。

### `fence denials --agent <名字> [--instance <实例>] [--limit <n>]`
- 只读。看某个 agent 被拦了什么。
- 不带 `--instance` 取这个名字最近一次运行。前端从 `corral status` 拿 `instance` 传进来，名字被重用也不会串。
```json
{"ok":true,"agent":"ranch/dev-x","instance":"0123abcd4567","profile":"dev","kind":"codex",
 "started":1760760000.1,"running":true,"exit":null,
 "monitors":{"network":true,"files":true},
 "denials":[{"t":1760760123.4,"type":"network","target":"example.com:443",
             "detail":"deny network-outbound example.com:443 (not on the allow list)"},
            {"t":1760760130.0,"type":"file-read","target":"/Users/u/.ssh/id_ed25519",
             "detail":"cat(4242) deny(1) file-read-data /Users/u/.ssh/id_ed25519"}],
 "truncated":false}
```
- `denials`：按时间先后；`--limit` 默认 200，取最后 n 条。
- `type`：`network`、`file-read`、`file-write`、`other`（如 mach-lookup）。以后可能有别的值。
- `monitors`：这次运行能记哪几类。文件监视没起来（比如 macOS 读不到系统日志）时 `files` 是 `false`，这时文件被拦只能在 agent 屏幕上看到。
- Linux 的文件记录是 seccomp 看到的写**尝试**再按规则比对出来的，srt 说明它只是诊断提示、路径可能不准。只有写，没有读。
- `running`：runner 进程还在。`exit`：`exit.json` 的内容（退出码或信号）。
- 不记 srt 事件里的 `command`：整个 agent 就是一条命令，那个字段就是 agent 的完整命令行，含首句。
- 没有这个 agent 的运行记录时报 `not_found`。

### 记录从哪来、存在哪、谁写谁读
- 来源：srt 的违规记录（`SandboxViolationStore`）。网络拦截来自 srt 自己的代理；文件拦截在 macOS 来自 `log stream` 里带 srt 会话标记的 Seatbelt 拒绝，在 Linux 来自 seccomp 观察。都是开了 `enableLogMonitor` 才有。这是待拍板第 1 条选 A 的前提；选 B 就只有网络这一类，靠解析 `--debug` 输出。
- R17a 没从系统日志里取到。当时用 `log show` 按 Sandbox 和路径筛，没按 srt 的标记筛，所以不能说明取不到。实现时第一件就是验这个，取不到就把 `monitors.files` 报成 `false`。
- 存放：`~/.fence/runs/<实例>/`。
  - `run.json`：fence run 写。
  - `settings.json`：fence run 写。
  - `denials.jsonl`：runner 追加，最多 5 MB，满了就停写、记 `truncated`。
  - `exit.json`：runner 写。
  - `tmp/`
  - 只有 fence 和 runner 写，都在墙外。墙里的 agent 读不到 `~/.fence`：HOME 不可读，`${tmp}` 只放行它自己那个子目录，fence 还禁止写 `$FENCE_HOME`。
- 读：只有 `fence denials`。
- 清理：每次 `fence run` 删掉已结束超过 7 天的运行目录。

### `corral` 这边的约定新增
- `corral start … --sandbox <档案> -- <命令>`：套 fence 的墙起 agent。
- `start`、`status`、`ls`、`where` 的返回多 `sandbox`：`{"profile":"dev"}` 或 `null`。
- 新错误：`sandbox_unavailable`、`sandbox_failed`（带 `fence`）。墙里调 corral 报 `sandbox`。
- `~/.corral/<名字>/meta.json` 多 `sandbox` 字段。

## 6. srt 怎么打进版本目录

- **固定版本**：
  - `crates/fence/srt/package.json` 只依赖 `"@anthropic-ai/sandbox-runtime": "0.0.80"`，加上它生成的 `package-lock.json`（每个包的版本和完整性校验都在里面）。两份都是纯数据文件。
  - 升级 srt 是单独任务：改版本号、重新生成锁文件、在本机和 Linux 各跑一遍验证。
- **打包时**，`tools/package` 多做这几步：
  1. 找 `node`、`npm`，查 `node --version` ≥ 22.12。没有就报错退出，什么都不写，提示「打包 fence 需要 node ≥ 22.12 和 npm」。不提供跳过 fence 的选项：现在只有本机和 5090 两台，都有 node（pi 用的就是 nvm 的 node）。
  2. 在暂存目录里 `npm ci --omit=dev --ignore-scripts --no-audit --no-fund --prefix <暂存>/lib/srt`。`--ignore-scripts`：srt 和它的四个依赖都不需要安装脚本，Linux 用的 seccomp 程序是包里预编译好的 `vendor/seccomp`。
  3. `cargo build` 多编 `fence`，拷进 `bin/fence`。
  4. 拷 `crates/fence/resources/runner.mjs` 到 `share/fence/runner.mjs`。
  5. 写 `share/fence/runtime.toml`：`node = "<绝对路径>"`、`node_version`、`srt_version`。
  6. `BUILD.txt` 加上 srt、node 两行，校验和加上 `bin/fence`。
- **版本目录**：
  ```
  bin/corral  bin/ranch  bin/fence
  share/corral/…
  share/fence/runner.mjs  share/fence/runtime.toml
  lib/srt/node_modules/@anthropic-ai/sandbox-runtime/…（连同依赖）
  BUILD.txt
  ```
  fence 按自己的真实路径找 `../share/fence`、`../lib/srt`。测试用 `FENCE_RUNNER` 指向合成的启动程序，不需要 node 和 srt。
- **运行时用哪个 node**：`runtime.toml` 里记的那个（待拍板第 3 条）。不在了，`fence check` 和 `fence run` 都报 `runtime_missing`。
- **大小**：srt 包本身 4 MB 压缩，里面带了 Windows 的程序和 Java 代理，加上依赖解开后估计二三十 MB，每个版本目录多这么多。实现时量一下，写进完成记录。
- **Linux**：做法一样。包是纯 JS 加上预编译的 x64/arm64 seccomp 程序；node 用那台的 nvm node。bwrap、socat、rg 不打包，是系统软件，要用户 sudo 装（R17c 做），缺了 `fence check` 报 `runtime_missing`。打包时 `npm ci` 要能连 npm 源，或者本机缓存里已有。
- 安装规矩不变：打包只生成目录；切链接、装技能是单独的部署操作。fence 不用单独切链接，corral 用同一目录里的那份。前端要调 `fence` 命令，部署时也要切 `~/.local/bin/fence`，和 corral、ranch 一起切。这是部署步骤的新增，写进 DESIGN §3。

## 7. 兼容

**对 paddock**：
- 全是兼容的新增：`corral start --sandbox`、`sandbox` 字段、新错误码、`fence` 自己的命令。paddock 不用动。
- P5-120 里接：
  - New Agent 选档案：`fence profiles`。
  - 卡片小锁：`ls` 的 `sandbox`。
  - 被拦记录面板：`fence denials --agent --instance`。
  - Settings 看档案：`fence profiles`、`fence check`。
- paddock 找 `fence` 和找 `corral` 一样：PATH，远端时先 `command -v fence`、再试 `~/.local/bin/fence`，看 `--version` 的 `contract`。
- 远端没装 fence 时，界面不给套墙的选项。

**新旧 corral 同时在**：
- 旧版 CLI 不认 `--sandbox`，报 `usage`，不会悄悄不套墙起一个。
- 旧版 `status`/`ls` 读新 pen 起的套墙 agent：`meta.json` 的 `sandbox` 不认识就忽略，别的照常，只是看不出套了墙。
- 旧版 `stop`/`pause`/`resume`/`send`：都经 pen，pen 是新版，照常。
- 新版读旧 pen 起的 agent：没有 `sandbox`，报 `null`，确实没套墙。
- `corral upgrade` 在新旧 pen 之间来回换：`metadata` 在快照里是原样的 JSON，旧 pen 不认识 `sandbox` 也会带着它；runner 不受影响。
- 墙里的 agent 调 corral（新版报 `sandbox`，旧版报权限错误）：两种都是失败，不会误操作。
- 旧版本目录里还有 runner 在跑时不能清掉，`ps` 能看到路径。

## 8. 实现怎么拆、怎么验

推荐作为**一件任务**交给 Codex（需求单原定如此），里面按下面的次序做。2 和 3 只靠 §1、§5 的约定，可以和 1 并行；要拆成两个分支并行，就是「fence 一侧（1、4、5）」和「corral 一侧（2）」，最后合起来做 6。

1. **fence 核心**：`crates/fence`，包括数据格式、内置数据、合并、占位符、校验、`fence --version`、`profiles`、`check`。不碰 srt。
   - 测试：
     - 隔离的 `HOME`、`FENCE_HOME`。
     - 用 `git init`、`git worktree add` 在临时目录建仓库，覆盖主仓库、worktree、游离 HEAD、带斜杠的分支、不在仓库里这几种。
     - 四家在合成 HOME 下生成的 srt 规则做成定值对照。
     - 各种错误码。
     - 仓库补充写了不许的键要报错。
     - 路径带通配符要报错。
     - Linux 段、macOS 段按平台取舍：分平台的逻辑抽成纯函数，传平台参数测两边。
2. **corral 接入**：
   - 内容：`--sandbox`、`CORRAL_FENCE` 和同目录查找、CLI 里的 `fence check`、pen 里在原命令外面套一层、`meta`/`status`/`ls`/`where`/`start` 加 `sandbox`、`FENCE_RUN` 防护、runner 先死时清进程组。
   - 测试用 `CORRAL_FENCE` 指向合成的 fence（一个 sh 脚本）：
     - 记下参数和环境，检查 check 也是它答。
     - `run` 时**另起子进程**跑原命令（不 exec），模拟多出来的一层；原命令用 `/bin/cat` 之类。
   - 要查的：
     - 套墙后 `kind`、钩子参数、首句和不套墙一样。
     - 合成 fence 的 check 失败时 `corral start` 报 `sandbox_failed`、没有 pen。
     - 找不到 fence 时报 `sandbox_unavailable`。
     - `stop` 各步信号到得了孙进程，`exit_code` 照实。
     - `pause`/`resume` 冻得住、解得开孙进程。
     - `status`/`ls` 有 `sandbox`、到时转 stopped。
     - 旧格式的 `meta.json` 读出 `null`。
3. **runner 和 `fence run`、`fence denials`**：
   - 写 `runner.mjs`，fence 建运行目录、`exec` runner、清理旧记录、`denials`。
   - `fence run` 和 `denials` 用 `FENCE_RUNNER` 换成合成启动程序测（写 `denials.jsonl`、`exit.json`，退出码 125 和原样透传）。
   - 真 srt 的测试单独标出来：`#[ignore]`，设 `FENCE_TEST_SRT=<srt 目录>` 才跑。用合成程序验墙本身，不用真 agent：
     - `sh -c 'cat <假密钥>'` 被拒；
     - 写放行之外的目录被拒；
     - `curl` 未放行的域名得到 403；
     - `denials` 里有网络那条（文件那几条看 `monitors.files`）；
     - 进程组收到 HUP 后 runner 按 agent 的方式结束；
     - 子进程退出后 runner 退出。
   - 这一组在合并前由做的人在本机跑。
4. **打包**：`crates/fence/srt/` 锁文件、`tools/package` 那几步、`BUILD.txt`。
   - 验证：打一个版本目录到临时位置，查目录结构、`runtime.toml`；用版本目录里的 `bin/fence check --profile dev --kind claude` 应该得到 `ok:true`。
5. **文档**：
   - `crates/fence/README.md`：数据来源（R17a）、资源文件说明。
   - CONTRACT（§5 那段）。
   - DESIGN §8（定下来的边界）、§3（多一个 `fence` 链接）。
   - DESIGN、CONTRACT 等方案定了由主控改，或者在实现任务里写明交给做的人改。
6. **真实 agent 验收**（只能这样验，对应需求单的验收）：
   - 每家各开一个 `ranch/test-sandbox-<种类>`（`--label role=test`），用 `--sandbox dev`。
   - 工作目录是 **HOME 下**一个合成仓库的 worktree，和真实用法一样（R17a 的仓库在 `/private/tmp`，没测到 HOME 下的情况）。
   - 带一个小 Rust crate 依赖，要验 crates.io 下载。

   | 验收（用户原话） | 怎么验 |
   | --- | --- |
   | 四家在本机先跑通，各干完一件活 | Claude Code、Codex、pi、omp 各改代码、`cargo build`、`cargo test`、`git commit`，退出码 0 |
   | 该拦的确实拦住了 | 读 HOME 下的假密钥 EPERM；写主仓库工作区 EPERM；`curl https://example.com` 403 |
   | 被拦的记录找得到 | `fence denials --agent ranch/test-sandbox-<种类>` 里有上面三条：网络一定要有；文件两条看 `monitors.files`，没有就记下，交用户 |
   | 关在墙里时状态照常更新 | `corral status` 走过 starting → working → idle，`sandbox.profile` 是 `dev`；`ls` 有 `sandbox` |
   | `corral stop`、暂停和恢复照常 | 空闲时 `pause`/`resume`；`stop` 后 `ls` 没有它，`ps` 里没有残留的 runner、srt 代理、`log stream` |

   另外要实测的（方案里拿不准的）：
   - Codex 只信任 `${repo}` 时 worktree 里不弹信任框。
   - 已有 `packed-refs` 的仓库里提交退出码仍是 0。
   - runner 让 HUP 后的 `exit_code`、`stopped_by` 和不套墙时一样。
   - macOS 上 `log stream` 能取到文件拒绝。
   - 用户的真实 `~/.gitconfig` 能用，提交带上正确署名。
   - 不放行 `~/.claude.json` 写入时 Claude Code 是否正常（R17a 能干活，但它平时会频繁写这个文件，看有没有报错）。
   - omp 的 `allow_pty` 是否必须：R17a 没单独试，不影响验收，可以不试。
   - 用完 `corral stop`，删掉合成仓库和 `~/.fence/runs` 里的测试记录。
7. **R17c（不在本轮）**：5090 上用 pi、omp 验 Linux：
   - `[linux]` 段的两处目录放行；
   - HOME 盖 tmpfs 后「写主仓库」的判法；
   - bwrap 下 pause/stop；
   - seccomp 写记录；
   - 本地模型地址放行。
