# 溯源记录：rhytido-backend / format

- **模块 / crate**：`rhytido-backend::format`（`Format`、`BigFloat::round_to_format`）
- **里程碑**：M0
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：FR-NUM-3（格式舍入）

## 1. 规格来源（实现依据的是公开思想，非参考表达）

| 类型 | 来源 | 说明 |
|---|---|---|
| 标准 | IEEE 754 各格式参数（公有标准） | binary16/32/64/128 与 x87 80-bit 的 significand 位数 |
| 后端 API | 本 facade 的 BigFloat::round_to | 复用定向舍入 |

## 2. 独立设计说明

- `Format` 枚举给出各格式 significand 精度（11/24/53/64/113 bit），为公开标准参数。
- `round_to_format` 复用 `BigFloat::round_to(precision, mode)`，NaN/∞ 保持不变。
- **M0 明确局限**（记录在案，M2 精化）：仅精度舍入；溢出/下溢/次正规（subnormal）行为与 DD/TD 展开格式留待 M2。
- 与任何参考表达无关：未参照 Sollya 源码结构。

## 3. 不复制声明

未复制 Sollya 的源码、注释或命名表达；实现源自 IEEE 754 公有标准参数 + 独立设计。

## 4. 行为验证（非对照源码）

- 10 个 Format 单元测试，覆盖四类：正常（各格式精度/舍入到 double/single/half/精确值不变/舍入模式差异）、极端（高精度→single、quad 保留更多）、异常报错（NaN/∞ 保持）、异常中断（全格式×全模式×特殊值不 panic、零舍入为零）。
- 1 doctest。全部通过（合计 crate 67 单元 + 5 doctest）。
- `cargo fmt --check` 无 diff；`cargo clippy --all-targets -- -D warnings` 零警告。

## 5. 复核

- **同伴复核**：待（小团队互审）
- **法务审查状态**：待审