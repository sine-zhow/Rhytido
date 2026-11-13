# ADR-9：语言 AST 与值的分层落位（M3-A · D1，方案 C 混合）

- **状态**：已接受（用户批准，2026-11-13）
- **日期**：2026-11-13
- **关联**：`docs/plan/02-architecture.md` §3/§5、`docs/plan/M3-plan.md` §1（D1）、ADR-10/11、AGENTS.md §3

## 背景

- M3-A 需要确定三样东西的落位：值类型、语言 AST（命令/控制流/模式/字面量）、会话状态
  （设置 + 符号表帧栈 + 消息抑制）。
- 架构文档口径存在内部张力：§3 crate 表把「Context/设置」划给 lang（L4）；§5 核心类型
  设计又把「值类型 / AST 约 250 节点按子枚举分组 / Context 帧栈」列在 core（L1）。
  M1 实际落地：core 已有 `Context`（仅设置，`Copy`）与消息目录。
- **硬约束是分层无环**（AGENTS.md §3）：proc/function 值必须引用语言 AST（过程体）。
  若 AST 在 lang，proc 值不能放 core；若 AST 全进 core，则需大改 core 的
  `Expr`/`Interner`/`print`——威胁已验收的 569 基线（M2 报告口径）。
- 候选：A=core 子枚举统一扩展（§5 字面方案，回归风险高）；B=lang 全独立含值类型
  （打破「值类型属 L1」口径）；C=混合切分（用户选定）。

## 决策

采用 **方案 C（混合落位）**，切分线 =「是否引用 AST」：

1. **core 只增不改**：新增 `crates/rhytido-core/src/value.rs`，承载**原子值** `Atom`——
   数值标量（BigFloat/Integer/Rational）、bool、string、range、error 一等值、void。
   原子值不引用任何 AST，纯数据。`expr.rs`/`eval`/`print`/`context`/`message`/
   `Interner` 零字节改动。
2. **新增 `rhytido-lang`（L4）** 承载：
   - **复合值** `Value`：list（元素可为任意 Value）/ proc（匿名一等，定义时仅解析）/
     function（数学式 core `NodeId` + range + 元数据）；`Value::Atom(Atom)` 包装 core
     原子值，数值语义不重复实现；
   - 命令/控制流/模式 AST：lang 自有 arena，与 core interner 无关（数学子树以 core
     `NodeId` 引用，复用结构共享）；
   - 会话 `Session`（设置 + 帧栈 Context + 抑制集，ADR-11）与解析器（M3-B）。
3. **修订 02-architecture §5 口径**：「约 250 节点统一在 core interning」修订为本切分。
   理由：语言 AST 从 interning 获益小（命令按副作用求值，无结构缓存诉求），而改动 core
   的回归代价高（正确性已验收的求值/打印/证书路径全部重测）。

## 后果

- (+) core 零改动 → 569 基线风险最小；lang 独立演进；无环分层保持。
- (−) 两层两类值表示（core `Atom` / lang `Value`）：以「lang 只包不造」约定消除数值语义
  重复；跨层转换单向（Atom→Value）。
- (−) 架构文档 §5 的该句以本 ADR 修订为准（文档回填见 M3-A 收口提交）。
- 值的字节级打印仍属 L6/打印器扩展（AGENTS 语义/表现分离）；A1/A2 不实现面向用户的
  Display 字节格式。

## 验证

M3-A1/A2 各自过三门禁且基线零回归；动态作用域 / error 一等值 / 符号回退语义由 M3-C
参考行为夹具钉住（先结构、后语义测试，分阶段可验收）。
