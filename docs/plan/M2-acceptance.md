# M2 · 求值引擎 验收报告

> 2026-11-13 lead 独立验收 ｜ 对照 03-roadmap M2 三判据 ｜ 全部子任务经 §7 验收协议（git 取证→分层重计数→三门禁→关键实现抽查）后推送，远端 `ls-remote` 核验。
> **结论：M2 验收通过。** workspace **569 全绿**（backend 269 / core 300），三门禁 EXIT=0，基线全程零丢失。

## 0. 交付与子任务溯源

| 子任务 | 交付 | 验收提交 | workspace |
|---|---|---|---|
| M2-A | 函数族补齐（8 BigFloat + 11 Interval） | 5b3624f | 271 |
| M2-B | erf/erfc 自研（级数+余项界+阈值） | c0f5d74 | 319 |
| M2-C | faithful Ziv 引擎 + 区间求值器 | cf100ba | 459 |
| M2-D | autodiff + Taylor 精化 | 4815765 | 509 |
| M2-E1 | Display 十进制往返 | a4e2f97 | 515 |
| M2-E2 | BaseFn 17→30 + 非光滑 Taylor 回退 | 8ed65ec | 555 |
| M2-E2b | 完整 IEEE 格式 + 最近端点包围 | bcfa9a9 | **569** |

溯源：docs/provenance/ 下 rhytido-backend-m2a / -erf / -display、rhytido-core-faithful / -diff-taylor / -basefn、rhytido-format-ieee 等。

## 1. 判据一 · 8 态质量证书正确 ✅

- **8 态**（Quality 0–7）：NotFaithful / Faithful / BelowCutoff / NonReal / Exact / FaithfulInexact / CorrectlyRounded / CorrectlyRoundedInexact，直接映射参考返回码。
- **判定逻辑**（faithful.rs，lead 抽查）：Ziv 循环 w₀=p+10、预算 512p；Test1 两端点 nearest 同值→正确舍入（点+可表示 Exact、点不可表示 CRInexact、有宽度 CR）；Test3 round_up(lo)==round_down(hi)→Faithful；Test2 相邻 next_above→FaithfulInexact（排除零/跨零）；same_bits 用 canonical_parts 区分符号零、NaN 永不证得；one-more-unit 重试。
- **可达性/蕴含/误差界**：tests/integration_engine.rs 8 态可达性 + 证书⇒误差界属性；tests/faithful.rs 已知正确舍入表值+证书断言。
- **Taylor 精化**（M2-D）：naive∩Taylor 仅收窄、空交回退、保号零；非光滑函数（M2-E2）回退 naive，包含性无条件。

## 2. 判据二 · 函数族差分 ✅

- **BaseFn 30/32**：sqrt/exp/ln/sin/cos/tan/asin/acos/atan/sinh/cosh/tanh/asinh/acosh/atanh/erf/erfc + log2/log10/log1p/expm1/abs/ceil/floor/nearestint + double/single/halfprecision/quad/doubleextended。**DD/TD 延后 M6**（展开算术，决策记录见 §5）。
- **公开参考值**：6 erf 常数（≤15 位首跑全过，未迁就输出）+ 独立 Simpson/Richardson oracle 交叉至 ~19–22 位；已知 binary64 正确舍入表（sqrt2/exp1/ln2/sin1/cos1/atan1/erf1/1÷3/2÷3）。
- **格式转换**（M2-E2b）：完整 IEEE（次正规/舍入后溢出/6 模式），最近端点→Exact/CorrectlyRounded；half(65520)→inf、2^53−1 不动点等。
- **Display 往返**（M2-E1）：ceil(p·log10(2))+1 位，p∈{11..300} 往返恒等，解除 ≤15 位限制恢复全精度交叉核对。
- **导数**（M2-D/E2）：30 变体导数恒等式 + 中心差分交叉；非光滑保守 a.e.。

## 3. 判据三 · cutoff 语义 ✅

- BelowCutoff：幅值语义（取 abs、仅严格正有效）、带内冲刷**保号零**；判定序 NonReal→cutoff→三测试（无界带内仍 NonReal）。tests/faithful.rs + integration_engine.rs 钉住。

## 4. 分层计数与门禁（最终实测）

- **backend 269**：单元 48 / 模块 202（bigfloat 82、erf 27、format 23、interval 35、integer 14、rational 14、error 7）/ 集成 8 / doctest 11。
- **core 300**：单元 57 / 模块 213 / 集成 25 / doctest 5。
- **三门禁**：`cargo test --workspace --offline` EXIT=0（569）；`cargo clippy --workspace --all-targets -D warnings` EXIT=0；`cargo fmt --check` EXIT=0。
- **bench 基线**：examples/bench.rs harness 已立（M0）；evaluate_faithful 微基准**延后性能阶段**——M2 以正确性为先（AGENTS.md §4 先正确后快），避免过早优化，理由成立。

## 5. 延后决策与遗留

- **DD/TD（doubledouble/tripledouble）延后 M6**：需展开算术（TwoSum/FMA 重正化），M2 以单组件格式为限；BaseFn 未含 DD/TD，打印/分派预留。
- **log 别名**：Ln 打印保持 "ln"，`log` 别名属 M3 解析层。
- **quad/de@53 证书 CorrectlyRounded**（引擎末尾向外舍入到工作精度使包围非点，值仍正确舍入）——既有精度模型，测试注释说明。
- 无未闭合正确性缺口（E2 两处格式缺口已由 E2b 收口）。

## 6. 验收结论

M2 三判据全部满足，569 测试全绿、三门禁 EXIT=0、基线零丢失、净室溯源齐备。**M2 验收通过，可启动 M3（语言+CLI，见 M3-plan.md）**。