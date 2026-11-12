# 溯源记录：rhytido-backend / interval

- **模块 / crate**：`rhytido-backend::interval`（`Interval`）
- **里程碑**：M0
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：FR-NUM-1（多精度区间算术 + NaN/空集语义）

## 1. 规格来源（实现依据的是公开思想，非参考表达）

| 类型 | 来源 | 说明 |
|---|---|---|
| 数学定义 | 区间算术（公有领域数学，Moore 区间分析） | [lo,hi] 向外舍入、四则的端点组合规则 |
| 语义约定 | IEEE 1788 区间语义思想（NaN/空集/无界） | 以公开标准思想为据，非复制任何实现 |
| 后端 API | 本 facade 的 BigFloat（定向舍入 round_to） | 端点向外舍入复用 BigFloat |

## 2. 独立设计说明

- `Repr::{Nan, Empty, Range{lo,hi}}` 三态枚举，显式表达 NaN/空集/有界，避免用端点大小隐式编码。
- 构造与算术**向外舍入**（lo 向下、hi 向上），保证结果包含真实范围；端点可为 ±∞（无界）。
- 四则按区间算术端点组合：add/sub 直接端点运算；mul 取四积的 min/max；div 用倒数区间转 mul。
- **M0 明确局限**（记录在案，M2 精化）：端点运算用 nearest-even BigFloat 后向外舍入（保守）；除以含零区间返回 NaN；全定向舍入区间内核留待 M2。
- 与任何参考表达无关：未参照 Sollya/mpfi 源码结构。

## 3. 不复制声明

未复制 Sollya/mpfi 的源码、注释或命名表达；实现源自公有领域区间算术 + IEEE 1788 语义思想 + 独立设计。

## 4. 行为验证（非对照源码）

- 13 个 Interval 单元测试，覆盖四类：正常（构造/访问器/点区间/add/sub/mul/div）、极端（向外舍入包含性、无界端点、大区间运算）、异常报错（NaN/空集/lo>hi、除以含零区间、NaN 传播）、异常中断（特殊值全组合不 panic、contains(NaN)=false）。
- 1 doctest。全部通过（合计 crate 57 单元 + 4 doctest）。
- `cargo fmt --check` 无 diff；`cargo clippy --all-targets -- -D warnings` 零警告。

## 5. 复核

- **同伴复核**：待（小团队互审）
- **法务审查状态**：待审