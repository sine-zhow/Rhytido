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
| **M3-A** 值/Context 地基 | 值类型（list/string/bool/range/proc/error/void）+ 帧栈动态作用域 Context + 设置骨架；定 D1/D2/D3（ADR）；四类测试 | core + lang 骨架 | M2 |
| **M3-B1** 词法+表达式文法 | lalrpop 接入 + lexer + 数学表达式/常数/基函数/优先级文法→AST；解析单测 | lang | A |
| **M3-B2** 命令/语句文法 | 赋值/设置/print/块/过程语法文法；解析单测 | lang | B1 |
| **M3-B3** match/控制流+迷你/内部模式 | 模式/控制流文法 + mini/internal 解析入口；解析单测 | lang | B2 |
| **M3-C** 动态作用域求值器 | 表达式调度 core 引擎、命令求值、变量查找、error 传播、符号回退；语义测试 | lang | B3 |
| **M3-D** match + proc/变长 | match 解释器（字符串/列表切割）+ proc 应用/变长/return/void；语义测试 | lang | C |
| **M3-E** 设置 + REPL + restart | 设置命令完整语义 + cli crate REPL/restart；简单脚本字节 diff | lang + cli | D |
| **M3-F** M3 验收报告 | 语义测试+脚本 diff 汇总、分层计数、门禁、溯源、台账收口 | docs | E |

- **Rust 优化要点**（按任务摘取）：A/B/C 用 B2 记忆化、B1 减少 clone、A3 精确判定；求值器 C 用 B3 避免重复求值、B4 早停；interning 结构共享（ADR-1）。正确性优先（动态作用域/error 契约不可为性能牺牲）。
- **上下文预算**：B 拆 B1/B2/B3 三段增量构建同一 lalrpop 文法（串行、每段可编译可测）；每任务"实现+测试+门禁"一次交付，若体量大再拆"实现/收尾"。

## 4. 调度与验收

- **串行**：A→B1→B2→B3→C→D→E→F（共享 lang 写作用域与 target 锁）。每任务 lead 按 §7 验收协议（git 取证→分层重计数→三门禁→关键实现抽查）后推送，再派下一个。
- **失败处理**：静默终止→git 取证→重派（写明失败模式）；连续两次失败→lead 亲自或拆小。
- **字节级**：简单脚本语料 diff（先复现怪癖后修复）；语义级：动态作用域/error/符号回退用例。

## 5. 启动检查单

- [ ] M2 E1/E2/E3 验收推送完毕（workspace 基线锁定）。
- [ ] D1/D2/D3 设计决策（M3-A 内定夺，必要时 ADR 升级人类）。
- [ ] lalrpop 依赖加入 workspace（离线缓存可用性先验证；版本 MSRV 兼容）。
- [ ] 参考语言行为用例蒸馏（parse/proc/match/动态作用域/设置）成测试夹具。