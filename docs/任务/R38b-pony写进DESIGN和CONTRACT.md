# 任务：R38b 把 pony 的设计写进 DESIGN，把 `pony status` 写进 CONTRACT

2026-10-21，ranch/main 交给 ranch/dev-r38b（omp，默认模型）。
路由：低 / 交叉审查不要 / 影响面：看得见（路由：轻（置信 0.97）、不要、看得见；按第 4 条定为低，类别其余，读 `low-other`，pi、omp 任选）
类型：样式／文案调整
依据：R38 的实现第一件。照 R38a 设计稿定稿，把 pony 写进 `docs/DESIGN.md`、把给 paddock 的 `pony status` 写进 `docs/CONTRACT.md`、`AGENTS.md` 开头那句加上 pony。只改文档，不写代码。合并后 paddock 照 CONTRACT 开 P5-147。
提示：沿用现有视觉和用语约定，聚焦指定的呈现结果。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-21：R38a 设计定稿（设计稿「用户定的（10-21）」一节，共 8 条）。
- 10-21：「加上 pony，写 R38b 任务书吧」（`AGENTS.md`、`CLAUDE.md` 开头那句加上 pony；`CLAUDE.md` 是指向 `AGENTS.md` 的链接，只改 `AGENTS.md`）。
- 底线：「不要动ranch中corral的核心代码。」

## 先读

- `docs/调研/R38a-Slack机器人设计.md`：「用户定的（10-21）」、第 4 节（`pony status`）、第 7 节（对 DESIGN 的改动建议），其余各节按需查。
- `docs/DESIGN.md` 全文的结构，特别是第 1、2、3、5 节和第 8 节（fence 写法可参照）。
- `docs/CONTRACT.md` 开头的规矩和已有各节的写法。
- `AGENTS.md` 开头一段。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r38b-pony-docs`，分支 `r38b-pony-docs`（已从 main 建好）。只动 `docs/DESIGN.md`、`docs/CONTRACT.md`、`AGENTS.md`，以及本任务文件末尾。

## 要做的

1. **`docs/DESIGN.md`**：照设计稿第 7 节改：第 1 节表格加 pony 一行；第 2 节全局位置加 `~/.pony/`、`~/.local/bin/pony`、`~/Library/LaunchAgents/local.ranch.pony.plist`；第 3 节安装加 `bin/pony`、多切一个链接、常驻服务的装和换是用户在场的部署操作、没有配置文件的机器什么都不连；第 5 节记范围扩到聊天软件机器人（用户 10-20、10-21）；新加第 10 节「聊天软件机器人 pony（R38）」，写第 7 节列的要点，连同「用户定的」8 条（含离线补收）。写定论和理由，不抄设计稿的备选比较，细节指向设计稿。
2. **`docs/CONTRACT.md`**：新加一节「`pony status`（R38）」，照设计稿第 4 节写命令、退出码、JSON 示例和每个字段的含义与取值，以及前端怎么找命令、遇到没写的字段和不认识的取值怎么办。写到 paddock 只看这一节就能做 Settings 显示。
3. **`AGENTS.md`**：开头「现在有 corral；以后还有……」那句加上 pony（R38，正在做）。其余不动。
4. 写清对 paddock 的影响：只是兼容新增；合并后 paddock 可照 CONTRACT 这一节开 P5-147，部署 R38c 之前 PATH 上还没有 `pony`。

## 怎么算做完

- DESIGN 写明了 pony 的范围、边界和用户定的 8 条，与设计稿定稿一致；CONTRACT 的 `pony status` 一节与设计稿第 4 节一致；`AGENTS.md` 开头提到 pony。
- 验证只做这些：`git diff --check`；用 `grep` 核对 CONTRACT 一节的字段名和设计稿第 4 节逐个对得上。觉得不够，在回复里说，不要自己加。

## 不要做

- 不写代码、不建 `crates/pony`、不加依赖、不改 `Cargo.toml`。
- 不改设计稿的内容。设计稿和你要写的有出入、或者设计稿哪里说不清，照下面「碰到要拿主意的事」那条问主控。
- 不改 corral 的代码（`crates/corral/`），也不改 CONTRACT 里已有的各节。
- 不新增设计稿以外的规矩。
- 不装技能，不动 `~/.claude`、`~/.agents`、`~/.gemini`、`~/.ranch`、`~/.corral`；不建 `~/.pony`。
- 不改 paddock、fence。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r38b-pony-docs` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：`docs/DESIGN.md` 照设计稿第 7 节改完——§1 表格加 pony 一行，§2 加 `~/.pony/`、`~/.local/bin/pony`、`~/Library/LaunchAgents/local.ranch.pony.plist` 和前端只经命令使用，§3 加 `bin/pony`、多切一个链接、常驻服务的装和换是用户在场的部署操作、没有配置文件的机器什么都不连，§5 记范围扩到聊天软件机器人，新增 §10 写范围、结构、插件式、用户定的 8 条、第一期规矩、常驻和文件、`pony status`、对 paddock 的影响、实现拆分。`docs/CONTRACT.md` 新增「`pony status`（R38）」一节，照设计稿第 4 节写命令、找法、退出码、JSON、逐字段取值、paddock 显示对应和兼容说明。`AGENTS.md` 开头一句加上 pony（R38，正在做）。
- 验证了什么：`git diff --check` 通过；用脚本把 CONTRACT 一节的字段名和设计稿第 4 节逐个对照，除 `chats[].kind` 的正确写法外没有缺项（`kind` 本身在设计稿和 CONTRACT 都在）；`git status` 只有 `AGENTS.md`、`docs/DESIGN.md`、`docs/CONTRACT.md` 和本任务文件改动。
- 拿主意的地方：新节放在 `CONTRACT.md` 文末，不改开头标题和已有各节；字段表补了设计稿第 4 节没单列的 `ok`、`contract`、`version`、`pid`、`started`（都在设计稿的 JSON 示例里），`last_exit` 在跑时按示例写 `null`；`config_message` 只写了 `invalid` 和 `ok` 两处，`missing` 时没猜；频道 `name`、`private` 拆成两行，便于逐个对照。
- 没做的事：不写代码、不建 `crates/pony`、不改 `Cargo.toml`、不改 corral 和 fence、不改 paddock、不部署、不动 `~/.pony` 和用户现有的 agent，也不合并、不推送。
- 踩过的坑：设计稿里内联写三个反引号的代码块说明会破坏 Markdown 行内代码解析，DESIGN §10 改成文字「三个反引号或三个波浪号」。
- 对 paddock：只是兼容新增；合并后 paddock 可照 CONTRACT 的「`pony status`（R38）」开 P5-147，R38c 部署之前 PATH 上还没有 `pony`，按「未安装」显示。
- 要主控决定的事：无。

## 返工记录

- 照主控指出的两处改 `docs/DESIGN.md`：§10「第一期的规矩」路由条在「送的就是用户打的字」后补上设计稿 §3.5 的例外——离线补收的话开头加一句说明（见用户定的第 8 条）；§3 新增的部署条把一起切的链接改为 `~/.local/bin/corral`、`~/.local/bin/ranch`、`~/.local/bin/fence`、`~/.local/bin/pony` 四个。别的不动。
- 验证：只跑了 `git diff --check`，通过。
- 要主控决定的事：无。
