# 溯源记录：rhytido-core / value（M3-A1）

- **模块 / crate**：`rhytido-core::value`（`Atom`、`Bound`、`Range`、`LangError`）
- **里程碑**：M3-A1 ｜ **作者 / 日期**：subagent 14ddff4b（lead 独立验收）/ 2026-11-13
- **关联需求**：ADR-9/10（落位与表示契约）、AGENTS.md §3 语义契约（error 一等值不可优化掉）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 行为/表示规格 | `docs/adr/0010-atom-value-representation.md` | lead 自研 ADR，用户批准；变体集合、全域构造、相等语义均据此 |
| 设计先例 | 本 crate `expr.rs` 的 interning 相等（canonical_parts） | 值世界与表达式世界共用同一等价类 |
| 依赖 | facade `BigFloat/Integer/Rational` | 不泄漏后端具体库 |

## 2. 独立设计说明

- `Atom` 八变体（Number/Integer/Rational/Bool/String/Range/Error/Void），字段私有；
  `Bound = Closed|Open|NegInfinity|PosInfinity`（开闭/无穷以变体表达，无 bool 标志）。
- 构造全域：退化 Range（lower>upper、NaN 端点）原样保存，合法性判定归 M3-C/D。
- 相等/哈希手写：数值走 canonical_parts（NaN 结构相等、带符号零区分、精度标签无关）；
  `Rational` 以分子分母入哈希；`String(Rc<str>)` 按内容。不实现 `Ord`、不实现字节级
  `Display`（表现属 L6）。`Default = Void`。
- `LangError { message: Rc<str>, code: Option<u32> }` 纯数据载体，传播规则属 M3-C。

## 3. 不复制声明

净室：未参照 Sollya 对象层源码或其表达；表示完全由 ADR-10 自研推导。

## 4. 行为验证

- 模块测试 64（normal 23 / extreme 20 / error 10 / robustness 11）+ 内联单元 7 +
  doctest 5；workspace 569→645 零回归；三门禁 lead 独立复跑 EXIT=0。
- 实证发现（归档）：**相邻工作精度的超越函数值可能 canonical_parts 相同**
  （pi_with_precision 200/201 重合）——"精度不同⇒值不同"不成立；M3-C/D 精度相关
  夹具须以"值变化⇒身份变化"为准。

## 5. 复核

- lead 独立验收：git 取证 / 分层重计数 / 三门禁 / 关键段抽查（float_eq、Atom PartialEq/
  Hash）通过。同伴复核：待；法务审查：待审。
