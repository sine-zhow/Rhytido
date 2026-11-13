# Rhytido · 正确且高效的 Rust 数值实现技巧（派发参考）

> lead 维护，供 subagent 派发时按任务摘取要点（AGENTS.md §7⑦）。原则：**先正确后快**（05-testing/§4）——严格性不可为性能牺牲；效率优化以基准与剖析为依据，不臆测。净室：本文件是通用 Rust/数值技巧，不含 Sollya 表达。

## A. 数值正确性（不可妥协）

1. **定向舍入构造界**：上界用 `Rounding::Up`、下界用 `Rounding::Down`；nearest 值本身**不能**当界（可能落在真值错误一侧）。界 = 计算值 ⊕ 显式舍入/截断裕量，再定向舍入。
2. **精度管理**：在 `prec + guard` 工作精度计算，**末尾一次性**向外舍入到目标精度；避免每步舍入累积误差。慎用 `precision=0`（unlimited）——dashu 对 precision=0 的某些操作 panic（`precision cannot be 0`）；仅在确需精确算术（如 canonical_parts 整数运算）时用。
3. **精确整数算术**：比较/相邻判定/符号零区分用 `canonical_parts() -> (Integer, exp)`（精度无关、规范化），避免浮点舍入误判；`same_bits` 语义须区分 ±0、NaN 永不证等。
4. **特殊值显式分支**：NaN / ±inf / ±0 / 空区间逐一处理并文档化，不依赖默认传播；定义域外返回 NaN 而非 panic。
5. **包含性契约**：区间/Taylor 结果必须包含真值；中值形式或复合运算产生非有界实数（NaN/空/无界）时**回退 naive**（可去奇点如 0·x⁻¹ 跨零）。交集成仅收窄、空交回退。
6. **测试不弱化**：数学常数字面量用 `std::f64::consts`（值恒等，规避 clippy approx_constant）；高精度参考用 backend 自身计算，不用外部低精度常数迁就。

## B. 效率（正确性确立后）

1. **减少 clone**：`BigFloat`/`Interval` 为任意精度，clone 昂贵。facade 按值取参，**必要时才 clone**；能借用先借用，避免链式无谓克隆。
2. **记忆化**：递归 DAG 遍历用缓存——`diff`/`eval_interval` 按 `(NodeId, prec)`、`contains_variable` 用 `HashSet<NodeId>`；共享子树只算一次。记忆化键须含影响结果的参数（如精度）。
3. **避免重复求值**：暴露中间结果（C-INTERMEDIATE）；循环不变量提到循环外；不在热路径重算已得值。
4. **早停 + 上限**：Ziv/迭代精化一经认证即 `break`；设迭代/项数上限防失控，触顶给保守可行输出并在界中体现。
5. **数据结构匹配**：成员判定 `HashSet`、记忆化 `HashMap`、有序序列 `Vec`；避免 O(n²) 扫描与热循环内分配。
6. **迭代器优先**：集合用迭代器而非索引；`fold`/`map` 表达归约，利于优化且少越界。
7. **整数位运算**：2 的幂用移位/`Integer::pow`，sig 位数用倍增循环而非浮点 log；精确且快。

## C. 本项目已知权衡（优化需谨慎，勿擅自改契约）

- `taylor.rs` 因 `differentiate` 需 `&mut Interner` 而 clone 整个 arena——大 DAG 下昂贵；属公开 API 不可变约束下的已知权衡（M4 可考虑独立导数 arena，需 ADR）。
- facade 按值取参致区间运算多 clone——后端签名约束；改签名属架构决策，升级 lead/人类，不在任务内擅改。
- 优化任何热点前先加基准（examples/bench.rs）量化，改后对比，防退化。

## D. 派发摘取示例

- 数值内核任务（erf/区间）：摘 A1–A5 + B4 + C。
- DAG/符号任务（diff/taylor/eval）：摘 A3/A5 + B1–B3 + C。
- 测试任务：摘 A6 + B（测试自身也要四类齐备、不弱化）。