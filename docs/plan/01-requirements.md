# 01 · 需求规格说明

> 草案 v1.0 ｜ 2026-11-12 ｜ 规格来源：Sollya 公开文档/论文 + 行为测试（净室，不复制源码）
> 优先级：**P0**=核心必须；**P1**=完整 parity；**P2**=已弃用/研究增强（可延后）。

---

## 1. 范围与目标

构建 Rust 数值引擎 Rhytido，覆盖 Sollya 的功能面，满足：内存安全、高效、高内聚低耦合、字节级 CLI + 语义级双兼容、C/C++/Python 绑定。**不复制 Sollya 代码**；以数学规格与行为测试为准。

## 2. 功能需求（按子系统）

### 2.1 数值求值（P0）
- **FR-EVAL-1** 任意精度浮点求值，精度可配置（默认 165 bit，参照 Sollya）。
- **FR-EVAL-2** 每次求值产出**舍入质量证书**，8 态：未达 faithful 但证明区间为实数闭子集 / faithful(精确性未知) / below-cutoff / NaN·Inf 无结论 / faithful+证明精确 / faithful+证明不精确 / correctly-rounded(未知) / correctly-rounded+证明不精确。
- **FR-EVAL-3** faithful 引擎：区间证明 + Ziv 精度递增 + cutoff（绝对误差阈值）+ 逐函数精度/cutoff 传播规则。
- **FR-EVAL-4** 32 个基本函数：sqrt, exp, log/log2/log10/log1p, expm1, sin/cos/tan, asin/acos/atan, sinh/cosh/tanh, asinh/acosh/atanh, erf/erfc, abs, ceil/floor/nearestint，及舍入算子 D/SG/HP/QD/DD/TD/DE。
- **FR-EVAL-5** 常量表达式求值到"sharp"区间；支持 π 等常数。
- **FR-EVAL-6** 自动微分 / Taylor 系数向量（f^(k)(x0)/k!，区间）。

### 2.2 区间与精确算术（P0）
- **FR-NUM-1** 多精度区间算术，端点向外舍入；**NaN / 空集 / 全域 / ±∞** 语义（Sollya mpfi-compat 扩展）。
- **FR-NUM-2** 精确整数 / 有理数算术（GMP 级）。
- **FR-NUM-3** 格式舍入：任意精度+舍入模式（RN/RZ/RU/RD），展开格式 DD/TD，各格式 D/SG/HP/QD/DE；十六进制/二进制 I/O。

### 2.3 多项式逼近（P0）
- **FR-APPROX-1** `remez`：广义 monomial 系统的交换算法；Chebyshev 初始点 + 扰动；Haar 条件监测与多点→单点降级；终止语义（target 不可达返回最后多项式 / satisfying 达成返回 / crash 返回 error）。
- **FR-APPROX-2** `fpminimax`：LLL 格归约求浮点系数 minimax；逐系数格式；精确有理插值矩阵。
- **FR-APPROX-3** `guessdegree`、`interpolate`（8.0 新增）、`bezout`（8.0 新增）。
- **FR-APPROX-4** 多项式表示：稀疏/复合/乘积/幂的 DAG，懒 Horner/规范化。

### 2.4 认证范数与根隔离（P0）
- **FR-NORM-1** `infnorm`/`accurateinfnorm`/`dirtyinfnorm`：高精度（非认证）∞-范数，细分+Newton+精度递增，极点检测。
- **FR-NORM-2** `supnorm`：**认证**上确界范数（绝对/相对模式）；Taylor 模型 + 奇点检测/移除（精确除 (x-x0)^k）+ 正性证明 + 二分；明确错误码。
- **FR-NORM-3** `checkinfnorm`。
- **FR-ROOT-1** `findzeros`/`fpfindzeros`/`dirtyfindzeros`/`numberroots`：Sturm 定理（精确有理）根计数/隔离；认证降级警告语义。

### 2.5 Taylor / Chebyshev 模型（P1）
- **FR-FORM-1** `taylor`/`taylorform`：区间 Taylor 模型（系数+认证余项+域+展开点）。
- **FR-FORM-2** `chebyshevform`：Chebyshev 模型（区间 Chebyshev 系数 + 预计算点/矩阵共享）。
- **FR-FORM-3** `autodiff`。

### 2.6 符号运算（P0）
- **FR-SYM-1** `diff`、`simplify`(safe)/`dirtysimplify`、`horner`、`canonical`、`expand`、`substitute`、`composepolynomials`。
- **FR-SYM-2** `coeff`、`degree`、`numerator`/`denominator`、`subpoly`、`roundcoefficients`、`rationalapprox`、`gcd`/`div`/`mod`。

### 2.7 代码生成与证明（P1）
- **FR-CODEGEN-1** `implementpoly`：多项式 → CRlibm 风格 C 代码（double/DD/TD 展开）+ Gappa 证明文件；精度预算对半、159bit/2^-140 约束、double-rounding 警告。
- **FR-CODEGEN-2** `implementconst`：结构制导合成 MPFR 程序（13 种指令 IR）。
- **FR-CODEGEN-3** 定理树（exprBound/equality/noZero/infnorm）+ Gappa 赋值 IR；证明策略（Taylor/Hôpital/单调/去相关）。

### 2.8 最坏情形（P1）
- **FR-WC-1** `worstcase`（制表者困境搜索）、`searchgal`（Gal 表值）。

### 2.9 语言（P0）
- **FR-LANG-1** 完整 Sollya 脚本语言：表达式、赋值、控制流（if/while/for/for-in）、列表/结构体/区间/字符串、过程 `proc`（**动态作用域**）、变长参数、`match` 运行时模式匹配（含字符串/列表拼接切割）。
- **FR-LANG-2** **error 一等值** + 求值失败**符号回退**。
- **FR-LANG-3** 三套解析模式：交互 REPL / 整文件执行 / 字符串解析。
- **FR-LANG-4** 设置项：prec/points/diam/display/verbosity/canonical/autosimplify/taylorrecursions/hopitalrecursions/timing/fullparentheses/midpointmode/dieonerrormode/rationalmode/roundingwarnings/showmessagenumbers/suppresswarnings。
- **FR-LANG-5** `restart`、`quit`、`execute`/`readfile`、`rename`/`bind`、`time`。

### 2.10 I/O 与显示（P0 字节级 / P1 其余）
- **FR-IO-1** 显示模式：decimal/dyadic/binary/hexadecimal、midpoint、canonical、fullparentheses。
- **FR-IO-2** `print`/`write`/`printfloat`/`printhexa`/`printbinary`/`printexpansion`（含文件重定向）。
- **FR-IO-3** MathML `readxml`/`printxml`（**P2，已弃用**）。
- **FR-IO-4** `plot`/`externalplot`/`asciiplot`（gnuplot，P1）。

### 2.11 外部扩展（P1）
- **FR-EXT-1** `externalproc`/`externaldata`（dlopen + 类型编组）。
- **FR-EXT-2** `library`/`libraryconstant` 绑定。
- **FR-EXT-3** `bashevaluate`/`bashexecute`（子进程）。
- **FR-EXT-4** `annotatefunction`（求值钩子：域上多项式替换 + 误差界）。

### 2.12 消息系统（P0 字节级）
- **FR-MSG-1** **487 条编号消息**目录，措辞/编号原样保留（含怪癖如 216 重号）。
- **FR-MSG-2** verbosity 分级、按编号抑制/解除抑制、消息回调（库模式）。

### 2.13 库 API（P1）
- **FR-API-1** `sollya_lib_*` 风格的对象构造/分解/求值/比较 API（供 C ABI 兼容）。

## 3. 非功能需求

| 编号 | 需求 | 验收要点 |
|---|---|---|
| NFR-1 | **内存安全** | 核心全 safe Rust；`unsafe` 仅限 FFI 边界并封装审计；无手工引用计数；Miri/审计覆盖 unsafe |
| NFR-2 | **性能** | 热路径免分配、scratch 复用、interning；基准对标 Sollya（先正确后快） |
| NFR-3 | **高内聚低耦合** | 分层无环依赖；后端 facade 隔离具体库；语义/表现分离；模块职责单一 |
| NFR-4 | **兼容性** | 字节级 CLI（diff 语料）+ 语义级（结果/语义比对）双模式 |
| NFR-5 | **多语言** | C ABI 稳定；C/C++ 头；Python 绑定 |
| NFR-6 | **可移植性** | Windows(MSVC 本机)/Linux/macOS；CI 矩阵 |
| NFR-7 | **可靠性** | 认证结果可信；失败优雅降级（error 值/明确错误码），不崩溃 |
| NFR-8 | **可测试性** | 语料 diff + 语义/属性测试 + 基准；trait 接口可 mock |
| NFR-9 | **可维护性** | 清晰架构 + 文档 + ADR；净室溯源记录 |

## 4. 兼容性需求（双模式）

- **CR-1 语义级**：数学结果与语义一致——动态作用域、error 一等值、符号回退、8 态质量证书、认证界、认证降级警告含义。验证：解析/归一化结果比对。
- **CR-2 字节级 CLI**：同一脚本 → **stdout 逐字节相同**（精确数字 `mpfr_get_str`、487 消息措辞/顺序/条数、显示模式、提示符）。验证：现成 33 脚本语料 `diff`。须复现影响输出的怪癖/bug（先复现、后修复并更新 reference）。
- **CR-3 模式切换**：表现层可配置；语义引擎不依赖表现层。

## 5. 多语言绑定需求

- **BIND-1 C ABI**：cdylib 导出 `sollya_lib_*` 同名同签名 surface；符号版本脚本复刻 `LIBSOLLYA_8.0.0`；边界捕获 panic（不让 unwind 进入 C）。
- **BIND-2 C/C++**：C 头文件（cbindgen 生成）+ C++ RAII 封装头。
- **BIND-3 Python**：ctypes/cffi  over C ABI，或 PyO3 原生扩展；提供 pythonsollya 风格接口。
- **BIND-4 调度**：支持外部语言调用引擎求值/逼近/代码生成；库模式不劫持宿主信号、支持多实例（Context 化）。

## 6. 验收准则

- 每里程碑跑对应测试子集（见 03 文档）。
- 字节级：33 脚本语料 `diff` 全绿（允许 MAYFAIL 计时类）。
- 语义级：349 C API 测试翻译子集 + 属性测试通过。
- 认证：supnorm/infnorm 结果与证书正确；正性证明可复核。
- 安全：unsafe 审计 + Miri（可行处）；无内存泄漏（所有权保证）。
- 性能：关键算子基准不显著劣于 Sollya（目标量化在 M0 设定）。

## 7. 范围分级

- **P0 核心**：数值求值/区间/精确算术、remez/fpminimax、infnorm/supnorm、根隔离、符号运算、语言、显示+消息（字节级）。
- **P1 完整**：Taylor/Chebyshev forms、代码生成+证明、最坏情形、MathML/plot、外部扩展、库 API、绑定。
- **P2 延后**：已弃用功能（readxml/printxml、infnorm 证明生成）；研究增强（TOMS 2025 联合优化、SOS 正性证明、性能极致）。

## 8. 需求追踪（节选）

| 需求 | Sollya 对应模块/命令 | 里程碑 |
|---|---|---|
| FR-EVAL-* | base-functions / infnorm(faithful) / autodiff | M2 |
| FR-NUM-* | mpfi-compat / double / polynomials | M0 |
| FR-APPROX-* | remez / fpminimax / polynomials | M4–M5 |
| FR-NORM/ROOT-* | infnorm / supnorm / sturm | M4–M5 |
| FR-LANG-* | parser/lexer / execute / assignment / match | M3 |
| FR-IO/MSG-* | printf / expression(print) / sollya-messages | M1 |
| FR-CODEGEN-* | implement / implementconst / proof | M6 |
| BIND-* | sollya.h / sollya.map / library | M7 |