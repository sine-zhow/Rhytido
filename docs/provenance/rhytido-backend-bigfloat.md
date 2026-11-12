# 溯源记录：rhytido-backend / bigfloat

- **模块 / crate**：`rhytido-backend::bigfloat`（`BigFloat`、`Rounding`）
- **里程碑**：M0
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：FR-EVAL-1（任意精度浮点）、FR-NUM-3（定向舍入）

## 1. 规格来源（实现依据的是公开思想，非参考表达）

| 类型 | 来源 | 说明 |
|---|---|---|
| 标准 | IEEE 754 特殊值语义（公有标准） | NaN/±∞/signed-zero 的传播与比较规则 |
| 数学定义 | 任意精度二进制浮点算术（公有领域数学） | 精度以 bit 计、定向舍入、四则/ sqrt/exp/ln |
| 后端 API | dashu-float 0.6.2 公开文档/源码 | `FBig`/`Repr`/`Context`、`with_rounding`/`with_base_and_precision`/`to_decimal`、`mode::*` |

## 2. 独立设计说明

- facade 封装 `FBig<mode::HalfEven, 2>`（base-2、bit 精度，匹配 Sollya/MPFR）；dashu 无 NaN，facade 以 `Value::{Nan, Num}` 枚举补齐。
- **特殊值在 facade 层显式处理**（IEEE 语义），仅有限操作数传入后端，规避后端 inf/panic 行为。
- 内部 base-2，`Display` 经 `to_decimal()` 转十进制；`from_str` 先按 base-10（`DBig`）解析再 `with_base_and_precision::<2>` 转二进制。
- 私有 `FBinExt` trait 统一后端谓词（dashu 将谓词置于 `Repr`/`Sign`，非 `FBig` 固有方法）。
- `round_to(prec, mode)` 用 `with_rounding::<M>()` + `with_precision` 实现运行时定向舍入（不改值只改模式，再按模式舍入）。
- 与任何参考表达无关：未参照 Sollya 源码结构。

## 3. 不复制声明

未复制 Sollya 的源码、注释或命名表达；实现源自 IEEE 754 公有标准 + 公有领域浮点算术 + dashu 公开 API + 独立设计。

## 4. 行为验证（非对照源码）

- 16 个 BigFloat 单元测试，覆盖四类：正常（构造/四则/sqrt/exp/ln/比较/精度）、极端（2^100、高精度舍入、定向舍入、i64 边界）、异常报错（0/0、inf/inf、inf-inf、0*inf、定义域、非法解析、NaN 比较）、异常中断（NaN 传播、特殊值全组合不 panic、inf 运算）。
- 1 doctest。全部通过（合计 crate 30 单元 + 2 doctest）。
- `cargo fmt --check` 无 diff；`cargo clippy --all-targets -- -D warnings` 零警告。

## 5. 复核

- **同伴复核**：待（小团队互审）
- **法务审查状态**：待审

## 6. M0 范围与后续

- 本期：nearest-even 四则 + `round_to` 定向舍入 + sqrt/exp/ln + 十进制解析。
- 后续（M0/M2）：pow、三角/双曲/erf 等其余基本函数、十六进制/二进制字面量、求值质量证书（`Approximation` 信息）。