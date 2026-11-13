# 溯源记录：rhytido-backend / Display 十进制往返（M2-E1）

- **模块 / crate**：`rhytido-backend::bigfloat`（Display + FromStr）；tests/bigfloat.rs、integration_numeric.rs、erf.rs
- **里程碑**：M2-E1
- **执行 / 日期**：subagent fb43f2c7（带根因修复+参考规格+Rust 优化条款）→ lead 独立验收 / 2026-11-13
- **关联**：docs/plan/M2-E-dispatch.md E1；docs/plan/05-testing.md §4b（怪癖收口）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 数学/标准 | IEEE-754 / C++ `max_digits10` 往返位数公式（公开标准） | d ≥ ceil(p·log10(2))+1 |
| 后端行为 | dashu `to_decimal`/`with_base_and_precision` 语义（subagent 实证，非探针残留） | to_decimal 自动 ~p/4 位为怪癖根源 |

## 2. 独立设计说明（lead 抽查确认）

- **Display**：有限非零值 `with_base_and_precision::<10>(digits)`，digits=ceil(bits·LOG10_2)+1；p=0（unlimited）用实际 significand 位数 repr().digits()。53→17 位、165→51 位。位数推导注释完整（相对间隙 2^-p vs 十进制分辨 10^-d，需细于半个二进制 ulp）。
- **FromStr 配套**（parse 半边，p=300 往返必需）：prec=max(DEFAULT_PRECISION, D·10/3+1)（log2(10)<10/3）；短字面量 ≤49 位仍 165 位行为不变，长字面量保所需位数。**向后兼容加法，lead 复核采纳**；core 从不 parse BigFloat 字符串（grep 核实 + 260 测试无扰佐证）。
- **特殊值**：NaN/±inf 规则未动；±0 走原 to_decimal 路径（基线 -0→"0" 原样保留，值恒等往返成立）；Debug 委托 Display 结构未改。

## 3. 不复制声明

未复制 Sollya 源码/表达；源自公开 IEEE-754 标准 + dashu 实证 + 独立实现；subagent 声明未读 reference/sollya。

## 4. 行为验证（lead 独立实测）

- `cargo test --workspace --offline` EXIT=0：**515 全绿**（509 零丢失 +6 往返属性；backend 255=249+6，bigfloat 76→82；core 260 无扰）。
- `cargo clippy --workspace --all-targets -D warnings` EXIT=0；`cargo fmt --check` EXIT=0。
- 往返属性 p∈{11,24,53,64,113,165,300}：1/3、2/7、pi 全宽 significand+负数+2^53−1、±0、2^-1074 非规格化/2^-1022/2^1023/1e±30、2^p−1 及 next_below、sweep。
- **解除 ≤15 位限制（加强非弱化）**：integration_numeric Display 交叉核对扩至全 binary64 精确域；erf 精度交叉核对升全 53 位+全公共常数点+erfc（backend 高精度参考）；assert_15 公共常数校验原样保留。

## 5. 偏差（lead 裁决）

- FromStr 精度保持：p=300 往返必要配套加法，向后兼容、core 无扰——**采纳**。
- ±0 基线渲染 -0→"0"：按"不得改动"原样保留，值恒等往返成立——**采纳**。
- 05-testing §4b 怪癖台账：超出 subagent 写作用域，lead 验收时收口——**已收口**。

## 6. 复核

- lead 独立复核：已完成（git 取证/分层重计数/三门禁/Display 位数推导+FromStr 兼容性抽查）。
- 同伴复核 / 法务审查：待。