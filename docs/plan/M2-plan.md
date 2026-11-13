# M2 实施计划 · 求值引擎（faithful + 函数族 + autodiff）

> 2026-11-12 ｜ 依据 03-roadmap M2；原则：先正确后快、每阶段独立可验收、AGENTS.md §1.1 工作纪律与 §4 三层测试。

## 0. 进度（lead 独立核验）

| 阶段 | 状态 | 证据 |
|---|---|---|
| M2-A 函数族补齐 | ✅ 完成 | backend 178 / workspace **271 全绿**（基线 210 零丢失 + 61）；fmt/clippy EXIT=0；严格 π 界 + 精确格点判定抽查合格 |
| M2-B erf/erfc 自研 | ✅ 完成 | backend 226 / workspace **319 全绿**（零丢失 +48：erf 单元 18 + 模块 27 + doctest 3）；6 公开常数首跑全过 + 独立 Simpson oracle 交叉验证 ~19–22 位；fmt/clippy EXIT=0；lead 抽查 Bracket/series_bound/阈值链合格 |
| M2-C 区间求值 + Ziv 引擎 | ✅ 完成 | workspace **459 全绿**（319 零丢失 +140）；Test1/2/3+cutoff+「再加一元」+512p 预算+8 态证书+hook 穿线，lead 抽查 certify/same_bits/pi_interval 合格；两执行者均中途终止，lead 按 §7 亲自收尾（clippy 值恒等修复/fmt/清理） |
| M2-D autodiff + Taylor | ⏳ 待派发 | 契约已备 docs/plan/M2-stage3-tasks.md |
| M2-E Display 往返 + 验收 | ⏳ 待启动 | |

## 1. 现状盘点（工具取证）

**已有**：
- backend：BigFloat 15 函数（sqrt/exp/ln/sin/cos/tan/asin/acos/atan/sinh/cosh/tanh/asinh/acosh/atanh）+ pi_with_precision/pow/is_integer/round_to/round_to_format(5 格式)/canonical_parts；Interval 四则 + 12 单调函数 + cosh（向外舍入）。
- core：Expr/Interner（15 BaseFn）、print_expr、eval_point（记忆化点求值）、Quality 8 态、FpResult、Context、MessageCatalog。
- 测试三层结构就绪（210 全绿）。

**缺口（对照参考 base-functions.c 的 32 函数权威清单）**：
- 函数族 17 缺：log2、log10、log1p、expm1、abs、ceil、floor、nearestint、erf、erfc、double、single、halfprecision、quad、doubledouble、tripledouble、doubleextended。
  - 5 个单分量格式转换（double/single/halfprecision/quad/doubleextended）→ 复用 `round_to_format`，仅需 core 分派。
  - doubledouble/tripledouble → 展开算术子系统，**正式延后至 M6**（roadmap 已列 M6 风险项），本计划记录该决策。
  - erf/erfc → dashu 无，需自研（严格级数 + 余项界）。
- Interval 缺 sin/cos/tan（极值/极点）及新函数的区间版。
- 无区间表达式求值器、无 Ziv faithful 引擎、无 autodiff/Taylor、无 hooks。
- 已知 Display 怪癖（53 位精度十进制仅 ~15 位有效数字）待修（见 05-testing.md §4b）。

## 2. Ziv 引擎语义（据参考公开行为理解，独立实现）

- 起始工作精度 `max(target+10, 给定)`；循环上限 `target*512`。
- 每轮：区间求值 yI ⊇ 真值；两端点按目标精度 nearest 舍入得 rlo/rhi。
  - **正确舍入**：rlo==rhi。精确性：yI 为点且可表示→Exact(4)；点但不可表示→CorrectlyRoundedInexact(7)；有宽度→CorrectlyRounded(6)。
  - **faithful A**：区间落在目标格式两相邻浮点数之间（nextabove(lo)==hi）；**faithful B**：round_up(lo)==round_down(hi)（恰含一个浮点数）。达到后试 p+10「再加一元」争取正确舍入，否则 Faithful(1)/FaithfulInexact(5)（区间有宽度⇒已知不精确）。
  - **cutoff>0** 且 yI ⊆ [-cutoff,cutoff] → BelowCutoff(2)。
  - NaN/Inf → NonReal(3)；超上限未决 → NotFaithful(0)。
- 证书不可丢弃（AGENTS.md §3），随 FpResult 返回。

## 3. 阶段划分（每阶段独立验收）

### M2-A · 函数族补齐（backend，可委托）
- BigFloat 新增 8：`log2/log10/log1p/expm1`（dashu log2/log10/ln_1p/exp_m1 包装，前置定义域/特殊值守卫：log2/log10 x>0，log1p x>-1；±∞/NaN 按 IEEE）；`abs`（-0→+0）；`ceil/floor/nearestint`（经 canonical_parts + Integer 除法；nearestint  ties-to-even）。
- Interval 新增 11：`sin/cos`（宽度≥2π→[-1,1]；否则端点值 + 用 floor/ceil 判定区间是否含极值点 π/2+2kπ、-π/2+2kπ / 2kπ、π+2kπ）；`tan`（含极点 π/2+kπ→[-∞,+∞]，否则单调端点）；`log2/log10/log1p/expm1/abs/ceil/floor/nearestint` 区间版（单调端点，定义域保守）。
- 测试：模块层 tests/bigfloat.rs、tests/interval.rs 追加四类用例；私有辅助（如极值判定）入单元层。
- 验收：`cargo test -p rhytido-backend` 全绿 + clippy 零警告；恒等式（log2(8)=3、expm1(ln1p(x))≈x、ceil/floor/nearestint 边界、sin/cos 极值区间包含性、tan 极点）。

### M2-B · erf/erfc 自研（backend，研究级，lead 主导）
- 方案：|x| 小→交错级数 erf(x)=(2/√π)Σ(-1)ⁿx^(2n+1)/(n!(2n+1))，严格余项界；|x| 大→erfc 连分式/渐近展开（严格界）；erfc=1-erf 仅在精度安全时用，否则独立计算避免相消。
- Ziv 式精度递增保证 faithful；特殊值：erf(0)=0、erf(±∞)=±1、erfc(0)=1、erfc(+∞)=0、erfc(-∞)=2、奇对称。
- 测试：已知高精度值交叉核对（erf(1) 等公开数字）、单调性、对称性、erf+erfc=1、大/小/边界参数。
- 验收：全绿 + 与公开参考值在声明精度内一致。

### M2-C · 区间求值器 + Ziv faithful 引擎（core，A 完成后）
- `eval_interval(it, id, x: &Interval, prec) -> Interval`：记忆化递归，全 BaseFn 分派（含格式转换→区间端点 round_to_format）。
- `evaluate_faithful(it, id, x, ctx, cutoff: Option<&BigFloat>) -> FpResult`：按 §2 语义；Context 穿线（零全局状态）。
- 最小 hooks：`EvalHook` trait（节点求值拦截），引擎先查 hook——对应 roadmap hooks 交付项的最小实现。
- 测试：模块层 + 集成层（证书属性：包含性、faithful⇒误差≤1ulp、correctly-rounded⇒误差≤0.5ulp、已知 exact 案例、cutoff 语义、8 态可达性）。
- 验收：证书属性测试全绿；已知 faithful/correctly-rounded 案例（如 sqrt(2)、exp(1)、sin(1) 在 double 精度）与公开正确舍入值一致。

### M2-D · autodiff + Taylor 精化（core）
- `differentiate(it, id) -> NodeId`：符号微分（和/差/积/商/幂/链式法则，15+8 函数导数表），interning 共享。
- Taylor 规则区间精化：f([a,b]) ⊆ f(m) + f'([a,b])·([a,b]-m)，递归次数由 Context 扩展字段 `taylor_recursions` 控制（默认 0=至少一次，对齐参考语义）。
- 测试：导数正确性（数值差分交叉核对 + 已知导数恒等式）、Taylor 包含性、精化收敛。

### M2-E · Display 往返 + 收尾验收
- 修 BigFloat Display：十进制位数 ≥ ceil(prec·log10(2))+1，保证往返（修 05-testing.md §4b 怪癖）；更新受影响测试断言。
- BaseFn 扩至 30 函数（+5 格式转换；DD/TD 延后 M6 记录于 ADR/本文）；print 名称对齐 Sollya（log=ln 别名在 M3 语言层）。
- M2 验收报告：对照 roadmap 验收（8 态证书正确、函数族差分、cutoff 语义）逐项给证据。

## 4. 委托与复核

- M2-A 委托 backend subagent（写作用域 crates/rhytido-backend/**，签名契约见 §3A）；lead 复核+门禁。
- M2-B lead 主导（研究级）；M2-C/D core 可委托（写作用域 crates/rhytido-core/**），与 M2-B 并行（crate 不相交）。
- 每阶段：subagent 报告 → lead 独立运行门禁核对计数 → 台账/溯源更新。

## 5. 风险

| 风险 | 缓解 |
|---|---|
| erf/erfc 正确舍入证明复杂 | 严格余项界 + Ziv；已知值交叉核对；必要时降级 faithful 并如实证书 |
| 区间 sin/cos 极值判定边界 | floor/ceil 精确判定 + 包含性属性测试（含极值点邻域） |
| Ziv 不收敛（制表者困境） | 精度上限 + NotFaithful 如实返回；cutoff 语义 |
| DD/TD 范围蔓延 | 正式延后 M6，本文记录决策 |