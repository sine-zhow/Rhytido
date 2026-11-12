# 溯源记录：rhytido-backend / 基本函数扩展（M2）

- **模块 / crate**：`rhytido-backend::bigfloat`（新增 12 个初等函数）
- **里程碑**：M2（函数族）
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：FR-EVAL-4（基本函数族）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 标准 | IEEE 754 特殊值/定义域语义（公有标准） | sin/cos/tan(±∞)=NaN；asin/acos 定义域 [-1,1]；acosh [1,∞)；atanh (-1,1)；atan(±∞)=±π/2 等 |
| 后端 API | dashu-float 0.6.2 公开文档/源码 | `sin/cos/tan/asin/acos/atan/sinh/cosh/tanh/asinh/acosh/atanh` 便利方法 |

## 2. 独立设计说明（含对后端行为的实证）

- facade 在调用后端前**显式守卫**特殊值与定义域（不依赖后端行为），仅有限且域内操作数传入后端。
- **实证的后端 panic 行为**（核读 dashu 源码确认）：`sin/cos/tan` 对 ∞ panic；`asin/acos` 对 ∞ 或 |x|>1 panic；`acosh` 对 x<1 panic；`atanh` 对 |x|≥1 panic。facade 全部前置守卫规避。
- **关键修正**：`atan(±∞)` 需 π/2，但无穷操作数 context 为无限精度，dashu 计算 π 触发 `assert_limited_precision` panic。facade 改为以有限精度 `2·atan(1)` 推导 π/2（`atan(1)` 为有限值、精度受限，可正常计算）。
- `tanh(±∞)=±1`、`cosh(±∞)=+∞`、`sinh/asinh(±∞)=±∞` 等由 facade 直接给出。
- 与任何参考表达无关：未参照 Sollya 源码结构。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；实现源自 IEEE 754 公有标准 + dashu 公开 API + 独立设计。

## 4. 行为验证

- 11 个四类测试：正常（零点值、sin²+cos²=1、cosh²−sinh²=1、tan=sin/cos、atan(1)=π/4）、极端（±∞/NaN 特殊值、定义域边界 asin(±1)/atanh(±1)/acosh(1)）、异常报错（定义域外→NaN、NaN 输入全函数→NaN）、异常中断（全函数×特殊值不 panic）。
- 全绿（backend 合计 78 单元）；`fmt --check` 无 diff；`clippy -D warnings` 零警告。

## 4b. 后续增补（pi / pow / is_integer）

- `pi_with_precision(prec)`：以 `4·atan(1)` 在指定精度计算 π（复用 atan 的有限精度路径）。
- `is_integer()`：经 `canonical_parts` 判定（significand 为 0 或 exponent≥0）；NaN/∞ 返回 false。
- `pow(&self, exp)`：IEEE 特殊值处理（x⁰=1、1ʸ=1、0ʸ、±∞ 底、±∞ 指数、负底非整指数→NaN）；负底整指数按奇偶定符号；一般情形调后端 `powf`（前置守卫规避其域外/无限精度 panic）。
- 增补 10 个四类测试（is_integer/pi/pow 的正常/极端/异常报错/异常中断）。backend 合计 88 单元，全绿，fmt/clippy 零警告。

## 5. 复核

- 同伴复核：待；法务审查：待审。