# 溯源记录：rhytido-core / faithful 引擎 + 区间求值器（M2-C）

- **模块 / crate**：`rhytido-core::faithful`、`rhytido-core::interval_eval`；`rhytido-backend::bigfloat`（next_above/next_below/next_at 加法新增）；expr/print/eval 分派扩展（BaseFn +Erf/Erfc）
- **里程碑**：M2-C
- **执行 / 日期**：subagent 9e1eec2b（实现主体，中途终止）→ subagent 3913e2d0（测试与修正，静默终止）→ lead 亲自收尾（8 处 clippy 值恒等修复、fmt、游离文件清理、三门禁、抽查）/ 2026-11-13
- **关联**：docs/plan/M2-stage2-tasks.md 任务 C；docs/plan/M2-plan.md §2（Ziv 语义）；docs/review/subagent-retrospective.md #6/#7

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 行为规格 | 对参考公开行为（Ziv 迭代/终止判据）的理解，经 lead 核读后写入 M2-plan §2 | 仅语义，未复制表达 |
| 数学定义 | 区间算术、中点/端点舍入判定（公有领域） | Test1/2/3 独立实现 |
| 后端 API | 本仓库 facade（Interval/BigFloat/Quality/FpResult/Context） | 组合既有类型 |

## 2. 独立设计说明（lead 抽查确认）

- **主循环**：w₀=p+10；预算 512·p（saturating）；每轮 eval_interval → certify_round（NonReal→Cutoff→三测试）；faithful 命中后 w+10「再加一元」重试，仅采纳 Test1 升级或 NonReal/Cutoff 结论（第二个 faithful 不采纳——不增强度，注释论证）；预算耗尽 → NotFaithful + nearest(lo) 确定性保守值。
- **certify**：Test1 两端点 nearest 同值 → 点区间且可表示 Exact(4)、点但不可表示 CorrectlyRoundedInexact(7)、有宽度 CorrectlyRounded(6)；Test3 round_up(lo)==round_down(hi) → Faithful(1)；Test2 相邻判定 next_above，**排除零与跨零对**（next_above(0) 为文档化约定非真后继）→ FaithfulInexact(5)。
- **same_bits**：canonical_parts 比较——精度无关、区分符号零、NaN 永不证得（防 ±0 跨零假证书）。
- **cutoff**：幅值语义（取 abs），仅严格正有效；带内冲刷为**保号零**；判定先于三测试、后于 NonReal（无界带内仍 NonReal）。
- **interval_eval**：(NodeId,prec) 记忆化；IntervalFnHook 对象安全 trait 显式穿线（零全局状态），hook 区间按给定采纳仅外扩；Pi → guard+2^(3-w) 裕量定向舍入严格界；Pow 正底 log 路径/负底整数奇偶/域外保守；16 函数全分派（含 erf/erfc）。
- **next_above/next_below**：canonical_parts 精确整数算术；不可表示→定向舍入即后继；可表示→自身指数 ulp 步进含进位折叠；零约定文档化且引擎不依赖。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；两个执行者均声明未读 reference/sollya，lead 抽查未见参考表达痕迹；实现源自 lead 编写的语义规格 + 公开数学 + 独立设计。

## 4. 行为验证（lead 独立实测）

- `cargo test --workspace --offline` EXIT=0：**459 全绿**（B 时点 319 零丢失 + C 新增 140：backend +23 = next_* 单元 6/模块 15/doctest 2；core +117 = 单元 41/faithful 模块 27/interval_eval 模块 24/integration_engine 15/eval+3/expr+2/print+2/doctest 3）。
- `cargo clippy --workspace --all-targets -D warnings` EXIT=0（lead 修复 8 处 approx_constant：以值恒等的 std::f64::consts 替换，断言强度不变）；`cargo fmt --check` EXIT=0。
- 已知正确舍入表（p=53）：sqrt2/exp1/ln2/sin1/cos1/atan1/erf1/1÷3/2÷3 值+证书断言全过；cutoff/NonReal/确定性/8 态可达性/hook 注入管线集成测试全过。

## 5. 执行过程如实记录

- 9e1eec2b：实现主体落盘（faithful/interval_eval/C1/Erf 分派），在修探针改名时以残缺工具调用终止；测试零落盘。lead 取证后清理探针、确认 check 全绿。
- 3913e2d0：接管补齐全部测试与修正（core 单元 9→50 等），但静默终止于门禁修复前，遗留 baseline_test.txt 与 8 处 clippy/fmt 问题。
- lead 按 §7「连续两次失败亲自完成」收尾：值恒等替换 8 处 approx_constant、cargo fmt、删除已核实的游离基线日志、三门禁全绿、判定逻辑抽查。

## 6. 复核

- lead 独立复核：已完成（取证/计数/三门禁/主循环与 certify/same_bits/pi_interval 抽查）。
- 同伴复核 / 法务审查：待。