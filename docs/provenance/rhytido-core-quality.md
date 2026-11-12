# 溯源记录：rhytido-core / quality（M1）

- **模块 / crate**：`rhytido-core::quality`（`Quality`）
- **里程碑**：M1（M2 求值引擎的前置语义契约）
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：8 态求值质量证书（AGENTS.md §3 语义契约，不可优化掉）

## 1. 规格来源（理解行为，不复制表达）

| 类型 | 来源 | 说明 |
|---|---|---|
| 行为规格 | Sollya 公开行为：8 态质量码 0–7 的语义 | 通过核读参考 `infnorm.c` 中描述返回值的注释（L6372–6398）**理解**各码含义；仅提取语义，未复制其代码/表达 |
| 设计思想 | 舍入强度 × 精确性知识 两维编码（公有领域概念） | faithful/correctly-rounded/exact 层级 + unknown/exact/inexact |

**8 态语义（据理解重述）**：0=未 faithful 但为有界实数/精确无穷；1=faithful、精确性未知；2=证明区间低于 cutoff；3=NaN/Inf 无法结论；4=faithful/正确舍入且已知精确；5=faithful 且已知不精确；6=正确舍入、精确性未知；7=正确舍入且已知不精确。

## 2. 独立设计说明

- `Quality` 枚举以 `#[repr(u8)]` 保留稳定数字码 0–7（兼容/序列化）；变体命名为独立设计。
- 提供查询方法 `is_faithful`/`is_correctly_rounded`/`is_exact`/`is_known_inexact`/`is_non_real`/`is_below_cutoff`，以及 `code`/`from_code`。
- 蕴含关系：exact ⇒ correctly-rounded ⇒ faithful（测试断言）。
- 与任何参考表达无关：未复制 Sollya 源码/命名；仅依据对公开行为语义的理解独立实现。

## 3. 不复制声明

未复制 Sollya 源码、注释或命名表达；规格来自对参考公开行为（质量码语义）的理解，实现为独立设计。

## 4. 行为验证

- 10 个四类测试：正常（码稳定/码往返/faithful 层级/正确舍入层级/精确性知识）、极端（特殊态、8 态互异）、异常报错（from_code 越界返回 None）、异常中断（全态谓词无 panic、蕴含关系恒成立）。
- 全绿；`fmt --check` 无 diff；`clippy -D warnings` 零警告。

## 5. 复核

- 同伴复核：待；法务审查：待审。