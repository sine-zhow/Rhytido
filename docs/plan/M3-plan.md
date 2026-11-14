# M3 · 语言 + CLI 任务方案

> 2026-11-13 lead 制定 ｜ 依据 03-roadmap M3、02-architecture L4/L7、ADR-4/5 ｜ **启动前置：M2 全部验收推送（E1/E2/E3 收口）**。
> 派发遵循 AGENTS.md §7①–⑦：执行纪律/证据式报告/探针清理/自包含/上下文预算/上下文 sink 规避/**参考项目行为规格 + Rust 优化要点**。每任务一个 subagent、串行、lead 独立验收后推送。

## 0. 范围与验收（roadmap M3）

- **交付**：lalrpop 解析器（3 模式，约 650 产生式）；动态作用域求值器；match 解释器；proc/变长参数；设置；REPL；restart。
- **验收**：语义测试（动态作用域 / error 一等值 / 符号回退）；简单脚本字节 diff。
- **依赖**：M1（AST/打印/消息）、M2（求值引擎）。实际实现 eval 引擎已在 `rhytido-core`，故 lang 依赖 core。
- **风险**：文法规模（~650 产生式）→ 拆小串行；动态作用域细节 → 语义用例钉住；模式匹配（字符串/列表切割）→ 专项测试；上下文预算 → 每任务中等体量。

## 1. 架构落位与关键设计决策（M3-A 先定，必要时 ADR）

- **crate**：新增 `rhytido-lang`（L4：解析/求值/match/帧栈设置，依赖 core/backend）与 `rhytido-cli`（L7：REPL/restart，依赖 lang/io）。遵循分层无环。
- **决策点 D1（AST 扩展策略）**：现 core `Expr` 为数学表达式（10 变体）。语言需命令/控制流/模式/字面量（架构 ~250 节点、子枚举分组）。**方案**：core 增语言值类型与 AST 子枚举（数学/命令/字面量/模式/控制流），或 lang 层独立 AST 复用 core 数学 Expr——M3-A 定夺并记 ADR，避免巨型 match。
- **决策点 D2（值类型）**：list/string/boolean/range/procedure/error 一等值/void/struct 的不可变表示 + interning（ADR-1）；error 一等值与符号回退是行为契约（ADR-5），不可优化掉。
- **决策点 D3（Context）**：设置（prec/points/verbosity/display…）+ 符号表帧栈 `Vec<Frame>`（动态作用域）+ 消息抑制集；显式穿线、零全局、多实例（ADR-4）；退出作用域自动释放帧。

## 2. 参考项目行为规格（lead 从 reference/sollya/doc 蒸馏；净室——行为规格非源码）

- **parse(string)**：解析字符串中的表达式，返回 function|error；语法错→警告+error 值；可含"返回表达式的命令"（含 parse 自身），解析后执行；**不接受纯副作用命令/指令序列**（需包进匿名 proc 立即应用）。
- **proc**：`proc(形参…){ 体 }` 或 `proc(形参…){ 体 return expr; }`；变长用" formal list parameter"；**定义时仅解析不解释、无类型检查**，应用时检查实参个数/类型；无 return→void；过程匿名、一等对象（可入列表、作实参、绑定标识符）。
- **3 解析模式**：主语言解析器（parser.y）/ 迷你解析器（miniparser.y，miniyy 前缀）/ 内部对象解析器（internparser.y，internyy 前缀）。**确切文法/入口由 M3-B 从参考公开行为蒸馏**，不复制产生式表达。
- **动态作用域**：变量查找沿帧栈；proc 应用压帧、退出弹帧；赋值作用域语义以参考行为用例钉住。
- **match**：模式匹配（含字符串/列表切割、通配）；语义以参考用例蒸馏。
- **设置/REPL/restart**：prec/display/verbosity 等设置命令；REPL 读-求值-打印；restart 重置状态。字节级输出走 L6 表现层（M1 打印器）。

## 3. 任务拆分（中等体量、串行、写作用域互不重叠）

> 每任务派发提示词含：对应参考行为规格（§2 摘取）+ Rust 优化要点（docs/research/rust-numerical-optimization.md 摘取）+ §7 纪律。基线以派发时 lead 实测为准。

| 任务 | 内容 | 写作用域 | 依赖 |
|---|---|---|---|
| **M3-A** ✅ 值/Context 地基（2026-11-13 完成） | 值类型（core 原子 Atom + lang Value/列表；**proc/function 表示延后 B1**——body 依赖语法树，行为解释器本属 M3-D）+ 帧栈动态作用域 Session + 设置骨架；D1/D2/D3 已定（ADR-9/10/11）；四类测试 | core + lang 骨架 | M2 |
| **M3-B1** ✅ 词法+表达式文法（2026-11-13 完成，f51bb59） | lalrpop 0.23.2 接入 + **内置 lexer** + 数学表达式/常数/基函数/优先级文法 → core AST；parse_expr+ParseError；往返集成测试。**proc/function 值表示实际落位 B2**（其 body=命令/过程 AST，属 B2 产物；B1 仅数学文法，与行内"表达式文法"范围一致） | lang | A |
| **M3-B2** ✅ 命令/语句文法（2026-11-14 完成，2d76ccd；subagent 收尾截断、lead 接管收口） | 赋值/设置/print/块/过程语法文法 + 语句/proc AST 与 Value::Proc/Function 表示落地；蒸馏文档 sollya-language-b2.md（三级置信+未决移交） | lang | B1 |
| **M3-B3** match/控制流+迷你/内部模式 | 模式/控制流文法 + mini/internal 解析入口；解析单测 | lang | B2 |
| **M3-C** 动态作用域求值器 | 表达式调度 core 引擎、命令求值、变量查找、error 传播、符号回退；语义测试 | lang | B3 |
| **M3-D** match + proc/变长 | match 解释器（字符串/列表切割）+ proc 应用/变长/return/void；语义测试 | lang | C |
| **M3-E** 设置 + REPL + restart | 设置命令完整语义 + cli crate REPL/restart；简单脚本字节 diff | lang + cli | D |
| **M3-F** M3 验收报告 | 语义测试+脚本 diff 汇总、分层计数、门禁、溯源、台账收口 | docs | E |

- **Rust 优化要点**（按任务摘取）：A/B/C 用 B2 记忆化、B1 减少 clone、A3 精确判定；求值器 C 用 B3 避免重复求值、B4 早停；interning 结构共享（ADR-1）。正确性优先（动态作用域/error 契约不可为性能牺牲）。
- **上下文预算**：B 拆 B1/B2/B3 三段增量构建同一 lalrpop 文法（串行、每段可编译可测）；每任务"实现+测试+门禁"一次交付，若体量大再拆"实现/收尾"。

### 3.1 B1 实证与未决项（2026-11-13，lead 验收时录）

**lalrpop 0.23.2 接入实证**（B2/B3 直接复用，勿再踩）：
- `.lalrpop` 文件必须显式 `grammar;` 头（0.23 起，否则首 token 报 unexpected token）。
- 内置 lexer：禁显式 `'input` 生命周期；lalrpop-util 需 `features=["lexer"]`；文法头状态参数可穿 `&mut Interner`（引用本身 Copy）。
- `lalrpop_mod!` 生成模块需 `#[allow(missing_docs, clippy::all, unused_imports, dead_code)]`（workspace 级 lint 会命中生成码）。
- 动作代码避免 turbofish；`ParseError<usize, lexer::Token<'_>, &'static str>` → 本地 thiserror 五变体（带字节位置）映射模式已立。
- 空白-only 输入的 UnrecognizedEof.location=最后 token 末尾偏移（无 token 时 0），非源长度。
- 编译时长：首次接入=全依赖链冷编译（分钟级，一次性）；其后 lang 增量 check/test 秒级。生成码规模：单入口 ~2.8K 行。
- 零全局合规先例：thread_local 仅缓存**无状态**生成解析器（regex 编译复用），语义状态全走参数穿线。

**未决蒸馏项**（勿臆测实现）：
- 字面量：`0x/0b/0o` 进制与 `@` 比例、数字下划线分组；`inf/infinity` 与小写 `nan` 拼写（现仅 `NaN`）——待 B2/B3 从公开手册蒸馏。
- **常量字面量精度政策**：backend FromStr 取 `max(165, digits·10/3+1)`，打印 51 位 → 非二进十进制经 print→parse 的 NodeId 不恒等（值级恒等已测）。B1 裁决：不擅改 core；求值层如何规范化常量精度，随 **M3-C** 参考行为蒸馏定夺（涉语义契约，必要时 ADR 升级人类）。
- 打印器左嵌套幂非单射：`(a^b)^c` 打印为 `a^b^c`，parse 归右结合（典范性无碍）；若需修 core 打印器加括号，**先蒸馏参考打印行为**再动（表现层属字节级兼容域）。

### 3.2 B2 实证与接管记录（2026-11-14，lead 验收时录）

**B2 文法/语言事实**（B3/C 复用）：语句全部 `;` 终止（末条可省性=待验证）；list 字面量
`[|..|]`；赋值 `=`（存表达式）/`:=`（求值后存）+ 后缀 `!`（静默）AST 均保留、语义归 C；
`var` 声明仅块首（文法位置强制）；`print` 支持 `>`/`>>` 文件重定向；`return` 限过程体尾；
proc 变长形参写作 `name = ...`；`procedure` 缩写保留独立节点。双链架构：PExpr 与 Expr 为
**独立 LR 自动机**，`BinaryOp/UnaryOp::fuse` 在纯数学子树降级 intern（与 parse_expr 得同一
NodeId），含名字子树留在程序层——设置名/标识符统一 Var 引用，不预扩记号集。
`> 与比较算符冲突`、下标/`@` 摊开、进制字面量等见蒸馏文档 §8（B3 起领）。

**lalrpop 追加实证**：match 块关键字优先+最长匹配可让保留字与普通标识符共存
（`printer`/`sine` 不被切碎）；grammar 多状态参数（interner+symbols）可用。

**流程教训（复盘 #16/L6）**：subagent 以 clippy→fmt 顺序收尾、**test 门禁从未跑穿**即截断
终止（clippy 不检查 doctest 与测试输入正确性）。对策（已入 B3+ 派发模板）：三门禁顺序改为
**test 先跑穿（含 doctest）→ clippy → fmt**；结束判据加"workspace test 全绿并逐 target 报数"。

## 4. 调度与验收

- **串行**：A→B1→B2→B3→C→D→E→F（共享 lang 写作用域与 target 锁）。每任务 lead 按 §7 验收协议（git 取证→分层重计数→三门禁→关键实现抽查）后推送，再派下一个。
- **失败处理**：静默终止→git 取证→重派（写明失败模式）；连续两次失败→lead 亲自或拆小。
- **字节级**：简单脚本语料 diff（先复现怪癖后修复）；语义级：动态作用域/error/符号回退用例。

## 5. 启动检查单

- [x] M2 E1/E2/E3 验收推送完毕（workspace 基线锁定，569）。
- [x] D1/D2/D3 设计决策已定夺（ADR-9/10/11，用户批准方案 C；M3-A 验收，基线 774）。
- [x] lalrpop 依赖：0.23.2 已联网预热离线缓存并**离线构建复验**（探针全链编译 + 生成解析器跑通；实证 .lalrpop 需 `grammar;` 头）；本机 rustc 1.99 通过，MSRV 1.86 为计划既有声明。正式 build-dependency 加入 lang 属 B1 首步。
- [x] 参考语言行为蒸馏首批落档：`docs/research/sollya-language-b2.md`（B2，公开帮助页/示例为源，三级置信+§8 未决移交清单）；match/控制流/动态作用域语义夹具随 B3/C 蒸馏。