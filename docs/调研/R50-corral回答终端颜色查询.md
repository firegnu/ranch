# R50 调研：corral 回答终端的前景色、背景色查询

2026-10-24，ranch/dev-r50a。任务书 `docs/任务/R50a-终端颜色查询调研与方案.md`，草稿 `docs/任务/R50-corral回答终端背景色查询.md`。本轮只调研、出方案，不改代码。

标记：【实测】本机录到的；【源码】对照开源代码；【推测】没直接证据的判断；【未知】没查到，写了原因。

用户 10-24 在调研中途说：「这个问题主要是codex」。所以 Codex 实测加源码查透；别的几家只做启动录制，不翻闭源程序。

## 0. 结论

- **Codex 就是浅色主题下显示成深色的那一家。** 【实测＋源码】Codex 0.164.0 启动时一次发出 OSC 10、OSC 11（问前景色、背景色），同时发 CPR、kitty 键盘查询、DA1，共用一个 250 毫秒期限。没人回答就把颜色记成「没有」，从此不再问；代码高亮用 Catppuccin Mocha（深色），输入框也不铺底色。corral 现在没有客户端连着时什么都不答，所以后台开的 Codex 一律按深色。
- **只回答 OSC 10、11 就够。** 【实测】只答这两个（DA1、CPR 都不答）时，Codex、omp、Kimi 用的颜色和真终端全答浅色时一样；pi 换成偏浅的一套（它还想要 16 色）。
- **回答来晚了或者来两份都不出事。** 【实测】Codex 的回答晚到（查询后 600 毫秒）会被忽略、仍按深色，但输入框里不会多出字；同一回答来两次，用第一次的。
- **推荐方案：** pen 读到 agent 输出里的 OSC 10/11 查询时，如果这一刻没有客户端连着，就用本机设置的颜色立即回答；有客户端连着就照旧转给客户端、自己不答。颜色只有一个来源：新命令 `corral terminal-colors --fg '#rrggbb' --bg '#rrggbb'` 写的本机设置，`corral start` 启动时读一次。没设置就不答，和现在一样。paddock 启动时、换主题时调这个命令；主控派活开的 agent 走同一个设置，自动跟上。全部是兼容新增。

## 1. 怎么测的

- 录制程序：临时目录里写的 Rust 小程序（不进仓库）。和 pen 一样开 PTY（40×120）、`setsid` 加控制终端；环境照 corral 的做法重建：空环境起 `zsh -l -i` 抓登录环境，去掉 corral 会去掉的那些变量，设 `TERM=xterm-256color`、`COLORTERM=truecolor`。工作目录是 worktree 外的临时目录。逐字节记下两个方向的数据和毫秒时间。
- 回答方式，每家每种各起一次：
  - **不答**：什么都不回。和现在没有客户端连着的 pen 一样：pen 把 PTY 输出只转给 attach 着的客户端（`pen.rs` 读 master 的分支），往 agent 写的只有客户端输入、`send`、`keys` 和首句，自己不回任何查询。
  - **全答**：用 `alacritty_terminal` 0.26 的默认设置解析输出、照它的回答回；颜色查询按主题回。paddock 用的是同一个库、同样的默认设置（`../paddock/app/src/terminal.rs`），所以相当于 paddock 一开始就连着。
  - **只答颜色**：只回 OSC 4/10/11/12；**只答 OSC 10/11**；**只答 DA1**；**晚答**（只答颜色，晚 600 毫秒）；**重复答**（马上答一次，600 毫秒后再答一次）。
  - 主题：浅色用 Catppuccin Latte（背景 `#eff1f5`、前景 `#4c4f69`），深色用 Catppuccin Mocha（背景 `#1e1e2e`、前景 `#cdd6f4`）。
- 运行中：第 8 秒改一次窗口大小；第 10 秒、10.5 秒送焦点进、出（只有 agent 自己开了焦点报告 `?1004h` 才送）；第 11 秒送「终端换成浅色」通知 `CSI ?997;2n`（只有 agent 开了 `?2031h` 才送）。第 10–15 秒对进程组发 TERM，2 秒不退再发 KILL，都按记下的 PID；事后查会话里没有残留进程。
- 判断用的是哪套颜色：比较输出里的 24 位颜色（`38;2;r;g;b`、`48;2;r;g;b`）。
- 只启动、不发话；没有用模型额度。Kimi 加了只对这次生效的 `KIMI_CODE_TRUST_WORKSPACE=1`；agy 停在信任框，没点；Claude Code、Codex 没弹信任框。Codex 用 PATH 上的 `codex`（IP Guardian 包装脚本，先查 IP 约 1.3–1.7 秒，再起真正的 Codex），所以它的查询要到启动后 1.3–2.0 秒才出现。
- 版本：Claude Code 2.1.310、Codex 0.164.0、pi 1.4.0（`@earendil-works/pi-coding-agent`）、omp 18.12.6、amp 0.0.1792756861-g6187a2、agy 1.4.7、Kimi Code 2.3.1。
- 没有经 `corral start` 真开测试 agent：「不答」录制和 pen 现在的行为一致（上面说的代码依据），paddock 已经报过真实现象。

下文原始片段里 `\e` 是 ESC，`\a` 是 BEL，`\e\\` 是 ST；行首数字是启动后的毫秒数。

## 2. 各家启动时问终端什么

### 2.1 一览

| 家 | 颜色查询 | 别的查询 | 不答时等多久 | 问不到时 | 之后还问吗 |
| --- | --- | --- | --- | --- | --- |
| Codex | OSC 10、11（成对才用） | CPR、kitty 键盘、DA1（经 ssh 时加 DA2） | 250 毫秒（共用期限） | 按深色 | 不问（有缓存） |
| Claude Code | 用户现在的设置下不问 | DA1、kitty 键盘、XTVERSION、OSC 7501 | 看不出在等 | 不涉及 | 没看到 |
| pi | OSC 10、11，OSC 4 的 0–15 号 | DA1、kitty 键盘、OSC 7501 | 不等 | 不用浅色也不用深色那套灰，改用终端 16 色 | 收到深浅变化通知会再问 |
| omp | OSC 11 | DA1（多次）、DECRQM、kitty 键盘、XTSMGRAPHICS、APC | 不等 | 按深色 | 答过才会在通知后再问 |
| amp | OSC 10、11、12，OSC 4 的 0–7 号 | DA1、CPR、DECRQM、kitty 键盘、XTVERSION、XTGETTCAP、kitty 图形、OSC 7501 | 约 2 秒（等的是 DA1） | 未能确定 | 没看到 |
| agy | OSC 11 | DA1、XTVERSION、DECRQM、kitty 键盘、OSC 7501 | 不等 | 未知（停在信任框） | 没看到 |
| Kimi | OSC 11（两次） | DA1、kitty 键盘、`CSI ?996n` | 第一次约 250 毫秒 | 按深色 | 没看到 |

「不等」指查询和画出界面在同一时刻（相差几毫秒以内）。

### 2.2 Codex（重点）

**问什么、什么时候问。** 【实测】进界面前一次写出：

```text
1746 OUT \e[>4;0m\e[>7u\e[?1004h\e[6n\e]10;?\e\\\e]11;?\e\\\e[?u\e[c
```

即 CPR（`\e[6n`）、OSC 10、OSC 11、kitty 键盘查询（`\e[?u`）、DA1（`\e[c`）。【源码】`codex-rs/tui/src/terminal_probe.rs` 的 `startup()` 组出同一串；DA1 只在要查键盘或经 ssh 时加，经 ssh 时还加 DA2（`\e[>c`）。

**等多久。** 【源码】`DEFAULT_TIMEOUT` 是 250 毫秒，这几个查询共用。CPR、颜色、键盘（看 DA1 的回答）都有了才提前结束（`startup_probe_complete`），否则等满。【实测】

| 回答方式 | 查询 | 进全屏（`\e[?1049h`） | 等了 |
| --- | --- | --- | --- |
| 不答 | 1746 | 1996 | 250 毫秒 |
| 只答颜色 | 1656 | 1907 | 251 毫秒 |
| 全答 | 2015 | 2015 | 0 |

所以只答颜色不会让 Codex 变快，和现在一样多等 250 毫秒。

**问不到时。** 【源码】`terminal_palette.rs`：启动结果写进缓存，就算是「没有」也记成已问过（`set_default_colors_from_startup_probe` 把 `attempted` 设成真），之后不再问。`render/highlight.rs` 的 `adaptive_default_theme_selection`：背景色已知且是浅色才用 `catppuccin-latte`，否则（包括不知道）用 `catppuccin-mocha`。`style.rs`：不知道背景色时，提交过的消息不铺底色。用户的 `~/.codex/config.toml` 没设 TUI 主题，走的就是这条自适应路线。

【实测】输出里数这几种颜色出现的次数：

| 颜色 | 不答 | 全答深色 | 全答浅色 | 只答颜色（浅） | 晚答（浅） | 重复答（浅） |
| --- | --- | --- | --- | --- | --- | --- |
| 前景 `171;223;167`（深色那套里的绿） | 2 | 3 | 0 | 0 | 3 | 0 |
| 底色 `57;57;71`（深色输入框） | 0 | 157 | 0 | 0 | 0 | 0 |
| 底色 `229;231;235`（浅色输入框） | 0 | 0 | 8 | 305 | 0 | 385 |

不答、晚答都用深色那套颜色（输入框不铺底色）；只答颜色、重复答和全答浅色一样。

**之后还问吗。** 【实测】第 8 秒改大小后只再发了 CPR（`8011 OUT \e[6n`），焦点进出都没再问颜色。【源码】颜色有缓存，不再问。所以已经在跑的 Codex，换主题后不会跟着变，只能重开。

**晚答、重复答。** 【实测】

```text
晚答：1438 OUT …\e[6n\e]10;?\e\\\e]11;?\e\\\e[?u\e[c
      1688 OUT \e[?2026h\e[?25l\e[?1049h          ← 等满 250 毫秒，进界面
      2046 IN  \e]10;rgb:4c4c/4f4f/6969\e\\        ← 晚到的回答
      2046 IN  \e]11;rgb:efef/f1f1/f5f5\e\\
重复答：1331 IN \e]10;… \e]11;…（马上）  1939 IN 同样两条（再来一次）
```

两次结束时输入框都是 `› Ask Codex to do anything`，没有多出字。【源码】`terminal_probe/startup_replay.rs`：探测期间收到的完整 OSC 回答不交给输入解析。期限过后才到的回答怎么处理，源码没细查，只有上面的实测。

**DA1 不能随便答。** 【源码】`terminal_probe.rs`：DA1 的回答里没带 kitty 键盘标志，就认定终端不支持增强键盘（`UnsupportedFallback`）。corral 如果替客户端答 DA1，之后 attach 上来的终端就算支持也用不上。

### 2.3 Claude Code

【实测】用户现在的设置（`~/.claude/settings.json` 里 `"theme": "light"`；`~/.claude.json` 里 `"theme": "custom:anthropic-warm"`，不是 auto）下，启动不问 OSC 10/11：

```text
209 OUT \e[>0q\e[?u
209 OUT \e]7501;?\a\e[c
1781 OUT \e[>0q\e[?u
1781 OUT \e]7501;?\a\e[c
```

即 XTVERSION、kitty 键盘查询、OSC 7501、DA1，进界面前、后各一次；开了 `?2031h`（终端深浅变化通知）和 `?1004h`（焦点报告）。所以 paddock 说「Claude Code 设成 auto」和本机现在的设置对不上。

【未知】主题设成 auto 时问不问 OSC 10/11：本机设置不是 auto；用户说这个问题主要是 Codex，没再往下查。它订阅了 `?2031` 通知，auto 也可能走 `CSI ?996n`／`?2031` 这条路而不是 OSC 11；如果是这样，只答 OSC 10/11 帮不了它。

### 2.4 pi

【实测】

```text
570 OUT \e[>7u\e[?u\e]7501;?\e\\\e[c
570 OUT \e]10;?\a\e]11;?\a<OSC 4 的 0–15 号各一条>\e[c
```

查询发出后几毫秒就画界面，不等回答。收到深浅变化通知会全部再问一次：

```text
11008 IN  \e[?997;2n
11009 OUT \e]10;?\a\e]11;?\a<OSC 4 的 0–15 号>\e[c
```

焦点进出、改大小都不问。不答时按背景算出来的那几种灰色都不出现，改用终端 16 色里的 1、3 号（几种和主题无关的强调色三次都一样）；全答浅色用 `136;136;136`、`103;103;103`，全答深色用 `130;130;130`、`160;160;160`；只答 OSC 10/11 用 `128;138;143`、`96;105;109`（偏浅色那边，但不完全一样，因为没拿到 16 色）。

### 2.5 omp

【实测】

```text
110 OUT \e[?u\e[c
111 OUT \e]11;?\a\e[c\e_25a1;s\e\\\e[c\e_tsp;q;{…}\e\\\e[c\e[?2031h\e[?2026$p\e[c\e[?2048$p\e[c\e[?2031$p\e[c…\e[?2;1;0S
```

只问 OSC 11，不问 OSC 10。每个查询后面都跟一个 DA1（【推测】用 DA1 的回答判断前面那个有没有回答），另有 DECRQM、XTSMGRAPHICS 和两条 APC。在查询之前（53 毫秒）就画出界面了，不等。不答时的颜色（前景 `107;114;128`、`42;48;56`，底色 `15;18;22`）和全答深色完全一样，就是按深色；只答颜色和全答浅色一样（前景 `118;118;118`，底色 `224;224;224`）。收到深浅变化通知会再问 OSC 11，但只在启动时它的查询有回答时才这样（不答那次没再问）。

### 2.6 amp

【实测】

```text
111 OUT \e[?1049h\e[H\e]66;w=1; \e\\\e[6n\e[?1049l\e]10;?\a\e]11;?\a
111 OUT \e]12;?\a<OSC 4 的 0–7 号>\e[?2026$p\e[?2027$p\e[?1016$p\e[?2031$p\e[?u\e[>0q\eP+q4d73\e\\\e_Gi=1,a=q\e\\\e_25a1;s\e\\\e]7501;?\e\\\e[c…
```

问得最多。**不答时要等约 2 秒才出画面，等的是 DA1：**

| 回答方式 | 出画面 |
| --- | --- |
| 不答 | 2126 毫秒 |
| 只答 OSC 10/11 | 2101 毫秒 |
| 只答 DA1 | 109 毫秒 |
| 全答 | 109 毫秒 |

这是现在后台开 amp 就有的现象：启动慢 2 秒，颜色问题另说。【未能确定】不答时用哪套颜色：界面文字用终端默认色，深浅两次只有欢迎画面的动画球颜色不同，动画每次不一样，比不出来。

### 2.7 agy

【实测】

```text
1648 OUT \e[>q\e[c
1667 OUT \e[?2026$p\e[?2027$p\e]11;?\a\e]7501;?\a
1668 OUT \e[>4m\e[?1049h…（画信任框）
```

问 OSC 11，不等回答。【未知】用不用回答：启动后停在「Do you trust the contents of this project?」，按规定没点；这一屏在不答、全答浅色、全答深色三次里颜色完全一样。

### 2.8 Kimi Code

【实测】

```text
263 OUT \e]11;?\a                         ← 画界面前先问一次
984 OUT \e[?2031h\e]11;?\a                ← 画界面时再问
984 OUT \e[?996n
```

第二批另有 DA1、kitty 键盘查询、`CSI ?996n`（另一种问「终端是深色还是浅色」的方式）。第一次问 OSC 11 没人答时，多等约 250 毫秒才往下走（第一、二次查询相隔：不答 721 毫秒（263→984），全答浅色 480 毫秒（267→747），全答深色 457 毫秒（264→721））。不答时的颜色（前景 `79;168;255`、`136;136;136`）和全答深色一样，按深色；只答颜色就和全答浅色一样（`21;101;192`、`69;69;69`）。录制用的终端库不答 `CSI ?996n`，光答 OSC 11 已经够用。焦点进出、深浅变化通知之后都没再问。

### 2.9 别的查询

- 几家都发、但 corral 不该替客户端答的：DA1、DA2、XTVERSION、CPR、DECRQM、kitty 键盘查询、XTGETTCAP、kitty 图形查询、XTSMGRAPHICS，还有 OSC 7501（Claude Code、pi、amp、agy 都发，不知道是什么，paddock 用的终端库也不答）、omp 和 amp 的 APC。这些的正确回答取决于最后连上来的是什么终端；答错会让 agent 关掉功能（Codex 收到 DA1 就认定没有增强键盘）或把光标位置算错。
- 不答造成的已知影响：amp 启动多等约 2 秒（等 DA1），Codex 多等 250 毫秒（等 CPR 和 DA1）。都是现在就有的，只是慢，不会画错。本方案不处理。

## 3. 推荐方案

### 3.1 什么时候由 corral 回答

**推荐：没有客户端连着时 pen 回答，有客户端连着时只转发。** 具体：pen 从 PTY 读到 agent 的输出、解析出 OSC 10 或 OSC 11 查询的那一刻，如果没有写者（`writer` 为空；第一个 attach 的客户端就是写者，所以写者为空就是没有任何客户端连着），并且这个 agent 有颜色，就立刻回答；有写者时照旧把输出转过去，自己不答。

- 为什么不一律由 corral 回答：连着的客户端知道这一刻真实的颜色（用户可能刚换了主题）；corral 也答的话会和客户端的回答重复。Codex 实测不怕重复，别家没测全。要不重复就得把查询从转给客户端的输出里删掉，那会改动 `corral read`、输出环、读屏看到的字节，牵连太大。
- 两边的回答会不会撞上：不会。是否回答在 pen 读到查询那一刻判断一次，要么只有客户端答，要么只有 pen 答。查询之后才 attach 的客户端收不到这条查询（attach 只重放终端模式，不重放之前的输出），不会补答。paddock New Agent 的「先 start、后 attach」：Codex 问的时候 paddock 连上了，就由 paddock 答；还没连上，就由 pen 用设置里的颜色答。两边颜色一样（paddock 换主题时已经更新了设置，见 3.2）。
- 有客户端连着但它不回答：第一步不处理。paddock、常见的终端都会答；不答的终端，agent 直接在它里面跑也一样问不到。要处理就得在 pen 里记下没答的查询，从写者的输入里认出回答，过一段时间没等到就自己补答。这个时间不好定（Codex 只等 250 毫秒），客户端慢一点又会两边都答。真碰到了再加。
- 只答 OSC 10、OSC 11 的查询形式（`ESC ] 10 ; ?`、`ESC ] 11 ; ?`），用 BEL 结尾就用 BEL 回，用 ST 结尾就用 ST 回。回答格式和 paddock 一样：`ESC ] 11 ; rgb:efef/f1f1/f5f5`，每个 8 位分量重复写成 16 位。设置颜色的 OSC 10/11、OSC 4、OSC 12、DA1、CPR、`CSI ?996n` 都不答（理由见 2.9）。xterm 允许一条 OSC 连着问几个（`OSC 10;?;?`），录到的几家都没这么用，不答。
- 回答怎么写进 PTY：放进单独的小队列，PTY 可写时先于 `input` 写；但不插进正在写的那一块中间（`send` 的一块可能只写了一半）。不受 `send` 块之间的延时影响，不算人在打字，不记进 `recent`。原因：Codex 只等 250 毫秒，排在带延时的 `send` 后面就可能超时。
- 暂停中的 agent 不会输出，不会有查询，不用另外处理。

### 3.2 颜色从哪来

**推荐只有一个来源：本机 corral 的一项设置。**

- 新命令 `corral terminal-colors` 读写，存在 `$CORRAL_HOME/.runtime/terminal-colors.json`（默认 `~/.corral/.runtime/`，和 `helper` 放在一起；`ls` 只认带 `lock` 文件的目录（`state.rs` 的 `list`），不会动它）。
- `corral start` 起 pen 前读一次，交给 pen；pen 一直用这组颜色，直到 agent 退出或 `corral upgrade` 交接下去。用这次读到的值，之后改设置不影响已经在跑的。
- 没有设置：不答，和现在一样。不猜默认颜色：猜错了（比如给深色终端答浅色）比让 agent 自己按深色更糟。
- 设置文件读不了或格式坏：当没设置，在 `start` 输出原有的 `warnings` 里说一句，不让启动失败。
- pen 在 `pen.log` 记一行用的是哪组颜色，方便查；不加进 `status`、`ls`、`start` 的 JSON。

不推荐（这一步）：

- `corral start` 加参数：主控在自己的 agent 里跑 `corral start` 派活，拿不到 paddock 的主题，派出去的 agent 还是不答；paddock 给旧版 corral 多传一个参数，`start` 会报 `usage` 失败。只调一个新命令，旧版报错就忽略，开 agent 不受影响。
- 环境变量：agent 的环境是 corral 重建的（`CORRAL_*` 会被去掉），派活时没有地方可靠地放；其余同上。

代价：一台机器同一时间只有一组颜色，不同前端开的 agent 不能用不同的颜色。用户现在只用 paddock，够用。以后真要分开，再加 `start` 参数覆盖本机设置（兼容新增）。

**换了主题之后。** paddock 在启动时、换主题时（包括跟着系统切深浅时）各调一次 `corral terminal-colors`。之后新开的 agent，不管是 paddock 开的、主控派活开的还是在终端里手动开的，都用新颜色。已经在跑的不变：Codex 只在启动时问一次，要换只能重开；corral 不替它重开。

**远端机器。** 设置是每台机器各一份。paddock 在远端开 agent 前，对那台机器的 corral 也调一次。那台机器上没调过就不答，和现在一样。

### 3.3 `corral upgrade` 和新旧版本同时在用

- 快照：pen 新增的两个字段（颜色、待写的回答）和 `Terminal` 里记未解析完查询的状态都加 `#[serde(default)]`，快照格式号 `SCHEMA` 不变（`pen/upgrade.rs`）。
  - 旧 pen 升到新版：快照里没有颜色，新 pen 不答，和以前一样。这些 agent 早就问过了，答也没用。
  - 新版升到新版：颜色随快照交过去。
  - 新版升回旧版（`upgrade --exe` 指旧程序）：旧版反序列化时忽略不认识的字段（`Pen` 没有 `deny_unknown_fields`），之后不再回答；agent 启动时的查询早已过去，基本没影响。不为这个改 `SCHEMA`，改了会挡住新旧版之间所有的升级。
- 命令行和 pen 混用：pen 没有新请求，socket、`status`、`attach` 的格式都不变。旧 CLI 开的 agent 不读设置，不答（旧行为）；新 CLI 管旧 pen 不受影响。
- 设置文件：只有新版读写。旧版 corral 不认识 `terminal-colors` 命令，返回 `{"error":"usage","message":"unknown or missing command","ok":false}`、退出码 1（本机现装的 8a6b239 实测）；旧版 `ls` 不碰 `.runtime` 下的文件。两个方向都兼容。

## 4. 给 paddock 的公开约定草案（兼容新增）

下面是写进 `docs/CONTRACT.md` 的草稿。

### `corral terminal-colors [--fg COLOR --bg COLOR | --clear]`（R50）

设定本机新开的 agent 启动时，在没有客户端连着的情况下，corral 拿什么颜色回答 agent 对终端前景色、背景色的查询（OSC 10、OSC 11）。

- `corral terminal-colors --fg '#4c4f69' --bg '#eff1f5'`：设定。`COLOR` 是 `#rrggbb`，大小写都行，输出一律小写。`--fg`、`--bg` 要一起给。
- `corral terminal-colors --clear`：清除。
- `corral terminal-colors`：只查。

成功都输出一行 JSON、退出码 0：

```json
{"ok":true,"terminal_colors":{"fg":"#4c4f69","bg":"#eff1f5"}}
```

没有设定时 `terminal_colors` 为 `null`。颜色格式不对、只给了一个、`--clear` 和颜色同时给，报 `usage`、退出码 1，原设置不变。写入是原子的。

效果：从下一次 `corral start` 起生效，不管是谁调的（paddock、派活的 agent、终端里的用户），已经在跑的 agent 不变。agent 启动后，只要没有客户端 attach 着，它问 OSC 10/11，corral 就用这组颜色回答；有客户端 attach 着就照旧由客户端回答。别的终端查询一概不答。没有设定时行为和以前一样。

旧版 corral 没有这个命令，返回 `usage`、退出码 1；前端当作不支持，照常开 agent。

### paddock 要做的

1. paddock 启动时、终端主题变化时（包括跟着系统切深浅时），调用 `corral terminal-colors --fg <主题前景色> --bg <主题背景色>`。颜色取 paddock 回答 OSC 10/11 时用的那两个值（主题的 `foreground`、`background`），格式 `#rrggbb`。
2. 在远端机器上开 agent 之前，对那台机器的 corral 也调一次（可以每次开 agent 前都调，代价是多一次 ssh 命令）。
3. 调用失败（旧版 corral）就忽略，不挡开 agent。
4. `corral start` 的参数、attach、各命令的 JSON 都不变。
5. 可选：Settings 里说明「已经在跑的 Codex 不会跟着换主题，要重开」。

主控派活（不经 paddock）开的 agent：不用改派活技能，它们在 `corral start` 时读同一个本机设置。只要 paddock 在这台机器上调过一次，就跟 paddock 的主题一致；从没调过，就不答，和现在一样。

## 5. 实现要改哪些地方（第二步派活用）

| 位置 | 改什么 |
| --- | --- |
| `crates/corral/src/terminal.rs` | 解析输出时认出 OSC 10/11 的查询形式，记下用的是 BEL 还是 ST；跨两次读的半截查询沿用现有 `pending`；设置形式和别的 OSC 不认。新状态加 `serde(default)`（`Terminal` 在升级快照里）。`attach.rs` 也调 `feed`，它不用这些结果。 |
| `crates/corral/src/pen.rs` | `Pen` 加颜色（`Option`）和待写回答队列，都 `serde(default)`；读 master 的分支在 `term.feed` 后，没有写者且有颜色时把回答放进队列；PTY 可写时先写这个队列，只在 `input` 的当前块写完之后插入；有待写回答时要等 POLLOUT；`worker` 从 cfg 取颜色，`pen.log` 记一行。 |
| `crates/corral/src/cli.rs` | 新命令 `terminal-colors`（解析、`--help`、出错）；总用法那一行加上它；`start` 读设置放进 cfg，坏文件加 warning。 |
| 新的小模块（如 `terminal_colors.rs`） | 设置文件路径、读、校验 `#rrggbb`、原子写（`state::write`）、拼回答字节。 |
| `crates/corral/src/pen/upgrade.rs` | `SCHEMA` 不变；只加测试。 |
| 文档 | `docs/CONTRACT.md` 加第 4 节那段；`docs/DESIGN.md` 加一小节（什么时候答、只答哪两种、为什么不答别的）；`crates/corral/README.md`。`resources/AGENT_USAGE.md` 是给 agent 看的，用不上这个命令，不加。 |

要补的测试（隔离的 HOME、CORRAL_HOME，用合成程序，如 `/bin/sh -c 'stty raw -echo; printf <查询>; 读回答写进文件'`）：

1. `terminal.rs` 单元测试：BEL、ST 两种结尾；查询切成两半分两次到；设置颜色的 OSC、别的 OSC、OSC 4 不算查询。
2. pen：有设置、没有客户端时，程序很快收到一模一样的回答（`\e]11;rgb:efef/f1f1/f5f5\e\\`）；没有设置时什么都收不到；查询时有写者 attach 着，pen 不答，客户端收到原样的查询字节。
3. 回答不排在带延时的 `send` 后面，也不插进正在写的块中间。
4. CLI：设、查、清、各种 `usage` 错误；设置文件坏了时 `start` 照常成功、带 warning。
5. 升级：没有新字段的快照能接上（旧到新）；新快照里的颜色交接后还在。可以放进 `tests/upgrade.rs`，attach 相关的放 `tests/protocol.rs`。
6. 做的人手工实测一次：设成浅色，`corral start ranch/test-<名字> --label role=test -- codex`，不 attach，`corral read` 看到浅色输入框底色（如 `48;2;229;231;235`），然后 `corral stop`。

粗估：代码 150–250 行，测试差不多一样多。改到 pen 主循环和升级快照，建议交叉审查。对 paddock 是兼容新增（第 4 节）。

## 6. 要用户定的

1. 颜色来源只用本机设置（推荐），还是同时加 `corral start` 参数。
2. 只答 OSC 10/11（推荐），不答 DA1。代价：amp 在后台开时仍多等约 2 秒。
3. 有客户端连着但它不回答时，第一步不处理（推荐）。

**用户定（2026-10-24）**：三条都按推荐——颜色只用本机设置（`corral terminal-colors`），不加 `corral start` 参数；只答 OSC 10/11，不答 DA1；有客户端连着但它不回答时，第一步不处理。

## 7. 没查到的

- Claude Code 主题设成 auto 时问不问 OSC 10/11：本机设置是 light，用户说主要是 Codex，没测。
- agy 用不用 OSC 11 的回答：停在信任框，按规定不点。
- amp 不答时用哪套颜色：动画干扰，比不出来。
- OSC 7501 是什么：没查。
- 原始录制在本轮的临时目录（不进仓库），本报告只摘了片段。
