# 溯源记录：rhytido-core / autodiff + Taylor 精化（M2-D）

- **模块 / crate**：`rhytido-core::diff`、`rhytido-core::taylor`；`context.rs`（taylor_recursions）；`faithful.rs`（naive∩Taylor 加性集成）；`expr.rs`（Interner Clone）
- **里程碑**：M2-D
- **执行 / 日期**：subagent a7320a9c（实现主体，上下文耗尽终止）→ subagent e8489784（收尾接管，含根因修复条款）→ lead 独立验收 / 2026-11-13
- **关联**：docs/plan/M2-stage3-tasks.md 任务 D；docs/review/subagent-retrospective.md #8/#9 与 §3.1

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 数学定义 | 符号微分法则（积/商/链式/幂）、Taylor 中值定理 f(xI)⊆f(m)+f'(xI)(xI−m)（公有领域） | 独立实现 |
| 行为规格 | taylorrecursions 公开语义（k=0 至少一次、k→k+1 次），经 lead 核读写入任务单 | 仅语义 |
| 后端/核心 API | 本仓库 facade（Interval/BigFloat/eval_interval/differentiate/Context） | 组合既有类型 |

## 2. 独立设计说明（lead 抽查确认）

- **diff**：记忆化符号微分，17 变体穷尽导数表（含 erf'=2/√π·e^{−x²}、erfc' 负），不做化简（M4）；结果经 intern 共享。
- **taylor**：中值形式；f(m) 用点区间 eval_interval 在 guard 精度包裹完整舍入容差（包含性关键）；midpoint 钳制于 [lo,hi]；k 递归作用于导数；**逐层保守回退**——中值结果非有界实数时回退同一表达式 naive（修复可去奇点：d/dx x⁰=0·x⁻¹ 跨零区间→NaN，化简属 M4），包含性无条件。
- **faithful 集成**：refine_with_taylor 三条件门（含 Variable/非点输入/naive 有界）；intersect_enclosures 交⊆naive 仅收窄、空交/NaN/非有界回退 naive（舍入伪影防护）、端点平局保 naive 符号零逐位复现；主循环与 one-more-unit 重试轮均接入；NonReal 判定序不变。
- **contains_variable**：HashSet 记忆化 DAG 遍历，重访节点跳过可靠性论证（首访 false 才重访，true 早已短路）。

## 3. 不复制声明

未复制 Sollya 源码/表达；两执行者均声明未读 reference/sollya；源自公开数学 + lead 任务单 + 独立实现。

## 4. 行为验证（lead 独立实测）

- `cargo test --workspace --offline` EXIT=0：**509 全绿**（C 收口 459 零丢失 + M2-D 50：diff 模块 14、taylor 模块 15、faithful 模块 +6、faithful 单元 +7、context 模块 +6、doctest +2）；backend 249 不变、core 260。
- `cargo clippy --workspace --all-targets -D warnings` EXIT=0；`cargo fmt --check` EXIT=0。
- 包含性属性：9 表达式×区间 33 点密集采样（200+ 位真值）；病态 x·x−(x−1)(x+1)−1 恒 0：naive 宽≥8、k=1 宽≤2⁻⁴⁰ 且含 0；prec{8,24,53,120}×k{0,1,2} 矩阵；点/NaN/空/无界逐位回退；确定性 + caller interner 零变异。
- **收尾者实测发现并修复前任真实包含性漏洞**（x²@[−1,1] k=2 三阶导 0·x⁻¹ 跨零→NaN），专项测试钉住——非弱化、乃加强。

## 5. 执行过程如实记录

- a7320a9c：落盘 diff/taylor/context/tests（459→481），因每轮冗长 URL 样板确认吞噬上下文 "ran out of room"（根因见复盘 §3.1 #8）。
- e8489784：带根因修复条款（§1.1 极简响应 + §7⑤⑥）接管，补齐 tests/taylor.rs、faithful 集成、clippy/fmt，发现并修复包含性漏洞，一次通过 lead 验收。**制度修复经实证有效**。

## 6. 复核

- lead 独立复核：已完成（git 取证/分层重计数/三门禁/refine_with_taylor·intersect_enclosures·contains_variable·taylor 回退抽查）。
- 同伴复核 / 法务审查：待。