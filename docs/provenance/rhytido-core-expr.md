# 溯源记录：rhytido-core / expr（M1）

- **模块 / crate**：`rhytido-core::expr`（`Expr`、`NodeId`、`BaseFn`、`Interner`）
- **里程碑**：M1
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：表达式 AST + interning（ADR-1）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 设计思想 | hash-consing / 结构共享（公有领域技术） | 不可变节点 + 结构相等即同一 NodeId |
| 数学结构 | 表达式树（公有领域） | 常量/变量/四则/幂/函数应用 |
| 后端 API | rhytido-backend `BigFloat::canonical_parts` | 常量规范化哈希/比较 |

## 2. 独立设计说明

- arena（`Vec<Expr>`）+ `NodeId` 句柄 + `HashMap<u64, Vec<NodeId>>` 哈希桶做 hash-consing；结构相等 → 同一 NodeId，缓存无需失效。
- 常量按**规范化值**（significand+exponent，经 `canonical_parts`）去重，与精度无关；**符号零区分**、NaN 常量去重。
- 子节点以 NodeId 引用，避免递归所有权与手工引用计数（全 safe）。
- M1 覆盖数学表达式；语言构造（命令/控制流/过程）M3 加入。
- 与任何参考表达无关：未参照 Sollya node 结构/命名。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；源自公有领域 hash-consing 思想 + 独立设计。

## 4. 行为验证

- 14 个四类测试：正常（intern/get、结构共享、常量按值去重、不同结构不同节点、嵌套共享）、极端（符号零区分、NaN、大常量、200 层深链）、异常报错（不同变体/常量 vs pi 不等）、异常中断（重复 intern 幂等、空 interner、全节点类型）。
- 全绿；`fmt --check` 无 diff；`clippy -D warnings` 零警告。

## 5. 复核

- 同伴复核：待；法务审查：待审。