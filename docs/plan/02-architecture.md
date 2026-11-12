# 02 · 架构设计

> 草案 v1.0 ｜ 2026-11-12 ｜ 目标：高内聚、低耦合、内存安全、高效；同时支撑字节级 CLI 与语义级兼容；C/C++/Python 绑定。
> 本文以 prose/表格/结构图描述设计，**不含任何代码**。

---

## 1. 架构原则

1. **分层无环依赖**：上层依赖下层，禁止反向/循环依赖。
2. **后端 facade 隔离**：具体大数库（rug/dashu/malachite）只被一个层接触，其余层面向抽象接口 → 低耦合、可切换、可 mock。
3. **语义与表现分离**：数值/语义引擎不依赖输出格式；字节级打印是独立表现层 → 双兼容模式共存。
4. **显式 Context，零全局变量**：设置/符号表/scratch 由 Context 承载并显式穿线 → 可多实例、可测试（改进 Sollya 全局状态导致的单实例限制）。
5. **不可变 + interning 的表达式**：以 hash-consing 取代手工引用计数与缓存失效 → 内存安全、结构共享、缓存永不失效。
6. **保留 Sollya 语义**：动态作用域、error 一等值、符号回退、8 态质量证书、认证降级——这些是行为契约，不可"优化掉"。
7. **unsafe 最小化且封装**：核心全 safe；unsafe 仅在 FFI 边界，集中审计。

## 2. 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│ L7 接口层   CLI(REPL) │ C ABI(cdylib) │ C++ 头 │ Python 绑定    │
├─────────────────────────────────────────────────────────────┤
│ L6 表现/IO  字节级打印器 │ 消息目录 │ 显示模式 │ MathML │ plot   │
├─────────────────────────────────────────────────────────────┤
│ L5 代码生成  C 发射器 │ Gappa IR │ 定理树 │ implementconst 合成 │
├─────────────────────────────────────────────────────────────┤
│ L4 语言     解析器(3模式) │ 动态作用域求值器 │ match │ 帧栈/设置 │
├─────────────────────────────────────────────────────────────┤
│ L3 逼近与认证 多项式DAG │ remez │ fpminimax/LLL │ sturm │      │
│             infnorm/supnorm │ taylor/chebyshev forms          │
├─────────────────────────────────────────────────────────────┤
│ L2 求值引擎  faithful引擎(Ziv+cutoff) │ 32基本函数 │ autodiff │ │
│             求值钩子(hooks)                                    │
├─────────────────────────────────────────────────────────────┤
│ L1 核心值/表达式  AST(interning) │ 值类型 │ 结构哈希 │ 质量证书 │
├─────────────────────────────────────────────────────────────┤
│ L0 数值后端 facade  BigFloat/Rational/Integer/Interval │ 定向  │
│             舍入 │ 格式转换  ← 唯一接触 rug/dashu/malachite    │
└─────────────────────────────────────────────────────────────┘
依赖方向：L7→L6→…→L0（无环）。横向：L4 语言调度 L2/L3；L5 消费 L2/L3。
```

## 3. crate 划分（workspace）

```
rhytido/
├─ crates/
│  ├─ rhytido-backend/    L0 数值后端 facade + 各后端实现(feature 切换)
│  ├─ rhytido-core/       L1 AST/interning/值类型/质量证书/结构哈希
│  ├─ rhytido-eval/       L2 faithful 引擎/基本函数/autodiff/hooks
│  ├─ rhytido-approx/     L3 多项式/remez/fpminimax/sturm/范数/forms
│  ├─ rhytido-lang/       L4 解析器/求值器/match/Context/设置
│  ├─ rhytido-codegen/    L5 C 发射/Gappa IR/定理树/implementconst
│  ├─ rhytido-io/         L6 打印器/消息目录/显示/MathML/plot/external
│  ├─ rhytido-cli/        L7 REPL/信号策略/restart
│  ├─ rhytido-ffi/        L7 C ABI(cdylib)/符号版本/panic 边界
│  └─ rhytido-python/     L7 Python 绑定(PyO3 或 ctypes 包装)
├─ tests/                 移植的脚本语料 + C API 测试翻译 + 属性/基准
├─ docs/                  plan/ research/ adr/
└─ (workspace 清单、CI、许可证与 NOTICE)
```

**职责与依赖规则**

| crate | 单一职责 | 允许依赖 |
|---|---|---|
| backend | 任意精度/区间/精确算术的统一抽象与实现 | 仅第三方大数库 + std |
| core | 表达式与值的不可变表示、interning、证书 | backend |
| eval | 求值与微分（语义） | core, backend |
| approx | 逼近与认证算法 | eval, core, backend |
| lang | 语言解析与调度 | eval, approx, core |
| codegen | 代码与证明生成 | approx, eval, core |
| io | 表现与外设 | core, lang(对象), eval |
| cli | 交互入口 | lang, io |
| ffi | C ABI 导出 | lang, eval, approx, io, core |
| python | Python 绑定 | ffi 或直接 lang/eval |

> 内聚：每 crate 一个清晰职责。耦合：仅向下依赖；backend 是唯一"库耦合点"；io 与引擎分离。

## 4. 数值后端 facade（统一封装，L0）

**目的**：把"可信、成熟、健壮的现成 Rust 库"统一封装为一组稳定接口，使引擎与具体库解耦，许可证/平台/性能权衡可逆。

**抽象对象**（以 prose 描述接口契约，非代码）：
- **BigFloat**：任意精度浮点；构造/精度管理；加减乘除、幂、根；初等函数族（exp/log/三角/双曲/erf 等）；**定向舍入**（Nearest/Zero/Up/Down/AwayZero）；每次运算返回**三元值/Ordering**（供质量证书）；NaN/±∞/±0；十进制/二进制/十六进制字符串转换（**字节级保真关键**）。
- **Integer / Rational**：精确整数/有理数；gcd、扩展欧几里得、精确比较与转换。
- **Interval**：多精度区间，端点为 BigFloat； outward 舍入；**NaN/空集/全域/±∞** 语义；区间初等函数；装饰（可选 IEEE 1788 风格）。
- **Format**：D/SG/HP/QD/DD/TD/DE 格式舍入与展开（TwoSum/FMA 重正化）。

**实现策略**：
- 以 **feature flag** 选择后端：`backend-rug`（默认，最成熟、MPFR 函数最全、字节级最易）/ `backend-dashu`（MIT 纯 Rust、MSVC 友好）/ `backend-malachite`（备选）。
- **rug 路径**：动态链接 LGPL 的 GMP/MPFR（合规，见 04）；`unsafe` 仅限 sys 调用封装。
- **dashu 路径**：纯 safe；需补 **NaN 包装层**与部分缺失函数；字节级数字生成需适配验证。
- **区间层**：无论哪个后端都**自研**（inari 仅 f64，不能替代）；基于后端 BigFloat 的定向舍入构建。
- **可测试性**：facade 为 trait，可注入 mock/参考实现做差分测试。

**M0 决策点**：原型对比 rug vs dashu 的（a）字节级数字保真、（b）函数覆盖、（c）性能、（d）Windows MSVC 构建摩擦，据此定默认后端。facade 保证该决策可逆。

## 5. 核心类型设计（L1，prose 描述）

- **表达式 AST**：约 250 种节点，按子枚举分组（数学表达式 / 命令 / 字面量 / 模式 / 控制流），避免单一巨型 match 失控。
- **hash-consing / interning**：节点不可变，结构相同 → 同一 `NodeId`（arena 或 generational index）。收益：结构相等=id 比较；结构谓词（hash/树大小/是否纯树/是否常数）在 intern 时计算；**缓存永不失效**；**消除 Sollya 的引用环**（缓存自引用问题）。代价：Sollya 的原地列表修改改为重建（行为等价）。
- **缓存**：求值/导数/化简/Horner 等缓存放 `NodeId` 键控侧表（而非节点内嵌），与不可变节点解耦。
- **值类型**：常数(BigFloat)、区间、整数、有理、列表、结构体、字符串、布尔、范围、过程、**error 一等值**。
- **质量证书 FpResult**：8 态枚举，直接映射 Sollya 返回码 0–7。
- **Context**：设置（prec/points/verbosity/显示模式…）+ 符号表帧栈（`Vec<Frame>`，动态作用域）+ scratch 池 + 消息抑制集。显式穿线，支持多实例。
- **消息目录**：487 条编号消息的枚举/常量表 + 精确措辞模板；verbosity 分级与抑制。

## 6. 内存安全策略

- 核心 **全 safe Rust**；所有权 + 借用 + arena/interning 管理生命周期，**无手工引用计数、无全局回收链表**。
- `unsafe` 仅出现在：后端 sys FFI、C ABI 边界、（必要时）性能热点；每处封装在最小模块并文档化不变量，集中审计。
- 动态作用域帧栈用所有权管理，退出作用域自动释放（取代 Sollya 手工 popFrame + 泄漏风险）。
- 信号/崩溃恢复：库模式不装信号处理器；CLI 逻辑错误用 Result/panic 边界；段错误级恢复可选 worker 进程隔离（Sollya 的 longjmp 不可安全平移）。
- 验证：unsafe 审计清单 + Miri（可行处）+ 模糊测试。

## 7. 性能策略

- **scratch/workspace 复用**：以显式 `&mut Workspace` 或 thread-local 承载临时大数，避免热循环反复分配（对应 Sollya 回收池/scratch，但安全）。
- **interning 结构共享**：减少重复子树分配与计算。
- **热路径免分配**：求值/矩阵构造循环预分配。
- **基准驱动**：M0 建立基准 harness，对标 Sollya 关键算子；先正确后快，避免过早优化。
- **后端选择**：rug（GMP/MPFR 原生性能）为性能基线；纯 Rust 后端以基准评估差距。

## 8. 兼容性架构（双模式）

```
        ┌────────────── 语义引擎（L1–L5）──────────────┐
输入 →  │ 解析 → 求值/逼近/认证 → 语义结果(值+证书)      │
        └───────────────┬──────────────────────────────┘
                        │ 语义结果（与表现无关）
            ┌───────────┴───────────┐
            ▼                       ▼
   语义级输出适配器          字节级表现层（L6）
   (归一化/结构化结果)       复刻打印器+487消息+显示模式
            │                       │
            ▼                       ▼
     语义比对测试              逐字节 diff 语料
```

- **语义级**：引擎产出结构化语义结果；适配器供测试/绑定比对。
- **字节级**：表现层把语义结果渲染为与 Sollya 逐字节一致的 stdout（数字生成、消息措辞/顺序、显示模式）。**倾向 rug 后端**复现同一 `mpfr_get_str`。
- 两模式共享同一语义引擎，表现层可插拔 → 满足"同时支持"。

## 9. 多语言绑定架构（L7）

- **C ABI（rhytido-ffi）**：cdylib；导出 `sollya_lib_*` 同名同签名 surface；链接器版本脚本复刻 `LIBSOLLYA_8.0.0`（只导出该前缀，其余 local）；`extern "C"` 边界**捕获 panic**，禁止 unwind 进入 C；对象以不透明句柄 + 引用计数管理跨语言生命周期。
- **C/C++**：cbindgen 生成 C 头；C++ RAII 封装头（句柄自动释放）。
- **Python（rhytido-python）**：PyO3 原生扩展或 ctypes/cffi over C ABI；提供 pythonsollya 风格 API；GIL 与长计算的处理（释放 GIL）。
- **调度**：外部语言可调用求值/逼近/代码生成；库模式多实例（Context）、不劫持信号。

## 10. 高内聚低耦合分析

| 维度 | 设计落实 |
|---|---|
| 高内聚 | 每 crate 单一职责；facade 聚合所有"库相关"；表现层聚合所有"输出相关" |
| 低耦合 | 仅向下依赖；backend trait 隔离具体库；语义/表现分离；Context 显式注入而非全局 |
| 可替换 | 后端 feature 切换；表现层可插拔；解析器/LLL 经接口隔离 |
| 可测试 | trait 可 mock；语义结果可独立比对；语料 diff |
| 可演进 | 研究增强（TOMS 2025/SOS）作为 approx 层可插拔算法，不动核心 |

## 11. 关键设计决策记录（ADR 摘要）

| # | 决策 | 理由 | 替代/权衡 |
|---|---|---|---|
| ADR-1 | AST 用 interning + 不可变节点 | 消除引用环、缓存永不失效、结构相等 O(1) | Rc+Weak（环风险）；arena 索引（选） |
| ADR-2 | 数值后端 facade + feature 切换 | 低耦合、许可证/平台可逆、可 mock | 直接绑定 rug（耦合高） |
| ADR-3 | 语义引擎与表现层分离 | 同时支持字节级+语义级 | 耦合输出（无法双模式） |
| ADR-4 | 显式 Context，零全局 | 多实例、可测试、库礼仪 | 全局状态（Sollya 式，单实例） |
| ADR-5 | 保留动态作用域/error 一等值/符号回退 | 行为契约，兼容必需 | 改词法作用域（破坏兼容） |
| ADR-6 | 区间层自研 | inari 仅 f64，无 MPFI 绑定 | 绑定 MPFI（LGPL+无维护绑定） |
| ADR-7 | unsafe 仅限 FFI 边界 | 内存安全可审计 | 广泛 unsafe（风险高） |
| ADR-8 | 默认后端 M0 原型定夺（rug 倾向） | 字节级+成熟度 vs 许可纯净 | 直接定死（不可逆） |