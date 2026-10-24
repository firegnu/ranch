# 任务：R51 `corral stop` 和 `corral ls` 同时跑时丢退出记录

（草稿，先不做、未派。用户 10-24：记草稿，以后再做。）

## 来历

R48e 交叉审查意见第 1 条（`docs/任务/R48e-审查.md`）发现，R48e 之前就有，所有 agent 都会碰上。

## 现象

- pen 停下时先写 `exit.json`，再删 socket、放锁。`corral stop` 要等 socket 连不上、锁也拿得到才去读 `exit.json`。
- 锁一放开，同时在跑的 `corral ls`（`state.rs` 的 `list()`：status 失败且拿得到锁时删掉整个目录）可能抢在 `stop` 下一次检查（每 50 ms）之前把目录连 `exit.json` 一起删掉。`stop` 就输出 `"exit_code":null,"stopped_by":null`，Kimi 的 `remaining_pids` 也跟着丢。
- paddock 侧栏默认每 1 秒刷新一次 `corral ls`，pony 也调 `ls`。审查实测：隔离环境里循环 start/stop `/bin/cat`，后台每秒跑一次 `ls` 时 200 次里有 8 次（4%）拿不到；`ls` 不停地跑时 40 次全拿不到；不跑 `ls` 时全正常。
- 影响：只是偶尔拿不到退出码和停止方式，`stopped_by` 为 null 时调用方看得出没取到记录，不会停错或误报停干净（R48e 已在 CONTRACT 写明）。

## 审查给的改法（供参考，派之前再定）

`stop` 的等待循环里，每次连不上 socket 就读一次 `exit.json`，记下 instance 匹配的那份；锁放开后如果再读不到，就用记下的。pen 先写 `exit.json` 再删 socket 和放锁，`ls` 又必须先拿到锁才能删，所以连不上时读到的一定是这次的完整记录。正常情况输出不变，碰上竞争时 null 变成真实值。测试可在 `final_kill_wait_is_bounded_and_records_remaining_processes` 里 `drop(lock)` 之后跑一次 `corral ls` 先做出失败。

## 待定

- 验收、派给谁，派之前问用户。改的是所有 agent 共用的 `stop` 输出（竞争时 null 变成真实值，兼容），要写对 paddock 的影响。
