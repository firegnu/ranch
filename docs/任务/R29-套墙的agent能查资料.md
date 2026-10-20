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
