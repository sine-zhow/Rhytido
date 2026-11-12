# 溯源记录：rhytido-core / fpresult（M1）

- **模块 / crate**：`rhytido-core::fpresult`（`FpResult`）
- **里程碑**：M1
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：值类型 / 求值结果（M2 引擎前置）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 设计思想 | 求值结果 = 数值 + 质量证书（公有领域概念） | 证书为一等组成部分，不可丢弃 |
| 依赖 | 本 crate `Quality`、后端 `BigFloat` | 组合既有 facade 类型 |

## 2. 独立设计说明

- `FpResult{value: BigFloat, quality: Quality}`；`new`/`exact` 构造，`value`/`quality`/`into_value` 访问，`is_exact`/`is_faithful`/`is_correctly_rounded`/`is_non_real` 委托 Quality。
- 字段私有；`into_value` 消耗所有权（C-CONV）。
- 与任何参考表达无关：独立设计。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；独立设计。

## 4. 行为验证

- 11 个四类测试：正常（构造/访问/exact/into_value/clone 相等）、极端（NaN/±∞ 非实、全 8 态可构造）、异常报错（NotFaithful/BelowCutoff 非 faithful）、异常中断（质量委托一致、特殊值不 panic）。
- 全绿；`fmt --check` 无 diff；`clippy -D warnings` 零警告。

## 5. 复核

- 同伴复核：待；法务审查：待审。