# M2 第二阶段任务单（B：erf/erfc；C：faithful 引擎）

> 2026-11-13 ｜ lead 制定。承接 M2-plan.md §3；基线：workspace 271 测试全绿，HEAD=8fd0ec6 已推送。

## 0. 执行策略

- **串行而非并行**：B（backend）与 C（core）虽写作用域不同，但共享同一 target 目录与后端**源码**——core 的门禁会构建 backend；一方编辑中会使另一方门禁出现假失败。故 B → 复核 → 本地提交 → C → 复核 → 本地提交 → 集中门禁 → 推送 → 远端核验。
- 每个 subagent 均须先读 AGENTS.md、05-testing.md 及本任务单对应节；禁止 git 操作、禁止 cargo fmt（lead 集中）、必须 `--offline`、四类测试齐备、既有测试与 doctest 不得弱化。
- lead 复核 = 独立重跑分层计数 + fmt/clippy/test 三门禁 + 抽查关键实现，全部符合才采纳。

## 任务 B（subagent；scope：crates/rhytido-backend/**）

实现 erf/erfc，算法严格遵循 docs/research/erf-erfc-design.md（幂级数 + 几何余项界 + 大参数阈值 + 工作精度 P = p + 3·ceil(x²) + guard）。

API（新增，全部不改既有函数）：
- `src/erf.rs` 新模块（lib.rs 声明）：私有级数/余项/阈值/√π 界内核 + 单元测试。
- `BigFloat::erf()/erfc() -> Self`：在操作数工作精度（0→DEFAULT）取最近舍入；特殊值 NaN→NaN、erf(±∞)=±1、erfc(+∞)=0、erfc(−∞)=2、erf(±0)=±0（奇对称保号）。
- `BigFloat::erf_checked()/erfc_checked() -> (Self, Self)`：值 + **严格绝对误差界**（value ± err ⊇ 真值；供 Interval 与未来求值器使用）。误差界的界（π、√π、atan 正确舍入 ≤0.5ulp）须按设计文档推导并注释。
- 阈值：由严格上界 `erfc(x) ≤ e^{−x²}/(x√π)` 判定可证明舍入为 0/1 的大参数，返回精确值（证书就绪）。
- `Interval::erf()`（单调增）/`Interval::erfc()`（单调减）：用 checked 值 + 外扩，保证严格包含。

测试（05-testing.md 三层）：
- 单元：src/erf.rs 内私有内核（级数递推、项比单调降条件 n>x²、余项界、阈值判定）。
- 模块：新文件 `tests/erf.rs` 四类齐备——公开数学常数交叉断言 ≤15 位有效数字：erf(1)=0.842700792949715、erf(0.5)=0.520499877813047、erf(2)=0.995322265018953、erf(3)=0.999977909503001、erf(4)=0.999999984582742、erfc(1)=0.157299207050285（若断言不符：**调查算法而非改常数**；若确认是转写错误，须给公开来源引用并在报告中说明）；性质：erf(−x)=−erf(x)、erf+erfc=1（界内）、单调采样、特殊值、checked 界自洽（界 < 1ulp@p）；Interval::erf/erfc 密集采样包含性（含阈值邻域、跨零、超窄区间）。
- 门禁：`cargo test -p rhytido-backend`、`cargo test -p rhytido-core`（确认不受扰）、`cargo clippy -p rhytido-backend --all-targets --offline -- -D warnings`。

## 任务 C（subagent；scope：crates/rhytido-core/** + backend bigfloat.rs 仅新增）

构建 faithful 求值引擎（M2-plan §2 已核心理解的 Ziv 语义）。

C1 backend 加法新增（仅此一处越界，须独立测试）：
- `BigFloat::next_above(&self, prec: usize) -> Self` / `next_below(&self, prec: usize) -> Self`：给定精度下严格大于/小于的最小/最大可表示浮点数（经 canonical_parts 精确整数运算，含符号数幅对称、±0、无穷返回自身并文档化）。单元 + 模块测试。

C2 core 新模块：
- `src/interval_eval.rs`：`pub fn eval_interval(it, root, x: &Interval, prec) -> Interval`，记忆化递归；Constant→精确点；Pi→**严格 π 区间**（pi_with_precision(w) ± 半 ulp 界，w>target，参照 trig_range 的严格界做法）；Variable→x；四则/Neg→区间运算；Pow→正底走 exp(y·ln x) 路径，底含非正→保守 NaN；Apply→后端区间函数。最小 hooks：`pub trait IntervalFnHook { fn apply_fn(&self, f: BaseFn, x: &Interval, prec: usize) -> Option<Interval>; }`，eval/faithful 接受 `Option<&dyn IntervalFnHook>`（显式穿线，零全局状态）。
- `src/faithful.rs`：`pub fn evaluate_faithful(it, root, x: &BigFloat, ctx: &Context, cutoff: Option<&BigFloat>, hook: Option<&dyn IntervalFnHook>) -> FpResult`。循环：w₀=max(p+10,…)，上限 512p；每轮 yI=eval_interval(点 x, w)：
  - **Test1（正确舍入）**：nearest(yI.lo@p)==nearest(yI.hi@p) → 该值；点区间且可表示 → Exact(4)，否则 CorrectlyRounded(6)。达 faithful 后"再加一元"：w+10 重试争取 Test1。
  - **Test2（faithful 跨界）**：a=nearest(lo)、b=nearest(hi)、a≠b 且 `a.next_above(p)==b` → 返回 a 或 b（取近者），证书 FaithfulInexact(5)。
  - **Test3（恰含一浮点）**：round_up(yI.lo@p)==round_down(yI.hi@p)=c → c，证书 Faithful(1)。
  - **cutoff**：Some 且 yI ⊆ [−c,c] → BelowCutoff(2)，值=nearest(lo@p)。
  - yI 含 NaN/无界 → NonReal(3)（值=NaN）。
  - 超上限未决 → NotFaithful(0)（值=nearest(lo@p)，文档化保守性）。

测试：模块 tests/next_*（C1）、tests/interval_eval.rs、tests/faithful.rs 四类；集成 tests/integration_engine.rs——
- 已知正确舍入（p=53 十进制 double 值）：sqrt(2)=1.4142135623730951、exp(1)=2.718281828459045、ln(2)=0.6931471805599453、sin(1)=0.8414709848078965、cos(1)=0.5403023058681398、1/3=0.3333333333333333、2/3=0.6666666666666666 等断言 evaluate_faithful 结果 == f64 值的最近 BigFloat，且证书 ∈ {Exact, CR, 6/4}。
- 证书⇒误差界性质：CR/Exact 结果与 p+100 参考求值一致；Faithful 结果距参考 ≤1ulp@p。
- 精确案例：2+3@p=3 → Exact(4)；ties：3/4@p=1 → Test2 命中（结果 ∈{0.5,1.0}，证书 5）。
- cutoff/NonReal/hook 生效/确定性（同输入两次一致）/特殊值×表达式矩阵不 panic 且上限内终止。
- 门禁：`cargo test --workspace --offline` 全绿（基线 271+B 新增零丢失）、`cargo clippy --all-targets --offline -- -D warnings` 零警告。

## 验收与上传

- 每任务完成：lead 独立复核（分层计数、三门禁、抽查关键路径）→ 更新 05-testing.md 台账、M2-plan §0 进度表、写溯源 → `git add` 相应路径 + Conventional Commits 提交。
- 两项均验收后：集中复跑三门禁 + `git push`（22 端口被阻，走已验证的 ssh.github.com:443 + 压缩/保活通道，推后恢复常规 URL）+ `git ls-remote` 核验远端。
- 本阶段后 M2 剩余：D autodiff+Taylor、E Display 往返/BaseFn 扩容/验收报告。