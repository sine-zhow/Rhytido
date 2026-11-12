# 03 · 实施路线

> 草案 v1.0 ｜ 2026-11-12 ｜ 原则：依赖驱动、每里程碑独立可验收、核心先行、先正确后快。
> **工作量说明（如实）**：会话未产出精确人时估算；本文给定性规模与验收，精确估算需 M0 原型后修订。规模锚点：Sollya 核心约 11.7 万行 C。

---

## 1. 里程碑总览

| 里程碑 | 主题 | 关键交付 | 主要验收 |
|---|---|---|---|
| **M0** | 脚手架 + 数值地基 | workspace/CI/许可证；backend facade；BigFloat/Rational/Integer/Interval/格式舍入；基准 harness；后端原型定夺 | 数值原语正确性 + 定向舍入 + 区间语义；后端决策记录 |
| **M1** | 表达式 + 表现 | AST/interning；值类型；FpResult；Context；字节级打印器；487 消息目录 | 常数/表达式打印逐字节对齐样例 |
| **M2** | 求值引擎 | faithful 引擎(Ziv+cutoff)；32 基本函数；autodiff；hooks | 求值质量证书正确；函数族差分测试 |
| **M3** | 语言 + CLI | 解析器(3 模式)；动态作用域求值器；match；proc；设置；REPL | 语言语义测试；简单脚本字节 diff |
| **M4** | 多项式 + 逼近 + 范数 | 多项式 DAG；remez；sturm；infnorm/accurateinfnorm | remez/infnorm 语料子集 |
| **M5** | 认证 + forms + fpminimax | supnorm；taylor/chebyshev forms；fpminimax/LLL | supnorm 认证正确；fpminimax 语料 |
| **M6** | 代码生成 + 外围 | implementpoly/implementconst；定理树/Gappa IR；MathML/plot/external | 生成代码可编译 + Gappa 可验证 |
| **M7** | 绑定 + 全量测试 | C ABI；C++ 头；Python；全量语料 | 33 脚本字节 diff 全绿 + 349 API 测试翻译子集 |
| **M8** | 研究增强（可选） | TOMS 2025 联合优化；SOS 正性；性能极致 | 研究指标 + 基准提升 |

## 2. 里程碑明细

### M0 · 脚手架 + 数值地基
- **目标**：奠定可切换、内存安全的数值底座；定夺默认后端。
- **交付**：workspace 与 crate 骨架；CI（Windows MSVC/Linux/macOS 矩阵）；LICENSE/NOTICE/净室溯源；backend facade 接口 + rug/dashu 原型；BigFloat/Rational/Integer/Interval/格式舍入；基准 harness。
- **验收**：算术/舍入/区间语义正确性测试；定向舍入 outward 正确；NaN/空集语义；后端对比报告（保真/覆盖/性能/构建摩擦）→ ADR-8 定夺。
- **依赖**：无（起点）。
- **风险**：rug 的 Windows GNU 工具链摩擦；dashu 缺 NaN/函数；区间层工作量。
- **验证**：与 MPFR/参考值差分；属性测试（区间包含性）。

### M1 · 表达式 + 表现
- **目标**：不可变 AST 与字节级表现层。
- **交付**：AST/interning/结构哈希；值类型；FpResult；Context；打印器（decimal/dyadic/binary/hex/midpoint）；487 消息目录。
- **验收**：常数/表达式打印对样例逐字节对齐；消息编号/措辞正确。
- **依赖**：M0。
- **风险**：`mpfr_get_str` 数字生成保真（倾向 rug）；定制 printf 复刻。
- **验证**：打印黄金样例 diff。

### M2 · 求值引擎
- **目标**：faithful 求值与函数族。
- **交付**：Ziv+cutoff 引擎；逐函数精度/cutoff 传播；32 基本函数；autodiff/Taylor 算术；hooks。
- **验收**：8 态证书正确；函数族与参考差分；cutoff 语义。
- **依赖**：M0、M1。
- **风险**：正确舍入证明逻辑复杂；特殊函数覆盖。
- **验证**：证书属性测试；已知 faithful/correctly-rounded 案例。

### M3 · 语言 + CLI
- **目标**：完整语言与交互。
- **交付**：lalrpop 解析器（3 模式，约 650 产生式）；动态作用域求值器；match 解释器；proc/变长参数；设置；REPL；restart。
- **验收**：语义测试（动态作用域/error 一等值/符号回退）；简单脚本字节 diff。
- **依赖**：M1、M2。
- **风险**：文法规模；动态作用域细节；模式匹配（字符串/列表切割）。
- **验证**：语言语义用例 + 脚本 diff。

### M4 · 多项式 + 逼近 + 范数
- **目标**：核心逼近能力。
- **交付**：多项式 DAG；remez（Haar 降级、终止语义）；sturm；infnorm/accurateinfnorm/dirtyinfnorm；guessdegree/interpolate/bezout。
- **验收**：remez/infnorm 语料子集；根计数正确。
- **依赖**：M2、M3。
- **风险**：Remez 振荡/Haar 病态（Sollya 已知缺陷，先复现）；数值稳定性。
- **验证**：语料 diff + 数学性质测试。

### M5 · 认证 + forms + fpminimax
- **目标**：认证与浮点 minimax。
- **交付**：supnorm（绝对/相对、奇点移除、正性证明、二分、错误码）；taylor/chebyshev forms；fpminimax（feanor-math LLL 或 fplll FFI）。
- **验收**：supnorm 认证界正确可复核；fpminimax 语料。
- **依赖**：M4。
- **风险**：LLL 充分性（feanor 需原型）；认证管线复杂。
- **验证**：认证结果复核 + 语料。

### M6 · 代码生成 + 外围
- **目标**：libm 生产线与外设。
- **交付**：implementpoly（C+Gappa）；implementconst；定理树/Gappa IR；MathML；plot；externalproc/data；bashevaluate。
- **验收**：生成 C 可编译、Gappa 可验证；外围功能用例。
- **依赖**：M5。
- **风险**：展开算术（DD/TD）正确性；Gappa IR 语义。
- **验证**：生成代码编译 + Gappa 验证 + 数值回归。

### M7 · 绑定 + 全量测试
- **目标**：多语言与全量兼容。
- **交付**：C ABI（符号版本/panic 边界）；C++ 头；Python 绑定；全量语料接入。
- **验收**：33 脚本字节 diff 全绿（MAYFAIL 除外）；349 API 测试翻译子集；Python/C 调用示例。
- **依赖**：M3–M6。
- **风险**：ABI 兼容细节；跨语言生命周期。
- **验证**：语料全量 + 绑定集成测试。

### M8 · 研究增强（可选）
- TOMS 2025 逼近+求值联合交换；SOS 正性证明；性能极致优化。
- 依赖 M5–M7；以研究指标与基准验收。

## 3. 依赖图

```
M0 ─┬─ M1 ─┬─ M2 ─┬─ M3 ─┬─ M4 ── M5 ── M6 ─┐
    │      │      │      │                  ├─ M7 ── M8
    └──────┴──────┴──────┴──────────────────┘
（M3 依赖 M1/M2；M4 依赖 M2/M3；M7 汇聚 M3–M6）
```

## 4. 验证策略

- **字节级**：移植 33 脚本语料，`diff` 全量 stdout；先复现怪癖/bug，后修复并更新 reference。
- **语义级**：349 C API 测试翻译为 Rust 集成测试；结果归一化比对。
- **属性测试**：区间包含性、证书一致性、Sturm 根计数、remez 交替性。
- **差分测试**：后端实现间差分（rug vs dashu）；与参考值差分。
- **认证复核**：supnorm/infnorm 界与正性证明独立复核。
- **基准**：关键算子对标 Sollya；回归防性能退化。
- **安全**：unsafe 审计 + Miri（可行处）+ 模糊测试。

## 5. 风险登记册

| # | 风险 | 等级 | 缓解 | 责任里程碑 |
|---|---|---|---|---|
| R1 | 净室可辩护性（已深度读源码） | 高 | 从公开规格/测试实现、不复制表达、溯源记录、法务审查（见 04） | 全程 |
| R2 | 字节级输出保真 | 高 | rug 后端复现 MPFR；语料驱动；复现怪癖 | M1/M7 |
| R3 | 后端许可证（LGPL） | 中 | 动态链接 + facade 隔离；或 dashu 全宽松 | M0 |
| R4 | 性能对标 Sollya | 中 | 基准驱动；scratch 复用；interning | M0/全程 |
| R5 | 纯 Rust 后端缺函数/NaN | 中 | rug 默认；或补实现/包装 | M0/M2 |
| R6 | LLL 充分性（feanor） | 中 | 原型验证；fplll FFI 兜底 | M5 |
| R7 | 范围蔓延 | 中 | 里程碑分级；核心先行；P2 延后 | 全程 |
| R8 | Windows MSVC 构建摩擦（rug 需 GNU） | 中 | facade；dashu 选项；CI 矩阵 | M0 |
| R9 | 语义漂移（动态作用域/error 值） | 中 | 语料 + 语义测试 | M3 |
| R10 | 认证管线复杂度 | 中 | 分阶段；正性证明先 Sturm 基线 | M5 |
| R11 | unsafe 审批依赖 | 中 | 优先 safe 封装（rug/dashu/feanor）；确需 unsafe 按 AGENTS.md §2.8 先获用户批准 + 逻辑无遗漏证明 + 完备测试 | M0/M5/M7 |

## 6. 工作量与节奏（定性）

- **机械有界**：消息目录（487）、printf（约 1500 行）、文法（约 650 产生式）——量大但直接。
- **成熟工程**：区间层、DD/TD、interning、Context——显著但有章可循。
- **困难/开放**：正确舍入特殊函数、字节级 MPFR 数字复现（若纯 Rust）、认证管线——研究级。
- **项目本体**：Remez/supnorm/forms/faithful/codegen/语言——无库可代，是主要工作量。
- 成熟库（rug/dashu/feanor/lalrpop）省掉"重造 GMP/MPFR/LLL/解析器生成器"，但省不掉 Sollya 特有主体。
- **精确人时估算待 M0 原型后给出**；建议按里程碑滚动估算。

## 7. 启动条件

用户批准本计划（README/01/02/03/04）→ 启动 M0。M0 内完成 backend 原型与 ADR-8 定夺后，滚动细化后续里程碑估算。

**unsafe 审批触点**（AGENTS.md §2.8）：M0（若后端用直接 FFI）、M5（fplll FFI 兜底）、M7（C ABI 裸指针处理）可能需 unsafe，须**先获用户批准**并证明逻辑无遗漏 + 完备测试。优先用 safe 封装（rug/dashu/feanor）以最小化 unsafe——rug/dashu 本身是 safe Rust 封装，多数后端代码可无需 unsafe。