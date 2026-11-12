# 溯源记录：rhytido-core / context（M1）

- **模块 / crate**：`rhytido-core::context`（`Context`、`MIN_PRECISION`）
- **里程碑**：M1
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：零全局状态、设置显式穿线（AGENTS.md §3 / 架构 ADR）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 设计思想 | 显式上下文穿线（公有领域工程模式） | 无全局设置，支持多实例并发求值 |
| 依赖 | 后端 `Rounding`、`DEFAULT_PRECISION` | 复用 facade 配置 |

## 2. 独立设计说明

- `Context{precision, rounding}`，字段私有；`new`/`Default` 用默认精度 + nearest-even。
- builder `with_precision`/`with_rounding`；精度低于 `MIN_PRECISION=2` 时**钳制**到下限（文档化、无 panic）。
- 派生 `Debug, Clone, Copy, PartialEq, Eq, Hash`（设置可缓存/比较）。
- M1 仅含精度 + 舍入；cutoff/显示/递归上限等随引擎扩展。
- 与任何参考表达无关：独立设计。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；独立设计。

## 4. 行为验证

- 10 个四类测试：正常（默认/设置/链式顺序无关/default==new）、极端（精度钳制 0/1、大精度、全舍入模式）、异常报错（非法精度不 panic）、异常中断（Copy/Hash 一致、重复设置幂等）。
- 全绿；`fmt --check` 无 diff；`clippy -D warnings` 零警告。

## 5. 复核

- 同伴复核：待；法务审查：待审。