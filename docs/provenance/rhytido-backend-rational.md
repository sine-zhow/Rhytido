# 溯源记录：rhytido-backend / rational

- **模块 / crate**：`rhytido-backend::rational`（`Rational`）
- **里程碑**：M0
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：FR-NUM-2（精确有理数算术）

## 1. 规格来源（实现依据的是公开思想，非参考表达）

| 类型 | 来源 | 说明 |
|---|---|---|
| 数学定义 | 有理数算术（公有领域数学） | 既约形式、正分母、四则/倒数/比较 |
| 后端 API | dashu-ratio 0.6.1 公开文档/源码 | `RBig::from_parts_signed`/`numerator`/`denominator`、std ops |

## 2. 独立设计说明

- facade 封装 `RBig`（既约/正分母），相等与序为结构化比较。
- `new(num, den)` 拒绝零分母（`Error::DivisionByZero`）；符号归一化由 `from_parts_signed` 完成。
- **facade 层补充校验**：后端 `RBig::from_str` 接受零分母（如 "1/0"），facade 在 `FromStr` 中显式拒绝——这是独立于后端的健壮性设计。
- 分子/分母经 crate 内部 `Integer::from_ibig` 返回 facade 类型，后端类型不出现在公共 API。
- 除法/倒数用 `checked_div`/`checked_recip`（`Option`），零除数不 panic。
- 与任何参考表达无关：未参照 Sollya 源码结构。

## 3. 不复制声明

未复制 Sollya 的源码、注释或命名表达；实现源自公有领域有理数算术 + dashu 公开 API + 独立设计。

## 4. 行为验证（非对照源码）

- 14 个 Rational 单元测试，覆盖四类：正常（构造/约分/符号归一/四则/访问器/解析/倒数）、极端（i64 边界、i64::MIN/-1 溢出、大约分、引用运算一致性）、异常报错（零分母、零除、非法解析含 "1/0"）、异常中断（既约形式稳定、零/符号边界不 panic）。
- 1 doctest。全部通过（合计 crate 44 单元 + 3 doctest）。
- `cargo fmt --check` 无 diff；`cargo clippy --all-targets -- -D warnings` 零警告。

## 5. 复核

- **同伴复核**：待（小团队互审）
- **法务审查状态**：待审