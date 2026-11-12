# 溯源记录：rhytido-backend / M2-A 函数族补齐

- **模块 / crate**：`rhytido-backend`（bigfloat / interval / integer）
- **里程碑**：M2-A
- **执行 / 日期**：backend subagent 实现，lead 独立复核 + 集中门禁 / 2026-11-12
- **关联**：docs/plan/M2-plan.md §3A；AGENTS.md §4 三层测试、§7 委托复核

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 标准 | IEEE 754 特殊值/定义域（公有标准） | log2/log10/log1p/expm1/abs/ceil/floor/nearestint 的 ±∞/NaN/域外语义 |
| 数学定义 | 三角函数极值/极点格点（公有领域数学） | sin 极大 π/2+2kπ、极小 −π/2+2kπ；cos 极大 2kπ、极小 π+2kπ；tan 极点 π/2+kπ |
| 后端 API | dashu-float 0.6.2 公开源码（panic 边界实证） | log2/log10/ln_1p/exp_m1；unwrap_fp 对 InfiniteInput/OutOfDomain panic |

## 2. 独立设计说明

- BigFloat +8：log2/log10/log1p/expm1（dashu 包装 + 前置守卫）、abs、ceil/floor/nearestint（canonical_parts + Integer 精确除法，ties-to-even）。`from_integer` 用 from_parts(ibig,0) 保任意宽整数精确。
- Interval +11：sin/cos/tan（严格极值/极点）+ 7 单调函数区间版 + abs（跨零特判）。
- **严格 π 界**：prec+10 位正确舍入 atan（误差≤0.5ulp=2^(1-w)，π∈[2,4)），显式裕量 2^(3-w) 为 4 倍，定向舍入只放宽。
- **极值/极点判定绝不漏纳**：格点索引用 canonical_parts 精确整数有理算术（ceil_ratio/floor_ratio，无舍入除法）；界不确定时只宽纳。端点映像加显式包含裕量后定向舍入，超窄区间（2^-159 宽）采样包含性仍严格。
- 与任何参考表达无关：未读/未复制 reference/sollya。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；源自 IEEE 754 公有标准 + 公有领域数学 + dashu 公开 API + 独立设计。

## 4. 行为验证（lead 独立实测）

- 新增 61 测试（单元 15 + 模块 46），四类齐备；恒等式（log2(8)=3、expm1↔ln1p、nearestint ties-to-even、abs(-0)=+0）+ 密集采样包含性属性测试（极值邻域/跨零/跨极点/宽度≥2π/超窄区间/粗 π 锚点）。
- `cargo test --workspace --offline` EXIT=0：**271 全绿**（基线 210 零丢失）；backend 178。
- `cargo clippy --all-targets --offline -- -D warnings` EXIT=0；`cargo fmt --check` EXIT=0 无 diff。
- 无 unsafe；仅新增，未改既有函数逻辑/既有测试/doctest。

## 5. 偏差（如实记录）

- 复合 expm1(log1p([0,1])) 端点一致性用 150 位约化精度断言（M0「先 nearest 后 outward」范式复合可损失末位；严格有向内核属 M2-C）；单函数包含性保持全精度。
- 宽度≥2π→[-1,1] 由极值判定自然达成 + 无穷端点短路，未做独立有限精度宽度比较（无裕量不严格）。

## 6. 复核

- lead 独立复核：已完成（门禁计数核对 + trig_range/pi_bounds 抽查）。
- 同伴复核 / 法务审查：待。