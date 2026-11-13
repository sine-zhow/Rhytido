# 溯源记录：rhytido-core / BaseFn 函数族收齐（M2-E2）

- **模块 / crate**：`rhytido-core`（expr/print/eval/interval_eval/diff/taylor/faithful）；BaseFn 17→30
- **里程碑**：M2-E2
- **执行 / 日期**：subagent 56e7b9bd（带根因修复+参考规格+Rust 优化条款）→ lead 独立验收 / 2026-11-13
- **关联**：docs/plan/M2-E-dispatch.md E2；docs/research/rust-numerical-optimization.md

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 行为规格 | lead 从 reference/sollya/doc/*.tex 蒸馏（log2/log10/log1p/expm1/abs/ceil/floor/nearestint/格式转换语义） | 净室：行为规格非源码 |
| 数学定义 | 导数法则、非光滑函数 a.e. 导数（公有领域） | 独立实现 |
| 后端 API | 本仓库 backend（BigFloat/Interval 8 新函数 + round_to_format，M2-A/M0 已实现） | 组合既有 |

## 2. 独立设计说明（lead 抽查确认）

- **BaseFn 17→30**：+Log2/Log10/Log1p/Expm1/Abs/Ceil/Floor/NearestInt + Double/Single/HalfPrecision/Quad/DoubleExtended；DD/TD 留 M6。
- **is_smooth**：穷尽 match 分类 21 光滑/9 非光滑（新变体须先分类才编译）。
- **非光滑 Taylor 回退**：eval_taylor 在 clone/求导前 contains_non_smooth（HashSet 记忆化 DAG）→ 返回 naive；光滑导数仍光滑，顶层判定覆盖递归。
- **导数表**：log2'=1/(x·ln2)、log10'=1/(x·ln10)、log1p'=1/(1+x)、expm1'=exp(x)（ln2/ln10 符号 AST）；非光滑保守 a.e.（阶梯/格式→0、abs→u/|u| 且 0 处 NaN）。
- **分派**：print 规范名（Ln 保持 ln）；eval 点方法/格式 round_to_format(Nearest)；interval_eval backend Interval 方法/格式 format_interval。

## 3. 不复制声明

未复制 Sollya 源码/表达；源自 lead 蒸馏行为规格 + 公开数学 + 独立实现；subagent 声明未读 reference/sollya。

## 4. 行为验证（lead 独立实测）

- `cargo test --workspace --offline` EXIT=0：**555 全绿**（515 零丢失 +40；backend 255 不变、core 260→300）。
- `cargo clippy --workspace --all-targets -D warnings` EXIT=0；`cargo fmt --check` EXIT=0。
- 非光滑回退专门包含性：9 函数×跨零/整数/半整数/格式 tie×33 点×prec{24,53,120}×k{0,1}，回退与 naive 逐位一致；30 变体导数恒等式+中心差分（Quad/DE 更小步长）；已知值 log2(8)=3/nearestint ties-to-even/格式 2^53−1 不动点；小 x log1p/expm1 相对误差界。

## 5. 遗留缺口（lead 裁决 → M2-E2b 补全，非 E2 过失）

- **backend round_to_format**：仅尾数精度舍入，无溢出→inf/次正规/指数钳制（M0 自述留待 M2 细化）；halfprecision(65520)→65536 而非 +inf。
- **format_interval 定向端点（Down/Up）**：有效包围但点输入下括住相邻两格式浮点，faithful Test2 返回下邻**未必最近**，与 Sollya 最近舍入语义不符。**源于 lead 的 Down/Up 指示**；修复=最近端点（round-to-nearest 单调，[rn(lo),rn(hi)] 更紧且点输入折叠为点）。
- 两者已由 **M2-E2b 补全收口**（2026-11-13，lead 独立验收，workspace 569 全绿，见 rhytido-format-ieee.md）：round_to_format 完整 IEEE（exponent_range/次正规网格/舍入后溢出判定/6 模式溢出规则，精确整数算术）；format_interval 改最近端点（点折叠→Exact/CorrectlyRounded，原 Down/Up 弱证书断言修正为加强）。E2 核心分派/非光滑回退合格。

## 6. 复核

- lead 独立复核：已完成（git 取证/分层重计数/三门禁/is_smooth·非光滑回退·format_interval 抽查）。
- 同伴复核 / 法务审查：待。