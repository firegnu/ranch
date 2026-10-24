# 任务：R50 没有客户端连着时，corral 也回答终端的前景色、背景色查询（OSC 10/11）

2026-10-24，ranch/main 起草（草稿，未定、未派）。
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
