# AGENTS.md · Rhytido 开发规范

> 本文件是所有参与者（人类与 agent，**含 subagent**）的常驻开发规范。派发 subagent 时须将本文件相关护栏写入其任务提示并指明本文件路径；subagent 产出由派发者复核后才采纳。
> 深层架构/算法/路线细节在 `docs/plan/` 与 `docs/research/`，**按需查阅**（§8 指针），不在本文件复述。
> 权威依据：[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)、[Rust Style Guide](https://doc.rust-lang.org/style-guide/)、[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)、rustfmt、Clippy、[Unsafe Code Guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)。

---

## 0. 项目速览与硬性护栏

Rhytido = 用 Rust **净室重实现** Sollya 的数值引擎（任意精度求值 + 认证多项式逼近 + 认证范数 + libm 代码/证明生成）。当前处于**实施阶段**（计划已获批准；M0–M2 已完成并验收，M3 待启动）。

**硬性护栏**（违反即停；括号内为正向目标）：
- **净室**：不复制 Sollya 任何源码或表达（从公开规格、论文、行为测试独立实现；§6）。
- **批准前不写实现代码**（先完成计划批准；本文件等规范文档除外）。
- **许可证**：自身代码 Apache-2.0；依赖优先宽松，LGPL 动态链接（§6）。
- **内存安全**：核心全 safe；unsafe 仅限 FFI 边界并封装（§2.8）。
- **unsafe 须用户批准**：任何 unsafe 使用前必须获用户批准；获批后仍须证明逻辑无遗漏并完备测试（§2.8）。
- **测试门禁**：只有测试过的代码才能使用与合并；每个函数/类型/trait 完成即跑全部测试（正常/极端/异常报错/异常中断，§4）。
- **语义契约**不可"优化掉"：动态作用域、error 一等值、符号回退、8 态质量证书（§3）。

**工具链**：edition 2024；MSRV **1.86**（lalrpop 0.23 要求）；**stable 工具链即可运行全部门禁**（本机 scoop rust 无 nightly；rustfmt.toml 仅启用 stable 选项，若日后启用 `group_imports` 等 nightly-only 选项再要求 `cargo +nightly fmt` 并同步 CI）。

## 1. 工作流程（每步有完成判据）

1. **定位**：读 `docs/plan/03-roadmap.md` 确认当前里程碑与任务，读对应需求（01）与架构（02）。*判据：能说出本任务属哪个里程碑、验收标准是什么。*
2. **实现**：按 §2 风格与 §3 架构纪律写代码。*判据：功能满足对应需求条目。*
3. **测试**：每完成一个函数/类型(struct/enum)/trait，即补并跑**全部**测试——正常/极端/异常报错/异常中断广覆盖（§4）。*判据：该单元各类用例齐备且 `cargo test` 全绿；未测代码不得进入下一步。*
4. **格式化与 lint**：`cargo fmt`（stable，按 rustfmt.toml；启用 nightly-only 选项后改 `cargo +nightly fmt`）+ `cargo clippy --all-targets -- -D warnings`。*判据：fmt 无 diff，clippy 零警告。*
5. **提交**：按 §5 Conventional Commits。*判据：信息合规、原子化。*
6. **评审与合并**：小 PR + 互审（§5）；**只有测试全绿的代码才能合并**。*判据：同伴 approve + 测试门禁通过。*

### 1.1 工作纪律（硬性）

- **工具优先**：文件读/写/改一律经 harness 系统工具（read/write/edit/glob/grep 等）；每次调用后**等待返回并认真反思其内容**，以返回为据决定下一步；不凭假设或过时叙述行动。
- **反循环**：严禁重复、无增量、无收益的输出（复述同一意图而不执行工具、在同一决策上空转等）。**一旦自我检测到无意义循环，立即终止输出，等待用户的恢复指令**。
- **注入提醒极简响应**：对反复注入的纪律提醒（如 URL/网络纪律），若本任务不涉及相关工具，**最多一句带过、立即继续实质工作**；严禁每轮重复冗长确认。此类重复样板会持续吞噬上下文，是 subagent "ran out of room" 的实证根因（M2-D：每条消息都冗长确认 URL 纪律，叠加任务体量致上下文耗尽）。每轮输出必须以实质增量为主，样板占比最小化。
- **可追溯、可验证**：每项改动可追溯（溯源记录/文档/提交信息）、可验证（测试与门禁全绿）；汇报进度必须附真实工具证据（测试计数、门禁退出码），不得虚报。
- **持续推进**：每轮工作须有实质增量（代码/测试/文档）；受阻时明确报告阻塞点与所需决策，不空转。
- **委托**：开发/测试任务可分发给 subagent；subagent 必须遵循本文件（§7），写作用域互不重叠，派发者复核其产出并集中跑门禁后才采纳。

## 2. 代码风格（精确约定）

> M0 将把本节编码为 `rustfmt.toml` / `clippy.toml` / workspace lint 配置；届时以配置文件为机器事实源，本节为其人类可读规格。

### 2.1 格式化（rustfmt 强制）
- 缩进 **4 个空格，禁用 tab**；行宽 **≤100 字符**，纯注释行 ≤80。
- 多行参数/实参用**块缩进**（非视觉对齐）+ **尾逗号**；长方法链每行一个 `.method()`。
- 换行符 **LF（Unix）**，跨平台一致（Windows 开发也用 LF）；条目/语句间 0 或 1 空行；行尾无空白。
- 注释用 `//`，sigil 后一个空格，完整句（首字母大写、句号结尾），解释**为什么**而非复述代码。
- 目标 `rustfmt.toml`（M0 落地）：
  ```toml
  edition = "2024"
  max_width = 100
  hard_tabs = false
  tab_spaces = 4
  newline_style = "Unix"
  use_small_heuristics = "Default"
  reorder_imports = true
  reorder_modules = true
  # 下列需 nightly rustfmt
  group_imports = "StdExternalCrate"
  imports_granularity = "Crate"
  wrap_comments = true
  format_code_in_doc_comments = true
  ```

### 2.2 命名（RFC 430 / C-CASE）
| 实体 | 约定 | 例 |
|---|---|---|
| 函数/方法/变量/模块/文件 | `snake_case` | `eval_faithful`、`backend` |
| 类型/struct/enum/trait | `UpperCamelCase` | `BigFloat`、`FpResult` |
| 常量/静态 | `SCREAMING_SNAKE_CASE` | `DEFAULT_PREC` |
| 生命周期 | 短小写 | `'a`、`'ctx` |
| 泛型参数 | 单大写或 Camel | `T`、`Backend` |
- 转换方法：`as_`（廉价借用视图）/ `to_`（昂贵转换）/ `into_`（消耗所有权）（C-CONV）。
- getter **不加 `get_` 前缀**（C-GETTER）；集合迭代 `iter`/`iter_mut`/`into_iter`（C-ITER）；迭代器类型名与方法对应（C-ITER-TY）。
- 构造器为静态固有方法 `new(...)`（C-CTOR）；词序一致（C-WORD-ORDER）；feature 名无占位词（C-FEATURE）。
- crate 名 `rhytido-<layer>`，与架构一致。

### 2.3 导入
- 顺序 **std → 外部 crate → 本 crate**，分组（`group_imports="StdExternalCrate"`）；按 crate 粒度合并（`imports_granularity="Crate"`）。
- 禁通配 `use foo::*`（测试/prelude 除外）；仅导入用到的项；深路径用 `use` 别名。

### 2.4 函数与调用（细致）
- 有明确接收者 → **方法**而非自由函数（C-METHOD）；**不用 out 参数**，返回值/`Result`（C-NO-OUT）。
- 参数序：接收者 → 主输入 → 配置/选项（最后）；参数 >5 个改用配置 struct。
- 所有权语义清晰：`&self`（读）/ `&mut self`（改）/ `self`（消耗）；借用优先于克隆；调用方决定拷贝与放置（C-CALLER-CONTROL）。
- **用类型表达含义，不用 bool/Option 传语义**（C-CUSTOM-TYPE）；标志集用 `bitflags`（C-BITFLAG）；复杂构造用 **builder**（C-BUILDER）；泛型最小化假设（C-GENERIC）。
- 调用多行时块缩进 + 尾逗号；错误传播用 `?`；暴露中间结果避免重复计算（C-INTERMEDIATE）。
- 校验入参（C-VALIDATE）；析构不失败（C-DTOR-FAIL）；读写泛型按值取 `R: Read`/`W: Write`（C-RW-VALUE）。

### 2.5 类型与 trait
- 公共类型积极实现常见 trait：`Debug, Clone, PartialEq, Eq, Hash, Default`（按需 `Ord/PartialOrd/Display`）（C-COMMON-TRAITS、C-DEBUG）。
- 转换用标准 trait `From/Into/AsRef/AsMut`（C-CONV-TRAITS）；集合实现 `FromIterator/Extend`（C-COLLECT）。
- **字段私有**（C-STRUCT-PRIVATE）；newtype 做静态区分（C-NEWTYPE）；可能作 trait object 的 trait 保持对象安全（C-OBJECT）；需防下游实现的 trait 用 sealed（C-SEALED）。
- 尽可能 `Send + Sync`（C-SEND-SYNC）；数值类型提供 `Hex/Octal/Binary` 格式化（C-NUM-FMT）。

### 2.6 错误处理
- 库 crate：`Result<T, E>`，`E` 用 **thiserror** 定义的本 crate 错误枚举；实现 `std::error::Error + Display + Debug`，有意义（C-GOOD-ERR）。
- 仅 CLI/二进制顶层可用 **anyhow**；库不泄漏 anyhow。
- 库代码**禁 `unwrap`/`expect`**（测试与带注释的不变量除外）；用 `?` 传播。
- panic 仅用于**文档化的不变量违反**；函数文档含 `# Errors` / `# Panics` / `# Safety`（C-FAILURE）。

### 2.7 文档
- 所有公共项 `///` 文档；crate/模块用 `//!`；文档含可运行示例（用 `?`，不用 unwrap）（C-CRATE-DOC、C-EXAMPLE、C-QUESTION-MARK）。
- 文档注释置于属性之前；`# Errors`/`# Panics`/`# Safety` 段落齐备；prose 用超链接（C-LINK）。
- Cargo.toml 元数据齐全（license/description/repository/keywords/categories 等）（C-METADATA）；release notes 记录显著变更（C-RELNOTES）。

### 2.8 unsafe（须用户批准）
- 核心逻辑**全 safe**；unsafe 仅限后端 sys FFI 与 C ABI 边界，封装在最小模块并提供安全抽象。
- **任何 unsafe 使用前必须获得用户批准**；未经批准不得写入 unsafe。申请时说明：为何必须 unsafe、涉及范围、安全不变量。
- **即便获批**仍须：(a) **证明逻辑无遗漏**——逐条论证安全不变量（前置/后置条件、边界、别名与生命周期、panic/unwind 安全、未初始化内存）；(b) **完备测试**——正常/极端/异常报错/异常中断 + Miri。
- 每处 unsafe 附 `// SAFETY:` 注释说明不变量；以 Miri/审计验证；禁止为"方便"引入 unsafe。

## 3. 架构纪律（项目专属）

- **分层无环**：依赖只向下（backend → core → eval → approx → lang → codegen/io → 接口）；禁止反向/循环。
- **facade 隔离**：rug/dashu 等具体库类型**只出现在 rhytido-backend**；其余层面向 facade 抽象，禁止泄漏后端类型。
- **零全局状态**：设置/符号表/scratch 由 `Context` 显式穿线；支持多实例。
- **不可变 + interning**：AST 节点不可变、hash-consing；缓存走 NodeId 侧表；不手工引用计数。
- **语义契约**：保留动态作用域、error 一等值、符号回退、8 态质量证书、认证降级警告。
- **语义/表现分离**：引擎不依赖输出格式；字节级打印在独立表现层（rhytido-io）。
- 深层设计与决策记录见 `docs/plan/02-architecture.md`（ADR-1…8）。

## 4. 测试与质量

- **测试门禁（硬性）**：**只有测试过的代码才能使用与合并**。每完成一个函数/类型(struct/enum)/trait，先跑全部测试再进入下一步；未测代码不得合并。
- **用例覆盖（广）**：每个单元至少覆盖四类——
  - **正常**：典型输入 → 预期输出。
  - **极端**：边界值、空/零/±∞/NaN、最大最小精度、超大度数、退化/点区间。
  - **异常报错**：非法输入、类型错误、越界、除零、定义域外 → 断言正确错误类型/消息。
  - **异常中断**：panic 路径、资源耗尽、（如适用）取消/超时 → 断言不泄漏、不破坏不变量。
- **测试三层分离（硬性结构）**：
  - **单元测试**：`src/` 内联 `#[cfg(test)] mod tests`，仅测**私有/内部实现细节**（内部辅助函数、宏、不变量）；公共行为不放此层。
  - **模块测试**：`crates/<crate>/tests/<module>.rs`，每个源模块一个文件，从外部视角测该模块**公共 API**，四类用例齐备。
  - **集成测试**：`crates/<crate>/tests/integration_*.rs`，**跨模块/跨 crate** 端到端场景（类型协作、管线、契约一致性）。
  - 测试函数命名 snake_case 描述行为（`normal_*`/`extreme_*`/`error_*`/`robustness_*` 或 `test_<行为>`）；迁移测试**不得丢失覆盖、不得弱化断言**。
  - 运行：`cargo test --workspace`（全部层）；`cargo test --lib`（仅单元）；`cargo test --tests`（模块+集成）。
- 属性测试（区间包含性、证书一致性、Sturm 根计数、remez 交替性）；后端差分测试（rug vs dashu）。
- **字节级**：移植 Sollya 脚本语料，`diff` 全量 stdout；先复现怪癖/bug，后修复并更新 reference。
- **门禁**：`cargo test` 全绿 + `cargo clippy --all-targets -- -D warnings` 零警告 + `cargo fmt --check` 无 diff（stable；nightly-only 选项启用后相应升级）。
- 性能：关键算子基准对标 Sollya，防退化；先正确后快。
- unsafe 处 Miri（可行处）+ 模糊测试。

## 5. 协作规范（小型团队互助）

- **提交**：Conventional Commits——`<type>(<scope>): <description>`；type=`feat/fix/docs/style/refactor/perf/test/build/ci/chore`；scope=crate 名（如 `feat(eval): ...`）；破坏性用 `!` 或 `BREAKING CHANGE:` footer；原子化、一次一事；body 说明动机。
- **分支**：`<type>/<短描述>`（如 `feat/faithful-engine`）；小步快跑。
- **PR**：小而聚焦（单一里程碑任务）；描述含动机/改动/测试/验收，关联需求条目。
- **互审**：至少一名同伴 approve 才合并；评审看正确性/契约/风格/测试；作者回应每条评论。
- **沟通**：阻塞/歧义尽早提出；决策记入 ADR；进度同步（异步亦可）。
- **互助**：结对解决难点；共享发现（源码缺陷、研究结论）写入 docs。

## 6. 许可证与净室

- 每个源文件头 SPDX 标识：`// SPDX-License-Identifier: Apache-2.0`。
- 仓库根 `LICENSE`（Apache-2.0）+ `NOTICE`；依赖许可登记（cargo-deny）。
- **净室**：不复制 Sollya 代码/表达；从公开文档/论文/行为测试独立实现；维护溯源记录（`docs/provenance/`）；法务审查。
- 依赖优先 MIT/Apache；LGPL（rug/malachite/fplll）动态链接 + facade 隔离；不链接 MPFI（自研区间层）。
- 详见 `docs/plan/04-license-and-cleanroom.md`。

## 7. Subagent 规范

- subagent **必须遵循本文件全部规范**；派发时把相关护栏（净室/批准前不写码/风格/架构纪律）写入其任务提示，并指明本文件路径。
- 派发前明确任务边界与完成判据；subagent 产出由派发者**复核**（风格/契约/测试）后才采纳。
- subagent 不得自行做许可证/架构取向决策；此类决策升级给人类。
- 只读调研类 subagent 同样遵守净室与引用规范（一手来源、标注已核实/待验证）。
- **派发提示词必备条款**（实证教训，docs/review/subagent-retrospective.md）：① 执行纪律——从第一个工具调用起顺序推进，**严禁以计划/意图文本结束回合**，结束前交付物必须落盘并通过门禁，无法完成须列"已落盘/未完成"清单；② 证据式报告——git status --short 真实输出、分层测试计数、门禁退出码；③ 探针/临时文件须在最终报告前删除；④ 提示词完全自包含（subagent 结束后**不可续作**，send_message 无法送达）；⑤ **上下文预算**——任务体量须与 subagent 上下文匹配，过大则拆小；对注入的纪律提醒极简响应（§1.1），严禁每轮冗长样板确认（M2-D 实证因此 "ran out of room"）；cargo/git 若被沙箱拒绝管道输出，改用 run_in_background 或提权，**不在受限模式反复重试同一条被拒命令**；⑥ **上下文 sink 规避（四类实证思考循环，事件流取证）**：(a) **增量落盘**——边设计边写文件并编译，严禁把整个实现囤在 reasoning 里一次性推导（#4 末条消息 13K token 在思考里设计完整个 erf.rs 却零落盘即耗尽）；(b) **探针极简**——探针只做一次性实证，需 >2–3 轮修编译错即弃用、改以真实测试验证，勿让探针沦为 edit→run→error 调试黑洞（#6 后端+核心两度探针循环耗尽）；(c) **测试追因有界**——同一 panic/错误反复出现即识别为已知边界（如 dashu `precision cannot be 0`）并针对性处理，不无限 run→diagnose→run（#7 追该 panic 循环耗尽）；(d) **样板极简**（§1.1，#8 每轮冗长 URL 确认耗尽）；⑦ **参考项目上下文 + Rust 优化**：派发提示词须含 (a) 由 lead 从参考项目**公开行为/文档蒸馏**的本任务**具体行为规格**——函数语义、定义域/边界、期望输出、特殊值、证书/精度要求（净室：给行为规格而非源码，subagent 据此独立实现、不复制表达）；(b) 与本任务相关的**正确且高效的 Rust 技巧**——引用 `docs/research/rust-numerical-optimization.md` 并按任务摘取要点写入提示词（正确性优先、再谈效率）。
- **验收协议（硬性）**：派发者不以 subagent 自报为事实，独立取证后方可采纳：git status（检测零输出静默终止与游离文件）→ 分层重计数 → 三门禁（test/clippy/fmt）→ 关键实现抽查阅读。
- **失败处理**：静默终止/零输出 → 先 git 取证，再重派（重派提示词写明上次失败模式与纪律条款）；同一任务连续两次失败 → lead 亲自实现或拆小任务，不空转重派。
- **调度约束**：共享 target 目录或源码依赖的任务（如同一 workspace 的 backend/core）**串行**派发，避免门禁互相污染；写作用域互不重叠是并行的前提。

## 8. 参考指针（按需查阅）

- **需求/架构/路线/许可**：`docs/plan/01-requirements.md`、`02-architecture.md`、`03-roadmap.md`、`04-license-and-cleanroom.md`。
- **测试分层细则与迁移台账**：`docs/plan/05-testing.md`。
- **Subagent 评估台账与经验总结（滚动更新）**：`docs/review/subagent-retrospective.md`。
- **研究/生态/许可证矩阵**：`docs/research/sollya-port-research.md`。
- **M3 任务方案**：`docs/plan/M3-plan.md`；**Rust 数值优化技巧（派发参考，§7⑦）**：`docs/research/rust-numerical-optimization.md`。
- **权威外部规范**：Rust API Guidelines、Rust Style Guide、Conventional Commits、rustfmt、Clippy、Rust Unsafe Code Guidelines（链接见文件头）。