# 溯源记录：rhytido-backend / integer

- **模块 / crate**：`rhytido-backend::integer`（`Integer`）
- **里程碑**：M0
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：FR-NUM-2（精确整数算术）

## 1. 规格来源（实现依据的是公开思想，非参考表达）

| 类型 | 来源 | 说明 |
|---|---|---|
| 数学定义 | 任意精度整数算术（公有领域数学） | 加/减/乘/截断除/余/幂/比较的标准整数语义 |
| 行为测试 | 本模块 14 单元测试 + 1 doctest | 输入→输出契约 |
| 后端 API | dashu-int 0.6.2 公开文档/源码 | `IBig::pow` / `from_str_radix` / `Div` / `Rem` 签名 |

## 2. 独立设计说明

- Rust facade：`Integer` 封装 `dashu_int::IBig`，暴露稳定 API（`From`/`FromStr`/std ops trait/`checked_div`/`div_rem`）。
- Rust 惯用机制：所有权、std ops trait、thiserror 错误、`Option`/`Result` fallible 除法（零除数不 panic）。
- 与任何参考表达无关：整数算术为通用数学，未参照 Sollya 源码结构。

## 3. 不复制声明

未复制 Sollya 的源码、注释或命名表达；实现源自公有领域整数算术 + dashu 公开 API + 独立设计。

## 4. 行为验证（非对照源码）

- 14 单元测试，覆盖四类：正常 / 极端（2^128、i64 边界、googol）/ 异常报错（零除、非法解析）/ 异常中断（零除不 panic、大计算收敛）。
- 1 doctest。全部通过。
- `cargo fmt --check` 无 diff；`cargo clippy --all-targets -- -D warnings` 零警告。

## 5. 复核

- **同伴复核**：待（小团队互审）
- **法务审查状态**：待审