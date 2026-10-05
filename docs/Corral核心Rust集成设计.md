# Corral 核心 Rust 集成设计

日期：2026-10-03。状态：**Rust 核心与消费者接入已实施并通过主控核对；隔离验证及限制已记录，未部署。**

依据：[源码调研](调研/Corral核心集成到Saddle-2026-10-02.md)、原 Corral `6923da1` 的 `docs/CONTRACT.md` 与实现、Saddle `b4ab2f2`。本轮由主控直接研究，不分派、不记录遥测、不操作真实会话或配置。

## 1. 已确定的产品约束

- 真正把 Corral 运行能力用 Rust 迁入 Saddle 仓库；不以附带旧 Python 程序代替迁移。用户进一步明确“不能有任何python的代码了”：迁入的实现、hook、worker、随包资源及配套新测试不带 Python 代码，构建/运行不调用 Python 或旧 Python CLI 兜底。原独立 Corral 仓库仍按既有要求保留不动。
- 一个 Saddle 安装包提供完整能力，保留 `corral start/status/send/reply/attach/stop` 等原有用法。
- 关闭界面不影响原 agent；普通终端可 attach，重开 Saddle 接续同一实例。自然完成工作导致状态变化正常，不能因界面重开重置会话。
- 原 Corral/corral-dispatch 仓库保持独立，不修改它们来适应 Saddle。
- Corral 核心不依赖 Saddle TUI、插件、Tasks、JEV 或遥测；不新增中央常驻服务。

## 2. 代码与可执行入口

workspace 新增独立 `crates/corral-core/` 包，含底层 library 与薄 `corral` binary target。

```text
Saddle TUI、agent 采集包装、插件消费者
                 │ 公开命令
                 ▼
         corral CLI（Rust）
                 │
                 ▼
         corral-core library
       registry / pen / PTY / hooks / adapters
                 │
                 ▼
          操作系统、外部 coding agent
```

底层包的 dependencies 不得包含根 `saddle` 包、plugin SDK/core-plugin、Drover/Dispatch 或遥测模块；底层测试也不通过依赖宿主来运行。Saddle 保留现有子进程边界，不在迁移时同时改成进程内执行，不需要把底层模型直接暴露给 UI。

同一个底层可执行文件可提供私有 pen/hook/after 工作模式，全部链接底层代码。独立 CLI/worker 不读取 Saddle 配置、连接 `saddle ctl` 或加载插件 catalog。安装包拥有多个进程和入口，不改变“一个产品”的目标。

模块按现有职责组织：命令解析与契约、登记/身份、pen 与 socket、attach/终端模式、事件状态机、各 agent 适配、环境重建与 hook、延后发送。先保留清晰模块，不增设多层动态后端或通用工作流框架。

## 3. 命令兼容

用户命令继续如下，使用前不需要打开 TUI：

```sh
corral start project/main --cwd /absolute/project -- claude
corral status project/main
corral attach project/main
```

完整命令族按原契约迁移，不能只实现 Saddle 当前采集的三个命令：

| 命令族 | 保持的关键行为 |
|---|---|
| start | 名称/unique、cwd、重复 env/label、prompt、首个 `--` 后原样传递、启动成功回执 |
| send / keys | idle 检查、人类操作保护、输入字节与按键、文本确认、草稿合并、未知不补发 |
| status / wait / reply / where / ls | 字段含义、实例身份、状态和等待语义、回复时间及完整正文 |
| read / attach | 原始字节/终端模式、Ctrl-]、attach --wait、多端只读与写者交接 |
| stop | agent 专属退出顺序，由 pen 执行，返回成功时已退出且名字可重新使用 |
| guide / install-skills / --version | 同版本 Corral 操作资源、安装选项与所有权保护、版本和契约标识 |

输出按契约比较 JSON 字段、类型、含义与退出码；不把键顺序/序列化空白误当兼容要求。read/reply/终端流保留各自原文规则。命令帮助和展示语言不借迁移改版。原 sandbox 拒绝规则、路径限制和错误优先级需要纳入兼容用例。

`saddle agent [采集选项] -- start|send|reply …` 保持原语法、回执与错误语义；底层仍不知道记录上下文。普通 `corral start` 不会自动开启遥测。

本次不必额外发明 `saddle corral` 等同义命令。以后如添加，应是上层入口，不作为独立 Corral 的运行前提。

## 4. 进程生命周期

start 经独立 worker 建立后台进程会话和 agent PTY，登记成功后返回 name/instance。具体 Unix spawn/句柄传递实现需单独核实；不在多线程 TUI 中照搬 Python fork 后执行复杂代码。

- pen 持有 agent PTY、socket 和名称锁；TUI 只持有 attach 客户端。
- 关闭客户端仅断开接入，不停止 pen 或 agent，不发业务按键。
- start 客户端退出后 pen 存活；stop 客户端中途退出后，已接受的停止序列仍由 pen 完成。
- `send --after` 使用独立等待 worker，绑定目标和被等待者的实例；`pending=true` 仍只代表排队，不能变成交付确认。
- 保留输出背压上限、慢接入者断开、PTY 尺寸调整和终端模式恢复。沿用 Saddle viewer 的显示职责。
- 不增加重启电脑后的进程恢复保证，不把重建会话冒充原进程继续运行。

## 5. 新旧互操作：直接兼容协议，不调用旧 Python CLI

具体建议是保持现有 `CORRAL_HOME` 命名空间与 v1 pen/event 格式，由 Rust CLI **直接**操作旧 pen，新 Rust pen 提供原 v1 能力。原 Python CLI 也应能管理新 pen。这让现存 agent 不需要搬家，也不需要用户为每个操作选择后端。

新运行核心不调用旧 Python CLI，无论正常路径、桥接还是失败兜底。若实现阶段发现无法满足兼容，应在切换前报告缺口，不静默降级成依赖原仓库的方案。

源码检查显示以下均属于兼容面，不只 socket：

| 层 | 需要保持 |
|---|---|
| 名称登记 | 锁路径、flock 持有期、inode 复核、同名冲突及 unique、启动未完成状态 |
| 清理 | `ls` 获取锁后才清失效栏位；不能误删活跃旧 pen、新 pen 或启动中的资源 |
| socket | proto=1、LF JSON 握手、attach 输入/resize 帧、错误字段、终端原始输出 |
| 元数据 | instance、kind、cwd、has_prompt、labels 和 exit 信息的意义/类型 |
| 事件 | v=1、实例/主会话过滤、时间、输入摘要、回复、后台任务计数 |
| cursor | 当前 schema=2 的 offset/快照/格式语义；并发查询不能把新旧状态算歪 |

特别注意：`status` 等读取会写 cursor，`ls` 会清理状态目录。不能把双客户端测试理解成完全只读。事件摘要还要核对 CRLF、Unicode 空白规范化与 UTF-8 字节偏移，避免 Rust 的近似实现让 send 确认失效。

以原 Corral 固定基线作为对照，不要求原仓库未来任意版本永远与新核心互通；以后按明确的协议版本支持策略演进，未知版本显式拒绝。上述格式只在底层内部兼容，不提升为插件可直接读取的接口。

## 6. Hook 与升级

2026-10-03 后续决定：用户已批准 [Corral通用升级设计](Corral通用升级设计.md) 的实施。下述固定版本绑定仍描述已部署旧版本；未来首个支持升级的版本按专项设计提供稳定外部 helper 入口、进程内采集器分离与 pen 原地升级。本次批准不覆盖真实部署或现存旧版本的首次迁移。既有不可变包不改写。

Claude/Codex hook 改为底层 Rust helper，事件仍输出兼容 v1；保留不写 stdout、错误不阻断 agent 的行为。参数按 agent 现有启动接口注入，不改用户全局配置。pi/omp 的 TypeScript 扩展作为同版本资源提供，不引入 Python Corral 依赖。

新安装采用不可变版本目录；启动时把 hook/helper 绑定到**该版本的绝对路径**。更新常用入口只影响新命令，不覆盖旧 agent 仍在使用的 helper。旧版本在仍有存活引用或不能确定安全删除时保留；本轮不附带自动垃圾回收系统。

开发调试也应从固定的暂存版本运行生命周期测试，不能让持久 agent 的 hook 指向下一次构建会覆盖的 target/debug 路径。安装路径与缓存位置需要在部署设计中具体落实，不能藏在 cwd/仓库路径假设里。

用户已确认旧会话原地保留，之后自行重开主控并使用已有 HANDOFF。旧 Python pen 及其 hook 在自然退出前继续使用原环境；新源码、新发布包及新会话完全无 Python。这不是把 Python 放进新产品的例外，不热替换旧 PTY，不自动停止用户会话。

保留登录 shell 环境重建与父会话身份清除。内部 helper 用绝对路径，不依赖调用者临时 PATH；不能让记录上下文或父 agent 身份自动串入新会话。

## 7. Saddle、插件与命令路径一致性

产品包提供相邻的 `saddle` 和 `corral` 可执行文件及资源。默认调用优先定位同包底层程序，不因 shell PATH 碰巧命中旧 Python 而形成不同会话行为；安装后的公开 `corral` 入口指向同一套运行包。

为保持现有配置形状，配置中的默认 `corral` 值可解释为产品默认程序；用户显式指定的外部程序路径与 `--corral PROGRAM` 保留。如何区别默认名字与用户有意配置的同名 PATH 程序，应在消费者适配时明确记录，不悄悄忽略自定义路径。

宿主向所有需要 agent 操作的插件提供通用的已解析程序路径；Drover 使用该通用路径，显式插件参数覆盖保持可见。此接口不包含项目、任务或遥测业务。不能只改记录路径、不改普通 send 路径。

发布包缺少底层程序时应报可理解的安装错误；业务已经尝试后绝不因错误改走另一后端重发。配置改变也不能把同名新实例当成原实例；保留 viewer 现有实例核对。

Corral 操作 skill 随产品提供匹配版本，独立 corral-dispatch 原仓库不改；已有全局同名技能的所有权保护必须保留。用户项目 AGENTS 仍人工维护。

## 8. 实施与部署分开

用户授权主控直接按以下顺序实施，未创建 Tasks 任务或派发 agent：

1. 底层独立包、完整契约映射、登记与协议兼容基础。
2. pen/PTY/attach、生命周期、终端模式、各 agent hook/事件、send/wait/after 对等实现。
3. Saddle/Drover/技能消费者统一路径；保留现有采集边界和 Tasks 流程。
4. 成套安装、版本资源固定及新旧互操作核验；通过后才计划真实切换。

切换时保留原 Corral 仓库和命令入口备份，已有 pen 原地保留，由用户自行关闭重开。安装新版本完整目录后再切入口，避免部分文件更新。旧 Python CLI 的回退可行性依赖互操作证据，且不能作为无 Python 新产品的运行路径；不能仅凭保留了一个软链接就宣布可无损回退。

### 8.1 安装和路径规则

通用升级实施后，未来版本的 pen/helper/after 切换遵循 [专项设计](Corral通用升级设计.md)；下述旧固定路径是兼容基线，不是要求新版本永远驻留旧 helper。打包仍不执行安装、链接切换或真实进程升级。

产品版本目录包含相邻的 `bin/saddle`、`bin/corral` 和官方插件包，常用命令通过链接指向版本目录。升级新增目录再切链接，不覆盖旧目录中的可执行文件。运行中的 pen、hook、after 均绑定启动时固定的绝对可执行路径；有存活会话的旧版本不删除。

默认配置值 `corral` 指同包程序；显式绝对/相对路径或其他程序名保持外部覆盖。需要特意使用 PATH 中名为 corral 的独立安装时，写出该程序路径以避免歧义。宿主、采集包装和插件必须共享此规则；不因为同包文件缺失而自动回退 PATH。开发构建中也生成相邻的两个程序，持久会话测试从不可变临时发布目录启动。

消费者统一时增加通用环境变量 `SADDLE_AGENT_BIN`，携带宿主已解析的绝对程序路径；Drover 的显式 `--corral` 优先，其次该变量，独立运行无变量时保留原 PATH 用法。底层 corral 不读取此变量来定位自己，也不依赖宿主。测试自定义假程序路径优先级保持。

本次不替换真实安装、不写用户配置或技能；交付源码与可验证发布构建方式，日常切换另行执行。

## 9. 核验场景

- 原 CLI→原 pen、Rust CLI→原 pen、原 CLI→Rust pen、Rust CLI→Rust pen：同一隔离运行目录，覆盖生命周期、状态/回复、交付、attach/keys/resize、停止与同名重启。
- 两种 CLI 同时查询/登记/清理；半行事件、过期 cursor、同名换 instance；错误不误清另一实现的存活会话。
- TUI 完全未运行也能 start/attach；关闭和重开 TUI 不改变 pen/agent 身份，不重放输入。接入/断开与写者交接仍符合契约。
- 输入确认超时不补发；人类活动拒绝；草稿合并；after 等待中的实例更换；停止客户端消失后的 pen 收尾。
- 升级命令入口后旧 hook 仍能执行；新 Rust 会话在无 Python Corral 安装的隔离环境下工作；旧 Python 会话不被假称已转成 Rust。
- 底层包独立构建/测试的依赖图不含上层包；关闭遥测及不启用任何插件不影响核心。

先用 Rust 编写的合成事件、假 agent 与隔离 HOME/CORRAL_HOME 做自动化；必要的真实 coding agent 冒烟仅使用自建会话，不使用用户 agent。不复制 Python 测试或夹具到新实现。原 Python 实现最多作为仓库外独立互操作对照；新产品自身构建、测试与运行不得以它为前提，对照验证与自身验证分开报告。

具体实现、实际执行的检查、原失败和剩余限制见 [实施核验](调研/Corral核心Rust集成-实施核验.md)。兼容范围限定原基线与实测场景，不把合成验证说成所有真实 coding agent 的端到端验收。

## 10. 实现细节与保留边界

- 底层新增依赖仅为成熟的 libc、serde_json、uuid、sha2、base64、regex 和 shell-words；测试用 tempfile。没有上层 crate 依赖，也没有动态加载旧 Python 的路径。
- 公共调用保留子进程/JSON 边界。私有 `__pen`、`__hook`、`__after` 绑定 canonicalize 后的当前二进制真实路径（macOS 的 current_exe 可能保留入口软链接）；pen 自持 PTY、socket、锁，启动客户端和界面的寿命都不拥有它。
- 登录环境捕获最多 10 秒（包括继承输出管道的子进程），启动回执等待最多 25 秒，覆盖环境重建和原 15 秒 pen 准备预算。超时不另起一遍；返回不确定失败后应查询原名称。
- 仅新建状态目录设 0700，已有父目录权限不改；文件/套接字仍私有。登记使用非阻塞 flock 和 inode 复核，`ls` 获取锁后才清失效栏位；未知协议拒绝。
- `install-skills` 保留原同意/预览/移除入口，但同名符号链接保守视为 foreign，不跟随写原仓库。现有全局链接的迁移属于另行部署，不自动占用。
- `scripts/package.sh` 只创建新版本目录，遇到已有目标拒绝覆盖；包含宿主、Corral、Drover/Diff 包、Corral 指引及构建哈希。不注册插件、不安装技能、不切命令链接。并发向同一目标发布不属于脚本支持范围。
- 新核心和新测试没有 Python 源码；`hook.py` 仅作为旧栏位保留文件名出现在兼容清理表。既有 Saddle 测试中的历史 Python 夹具不属于本次迁移；外部旧基线仅用于显式选择的互操作对照测试。
