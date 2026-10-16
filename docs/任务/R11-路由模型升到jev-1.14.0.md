# 任务（草稿）：`ranch dispatch route` 的分类模型升到 jev-1.14.0

2026-10-16 起草，ranch 主控（ranch/main）。**草稿：用户 10-16 说“再写一个draft task”。动手前再给用户看一遍，定下验收。**
路由：未定（动手时再跑）
类型：依赖升级

## 用户要的
- 10-16：“另外再写一个draft task，jev的api到1.4.0”；主控问是哪个版本，用户选的是 jev-1.14.0。

## 现状（主控 10-16 查的）
- `ranch dispatch route` 用 `TYPESAFE_API_KEY` 请求 TypeSafe 的分类模型（`https://api.typesafe.ai/v1/systemone`，`crates/dispatch/src/transport.rs`），模型名写死在 `crates/dispatch/src/rules.rs` 的 `MODEL = "jev-1.13.0"`。
- 用到 `jev-1.13.0` 的还有：`crates/dispatch/tests/fixtures/request.json`、`crates/dispatch/src/route_tests.rs` 的请求断言、`crates/dispatch/README.md`。
- 请求里的问题、阈值是从 Python corral `6923da1` 的 route.py 照搬的（`rules.rs` 开头注释）。1.14.0 有没有改请求或回复的格式、打分的含义，主控还没查。

## 要做的
1. **先查**：1.14.0 相对 1.13.0 改了什么（TypeSafe 的文档或更新说明）：请求、回复格式变没变，`score`、`noul` 两类问题的打分含义、阈值要不要跟着改。查不到说明的，先拿几条真实摘要分别用两个版本各跑一次比一比，再定。
2. **改**：模型名改成 `jev-1.14.0`，测试和 README 跟着改。格式或含义变了的，照新版改，每处写明依据；要改 `route` 的 JSON 输出（公开约定）的，停下来问用户。
3. **实测**：本机用真实 key 跑几次 `ranch dispatch route`，确认请求的是 1.14.0、输出正常。

## 不做
- 不改请求里的问题和阈值，除非 1.14.0 要求改（改了要写依据）。
- 不改 `route` 的 JSON 输出、退出码。
- key 不进仓库，实测的摘要用合成的，不用真实任务书。

## 怎么算做完（主控提议，等用户定）
- `ranch dispatch route` 请求的是 jev-1.14.0，真实调用输出正常。
- `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 全过。

## 对 paddock 的影响
- 只改内部实现时，paddock 不用动。
