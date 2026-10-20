# 任务：套 dev 墙的四家 agent 用同一套办法查资料；dev 白名单加 GitHub 和 Rust 文档

2026-10-20，ranch/main 交给 ranch/dev-r29（Codex，gpt-6-astra / high）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.71）、要、碰要害）
类型：功能变更
依据：套 dev 墙后 pi 查不了资料、omp 的搜索全被拦、Claude Code 读网页被拦，只有 Codex（带 `--search`）和 Claude Code 的 WebSearch 能搜。本轮让 Claude Code、Codex、pi、omp 套墙时都通过 Exa 的 MCP 搜索和读网页，dev 档案放行 Exa 和 GitHub、Rust 文档几个站；不做 fence 自己的上网网关，不放行别的站。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-20：「等一下，那么pi需要查资料的时候会怎么做？」「所以说，我想的是，这几个能不能用一套方案？」
- 10-20：主控调研后建议（摘要）：1. 四家统一接 Exa 的 MCP，fence 只多放行 `mcp.exa.ai`；3. dev 白名单加 GitHub 等常用站。用户：「我的意见呢，就是根据你的建议：做1，3」。
- 10-20：第 3 条选「GitHub + Rust 文档」：github.com、api.github.com、raw.githubusercontent.com、codeload.github.com、objects.githubusercontent.com、docs.rs、doc.rust-lang.org、static.rust-lang.org。
- 10-20：放行 GitHub 后可能借钥匙串里的凭证 git push，用户选「写进验收，必须推不上」：开发时实测；推得上就想办法挡住，挡不住就停下来问。
- 10-20（第一轮停下后改）：Claude Code 登录也靠钥匙串，挡推送和 Claude 登录冲突。用户：「我觉得不用限制这么严格。如果平时直接使用claude code或者codex岂不是也有这样的问题？而且经常开始by pass」。**推送不挡、不验收**，照常放行 github.com，钥匙串读取照旧；推送靠任务书「不推送」管。原来的第 3 条和推送实测取消。

## 先读

- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」和「实现中要改设计，先改 DESIGN.md」。
- `docs/DESIGN.md` §8 sandbox。
- `crates/fence/README.md`（数据怎么合并、`args` 只能写在种类数据里、`[services]`）。
- `crates/fence/data/profiles/dev.toml`、`crates/fence/data/agents/*.toml`、`crates/fence/data/services/*.toml`。
- `docs/CONTRACT.md`「sandbox 与 `fence`」一节。
- Exa MCP 官方说明：远程地址 `https://mcp.exa.ai/mcp`，默认提供 `web_search_exa`（搜索）和 `web_fetch_exa`（读指定网页，由 Exa 服务器抓取），不需要 API key（免费但限频）。

## 主控查到的（10-20，实测）

- 套 dev 墙：Codex 带 `--search` 能搜；Claude Code 的 WebSearch 能搜，WebFetch 读 blog.rust-lang.org 报 `EGRESS_BLOCKED`；pi 只有 read/bash/edit/write，`curl` 被拦；omp 内置的 `web_search` 依次试 parallel、exa、firecrawl、startpage、duckduckgo 都被拦，ecosia/google/mojeek 要开本机浏览器也被拦，`read` 读网页被拦。
- 用隔离的 FENCE_HOME 只多放行 `mcp.exa.ai`，omp 的 `web_search` 就能搜到结果（走 exa）；只放行 `html.duckduckgo.com` 仍搜不到。
- 本机 git 的 GitHub 凭证由 `/opt/homebrew/etc/gitconfig` 里的 `credential.helper osxkeychain` 提供；dev 墙给 claude 种类放行了读 `~/Library/Keychains`。`~/.config/gh` 在墙里读不到。
- `corral start --sandbox` 起的 fence 不带调用方设的 `FENCE_HOME`；要用隔离配置实测，直接用 `fence run`。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r29-research-access`，分支 `r29-research-access`（已从 main 建好）。只动 `crates/fence/`、`docs/DESIGN.md` §8、`docs/CONTRACT.md` 里 fence 相关的句子，以及本任务文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r29-research-access`。

## 要做的

1. dev 档案放行 `mcp.exa.ai`，以及用户选定的 8 个域名（见「用户原话」），四家都生效。只加这些。
2. Claude Code、Codex、pi、omp 套 dev 墙启动时，都能用 Exa 的 MCP 搜索和读网页，不用改用户各家的全局配置（不写 `~/.claude*`、`~/.codex`、`~/.pi`、`~/.omp`）。怎么接上由你定，优先沿用 fence 现有机制（如种类数据的 `args`）；某一家做不到，停下来报告，说清原因和可选办法。omp 已内置 exa，能用就不必另接。
3. ~~套墙时推不上 GitHub~~（10-20 用户取消，见「用户原话」最后一条）：不挡推送，不改钥匙串等读写路径。
4. 先改 DESIGN.md §8 写清这次的设计（查资料走 Exa MCP、白名单范围、为什么不放别的站、推送不挡及理由），再实现。写清对 paddock 的影响。

## 怎么算做完

- 用户原话：「这几个能不能用一套方案？」「做1，3」：四家套 dev 墙后都通过 Exa 的 MCP 能搜到资料、能读一个指定网页；dev 白名单多了 `mcp.exa.ai` 和那 8 个域名。
- 验证只做这些：
  - 改测试里内置规则的期望（域名、各家参数），先只改测试跑一次看它失败，再改数据让它通过；fence 包的测试跑一次；`git diff --check`。
  - 项目标准检查各跑一次：`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。
  - 用编译目录里的程序，四家各套 dev 墙用非交互方式跑一次（`fence run ... -- <agent> -p "<只查不改的问题>"`，Codex 用它的非交互方式），各做一次 Exa 搜索、一次 Exa 读网页（比如 https://blog.rust-lang.org/），记下用的工具名和结果；再各用 bash 读一次 `https://raw.githubusercontent.com/rust-lang/rust/master/README.md` 的前几行。
  - 觉得不够，在回复里说，不要自己加。

## 不要做

- 不推送到任何远端，也不做推送实测。不用 `git credential fill` 之类会把凭证打印出来的命令。
- 不放行用户选定之外的域名，不改各家已有的读写路径（包括 claude 的钥匙串读取）。
- 不做 fence 自己的上网网关，不接 Exa 以外的搜索服务，不注册或使用 Exa 的 API key。
- 不改 corral 的程序和 corral、corral-dispatch 技能。
- 不动 `~/.fence`、各家 agent 的全局配置和认证文件。
- 不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 遇到上面「要做的」第 2 条说的做不到的情况，停下来报告，等决定，不要自己换别的办法。
- 不合并到 main，不推送。只在 `r29-research-access` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

状态：**阻塞，功能未完成、不可合并部署为 R29 实现**。2026-10-20，ranch/dev-r29 按第 2、3 条的停止条件交回主控。

- 做了什么：先读规矩、设计和任务；核对 Exa 官方说明、本机 CLI 帮助、pi 本机文档及 omp 官方源码，先在 DESIGN §8 写候选设计，再尝试九个域名和四家临时 MCP 接入。Claude 用 `--mcp-config`，Codex 用 `-c`，pi 用实例临时扩展注册 MCP，omp 候选用临时插件。为阻止 GitHub 凭证访问，尝试撤销 Claude 的钥匙串读取。Claude 认证失败后停止，撤回全部实现与测试改动，只提交本记录和 DESIGN 的阻塞说明；paddock 不用动。
- 验证了什么：先只改 `four_agents_match_the_development_profile` 的域名和参数期望，运行单测，确实因缺少新增域名失败（RED）。改实现后跑了一次 `cargo test -p fence`：该规则测试通过；单元测试 2 过，CLI 测试 14 过、2 失败（另一个旧域名期望未同步；合成 `/bin/sh` 不接受新注入的 `--mcp-config`）。尝试调整夹具后未重跑，随后随实现撤回，不能据此宣称整包 GREEN。
- 真实实测：使用任务独立编译目录里的 `debug/fence`，在编译目录配备现装 srt 0.0.80 的运行资源，隔离 `FENCE_HOME` 和缓存目录；临时仓库只建本地空提交，remote 为指定的 `https://github.com/firegnu/ranch.git`。前台执行一次 Claude 非交互验收，提示词依次要求 Exa 搜索、Exa 读指定网页、bash 读 raw README、指定 `git push --dry-run`。初始化记录显示动态 `fence_exa` 为 `connected`；紧接着最终结果为 `is_error:true`、`Not logged in · Please run /login`，进程退出码 1。四项工具调用均未发生，不能算搜索、读网页或拒绝推送验收通过；没有真实推送，也没有执行任何打印凭证的命令。原始日志含本机路径和其他插件状态，只留在本机临时目录，未提交。
- 拿主意的地方：不把清空 `credential.helper` 等可被子进程撤销的环境设置当作墙的安全边界；不为通过认证恢复钥匙串读取后就宣称推送已被挡住。pi 本机已支持原生 MCP，因此无需另做 MCP 协议客户端；omp 搜索能用 Exa，但不能由此推断其 `read` 也走 Exa。
- 没做的事：Codex、pi、omp 的真实验收未运行；四家的 `git push --dry-run` 均未执行。因遇到停止条件，没有继续整项目测试、clippy 或 fmt 检查；最终文档变更用 `git diff --check` 验证。没有改全局配置、认证文件、用户 agent、其他仓库；没有安装、部署、合并或推送。
- 踩过的坑：Claude 的钥匙串读取不仅可能涉及 GitHub 凭证，也用于 Claude 自身登录；整段撤销会使 Claude 在模型调用前就未登录。真实启动还报告现有 cairn SessionEnd hook 的路径被墙拒绝，未修改 hook。新增 Claude 启动参数会影响借用 claude 种类运行 `/bin/sh` 的合成夹具；后续实现需要同步隔离这些夹具的 CLI 参数。
- 要主控决定的事：是否允许另行评估“墙外仅取 Claude 登录凭证、临时传给墙内 Claude，同时禁止墙内读取钥匙串”的方案，或指定其他凭证隔离办法。尚未证明这条候选路线可用，本轮没有读取、复制或注入登录凭证。不能把此次认证失败说成 GitHub 推送隔离已完成，也不能据此断言所有其他办法都做不到。

核对依据：[Exa MCP 官方说明](https://exa.ai/docs/get-started/exa-mcp)、[pi 当前会话 MCP 注册接口](https://pi.dev/docs/latest/extensions)、[omp MCP 配置](https://github.com/can1357/oh-my-pi/blob/main/docs/mcp-config.md)、[omp 内置 Exa 搜索实现](https://github.com/can1357/oh-my-pi/blob/main/packages/coding-agent/src/web/search/providers/exa.ts)。

### 第二轮（2026-10-20）

状态：**新版任务已完成，待主控审查；未合并、未部署、未推送**。本轮先 `git merge main` 取得取消推送限制的新任务；上面的阻塞记录保留为第一轮事实，不再是当前阻塞。

- 做了什么：先改 DESIGN §8，明确推送不挡及用户理由，再实现 `dev` 九个指定域名的新增。四家通过原生 MCP 客户端接 Exa：Claude/Codex 注入启动配置，pi 加会话扩展，omp 加临时插件。资源编进 fence，每次写在实例临时目录，沿用原有七天清理；种类仍只用既有 `args` 机制选择接法。没有改各家已有读写路径和全局配置。同步 README、CONTRACT，命令和 JSON 格式不变，paddock 不用改。
- 验证了什么：第二轮先只改规则测试期望，单测因九个域名缺失失败；实现后四家域名、参数断言通过。fence 包第一次受上轮留在编译目录的真实 srt 资源影响，“缺少运行时”的夹具失败；移开该资源后重跑，20 过、8 个真实 srt 测试按原设置忽略。`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次均通过（全量测试 180 过、11 忽略）。之后真实实测发现 pi 文件后缀问题，参数单测再次 RED→GREEN；后续仅修正 pi 免认证头和 omp URL 工具列表，重新编译并用真实调用复验，没有扩展验证范围。最终 `git diff --check` 通过。
- 真实实测：使用独立编译目录中的 `debug/fence` 和现装 srt 0.0.80，隔离 FENCE_HOME 与缓存目录，所有命令在前台等待退出。版本：Claude Code 2.1.304、Codex CLI 0.163.0、pi 1.3.0、omp 18.12.3。每家成功验收均完成一次搜索 `Rust official blog`、一次读取 `https://blog.rust-lang.org/`，两项都返回标题 `The Rust Programming Language Blog`；再通过 shell 执行指定 raw GitHub README 命令，四家均读到从 `<div align="center">` 开始的前五行，退出码 0。没有任何推送或推送实测。

| agent | 实际搜索／读网页工具 | raw README 工具 | 结果 |
| --- | --- | --- | --- |
| Claude Code | `mcp__fence_exa__web_search_exa`、`mcp__fence_exa__web_fetch_exa`（先经 ToolSearch 加载） | `Bash` | 三项成功，进程退出 0 |
| Codex | `mcp__fence_exa__web_search_exa`、`mcp__fence_exa__web_fetch_exa`（经 `functions.exec` 的工具目录发现并调用） | `functions.exec_command` | 三项成功，进程退出 0 |
| pi | `mcp__fence_exa__web_search_exa`、`mcp__fence_exa__web_fetch_exa` | `bash` | 三项成功，进程退出 0 |
| omp | `mcp__fence_exa_fence_exa_web_search_exa`、`mcp__fence_exa_fence_exa_web_fetch_exa`（原生 `xd://` 设备的 read 查 schema、write 调工具；不是写本地文件） | `bash` | 三项成功，进程退出 0 |

- 拿主意的地方：不用 fence 自己实现 MCP 协议或上网代理。pi 原生 HTTP MCP 默认会触碰 OAuth 存储，因此显式给免认证 Exa 空 `Authorization` 头；既不提供 API key，也不新增认证文件写权限。omp 内置搜索无法替代读网页，所以显式请求这两个工具并保留临时 MCP。临时资源统一写出，实际由种类参数加载，避免在 Rust 启动流程里按 agent 名字分派。对用户同名覆盖仍维持整份替换，不悄悄改用户规则。
- 没做的事：按新版决定不挡推送、不测推送；没改钥匙串或其他原有读写路径。未改 corral、技能、paddock、用户 agent、认证文件或全局配置；未安装、部署、合并到 main、推送。Linux 真实环境仍属于暂缓的 R17c，本轮没有额外测试。真实日志只留本机临时目录，未把插件状态、用户路径或模型会话日志提交到仓库。
- 踩过的坑：
  - Codex 首次通过 `tool_search.tool_search_tool` 发现工具时，客户端报 `unsupported call: tool_searchtool_search_tool`；raw 读取成功，Exa 未调用。复验提示它使用现有 `functions.exec`／`ALL_TOOLS` 目录，实际 MCP 调用成功。未修改 Codex 配置、版本或模型；这是本机当前客户端的工具发现限制，不能把首次失败省略成一次全过。
  - pi 首次 `.mjs` 扩展被加载器跳过，改 `.ts` 后又因默认 OAuth 读取尝试创建 `~/.pi/agent/mcp-auth.json` 而失败（墙已拦住）。定位期间前台运行过一次 `/mcp` 状态命令，没有模型调用；空认证头绕过不需要的 OAuth 加载后，两项 MCP 调用成功。
  - omp 会过滤未显式请求额外工具的 Exa MCP 配置；首次未加载工具，它自行尝试 eval/Python 和设备发现但失败，没有产生实现或测试脚本。后续提示明确禁止 eval/Python；URL 加 `tools=web_search_exa,web_fetch_exa` 后，原生 MCP 设备成功调用两项工具。
  - Claude 仍有现有 cairn SessionEnd hook 路径被墙拒绝的提示；Codex 有其他全局 MCP／技能路径被墙拒绝的提示。没有放开这些路径或域名，三项目标操作和进程正常完成。
- 要主控决定的事：没有新增决策项。主控审查时注意上述 Codex 工具发现限制；如果要独立修 Codex 客户端，另开任务，不在本轮扩大范围。

补充依据：[pi MCP 权限与 OAuth 规则](https://pi.dev/docs/latest/mcp)、[omp Exa MCP 过滤逻辑](https://github.com/can1357/oh-my-pi/blob/main/packages/coding-agent/src/mcp/config.ts)、[Codex MCP 配置](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)。
