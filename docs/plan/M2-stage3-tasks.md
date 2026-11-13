# M2 阶段三任务单（D：autodiff+Taylor；E：Display 往返 + BaseFn 30 + 验收）

> 2026-11-13 lead 制定 ｜ 前置：C 验收通过并推送后按序派发。**串行理由**：D/E 与 C 共享 core 写作用域与 target 构建状态（§7 调度约束）。基线以派发时点的 lead 实测计数为准（C 完成后确定，非本文件的过时数字）。派发时任务单引用本文件对应节，纪律条款按 AGENTS.md §7。

## 前置核对（lead 在派发 D 前执行）

- grep BaseFn：若 C 已加 Erf/Erfc 变体（其契约中"16 个含 erf/erfc"暗示会加），则 E 的扩容清单相应减项；以磁盘事实为准，不假设。

## 任务 D（subagent；scope：crates/rhytido-core/**，含 context.rs 加性字段）

### D1 autodiff：`src/diff.rs`
- `pub fn differentiate(it: &mut Interner, id: NodeId) -> NodeId`——对 interned DAG 求符号导数（对唯一自由变量 Variable），**记忆化**（HashMap<NodeId,NodeId>）；结果经 intern 共享子树。
- 规则：Constant/Pi→0；Variable→1；Add/Sub/Neg 线性；Mul 积法则；Div 商法则；Pow：指数为 Constant→幂法则 `y·x^(y−1)`，底为 Constant→`c^g·ln(c)·g'`，一般→`x^y·(g'·ln x + y·x'/x)`；Apply(f,u)→链式法则 × 导数表（覆盖当前 BaseFn **全部变体**：sqrt、exp、ln、sin/cos/tan、asin/acos/atan、双曲/反双曲、erf/erfc（若已在 BaseFn））。
- 不做符号化简（autosimplify 属 M4，文档注明表达式可能膨胀）。
- 测试：known-derivative 数值交叉核对（导数表达式 eval_point 与 eval_point 的**中心差商**在多点上以容差一致，容差由精度论证写注释）；链式复合、积/商、常数→零、共享子树缓存命中处；四类命名齐全，放 tests/diff.rs（模块层）+ 集成（diff 接入引擎路径：见 D2）。

### D2 Taylor 精化（中值包围，保证严格包含）
- Context 加性字段 `taylor_recursions: u32`（默认 0；builder `with_taylor_recursions`；语义对齐公开行为：规则**至少应用一次**，k=0→一次，k→k+1 次）；tests/context.rs 追加四类。
- `src/taylor.rs`：`eval_taylor(it, id, xI, prec, k, hook)`——`f(xI) ⊆ f(m) + f'(xI)·(xI−m)`，m=区间中点；f(m) 在 guard 精度 nearest 计算并**包裹其舍入容差成区间**（ inclusion 关键，注释推导）；f'(xI) 用 differentiate + 区间求值（k>0 时对导数递归同一规则）。
- 引擎集成（faithful.rs 加性改造）：当表达式含 Variable 且 naive 与 taylor 均为有界包含时取**交**（两者都严格包含真值⇒交仍包含，注释论证），用于收窄依赖和声（示例：`x*x − (x−1)*(x+1) − 1` 恒为 0，naive 宽、Taylor 紧）。
- 测试：包含性属性（密集采样含依赖病态表达式）、交不破坏既有已知正确舍入断言、k 语义（0/1/2 宽度单调不增于同一样例——仅对可单调比较样例断言，注释说明保守）、确定性、四类齐备；新文件 tests/taylor.rs + faithful 集成追加。

### D 门禁与报告
- 按 §7：分层重计数（基线零丢失+新增）、clippy workspace -D warnings EXIT=0、只读 rustfmt --check；证据式报告（git status --short、逐二进制计数、退出码、偏差清单）。护栏同 §7/§1.1（零 unsafe、不碰 reference、无 git 写、不跑 fmt、--offline、严禁计划文本结束回合、探针用后即删）。

## 任务 E（subagent；scope：crates/rhytido-backend/** + crates/rhytido-core/**；在 D 验收后串行）

### E1 Display 往返修复（backend，解决 05-testing.md §4b 既有怪癖）
- `to_decimal` 渲染位数改为**可往返**：p-bit 值至少 `ceil(p·log10(2))+1` 位有效十进制（推导注释；用 to_decimal 后再 with_precision 或等价路径控制位数——先实证 dashu Rounded 行为再定实现，写探针删除）。
- 影响面：更新依赖旧渲染的测试断言（erf doctest、bigfloat Display、integration_numeric 中 ≤15 位限制条款**解除并恢复全值域交叉核对**——这是"未弱化"的反向修正，须注释说明），新增往返属性测试（p∈{11,24,53,64,113,165,300}：value→string→parse→同精度比较恒等；含 2^53−1 用例恢复精确显示）。
- 语义契约不变：NaN/±∞/±0 显示规则不动。

### E2 BaseFn 扩至完整族（core，D 之后事实基线上）
- 变体补齐：log2/log10/log1p/expm1/abs/ceil/floor/nearestint + 格式转换 Double/Single/HalfPrecision/Quad/DoubleExtended（→ 引擎/区间/点求值分派到 backend 既有实现；格式函数打印名沿用 Sollya 公开词：double/single/halfprecision/quad/doubleextended；Ln 打印保持 "ln"，别名 log 属 M3 解析层——注释记录）。
- print.rs 名称表、eval.rs/interval_eval.rs/faithful.rs/taylor.rs/diff.rs 分派穷尽更新；各模块四类测试增补；DD/TD 明确不做（M6 展开算术，台账已有记录）。

### E3 M2 验收报告
- docs/plan/M2-acceptance.md 对照 roadmap M2 三判据：8 态证书正确（可达性/蕴含/误差界属性测试汇总）、函数族差分（公开参考值：6 erf 常数 + 已知 double 正确舍入表 + oracle 交叉）、cutoff 语义；附分层计数、门禁退出码、bench 基线追加（evaluate_faithful 微基准进 examples/bench.rs 或注明理由）；溯源补 diff/taylor/E 三记录；05-testing/M2-plan 台账收口。

## 收口后

M2 全部验收推送 → 汇报并规划 M3（解析器 3 模式/动态作用域/match/proc/REPL），M3 首个任务单沿用本工作流。