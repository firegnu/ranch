# R41：`ranch deploy` 一条命令部署（设计）

2026-10-22，ranch/dev-r41。只出设计，不写代码、不改 DESIGN、不部署。依据：`docs/任务/R41-ranch-deploy一条命令部署.md`（用户原话和要回答的 8 条）、AGENTS.md「安装」「不要干扰用户正在用的 agent」、HANDOFF「本机安装」「部署时要记得」、DESIGN §3、§10「常驻、状态、凭证、日志」、`crates/pony/README.md`「部署与退回」、`tools/package/src/main.rs`、`crates/ranch/src/main.rs`、corral 的 `skills.rs`、`integration.rs`、`hooks.rs`（helper）、`upgrade.rs`，dispatch 的 `skills.rs`，pony 的 `service.rs`、`main.rs`。引用的现有命令和输出都在本机只读跑过，见第 9 节。

## 一句话

`ranch deploy <版本>` 先只读地检查一遍，打印「要做的、不做的、做完怎么退回」；用户确认后按顺序做：切四个链接 → 技能有变就重装 → 全局插件有变就重装 → pony 该重启就重启 → 核对 → 记一行部署记录 → 打印退回命令。退回就是对旧版本再跑一次 `ranch deploy`。中途失败不自动撤回，停下来列清楚做到哪，重跑同一条命令就从没做完的地方接着做。不打包、不碰在跑的 agent、不 `corral upgrade`、不切共用 helper（只提醒）。corral、pony 一行代码都不用改。

## 1. 做哪些事、怎么判断要不要做

### 1.1 先检查（不改任何东西，有一项不过就拒绝）

- `<版本>` 必须是 `~/.local/share/ranch/versions/` 下面直接存在的目录名，不含 `/`、不以 `.` 开头（打包时半成品叫 `.ranch-package.<pid>`，不能部署它）。
- 读 `BUILD.txt`：提交、目标平台、工作区是否干净、分支、每个程序的 sha256。`bin/` 里每个程序都要在 `BUILD.txt` 里有一行，实际文件的 sha256 要和它相同；对不上就是版本目录被改过，拒绝。四个程序算一遍约 35 毫秒。
- `bin/` 里只能有 deploy 认识的程序（`corral`、`ranch`、`fence`、`pony`）。有不认识的（以后的新程序），拒绝，提示用这个版本自己的 `bin/ranch` 来部署（见第 5 节）。没有 `corral` 也拒绝。
- 每个程序都能跑：`corral`、`fence`、`pony` 的 `--version` 报 `ok:true`、`contract:"1"`；`ranch` 没有 `--version`（退出 2），用 `--help` 退出 0 代替。每个限 5 秒。这能挡住别的平台编的、坏掉的程序。
- `~/.local/bin/<程序>` 只能是不存在或符号链接；是普通文件、目录就拒绝，不替换别人放的东西。`~/.local/bin` 不存在就建（新机器上第一次装 corral 也能用）。
- 用 root（sudo）跑、在 fence 或 Codex 的墙里跑都拒绝：corral 在墙里本来就不跑，`pony service` 本来就不让 root 跑；用 sudo 时链接和技能还可能落到 root 名下。

### 1.2 四个链接

- 目标版本里有的程序，链接一律切到 `versions/<版本>/bin/<程序>`，**程序没变的也一起切**。DESIGN §3 本来就写「四个一起切」；一起切之后「现装哪一版」只有一个答案，退回也只要一个版本号。链接换到内容相同的程序，对在跑的和新起的进程都没有区别。
- 计划里每个链接标一句：`程序变了`、`程序相同`、`不变`（已经指这里）、`新建`。「变没变」比的是两边文件的 sha256。
- 目标版本没有的程序（R38c 之前没有 pony，R17b 之前没有 fence，df46247 只有 corral），那个链接不动，计划里写明。deploy 从不删链接：pony 服务装着时删掉 `~/.local/bin/pony`，launchd 会一直起不来、反复重试（pony README「部署与退回」第 5 步）。
- 切法：在 `~/.local/bin` 里建一个临时链接，再改名盖过去，每个链接单独是原子的（corral 切 helper 也是这样）。四个链接不是一起原子的，中间有几毫秒新旧混着，可以接受。

### 1.3 技能和全局插件

技能、插件的内容编在程序里，所以一律问**目标版本自己的程序**，不问 PATH 上的：

- 技能：`<目标>/bin/corral install-skills --dry-run`、`<目标>/bin/ranch dispatch install-skills --dry-run`。有 `create` 或 `overwrite` 就要重装，用同一个程序加 `--yes` 跑（只跑有变化的那一个）；全是 `same` 就不重装；`foreign`（技能目录是用户自己放的链接）不动，计划里列出警告。dry-run 的 `warnings`（例如经非交互 ssh 时的「corral is not on PATH」）原样列在计划里提醒，不拦。
- 全局插件：`<目标>/bin/corral integration status`，看带 `install` 的几家（现在是 amp、agy）。`overwrite` 就用 `<目标>/bin/corral integration install --agent <家> --yes` 重装；`create`（这台机器没装过）不装，计划里写「没装，不装」；`foreign` 不动。插件 R8 以后没改过，一般都是 `same`。这条命令要等各家的 `--version`，本机 3.5 秒，一次部署跑一次可以接受。
- 目标版本的程序不支持某项检查（很旧的版本），那一项不做，计划里写明。

### 1.4 pony

- 用 `<目标>/bin/pony status` 读 `service`、`running`、`pid`（它只读配置、`run/status.json` 和锁，不读凭证）。
- 用 pid 查正在跑的 pony 的真实程序：macOS 的 `proc_pidpath`，结果和 `lsof -p <pid> -d txt` 一样（本机核对：链接是 `~/.local/bin/pony`，真实程序是 `versions/da45737/bin/pony`）。拿它的 sha256 和目标的 `bin/pony` 比。
- 用 `<目标>/bin/pony service install --dry-run` 看 plist 定义变没变（`status`）。

| 情况 | 做什么 |
| --- | --- |
| 服务没装（`not_installed`）或不是 macOS（`unsupported`） | 只切链接。第一次装服务是用户在场的另一件事（`pony service install`，以后 R40b 的 `pony setup`），deploy 不装 |
| plist 要换（`replace`） | `<目标>/bin/pony service install --yes`：它先卸下旧定义再加载新的，等于重启，不再另外 kickstart |
| plist 相同，在跑，程序和目标不同 | `launchctl kickstart -k gui/<uid>/local.ranch.pony`（就是今天手敲的那条） |
| plist 相同，在跑，程序相同 | 不重启 |
| 服务装着但没在跑 | 不重启，计划里写原因（`last_exit`）；launchd 下次起它就是新程序 |
| 在跑，但父进程不是 launchd（在终端里手动 `pony run`） | 不重启，提醒用户自己重启 |
| 服务装着，目标版本的 pony 不认 `service`（R38f 之前，本机只有 c3a0b86） | 拒绝：README 第 6 步要求先用现在的版本卸服务再切链接 |

- 重启后每半秒查一次 `pony status`，最多 30 秒，等到 `running:true`、pid 换了、真实程序是目标的 pony 才算成功。等 30 秒是因为：旧的退出才放锁，新的起得太早会拿不到锁退出 1，launchd 10 秒后再起。Slack 连没连上（`chats[].connection`）照实打印，不作为成功条件，网络慢时可能还在 `connecting`。
- 重启不丢东西：线程表、在等的回复、排队的话都在 `~/.pony/state.json` 里，和今天手敲 kickstart 一样。

### 1.5 共用 helper：只提醒

读 `$CORRAL_HOME/.runtime/helper`（默认 `~/.corral/.runtime/helper`）指向的程序，和目标的 corral 比 sha256。不同就在「不做的」里写一行：「共用 helper 指 aef2b4a，和这一版的 corral 不同；deploy 不切，要换见 HANDOFF『部署时要记得』」。这个版本的改动要不要新 helper（R24 要，R25 不要），deploy 判断不了，那要看任务的完成记录；deploy 只报事实。

### 1.6 顺序

先检查 → 打印计划 → 确认 → 拿锁（`~/.local/share/ranch/deploy.lock`，同时只能有一个 deploy 在改）→ 复查链接和计划时一样（等用户确认的这段时间被人改过就停下，请重跑）→ ① 切链接 → ② 技能 → ③ 全局插件 → ④ pony → ⑤ 核对（重新 readlink 四个链接；两份技能 dry-run 全是 `same`；pony 跑的是目标程序）→ 写记录 → 打印结果和退回命令。

先切链接再装技能：技能讲的是 PATH 上那份命令怎么用，两者应当一致；pony 的 plist 启动的是 `~/.local/bin/pony`，重启必须在切链接之后。整个过程中调用的程序一律用目标版本的绝对路径，不经 PATH 找 `corral`、`ranch`、`pony`，这样中途链接换了也不影响。

## 2. 用户怎么确认

- `ranch deploy <版本>`：打印计划；在终端里问「确认做以上 N 步？[y/N]」，回 `y` 才做。
- `--dry-run`：只打印计划，什么都不改，退出 0。主控事先用它把计划贴给用户看（它全程只读：dry-run、`integration status`、`pony status`、readlink）。
- `--yes`：不问，直接做，给不在终端里的场合用（例如经 ssh 在 5090 上）。不在终端、又没带 `--yes`：打印计划，说「没有终端，没法确认，什么都没改；看过计划后加 --yes 再跑」，退出 1。`--dry-run` 和 `--yes` 一起给时按 dry-run。这和两份 `install-skills` 的规矩一样（不在终端又没 `--yes` 时报 `confirmation_required`、什么都不写）。
- 已经是这个版本、没有要做的：打印「已经是 <版本>，没有要做的」和「不做的」几行，退出 0，不记录。
- 输出是给人看的中文文字，打在 stdout 上，不像 ranch 别的命令那样输出一行 JSON：它只给人用，不给前端（见第 7 节）；要机器读的部分在部署记录里。退出码：0 做完、没事可做或 dry-run；1 拒绝、失败、用户回了 N 或需要确认；2 参数错。

今天 R40a2 那次（部署前 corral、ranch、fence 指 2513b3a，pony 指 a6e594e；da45737 的 corral、ranch、fence 和 2513b3a 逐字节相同）如果用这条命令，会是这样（pid 是示意）：

```
$ ~/.local/share/ranch/versions/da45737/bin/ranch deploy da45737
部署 da45737（提交 da457379b9a0，分支 main，aarch64-apple-darwin，打包时工作区有改动）
四个程序和 BUILD.txt 的校验和一致，都能运行。

要做的：
  1. 切链接（~/.local/bin）
       pony    a6e594e → da45737   程序变了
       corral  2513b3a → da45737   程序相同
       ranch   2513b3a → da45737   程序相同
       fence   2513b3a → da45737   程序相同
  2. 重启 pony：launchctl kickstart -k gui/501/local.ranch.pony
       现在跑的是 a6e594e 的 pony（pid 40211），换成 da45737 的

不做的：
  技能          corral 3 个、corral-dispatch 9 个文件都是 same，不重装
  全局插件      amp、agy 都是 same
  pony 服务定义 plist 没变（same），不用 pony service install
  共用 helper   指 aef2b4a，和这一版的 corral 不同；deploy 不切，要换见 HANDOFF「部署时要记得」
  在跑的 agent  不动，不 corral upgrade；新开的 agent 用新版

上次部署：没有记录
做完后退回：~/.local/share/ranch/versions/da45737/bin/ranch deploy a6e594e
确认做以上 2 步？[y/N] y

1. 切链接 …… 完成
2. 重启 pony …… 完成（pid 40377，da45737 的 pony，Slack connected）
核对：四个链接都指 da45737；技能都是 same；pony 跑的是 da45737 的程序。
已记到 ~/.local/share/ranch/deploys.jsonl
退回：~/.local/share/ranch/versions/da45737/bin/ranch deploy a6e594e
```

## 3. 退回

- **退回就是 `ranch deploy <旧版本>`**：同一条命令，同样先列计划再确认。技能会被旧版本的程序写回旧内容，pony 照样按 1.4 判断要不要重启。不另做 undo 命令。
- **上一版从哪来**：每次部署按当时的链接算出「部署前是哪一版」。四个链接都在同一个版本里，就是它。混着的（只有第一次用 deploy、或有人手敲过 `ln` 时会这样），在这些链接指向的几个版本里找一个程序全都逐字节相同的：R40a2 之前是 corral、ranch、fence 指 2513b3a 加 pony 指 a6e594e，而 a6e594e 的 corral、ranch、fence 和 2513b3a 相同，所以部署前就等于 a6e594e，退回命令是 `ranch deploy a6e594e`。找不到这样的版本，就打印逐个链接的 `ln -sfn` 作为退回办法。
- **部署记录**：`~/.local/share/ranch/deploys.jsonl`，挨着 `versions/`（`versions/` 里的目录仍然不可变）。放这里是因为它是 ranch 自己的数据目录；`~/.corral` 归 corral，`~/.ranch` 放用户自己改的 `dispatch.toml`，都不合适。每次真动手的部署记两行，开始前一行、结束后一行；dry-run、回了 N、没事可做都不记。

  ```
  {"at":"2026-10-22T09:50:41+08:00","by":"/Users/u/.local/share/ranch/versions/da45737/bin/ranch","to":"da45737","before":{"corral":"2513b3a","ranch":"2513b3a","fence":"2513b3a","pony":"a6e594e"},"before_version":"a6e594e","result":"started"}
  {"at":"2026-10-22T09:50:49+08:00","to":"da45737","after":{"corral":"da45737","ranch":"da45737","fence":"da45737","pony":"da45737"},"steps":[{"step":"links","result":"done"},{"step":"pony_restart","result":"done","pid":40377}],"result":"done","rollback":"/Users/u/.local/share/ranch/versions/da45737/bin/ranch deploy a6e594e"}
  ```

  `before`、`after` 里链接指在 `versions/` 下就记版本名，指在别处记完整路径，不存在记 `null`。开始前先记一行，是为了中途被 Ctrl-C 或杀掉时，部署前的样子也已经记下；只有开始、没有结束的那次，下次的计划里会写「上次部署没有结束记录（可能被中断）」。
- 每次的计划都打印上一次的记录（「上次部署：10-22 09:50 a6e594e → da45737，做完」），用户和主控不翻 HANDOFF 也知道上一版是哪个。HANDOFF「本机安装」以后可以只写现装哪一版、指到这份记录（由主控定）。
- 退回到还没有 deploy 的旧版本后，PATH 上的 `ranch` 就没有 `deploy` 了。所以打印的退回命令一律用刚才跑 deploy 的那份 ranch 的绝对路径（版本目录不删，这个路径一直有效）；退回到这种版本时，结果里再提醒一句「PATH 上的 ranch 现在没有 deploy，再部署用 <这个路径>」。
- 旧版本目录一个都不删（在跑的 pen、helper、agy 可能还引用着）。

## 4. 中途失败

- **不自动撤回**。停在失败的那一步，后面的不做；打印每一步的结果（完成 / 失败和原因 / 没做）、四个链接现在指哪、pony 在不在跑，再给两条路：找到原因后重跑同一条 `ranch deploy <版本>`，或者用打印的退回命令退回。记录的结束行写 `result:"failed"` 和每步结果，退出 1。
- 为什么不自动撤回：
  1. 撤回本身也是一次部署，碰到的是同一块磁盘、同一个 launchctl，很可能一起失败；无人看着地再失败一次，最后的状态比第一次失败更难说清。
  2. 部署本来就是用户在场时跑的，失败了用户就在终端前，接着做还是退回，一条命令的事。
  3. 最可能失败的是重启 pony。launchd 每 10 秒会自己再起一次，过一会儿可能自己就好了；自动撤回会和它抢。
  4. 每一步做之前都先判断「还要不要做」，重跑同一条命令时，做过的会显示「不变」「same」而跳过，所以重跑就是从失败处接着做。
- pony 重启失败时它可能正停着，Slack 暂时不通。结果里把这句放在最前面，并给出日志位置（`~/.pony/logs/launchd.log`、`pony.log`）。

```
2. 重启 pony …… 失败：30 秒内没等到新的 pony 起来（pony status：running false，last_exit 1）
pony 现在没在跑，launchd 每 10 秒会再试；看 ~/.pony/logs/launchd.log、~/.pony/logs/pony.log
  完成  1. 切链接（四个都指 da45737）
  失败  2. 重启 pony
  没做  核对
接着做：找到原因后再跑一次 ranch deploy da45737，做过的会跳过
退回：~/.local/share/ranch/versions/da45737/bin/ranch deploy a6e594e
已记到 ~/.local/share/ranch/deploys.jsonl
```

## 5. 谁来跑

- 任何一份带 `deploy` 的 ranch 都能部署任何版本：怎么部署写在正在跑的这份 ranch 里，和版本有关的东西（程序本身、技能内容、插件内容、plist 内容）都问目标版本自己的程序。所以平时用 PATH 上的 `ranch deploy <版本>` 就行。
- 部署新版本时，用新版本自己的 `bin/ranch` 更稳：deploy 自己修了 bug、多了步骤，新版本的才知道。主控把新版本交给用户时，写 `~/.local/share/ranch/versions/<新>/bin/ranch deploy <新>`；deploy 打印的退回命令用的是刚跑完的那份（比旧版本新）。这样不管往前还是往回，用的都是两份里新的那份，用户照抄就行。
- 第一次：现在 PATH 上的 ranch（2513b3a）还没有 `deploy`，只能用带 deploy 的那个版本的绝对路径跑。
- 正在跑的 ranch 不认识目标版本里的程序时，按 1.1 拒绝，并提示用目标版本自己的 `bin/ranch`。
- **换掉自己的链接没有问题**：程序起来时已经从版本目录里的那个文件加载了，把新链接改名盖到 `~/.local/bin/ranch` 上，不影响正在跑的这份；版本目录也从不覆盖、不删。实现上要守两条：开始时就把自己的真实路径解析出来（macOS 报的程序路径可能就是链接本身，切完再解析会变成新版本），记录和退回命令都用它；整个过程只按绝对路径调目标版本的程序（1.6）。
- **谁按回车**：用户在普通终端里跑（它会写用户目录、会重启 pony），主控只跑 `--dry-run`，事后用部署记录、readlink、`pony status` 核对。和现在「部署由用户在普通终端里跑，主控事后核对」一样。
- **5090（Linux）**：链接、技能、全局插件照常；`pony service` 报 `unsupported`，pony 的几步跳过。那边由用户或那台机器上的 pi、omp 跑；经非交互 ssh 时 PATH 里要有 `~/.local/bin`，否则 corral 技能的 dry-run 会提醒「corral is not on PATH」，deploy 把这条列在计划里。

## 6. 不做的事

- **不打包**：打包要源码仓库、cargo、node 和 npm，要几分钟；deploy 只用已经打好的不可变版本目录。保持 DESIGN §3「打包只生成目录、部署另做」的分工，deploy 也不依赖本机有没有源码。
- **不碰在跑的 agent，不 `corral upgrade`**：AGENTS.md 不让对用户的 agent `upgrade`，用户 10-22 同意的做法也是不碰正在跑的 agent；DESIGN §3 记着用户 10-05 定第 1 步切换时也不做 upgrade。upgrade 会换掉用户每个 agent 的 pen。链接切完，新开的 agent 自然用新版。
- **不切共用 helper，只提醒**：所有 agent 的钩子都调这个 helper，切它等于一下子换掉所有在跑 agent 的钩子处理，也就是部分地升级了用户的 agent。现在的办法（开一个 `/bin/cat` 测试 agent 单独 `corral upgrade`）是另一件要人决定的事；而且 deploy 看不出这一版到底要不要新 helper。
- 也不做：删旧版本目录；第一次装 pony 服务、写 `~/.pony`；装这台机器没装过的全局插件；改 `~/.fence/local.toml`；动远端机器。

## 7. 对 paddock 的影响

- 只是多一条给人用的命令，paddock 不调它、不用显示它，不用动。
- 不写进 `docs/CONTRACT.md`：CONTRACT 记的是给前端用的命令和 JSON；deploy 输出的是给人看的文字，格式随时可以改。
- 部署之后 PATH 上的 `corral`、`pony` 换成新版，这和今天手敲 `ln` 的效果一样，对 paddock 没有新的影响。
- 实现时要改的文档：DESIGN §3 加一段「部署用 `ranch deploy`」；`crates/pony/README.md`「部署与退回」第 3、5、6 步指向它。HANDOFF「部署时要记得」可以缩短，由主控改。

## 8. 怎么拆实现

| 件 | 内容 | 大小 | 档位和审查 |
| --- | --- | --- | --- |
| R41b `ranch deploy` 主体 | `crates/ranch` 加 `deploy` 子命令：1.1 检查、1.2 链接、1.3 技能和全局插件、1.5 helper 提醒、第 2 节的计划和确认、锁和复查、第 3 节的记录和退回（含混着的链接怎么算上一版）、第 4 节的失败汇报。pony 这一步先只**打印**「要重启 pony：launchctl kickstart …」，不自己做。改 DESIGN §3 和 pony README | 代码约 500 行，测试约 400 行 | 碰要害（写 `~/.local/bin` 和技能目录，R39 草稿也写了按碰要害做），交叉审查 |
| R41c pony 那几步 | 1.4 整节：`pony status` → pid → 真实程序、plist dry-run、`service install --yes` 或 kickstart、等 30 秒核对。macOS 才做，Linux 跳过 | 代码约 200 行，测试约 200 行 | 碰要害（重启用户在用的服务），交叉审查 |

- 拆两件的理由：R41b 做完就能替掉手敲 `ln` 和重装技能，pony 那步暂时照提醒手敲；R41c 有自己的失败方式（launchctl、等待、锁），分开审各自都小。R41c 接在 R41b 后面做，不能并行。也可以一件做完，约 700 行代码，少一轮审查，但一次要审的多。
- 不用改 corral、pony：用到的都是现有命令（`install-skills --dry-run/--yes`、`integration status/install`、`pony status`、`pony service install --dry-run/--yes`）。ranch 不在 Cargo 里依赖 `corral-core`、`pony`，只调目标版本的程序。新依赖不用加：`sha2`、`libc` 工作区里已经在用。
- 测试：隔离的 HOME、`CORRAL_HOME`、`PONY_HOME`；临时的 `versions/` 里放工作区编出的 corral、ranch（技能真装进隔离 HOME）和合成程序，配一份对得上的 `BUILD.txt`；也要测 sha 对不上、链接不是链接、混着的链接算上一版、中途失败后重跑接着做、不在终端没 `--yes`。launchctl 和查 pid 的程序照 `pony service` 的做法做成可替换的函数，测试里从不调真的 launchctl。
- 两件都合并、打包之后，第一次真用就是验收：主控先跑 `~/.local/share/ranch/versions/<新>/bin/ranch deploy <新> --dry-run` 给用户看，用户在终端里跑、确认，主控事后核对记录、链接和 `pony status`。

## 9. 核对过的命令和输出（10-22，本机，全是只读）

- `readlink ~/.local/bin/{corral,ranch,fence,pony}`：前三个指 `versions/2513b3a/bin/`，pony 指 `versions/da45737/bin/pony`。
- `cat versions/{2513b3a,a6e594e,da45737}/BUILD.txt`：三份里 corral、ranch、fence 的 sha256 相同，pony 各不相同；格式是 `revision`、`target`、`working-tree`、`source`、`branch`、`srt`、`node` 加每个程序一行 sha256。看了全部 37 个版本目录的 `bin/`：df46247 只有 corral，R17b 之前没有 fence，R38c 之前没有 pony；旧的 `BUILD.txt` 没有 `srt`、`node` 两行，sha 行照样有。
- `readlink ~/.corral/.runtime/helper`：指 `versions/aef2b4a/bin/corral`，sha256 和 2513b3a 的 corral 不同。
- `corral install-skills --dry-run`：3 项都是 `same`，`written:false`；`ranch dispatch install-skills --dry-run`：9 项都是 `same`。
- `corral integration status`：amp、agy 的 `install.status` 都是 `same`，跑了 3.5 秒。`corral integration install --agent amp|agy --dry-run` 也是 `same`、很快；`--agent claude` 报 `usage`（它不装全局插件）。
- `pony status`：`service: installed`、`running: true`、有 `pid`、Slack `connected`。`pony service install --dry-run`：`status: same`、`applied: false`，plist 的 `ProgramArguments` 是 `~/.local/bin/pony run`。
- `ps -o pid=,ppid=,lstart=,command= -p <pid>`：父进程是 1，命令行是 `~/.local/bin/pony run`；`lsof -p <pid> -d txt`：真实程序是 `versions/da45737/bin/pony`。
- `corral --version`、`pony --version`、`fence --version`：都是 `ok:true`、`contract:"1"`；`ranch --help` 退出 0，`ranch --version` 退出 2。
- `versions/c3a0b86/bin/pony service install --dry-run`：报 `usage`、退出 2（那时的 pony 还没有 `service`）。
- 没跑：任何 `launchctl`（包括只读的 `print`）、不带 `--dry-run` 的安装、切链接、`corral upgrade`；没读 `~/.pony/secrets`。

## 要用户定的（按重要程度）

1. **中途失败要不要自动撤回**
   - A（推荐）：不自动撤回。停在失败那一步，列清楚做到哪、现在什么样，给「重跑同一条命令接着做」和「退回命令」两条路（第 4 节）。
   - B：自动把已经做过的几步倒回去。倒回也可能失败，最后的状态更难说清；还会和 launchd 自己重起 pony 抢。
2. **退回是不是同一条命令指向旧版本**
   - A（推荐）：是，`ranch deploy <旧版本>`，计划和结果里直接打印这条，照抄即可。
   - B：另加 `ranch deploy --undo`，按记录把链接原样恢复成上一次部署前的样子。多一种用法，连着 undo 两次是什么意思也说不清。
3. **要不要 `--dry-run`**
   - A（推荐）：要。只打印计划、什么都不改、退出 0；主控事先用它把计划给用户看。在终端里直接跑仍会问 y/N，不在终端要加 `--yes`。
   - B：不要，只靠在终端里回 N。主控和脚本就没法只看计划。
4. **程序没变的链接切不切**
   - A（推荐）：四个一起切（DESIGN §3 本来就这么写）。现装只有一个版本，退回也只要一个版本号。
   - B：只切程序变了的，和今天手敲一样；现装会是几个版本混着，记录和退回都要逐个链接算。
5. **部署记录**
   - A（推荐）：记在 `~/.local/share/ranch/deploys.jsonl`，每次两行，计划里显示上一次。
   - B：不记文件，只在结果里打印退回办法，靠主控抄进 HANDOFF（今天的做法）。
6. **全局插件（amp、agy）变了怎么办**
   - A（推荐）：和技能一样顺手重装，只重装这台机器已经装着的。
   - B：只提醒，不装。插件 R8 以后没改过，选哪个现在都没区别。
7. **怎么拆**
   - A（推荐）：两件，R41b 主体、R41c pony 那几步，都按碰要害做、交叉审查（第 8 节）。
   - B：一件做完，同样交叉审查。
