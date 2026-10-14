# 任务：接入 amp（连同全局安装框架）

2026-10-14 起草，ranch 主控（ranch/main）自己做（用户直接和主控讨论定的；按 R3、R4 的做法由主控做）。
路由：重 / 交叉审查要（用户 10-14 同意）/ 影响面：改行为（新增一家 agent、新增公开命令；第一次往用户的全局配置里装东西）
类型：功能变更
依据：R5 调研报告 `docs/调研/R5-agy与amp接入、全局安装和读屏兜底.md` 和用户 10-14 定的五条（`docs/任务/R5-agy与amp接入调研.md` 末尾）。这件只做 amp 和全局安装框架；agy、读屏兜底另立。
提示：现有四家（claude、codex、pi、omp）行为一点不变，R3 的快照照旧通过；全局插件只动 corral 自己的一个文件；真实 amp 实测用户在场。

## 用户要的

- paddock P5-98：“再加一个draft task，我还有amp需要集成”。
- R3 时：“先恢复适配器结构，全局配置可以放宽。”（条件见 DESIGN §7：经用户确认才装；钩子不在 corral 启动的 agent 里就什么都不做；只动自己那块；原子写；能卸干净）
- R5 第六节，用户同意：两个全局位置（amp 是 `~/.config/amp/plugins/corral.ts`），知道每个 amp 多一个闲置 Bun 进程；先做 amp，全局安装框架随 amp 一起做；amp 先不支持 `--prompt`。
- 10-14 和主控讨论：
  - “amp我已经登录了，注意下是amp cli。另外还有一个app是桌面版本的amp，不要动。”
  - 读屏：“这个不能影响现在的已经接入的不靠读屏幕就能工作的，一定要注意，读屏幕只是兜底给agy，amp这类用的”（读屏不在这件）。
  - 桌面版也可能加载这个插件：“我觉得我可以接受。但是接入的时候要在settings里面讲清楚，不过这是paddock界面的事情，暂时你不用关心。”
  - 实测：“同意A”（在真实 HOME 里测，装的时候用户在场）。
  - 阶段：“同意你的建议”（一 amp → 二 读屏兜底 → 三 agy）。

## 在哪里干活
- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r6-amp`，分支 `r6-amp`。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r6-amp`。

## 要做的

1. **适配器接口加「全局安装」**：`Adapter` 新增一个方法，说明这家要不要装进全局、装什么（位置、内容、版本号）；默认没有，现有四家不写，行为不变。登记表加 `amp`（排在 omp 后面）。
2. **amp 适配器**（`agents/amp.rs`）：
   - 启动时不加任何参数（`hook_args` 为空）；不支持 `--prompt`（和不认识的程序一样报错）。
   - 停止步骤：实测后定，先用默认的 HUP／TERM。
   - 技能位置：amp 读哪些技能目录要先核对（文档、程序），只登记 corral 能装的位置；查不清就不登记。
3. **amp 插件 `corral.ts`**（编进程序，装到 `~/.config/amp/plugins/corral.ts`）：
   - 只在 `CORRAL_EVENTS` 有值、并且 `CORRAL_KIND` 是 `amp` 时才注册处理函数；否则什么都不做（用户自己开的 amp、桌面版 Amp、别的 agent 里跑的 amp 都是这样）。
   - 直接往 `CORRAL_EVENTS` 追加现有的 v1 事件（和 claude、codex 的钩子写的一样）。事件读取和状态机不改：
     - `session.start` → `SessionStart`
     - `agent.start` → `UserPromptSubmit`（带输入原文）
     - `tool.result` → `PostToolUse`
     - thread 状态 `awaiting-approval` → 等批准
     - `agent.end` → `Stop`（带回复原文；`error`／`cancelled` 也结束这一轮）
   - 不挂 `tool.call`：它要表态，可能绕过 amp 自己的权限。
   - 只记主 thread，子 thread 按父 thread 过滤（字段实测后定）。
   - 文件里带版本标记（`corral-integration: 1`），只在插件内容变了时加一。
4. **pen 设 `CORRAL_KIND`**：只给要全局安装的种类设（现在只有 amp），现有四家的环境不变。
5. **`corral integration install|uninstall --agent <kind> [--dry-run|--yes]`**：
   - 不带 `--yes` 只报要做什么。`--yes` 时原子写（同目录临时文件再 rename），卸载只删这一个文件。
   - 位置上已有的不是 corral 写的（没有版本标记、或是符号链接），报 `foreign`、不动。
   - 对不需要全局安装的种类报 `usage`。
   - 输出一行 JSON。
6. **`integration status`**：
   - amp 一项：`hooks: "global"`，配 `install:{path,status,version}`，status 用 `create`／`same`／`overwrite`／`foreign`。
   - 现有四家照旧 `launch`，字段不变。
   - 写进 `docs/CONTRACT.md`；DESIGN §7 写全局安装这一档。

## 对 paddock 的影响（写进完成记录）
- 兼容新增：status 多一项 amp，`hooks: "global"` 和 `install` 字段；新命令 install／uninstall。
- paddock 的 Settings 要讲清楚：桌面版 Amp 可能也加载这个插件，但在 corral 外什么都不做；每个 amp 多一个闲置进程（用户原话见上）。
- New Agent 选 amp 时不给 Prompt。

## 怎么算做完
- 用户没另说验收。主控建议下面几条，请用户定：
  1. 现有四家行为不变：R3 的快照不改、照旧通过。
  2. 合成测试（隔离的 HOME、CORRAL_HOME，假的 amp 程序，不用真实 amp）：
     - install／uninstall／dry-run 四种状态；
     - foreign 不动；
     - 卸干净；
     - 原子写；
     - status 的新字段；
     - `CORRAL_KIND` 只给 amp 设。
  3. 插件的「不在 corral 里就什么都不做」：用 `amp plugins exec` 在临时 HOME 里测加载时的判断。
  4. 真实一轮（办法 A，用户在场）：
     - 先给用户看要装的 `corral.ts`，用户同意后 `install` 进真实的 `~/.config/amp/plugins/`；
     - 用一个 `ranch/test-amp`（`--label role=test`，独立的 CORRAL_HOME）发两句简短的话；
     - 看到工作中 → 空闲、回复对；试一次等批准；试 `stop` 能停干净；
     - 用完 stop。插件留着还是卸，由用户定。
- `git diff --check`；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。

## 不要做
- agy、读屏兜底（阶段二、三另立）。
- 不碰桌面版 Amp（`/Applications/Amp.app`），不打开它。
- 不读 `~/.local/share/amp/` 下的登录文件；不改 `~/.config/amp/settings.json`。
- 不碰用户开着的 agent 和 amp。

## 部署（用户在场）
- 主控打包、核对；用户切 `~/.local/bin/corral`、`~/.local/bin/ranch`。
- pen 加了 `CORRAL_KIND`。已在跑的四家不受影响，所以 `corral upgrade --all` 可做可不做。
- 插件是否装上（`corral integration install --agent amp --yes`），由用户决定，或者以后从 paddock 的 Settings 点。

## 用户定的（10-14）

用户问清装的是什么（只有 `~/.config/amp/plugins/corral.ts` 一个文件）后：“同意。你按照你上面的建议开始吧。”即：
1. 验收按上面「怎么算做完」那几条。
2. 交叉审查要一轮，重点看「不在 corral 里就什么都不做」和装、卸只动自己那个文件。
3. 真实一轮用用户的 amp 账号发两句话，可以。

## 完成记录
