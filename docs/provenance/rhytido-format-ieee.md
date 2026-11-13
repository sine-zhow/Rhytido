# 溯源记录：格式转换完整 IEEE 语义（M2-E2b）

- **模块 / crate**：`rhytido-backend::format`（round_to_format 重写 + Format::exponent_range）、`bigfloat`（pub(crate) from_parts）；`rhytido-core::interval_eval`（format_interval 最近端点）
- **里程碑**：M2-E2b（补全 M2-E2 诚实标记的两处缺口）
- **执行 / 日期**：subagent 32ac1684（带根因修复条款）→ lead 独立验收 / 2026-11-13
- **关联**：docs/provenance/rhytido-core-basefn.md §5；docs/plan/M2-E-dispatch.md

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 标准 | IEEE-754 公开标准（binary64/32/16/128 指数范围/次正规/溢出规则） | 净室蒸馏 |
| 参考公开文档 | reference/sollya/doc/doubleextended.tex（binary79-like、Itanium 行为、非 Intel x87 伪 denormal） | 仅公开 doc，未读源码 |

## 2. 独立设计说明（lead 抽查确认）

- **IEEE 参数**：Double(53,−1022,1023)/Single(24,−126,127)/Half(11,−14,15,max 65504)/Quad(113,−16382,16383)/DoubleExtended(64,−16382,16383)。
- **网格量化**：canonical_parts→(sig,exp)，前导指数 e=exp+bit_len−1；grid=max(e−p+1, emin−p+1)（正常域 p 位/次正规固定网格，max 折叠下溢，次正规进位恰落最小正常数）；精确 IBig div_rem 量化 + 模式调整；|x|<半网格快速路径避免巨除数。
- **舍入后溢出判定**：grid+bit_len(q)−1>emax（覆盖 emax binade 进位，half(65520)→2^16→inf）。溢出规则 Nearest/HalfAway/Away→±inf、Zero→±max_finite、Up→+inf/−max、Down→+max/−inf；下溢→次正规网格/±0 保号。
- **format_interval 最近端点**：round-to-nearest-format 单调，[rn(lo),rn(hi)] 最紧有效包围；点输入折叠为点→faithful Test1 得 Exact/CorrectlyRounded（修复 Down/Up 返回下邻未必最近）。
- 全程精确整数算术（A3），零 unsafe。

## 3. 不复制声明

未复制 Sollya 源码；源自 IEEE-754 公开标准 + 参考公开 doc 蒸馏 + 独立实现；subagent 声明未读 Sollya 源码。

## 4. 行为验证（lead 独立实测）

- `cargo test --workspace --offline` EXIT=0：**569 全绿**（555 零丢失 +14；backend 269=format 10→23+doctest+1；core 300 不变仅修正断言）。
- `cargo clippy --workspace --all-targets -D warnings` EXIT=0；`cargo fmt --check` EXIT=0。
- 13 新测试四类：指数范围表/正常域与 round_to 交叉/边界不动点/次正规网格 tie/上溢→inf（half 65520、single 2^128 等）/定向溢出/下溢次正规/定向下溢/次正规进位/特殊值传播/幂等包围/nearest 单调/极端指数不 panic。
- 断言修正（加强非弱化）：faithful 1/3×5 格式各自位宽 FaithfulInexact+Down 像→Exact+Nearest 像；quad/de@53→CorrectlyRounded；integration single(double(1/3))@24→b+Exact；interval_eval 端点 contains→endpoints_eq。

## 5. 偏差（lead 裁决，均采纳）

- quad/de@53 证书 CorrectlyRounded（引擎末尾 round_outward 加宽到 63-bit 工作精度使包围非点，值仍正确舍入）——引擎既有精度模型，测试注释说明。
- prec<格式位宽时点像按契约括为 1 ulp(prec)。
- 新公共 API Format::exponent_range()（文档+doctest 齐备）；bigfloat 仅加 pub(crate) from_parts。

## 6. 复核

- lead 独立复核：已完成（git 取证/分层重计数/三门禁/round_to_format 量化·溢出·模式规则 + format_interval 单调性抽查）。
- 同伴复核 / 法务审查：待。