# 任务：R50 没有客户端连着时，corral 也回答终端的前景色、背景色查询（OSC 10/11）

2026-10-24，ranch/main 起草。已完成：方案见 R50a（`docs/调研/R50-corral回答终端颜色查询.md`），实现见 R50b，10-24 部署（`03c265e`）；paddock 在 P5-181 接入，本机颜色设置已由 paddock 送上。
优先：高

## 用户原话

- 10-24，经 paddock/main 转：「发给 ranch 主控,先不做，先新建一个draft task」。

## 现象（paddock/main 转述、查的，10-24）

- 用户的 paddock 用浅色主题（Catppuccin Latte），Claude Code、Codex 的主题都设成 auto（跟终端一致），但从 corral 开的 agent 都显示成深色。
- 原因（paddock 主控查的）：Claude Code、Codex 启动时向终端发 OSC 11（问背景色，可能还有 OSC 10 问前景色），问不到就按深色。corral 自己不回答这类查询，只转给当时连着的客户端。
  - 主控 `corral start` 在后台开的 agent，启动时没有客户端连着，查询没人回答。
  - paddock New Agent 开的，也可能在 paddock attach 之前就问完了。
  - 之后再 attach 也没用，它们只在启动时问一次。
- paddock 自己的终端会照主题如实回答 OSC 10/11（paddock `app/src/terminal.rs`），所以问题在 corral 这层。

## 期望（paddock/main 转）

- 没有客户端连着（或还没连上）时，corral 也能回答 OSC 10/11。
- 颜色从哪来（例如 `corral start` 参数、环境变量、配置）由 ranch 定。
- 需要 paddock 配合传什么，写成公开约定（CONTRACT）告诉 paddock。

## 要摸清、要定的（派之前）

- 各家（Claude Code、Codex、pi、omp、amp、agy、kimi）启动时问不问 OSC 10/11，问了等多久、问不到时按什么处理；有没有别的查询（如 OSC 4、DA）也有同样问题。
- corral 在 pen 里什么时候自己回答：只在没有客户端连着时，还是一律由 corral 回答、不再转给客户端；有客户端连着但它不回答时怎么办。
- 颜色来源和优先次序：`corral start` 参数、环境变量、`~/.corral` 里的配置；都没有时回不回答（不回答就保持现在的行为）。用户换了 paddock 主题之后，新开的 agent 怎么跟上。
- `corral upgrade` 交接、旧版 corral 同时在用时的兼容。

## 对 paddock 的影响

- 预计是兼容新增：paddock 开 agent 时把当前主题的前景色、背景色按新的公开约定传给 corral。具体格式定了写进 CONTRACT。

## 结果

- R50a（`docs/任务/R50a-终端颜色查询调研与方案.md`）：查明只有 Codex 因启动时问不到 OSC 10/11 而按深色；用户定了三条取舍（只用本机设置、只答 OSC 10/11、客户端不答时不补答）。
- R50b（`docs/任务/R50b-corral回答终端颜色查询.md`）：新增 `corral terminal-colors`，pen 在没有客户端连着时回答 OSC 10/11；交叉审查可以合并，10-24 部署 `03c265e`。
- paddock P5-181：启动、换主题时调 `corral terminal-colors`，远端开 agent 前也调；10-24 已合并安装，本机设置已是 Latte（`#4c4f69` / `#eff1f5`）。
- 没做：Claude Code 主题设成 auto 时的表现没测（用户现在设的是 light，不问终端）；5090 没部署。
