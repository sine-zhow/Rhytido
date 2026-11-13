# M2-E 派发任务单（Display 往返 · BaseFn 收齐 · M2 验收）

> 2026-11-13 lead 制定 ｜ 承接 M2-stage3-tasks.md §E，细化为 dispatch 就绪的中等难度任务。
> **调度（硬性，AGENTS.md §7）**：D → E1 → E2 → E3 **串行**。理由：E1 改 backend（core 的源依赖），E2/E3 与 D 共享 core 写作用域与 target 构建锁；并发会互相污染门禁。**每个任务一个独立 subagent**，lead 逐个独立验收后才派发下一个。
> **通用纪律**（每个 subagent 提示词必含）：AGENTS.md §1.1/§4/§7；增量编译/测试；禁探针/游离文件；禁以计划文本结束回合；clippy 常量陷阱（测试用 std::f64::consts，不用近似字面量）；--offline + PATH 前缀；不跑 cargo fmt、无 git 写；证据式报告。lead 兜底收尾。

## 基线（派发时以 lead 实测为准）

D 完成后 workspace 计数会变化；E1 派发前 lead 重跑 `cargo test --workspace --offline` 记录逐二进制基线，写入各任务提示词。当前（C 收口）为 459（backend 249 / core 210）。

---

## E1 · Display 往返修复（backend，小任务，独立 subagent）

**精确上下文**：
- 位置 `crates/rhytido-backend/src/bigfloat.rs:1023-1041` `impl fmt::Display for BigFloat`；有限值走 `fmt::Display::fmt(&v.to_decimal().value(), f)`（L1036）。
- 怪癖（05-testing.md §4b）：`to_decimal` 按值精度渲染，53-bit 值仅约 15 位有效数字，`(2^53−1).round_to_format(Double,Nearest).to_string()=="9007199254740990"`（数值本身精确）。
- NaN/±inf/±0 显示规则（L1026-1033）**不得改动**。

**任务**：有限值渲染改为**可往返**——至少 `ceil(p·log10(2))+1` 位有效十进制（p=值精度；推导写注释）。先实证 dashu `to_decimal`/`Rounded` 的位数控制行为再定实现（用真实测试验证，**不建探针文件**）。`Debug` 委托 Display（L1043-1047）随之受益，不改其结构。

**测试**：新增往返属性（p∈{11,24,53,64,113,165,300}：value→to_string→parse→同精度比较恒等；含 2^53−1、负数、±0、极小/极大）；**解除** integration_numeric.rs 与 erf doctest 中"≤15 位有效数字"的限制条款、恢复全值域交叉核对（这是反向修正，注释说明，非弱化）。既有 Display 相关断言若依赖旧渲染须更新为往返正确值（不弱化）。

**门禁**：`cargo test -p rhytido-backend`（基线零丢失+新增）、`cargo test -p rhytido-core`（无扰）、clippy backend -D warnings EXIT=0、只读 rustfmt --check。写作用域仅 crates/rhytido-backend/**。

---

## E2 · BaseFn 收齐（core，中等任务，独立 subagent；依赖 D 与 E1 完成）

**精确上下文**：
- `crates/rhytido-core/src/expr.rs` `BaseFn` 现 **17 变体**（read 确认，含 Erf/Erfc）。backend 已实现但**尚未接入 BaseFn** 的函数：log2/log10/log1p/expm1/abs/ceil/floor/nearestint（M2-A）+ 格式转换 round_to_format（M0）。
- 分派点（read 确认）：`print.rs` 名称表、`eval.rs` apply_fn、`interval_eval.rs` apply_interval、`faithful.rs`（经 interval_eval）、`diff.rs` 导数表、`taylor.rs`（经 diff/interval_eval）。

**任务**：
1. BaseFn 增 **13 变体**：Log2/Log10/Log1p/Expm1/Abs/Ceil/Floor/NearestInt + Double/Single/HalfPrecision/Quad/DoubleExtended。**DD/TD 不做**（M6 展开算术，台账已记录）。
2. 全分派点穷尽更新（match 穷尽由编译器强制）。打印名沿用 Sollya 公开词：log2/log10/log1p/expm1/abs/ceil/floor/nearestint/double/single/halfprecision/quad/doubleextended；Ln 打印保持 "ln"（别名 log 属 M3 解析层，注释记录）。格式转换分派到 `round_to_format(Format::…)`。
3. diff.rs 导数表补：log2'=1/(x·ln2)、log10'=1/(x·ln10)、log1p'=1/(1+x)、expm1'=exp(x)；**非光滑函数保守处理**（见下）。
4. **正确性关键（务必）**：abs/ceil/floor/nearestint/格式转换是**非光滑函数**，Taylor 中值定理要求可微，**对这些函数 eval_taylor 必须回退 naive 区间求值**（不得跨间断点用中值形式）；diff 表给保守 a.e. 导数（ceil/floor/nearestint→0，abs→sign 且 0 处不定、格式转换→保守），并在文档注释说明。taylor.rs 增加"表达式含非光滑函数→回退 naive"的判定与测试。

**测试**：tests/expr.rs、print.rs、eval.rs、interval_eval.rs、diff.rs、taylor.rs、faithful.rs 各追加四类（新函数求值/打印/区间包含/导数/非光滑回退）；集成 tests/integration_engine.rs 追加新函数管线。**非光滑 Taylor 回退必须有专门包含性测试**（跨整数/半整数/零/舍入边界）。既有断言不弱化。

**门禁**：`cargo test --workspace`（基线零丢失+新增）、clippy workspace -D warnings EXIT=0、只读 rustfmt --check。写作用域 crates/rhytido-core/**（如确需 backend 加法，单列报告由 lead 裁决）。

---

## E3 · M2 验收报告（docs，小任务，lead 或独立 subagent；依赖 E1+E2）

对照 03-roadmap M2 三判据，写 `docs/plan/M2-acceptance.md`：
1. **8 态证书正确**：可达性/蕴含/误差界属性测试汇总（引 integration_engine、faithful 测试与计数）。
2. **函数族差分**：公开参考值证据（6 erf 常数 + 已知 binary64 正确舍入表 + Simpson oracle 交叉 + 新函数往返）。
3. **cutoff 语义**：BelowCutoff/保号零/判定顺序测试汇总。
附：分层计数、三门禁退出码、bench 基线（evaluate_faithful 微基准进 examples/bench.rs 或注明理由）、溯源补 E1/E2 记录、05-testing 与 M2-plan 台账收口、DD/TD 延后 M6 的决策记录。

---

## 收口后

M2 全部验收推送 → 汇报并规划 **M3（语言+CLI：lalrpop 解析器 3 模式/动态作用域求值器/match/proc/设置/REPL）**，M3 首个任务单沿用本工作流（拆分+精确上下文+串行派发+lead 验收+上传）。