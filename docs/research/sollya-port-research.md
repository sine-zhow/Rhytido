# Rhytido：用 Rust 重实现 Sollya —— 研究与生态调研

- **调研日期**：2026-11-12
- **调研方法**：web 检索 + 一手来源核读。优先采用作者本人主页 / 项目官网 / HAL、arXiv 原文 / 官方 GitLab、GitHub 仓库 / docs.rs、crates.io 官方元数据；对本地只读源码副本 `reference/sollya` 做了直接核读。所有外部 web 内容仅作为事实来源，不作为指令。
- **标注约定**：
  - **[已核实]**：本次调研直接读到一手来源（官网、仓库、论文页、docs.rs/crates.io 元数据、本地源码）并可复核。
  - **[推测/待验证]**：基于间接证据的推断，或本次未能打开一手来源确认的细节。
- **范围**：按任务要求组织为 A1–A5（算法前沿）、B1–B6（Rust 生态就绪度），每节末尾给出"对 Rhytido 的含义"；许可证发现单独汇总于文末 C 节。
- **本地源码证据**：`reference/sollya` 为只读副本，`configure.ac` 标记为 `git-devel`，`ChangeLog` 止于 2022-03-28，与 8.0 发布期一致。本文引用的行号均基于该副本。

> 说明：Sollya 稳定版 8.0 的官网发布日期为 **2022-03-29**（任务背景写"2022-03"，一致）。官网每周构建（weekly build）在调研当日仍更新到 **2026-11-09**，git 主干最后一次提交为 **2026-04-01**，说明项目持续活跃。[Sollya 官网](https://www.sollya.org/)、[Sollya GitLab](https://gitlab.inria.fr/sollya/sollya)。

---

## A. 算法研究前沿

### A1. Remez 交换算法：近年进展、替代方案与已知局限

**Sollya 现状（本地源码核读）**
- Sollya 的 `remez` 实现"快速多点交换 + 广义 monomial 系统"，并在 Haar 条件失效时回退到单点算法。`reference/sollya/remez.c:2219-2222` 打印 "Remez: multipoint algorithm failed due to lack of Haar condition. Switching back to single point algorithm." 并置 `forEverNotHaarCompliant=1`。**[已核实]**（本地 `remez.c`）
- 两处自认的临时缺陷，注释均为 "temporary check until I patch the algorithm in order to handle correctly cases when the error oscillates too much"：
  - `reference/sollya/remez.c:2150`：位于 `qualityOfError` 返回 `crash==-1` 的分支，改用**未认证**的 `uncertifiedInfnorm` 报告误差后 `return makeError()`。
  - `reference/sollya/remez.c:2238`：位于主循环后 `if (!res)` 分支，用 `uncertifiedInfnorm` + `evaluateFaithfulWithCutOffFast` 复核是否达到 satisfying error。
  - 二者共同点：当误差函数振荡过多（极值点过多）时，算法无法给出认证结论，只能退化为非认证估计。**[已核实]**（本地 `remez.c`）

**重心插值 / Chebyshev 基方向（一手论文）**
- Pachón & Trefethen, *Barycentric-Remez algorithms for best polynomial approximation in the chebfun system*, **BIT 49(4):721–741, 2009**。这是 Chebfun 中重心形式 Remez 的奠基工作，用重心插值表示替代直接求解病态 Vandermonde 系统，显著提升大度数下的数值稳定性。**[已核实]**（[zbMATH Open 记录 05661807](https://zbmath.org/05661807)，经 [OAI 元数据](https://oai.zbmath.org/v1/?verb=GetRecord&metadataPrefix=oai_dc&identifier=oai:zbmath.org:5661807)核读作者/年份/卷期）
- Filip, Nakatsukasa, Trefethen & Beckermann, *Rational minimax approximation via adaptive barycentric representations*, [arXiv:1705.10132](https://arxiv.org/abs/1705.10132)（v1 2017-05，v3 2018-05）。提出自适应选取支撑点的有理重心表示，给出三种变体（经典 Remez、AAA–Lawson 迭代加权最小二乘、微分修正），Chebfun `MINIMAX` 采用"先 AAA–Lawson 后 Remez"以获得二次收敛；可在标准 16 位浮点下算到 |x| 的 (80,80) 型逼近（此前 Varga–Ruttan–Carpenter 需 200 位扩展精度）。**[已核实]**（arXiv 摘要页）
  - 含义：对"大度数 / 近奇点"情形，重心表示 + 自适应支撑点是比朴素多项式基更稳健的路线；但它主要面向**实系数**多项式/有理 minimax，并不直接处理 Sollya 的"浮点系数 + 广义 monomial + 认证"约束。

**同时优化逼近误差与有限精度求值误差（最新、与 Sollya 直接相关）**
- Arzelier, Bréhard, Hubrecht & Joldes, *An Exchange Algorithm for Optimizing both Approximation and Finite-Precision Evaluation Errors in Polynomial Approximations*, **ACM TOMS, 2025（in press）**, DOI [10.1145/3770066](https://doi.org/10.1145/3770066)，HAL [hal-04709615](https://hal.science/hal-04709615)（v1 2024-09，v2 2025-11）。把"系数舍入误差 + 求值舍入误差"之和的上确界范数建成**半无限线性优化**问题，用一个可视为 Remez 扩展的迭代交换算法求解；给出**基于 Sollya 库的开源 C 实现**，并与 Sollya 现有 routines（含 fpminimax）对比。**[已核实]**（HAL 页含作者、摘要、DOI）
  - 这是任务所问"Remez/fpminimax 之后改进"的最直接一手证据：它不再把求值误差当作事后 ad-hoc 处理，而是与逼近误差联合优化。作者之一 Joldes 即 Sollya 作者。

**已知局限（与 Sollya 源码对应）**
- Haar 条件失效（广义 monomial 系统不构成 Chebyshev 系时）→ 多点交换退化。Sollya 用单点回退兜底。**[已核实]**（本地 `remez.c`）
- 误差振荡过多（极值点数超过预期）→ 无法认证，落入两处 "temporary check"。**[已核实]**（本地 `remez.c`）
- 大度数下直接基（monomial/Vandermonde）病态 → 重心/Chebyshev 基是文献主流缓解手段。**[已核实]**（上述 BIT 2009 / arXiv 2017）

**对 Rhytido 的含义**：Rust 版 Remez 应从一开始就用 Chebyshev/重心表示承载大度数情形，并把"Haar 失效"和"误差振荡过多"设计为**显式、可认证降级**的错误路径（而非 Sollya 式的临时非认证兜底）；同时把 TOMS 2025 的"逼近+求值联合交换"作为 fpminimax 的演进目标纳入路线图。

---

### A2. 认证上确界范数（supnorm）与严格多项式逼近

**Chevillard–Lauter 认证 supnorm 及其后续（一手 HAL）**
- Chevillard & Lauter, *A certified infinite norm for the validation of numerical algorithms*, 研究报告 RR2006-49, LIP, 2006，HAL [ensl-00119810](https://hal.science/ensl-00119810)。提出用区间算术计算 ∞-范数、针对高 cancellation 的相对/绝对误差函数优化、可处理仅由连续延拓定义的点、并保留计算树以生成正确性证明。**[已核实]**（HAL 页）
- Chevillard, Harrison, Joldes & Lauter, *Efficient and accurate computation of upper bounds of approximation errors*, **Theoretical Computer Science, 2011**，HAL [ensl-00445343](https://hal.science/ensl-00445343)（即 Sollya 官网所引 supnorm 文献）。核心：带误差界的中间逼近多项式 + 基于**平方和（sum-of-squares, SOS）**的非负性判定，给出先验质量保证、迈向形式化认证 supnorm；已在 Sollya 中实现。**[已核实]**（HAL 页含作者、摘要、关键词 Taylor Models / SOS）
  - 注意：任务背景把 supnorm 描述为"Taylor 模型 + 奇点移除 + Sturm 正性证明 + 二分"。本地源码确有 `taylorform.c`、`sturm.c`、`supnorm.c`、`chebyshevform.c` 等模块；2011 TCS 论文摘要引入 SOS（平方和）非负性判定作为正性证明路径。**[已核实]**（本地文件列表 + HAL 摘要）
  - **Lead 复核更正**：对当前 git HEAD 全部 .c 检索 "sum of squares / sum-of-squares / sos" **无匹配**；已核读的 supnorm.c `showPositivity`（L326）走 Sturm 零根计数 + 中点 faithful 求值。故 SOS 是论文层技术，**当前源码是否集成 SOS 未获证实**（可能未集成、已移除或命名不同），列为待验证。移植以源码实证的 Sturm 路径为基线，SOS 作为研究增强项。**[已核实]**（本地 grep + supnorm.c 核读）

**Taylor 模型（Berz–Makino 系）与开源实现**
- Taylor 模型 = (n 次 Taylor 多项式 P, 区间余项 Δ)，满足 f(x)−P(x)∈Δ。开源代表 [TaylorModels.jl](https://juliaintervals.github.io/TaylorModels.jl/latest/)（作者 Luis Benet、David P. Sanders，UNAM），组合 IntervalArithmetic.jl 与 TaylorSeries.jl，用于全局优化、range bounding、ODE 验证解、严格求积；文档明确引用 Joldes 博士论文 *Rigorous Polynomial Approximations and Applications*（[tel-00657843](https://tel.archives-ouvertes.fr/tel-00657843), 2011）。**[已核实]**（TaylorModels.jl 官网）
  - Berz–Makino 的 COSY INFINITY / 微分代数方法是 Taylor 模型用于 ODE 验证积分的源头；本次未逐一核读其近年论文原文，列为 **[推测/待验证]** 的具体近年进展。

**Chebyshev 模型 / 插值（Brisebarre–Joldes 等）**
- Brisebarre & Joldes 等, *Chebyshev interpolation polynomial-based tools for rigorous computing*, **ISSAC 2010**, DOI [10.1145/1837934.1837966](https://dl.acm.org/doi/abs/10.1145/1837934.1837966)。用 Chebyshev 插值多项式做严格计算的工具。**[已核实]**（ACM 页，经检索定位；作者细节以 ACM 页为准）
- Benoit, Joldes & Mezzarobba, *Rigorous Uniform Approximation of D-Finite Functions Using Chebyshev Expansions*, **Mathematics of Computation 86(305):1303–1341, 2017**, DOI [10.1090/mcom/3135](http://dx.doi.org/10.1090/mcom/3135)，HAL [hal-01022420](http://hal.archives-ouvertes.fr/hal-01022420)。对 D-finite 函数用 Chebyshev 展开做严格一致逼近。**[已核实]**（Mezzarobba 主页出版物列表）

**Mezzarobba 的严格数值求值（一手主页）**
- Marc Mezzarobba（CNRS，LIX，MAX 团队）主页 [marc.mezzarobba.net](http://marc.mezzarobba.net/) 列出完整出版物与软件：
  - *Truncation Bounds for Differentially Finite Series*, **Annales Henri Lebesgue 2:99–148, 2019**, DOI [10.5802/ahl.17](https://doi.org/10.5802/ahl.17)：D-finite 级数的截断误差界。**[已核实]**
  - *Computing Hypergeometric Functions Rigorously*, **ACM TOMS, 2019**, DOI [10.1145/3328732](https://dl.acm.org/doi/fullHtml/10.1145/3328732)：严格计算超几何函数。**[已核实]**（检索定位 DOI）
  - *Rounding Error Analysis of Linear Recurrences Using Generating Series*, **ETNA 58:196–227, 2023**；*Computing error bounds for asymptotic expansions of regular P-recursive sequences*（与 Dong、Melczer），**Math. Comp. 93:977–1017, 2024**。**[已核实]**（主页）
  - 软件：**NumGfun**（Maple，D-finite 数值/解析计算）**已不再活跃开发**，由 SageMath 的 **ore_algebra-analytic** 取代；NumGfun 以 LGPL v2.1 随 gfun 分发。**[已核实]**（主页"Logiciels"节）
  - Mezzarobba 亦参与 **PythonSollya**（与 Nicolas Brunie 合作的 Cython 接口）。**[已核实]**（主页 + pythonsollya 提交记录）

**对 Rhytido 的含义**：supnorm 是 Sollya 的"安全"卖点，Rust 版必须同时实现 Taylor 模型与 Chebyshev 模型两条严格逼近管线，正性证明以源码实证的 Sturm 路径（零根计数 + 中点求值）为基线，SOS 作为论文启发的可选增强（当前源码未证实集成）；可借鉴 TaylorModels.jl 的 (P,Δ) 抽象与 Mezzarobba 的 D-finite 截断界理论来给出先验质量保证，而不是仅做事后区间估计。

---

### A3. fpminimax 与 LLL：方法演进与 fplll 现状

**LLL minimax 方法（一手 HAL）**
- Brisebarre & Chevillard, *Efficient polynomial L^∞-approximations*, **18th IEEE Symposium on Computer Arithmetic (ARITH 2007), Montpellier, pp.169–176**, DOI [10.1109/ARITH.2007.17](https://doi.org/10.1109/ARITH.2007.17)，HAL [inria-00119513](https://hal.science/inria-00119513)（即 Sollya 官网所引 "floating-point minimax" 文献）。用**格基归约（LLL）+ 最近向量问题（CVP）**求浮点系数多项式逼近，"常能给出最优、多数时候给出非常好的逼近"。**[已核实]**（HAL 页含作者、摘要、关键词 LLL/CVP）
  - 相关期刊版：*Computing machine-efficient polynomial approximations*, **ACM TOMS, 2006**, DOI [10.1145/1141885.1141890](https://dl.acm.org/doi/10.1145/1141885.1141890)。标题/DOI 已核实；**作者署名本次未能打开一手页确认，列为 [推测/待验证]**（dblp 记录键为 `BrisebarreMT06`）。
- **方法之后的改进**：见 A1 所引 Arzelier–Bréhard–Hubrecht–Joldes（TOMS 2025，[hal-04709615](https://hal.science/hal-04709615)）——把求值误差纳入交换优化，直接对标并改进 Sollya fpminimax。**[已核实]**
- 应用侧演进：Volkova & Muller, *Semi-automatic implementation of the complementary error function*, **ARITH 2019, Kyoto**，HAL [hal-02002315](https://hal.science/hal-02002315)。借助 **Metalibm** 生成 erfc 的定制实现，用 double-word 算术在大参数下达到约 50 bit 精度。**[已核实]**（HAL 页）

**fplll 库现状（一手仓库/发行注记）**
- fplll 最新发行版 **5.5.0，发布于 2024-11-12**；上一版 5.4.5（2023-10-17）。维护者含 Martin Albrecht、Shi Bai、Joe Rowell、Ludo Pulles 等。5.5.0 因 PR #526（q-ary 格生成中 q 的选取）**与旧版二进制不兼容**。**[已核实]**（[GitHub releases.atom](https://github.com/fplll/fplll/releases.atom)）
- 许可证 **LGPL-2.1+**，依赖 GMP、MPFR。**[已核实]**（[nixpkgs fplll default.nix, release-25.11](https://github.com/NixOS/nixpkgs/blob/release-25.11/pkgs/development/libraries/fplll/default.nix)）
- Sollya 8.0 的 NEWS 明确把"让 Sollya 重新能配合较新 fplll 编译"列为关键修复，说明 fplll API/ABI 演进对 Sollya 有实际影响。**[已核实]**（本地 `reference/sollya/NEWS`）
- Sollya 通过 C++ 包装 `fplll_wrapper.cpp` 调用 fplll。**[已核实]**（本地文件列表）

**对 Rhytido 的含义**：fpminimax 依赖 LLL/CVP，Rust 侧要么 FFI 到 fplll（LGPL-2.1+，注意 5.5.0 的 ABI 断裂），要么用纯 Rust LLL（见 B3）。算法上应把 TOMS 2025 的联合误差交换作为目标，而非仅复刻 2007 的 LLL 版本。

---

### A4. faithful / 正确舍入求值：Ziv、制表者困境、最坏情形、CORE-MATH

**综述与权威资源页（一手，2026 年仍在更新）**
- Brisebarre, Lauter, Muller 等维护的 *Correct Rounding of Elementary Functions in IEEE754 Formats* 资源页（[reliable-computing.org/correctrounding.html](https://www.reliable-computing.org/correctrounding.html)，页面标注 Last update 2026-07-16）。汇总 Ziv 策略、制表者困境（TMD）、最坏情形搜索、以及 Sollya/Gappa/MPFR/CORE-MATH/RLIBM/BACSEL 等工具。**[已核实]**
- Brisebarre, Hanrot, Muller & Zimmermann, *Correctly-rounded evaluation of a function: why, how, and at what cost?*, **ACM Computing Surveys 58(1), 2026**, DOI [10.1145/3747840](https://dl.acm.org/doi/10.1145/3747840)（资源页列为近年综述）。**[已核实]**（资源页）
- Muller, *The Table Maker's Dilemma and Correctly-Rounded Functions: A Quick Review of 25 Years of Work*, **ARITH 2026 主题演讲（Fulda, 2026-06）**，[幻灯片 PDF](https://perso.ens-lyon.fr/jean-michel.muller/Keynote_ARITH2026.pdf)。**[已核实]**（Muller 主页 + 资源页）

**最坏情形搜索（Lefèvre–Muller 系）**
- 资源页列出 **BACSEL**（[gitlab.inria.fr/zimmerma/bacsel](https://gitlab.inria.fr/zimmerma/bacsel)，作者 Hanrot、Lefèvre、Stehlé、Zimmermann）用于计算舍入最坏情形；并给出 Zimmermann 汇编的 [binary64 全域最坏情形表](https://core-math.gitlabpages.inria.fr/worst.html)（exp/log/sin/cos/tan 等）。**[已核实]**（资源页）
- Lefèvre, Ly & Zimmermann, *Computing hard-to-round Cases of sin, cos, tan in Double Precision*, **ARITH 2026**（[PDF](https://www.arith2026.org/papers/Computing%20hard-to-round%20cases%20of%20sin_2C%20cos_2C%20tan%20in%20double%20precisio.pdf)）。**[已核实]**（资源页）
- 经典 Lefèvre–Muller 最坏情形算法的原始论文本次未逐一核读原文，列为 **[推测/待验证]** 的具体出处；但资源页与 CORE-MATH 参考文献页均指向该脉络。

**CORE-MATH 项目（重点，一手官网）**
- 官网 [core-math.gitlabpages.inria.fr](https://core-math.gitlabpages.inria.fr/)：使命是提供**正确舍入、高性能、开源**的初等函数，可直接并入 glibc、Intel/AMD/Apple/Microsoft libm、Musl、OpenLibm、Newlib、llvm-libc、CUDA/ROCm 等。**[已核实]**
  - **许可证 MIT**。**[已核实]**（官网"Available CORE-MATH functions (under MIT license)"）
  - 已并入 **glibc 2.43** 的 binary64 函数：acosh、asinh、atanh、erf、erfc、lgamma、tgamma；大批 binary32 函数并入 **glibc 2.42**。**[已核实]**（官网）
  - AMD libm（4.2 起）采用其 tanhf；Houdini 22 以 CORE-MATH 为底层数学库。**[已核实]**（官网）
  - 覆盖 float16、bfloat16、binary32/64/80/128；对 binary32 一元函数用**穷举**验证正确舍入，更大格式用已知最坏情形验证。**[已核实]**（官网 Notes）
  - 项目论文：Sibidanov, Zimmermann & Glondu, *The CORE-MATH Project*, **ARITH 2022**，HAL [hal-03721525](https://hal.inria.fr/hal-03721525)。**[已核实]**（CORE-MATH [references 页](https://core-math.gitlabpages.inria.fr/references.html)）
  - **与 Sollya 生态的关系**：资源页把 Sollya 列为"逼近/认证"上游工具，CORE-MATH 是"正确舍入实现"下游；两者互补而非竞争。CORE-MATH 参考文献页还引用 Lauter–Lefèvre 的 pow 舍入边界测试（2009）等 Sollya 作者工作。**[已核实]**（资源页 + references 页）
- **Rust 相关**：资源页列出 [metallic-rs](https://github.com/jdh8/metallic-rs)（Chen-Pang He 开发的 Rust 正确舍入库）。**[已核实]**（资源页列出；其成熟度/许可证本次未核读，列 [推测/待验证]）

**竞争/相邻方案**
- **RLIBM**（Rutgers，Nagarakatte 组）：Lim & Nagarakatte, PLDI 2021（[预印本](https://people.cs.rutgers.edu/~sn349/papers/rlibm32-pldi-2021-preprint.pdf)）、POPL 2022（[DOI](https://dl.acm.org/doi/abs/10.1145/3498664)）、RLibm-MultiRound（[arXiv:2504.07409](https://arxiv.org/abs/2504.07409), 2025）。用单个多项式逼近同时覆盖多表示/多舍入模式。**[已核实]**（CORE-MATH references 页 + 资源页）
- **LLVM-libc**、**CRlibm**（已不再维护，仅存镜像 [github.com/taschini/crlibm](https://github.com/taschini/crlibm)）。**[已核实]**（CORE-MATH 官网"Other correctly-rounded implementations"）
- C23 草案 N3685 已为正确舍入函数保留 `cr_exp`、`cr_log`、`cr_sin` 等名（7.34.9）。**[已核实]**（CORE-MATH references 页）

**专著版本（一手作者主页）**
- Muller 等, *Handbook of Floating-Point Arithmetic*, **第 2 版，Birkhäuser, 2018**（[Springer 9783319765259](https://www.springer.com/la/book/9783319765259)，Muller 主页确认"2nd edition: 2018"）。**[已核实]**（[Muller 主页](https://perso.ens-lyon.fr/jean-michel.muller/)）
- Muller, *Elementary Functions: Algorithms and Implementation*, **第 3 版，2016-12**（[Springer 10.1007/978-1-4899-7983-4](http://link.springer.com/book/10.1007/978-1-4899-7983-4)，Muller 主页确认"3rd edition, December 2016"）。**[已核实]**（Muller 主页）

**对 Rhytido 的含义**：faithful/正确舍入求值是 Sollya 的核心能力，Rust 版应实现 Ziv 迭代 + 可配置舍入质量证书；最坏情形数据可直接复用 CORE-MATH 的公开 .wc 表（MIT）做回归测试。CORE-MATH 本身是"实现端"，Rhytido 定位在"逼近/认证端"，二者可形成上下游协作而非重复造轮子。

---

### A5. Sollya 生态现状：版本、构建、周边项目与维护状态

**Sollya 本体**
- 稳定版 **8.0（2022-03-29）**；官网每周构建快照在调研当日为 **2026-11-09**。**[已核实]**（[sollya.org](https://www.sollya.org/)）
- GitLab 主干最后提交 **2026-04-01**（Chevillard，"Refactoring code"）；2025-07-28 合入 Jerome Benoit 提交的 **C23 兼容**修复（源自 Debian）；2025-02-19 修复 supnorm 假奇点 bug（issue #30152）。仓库约 3268 次提交、15 分支、19 标签。**[已核实]**（[commits atom](https://gitlab.inria.fr/sollya/sollya/-/commits/master?format=atom)、[项目页](https://gitlab.inria.fr/sollya/sollya)）
- 依赖：GMP、MPFR、MPFI、FPLLL、libxml2；gnuplot 可选。**[已核实]**（官网 + 本地 `README`）
- 8.0 新增 `interpolate`、`bezout` 命令；弃用 `readxml`/`printxml` 及 infnorm 的证明生成功能。**[已核实]**（本地 `NEWS`）

**周边项目维护状态**
- **pythonsollya**（[gitlab.com/metalibm-dev/pythonsollya](https://gitlab.com/metalibm-dev/pythonsollya)）：Sollya 的 Python/Cython 封装，**CeCILL v2.1** 许可；约 287 次提交；最后提交 **2026-03-15**（补 bigfloat 来源文件的许可证文本），2025-06-22 修复新版 CPython 的 `PyInt_AsLong`→`PyLong_AsLong`。由 Nicolas Brunie 与 Marc Mezzarobba 等维护。**[已核实]**（项目页 + [commits atom](https://gitlab.com/metalibm-dev/pythonsollya/-/commits/master?format=atom)）
- **Metalibm**：数学函数代码生成器。论文 Kupriianova & Lauter, *Metalibm: A Mathematical Functions Code Generator*, **ICMS 2014, Seoul, pp.713–717**, DOI [10.1007/978-3-662-44199-2_106](https://doi.org/10.1007/978-3-662-44199-2_106)，HAL [hal-01513490](https://hal.science/hal-01513490)；按"范围归约→多项式逼近→重构"方案生成参数化 C 代码。**[已核实]**（HAL 页）。开发组织 [gitlab.com/metalibm-dev](https://gitlab.com/metalibm-dev)（"Metalibm Developpers"）；存在 Kalray 维护的公开镜像/分支 [github.com/kalray/metalibm](https://github.com/kalray/metalibm)。**主仓库当前是否公开可访问本次未能确认（gitlab 项目页返回需登录），列 [推测/待验证]**。
- **Gappa**（[gappa.gitlabpages.inria.fr](https://gappa.gitlabpages.inria.fr/)）：浮点/定点程序性质的自动证明工具，可作 Why3 后端与 Rocq/Coq 策略。最新发行 **1.8.3**；源码 [gitlab.inria.fr/gappa/gappa](https://gitlab.inria.fr/gappa/gappa)。**许可证为 CeCILL v2.1（完整 CeCILL，非 CeCILL-C）+ GNU GPL 双许可**；Coq 支持库为 LGPL。**[已核实]**（官网 + [COPYING 原文](https://gitlab.inria.fr/gappa/gappa/-/raw/master/COPYING)）
  - **更正任务背景**：背景称"Gappa CeCILL-C"，实际为 **CeCILL（v2.1）+ GPL**，与 Sollya 的 CeCILL-C 不同，见 C 节。
- **MPFI**（[gitlab.inria.fr/mpfi/mpfi](https://gitlab.inria.fr/mpfi/mpfi)）：多精度区间算术库。最新标签 **1.5.5（2026-09-27，Fabrice Rouillier）**，上一版 1.5.4（2018-09-06）——说明 2026 年仍有维护动作。**[已核实]**（[tags API](https://gitlab.inria.fr/api/v4/projects/mpfi%2Fmpfi/repository/tags)）
- **FPLLL**：见 A3，5.5.0（2024-11-12），LGPL-2.1+。**[已核实]**

**有无后继/竞争项目**
- 未发现 Sollya 的"直接 Rust 后继"。相邻/下游：CORE-MATH、RLIBM、LLVM-libc（正确舍入实现端，见 A4）；上游/互补：Gappa（证明）、Metalibm（代码生成）、TaylorModels.jl（严格逼近，Julia）。**[已核实]**（各官网；"无直接 Rust 后继"为本次检索范围内的结论，列 [推测/待验证] 的穷尽性）

**对 Rhytido 的含义**：Sollya 生态仍活跃但体量小、依赖 C/C++ 老库（MPFI/fplll/libxml2）。Rhytido 的价值在于用 Rust 提供内存安全、可嵌入、许可证更灵活的替代；应优先保证与 Sollya 脚本/库语义兼容，并把 Gappa 证明输出、Metalibm 式代码生成作为可选后端而非核心耦合。

---

## B. Rust 生态就绪度

### B1. rug / gmp-mpfr-sys：成熟度、维护、API 覆盖、许可证

- **rug**：最新 **1.30.0（2026-04-27）**，作者 Trevor Spiteri（版权 2016–2026），仓库 [gitlab.com/tspiteri/rug](https://gitlab.com/tspiteri/rug)。提供 `Integer`/`Rational`/`Float`/`Complex` 及 `Mini*` 类型，基于 GMP/MPFR/MPC，"正确舍入"。**许可证 LGPL-3.0+**（README 明示 LGPL v3 或更高；docs.rs 标 LGPL-3.0+）。**[已核实]**（[docs.rs rug README](https://docs.rs/crate/rug/latest/source/README.md)、[Round 枚举页](https://docs.rs/rug/latest/rug/float/enum.Round.html)）
  - **定向舍入**：`rug::float::Round` 含 `Nearest / Zero / Up / Down / AwayZero`，配合 `*_round` 方法与 `AssignRound` trait，可对任意精度 `Float` 做向上/向下舍入——这是自建区间层的关键原语。**[已核实]**（Round 枚举页）
  - **MPFR 函数族**：rug `Float` 暴露 exp/ln/三角/双曲等及 `*_prec`/`*_round` 变体（1.29/1.30 仍在为 `Complex` 增补 exp2/exp10/log2 等）。**[已核实]**（README"What's new"）
- **gmp-mpfr-sys**：最新 **1.7.1（2026-04-25）**，LGPL-3.0+，仓库 [gitlab.com/tspiteri/gmp-mpfr-sys](https://gitlab.com/tspiteri/gmp-mpfr-sys)。**内置源码** GMP 6.3.0、MPFR 4.2.2、MPC 1.4.1，从源码构建；MSRV 1.71。**[已核实]**（[docs.rs README](https://docs.rs/crate/gmp-mpfr-sys/latest/source/README.md)、[crates.io 元数据](https://crates.io/api/v1/crates/gmp-mpfr-sys)）
  - **构建约束（对 Windows 重要）**：Linux 需 gcc/m4/make/diffutils；macOS 需命令行工具；**Windows 需 GNU 工具链 + MSYS2（MinGW64），不支持 MSVC**；**构建路径不能含空格**；交叉编译需实验性 `force-cross`。**[已核实]**（README）
- **对 MIT 项目的影响**：rug 与 gmp-mpfr-sys 均为 **LGPL-3.0+**，底层 GMP/MPFR/MPC 亦 LGPL。一个 MIT/Apache 的 Rhytido 若**动态链接**这些 LGPL 库，通常可在满足 LGPL（允许用户替换/重链接该库）的前提下保持自身代码的宽松许可；若**静态链接**则需履行 LGPL 的重链接义务（提供目标文件或源码）。详见 C 节。**[已核实]**（许可证文本）+ 合规推断 **[推测/待验证]**（具体法律结论应咨询法务）。

**对 Rhytido 的含义**：rug 是成熟、活跃、API 覆盖最全的任意精度后端，定向舍入原语齐备，适合作为数值内核；代价是 LGPL 与 Windows/MSVC 构建摩擦。若 Rhytido 坚持 MIT 且要静态分发，需评估 LGPL 合规或改用纯 Rust 后端（B5）。

### B2. 区间算术：inari 及替代、自研区间层可行性

- **inari**：最新 **2.0.0（2024-08-07）**，**MIT**，仓库 [github.com/unageek/inari](https://github.com/unageek/inari)。符合 **IEEE Std 1788.1-2017** 并实现 IEEE 1788-2015 子集；支持 x86-64（Haswell+）与 AArch64；默认 `gmp` feature 依赖 rug/gmp-mpfr-sys（因此 MSVC 不支持）。**[已核实]**（[README](https://static.crates.io/readmes/inari/inari-2.0.0.html)、[crates.io 元数据](https://crates.io/api/v1/crates/inari)）
  - **NaN / 空集语义**：inari 提供 `Interval::EMPTY`、`ENTIRE`，`DecInterval`（带装饰 com/dac/def/trv/ill）与 `is_nai()`（NaI = Not-an-Interval），即**支持空集与 IEEE 1788 的 NaI/装饰语义**；无效输入返回 `Err`。**[已核实]**（[conformance 文档](https://docs.rs/inari/latest/inari/_docs/conformance/index.html)）
  - **关键局限**：inari 的区间端点是 **f64（binary64）**——conformance 问卷明确 `interval!(a,b)` 接收两个 f64。它**不是任意精度区间库**，不能替代 MPFI 承担 Sollya 的多精度区间运算。**[已核实]**（conformance 问卷）
  - **维护状态**：最后发行 2024-08，距调研约 2 年，活跃度低于 rug。**[已核实]**（crates.io 时间戳）
- **其他选择**（crates.io 检索"interval arithmetic"）：`honestintervals`（2017，已陈旧）、`intervals-good`（2022）、`intervals-general`（2024）、`maryada`（2026，no-std binary64、IEEE 1788.1）、`tpt-math-exact`（2026，任意精度有理+区间，较新/小众）、`ferric-certify`（2026，区间+分支定界认证，小众）。**未发现维护中的 MPFI Rust 绑定（mpfi-sys 之类）**。**[已核实]**（[crates.io 检索](https://crates.io/api/v1/crates?q=interval+arithmetic&per_page=10)；"无 MPFI 绑定"为本次检索范围内结论，列 [推测/待验证]）
- **自研区间层（基于 rug::Float 定向舍入）可行性**：rug `Float` 支持任意精度 + `Round::Up/Down`（B1 已核实），因此完全可以实现 MPFI 风格的"端点向外舍入"任意精度区间层：每个端点用 `rug::Float`，下端点 `Round::Down`、上端点 `Round::Up`，初等函数用 MPFR 的定向舍入变体。**技术上可行**，工作量在于覆盖 Sollya 所需的全部区间初等函数与装饰/空集语义。**[推测/待验证]**（可行性为基于已核实原语的推断，未做原型验证）

**对 Rhytido 的含义**：不要指望 inari 直接替代 MPFI（它是 binary64）。推荐**基于 rug::Float 自研任意精度区间层**，把 IEEE 1788 的空集/装饰语义（可参考 inari 的 conformance 设计）纳入；这样既获得多精度能力，又复用 rug 的定向舍入。

### B3. Rust 中的 LLL：fplll 绑定与纯 Rust 实现

- **fplll 绑定**：crates.io 检索 "fplll" **无结果**；检索 "lattice reduction" 未见 `fplll-sys`。**未发现现成、维护中的 fplll Rust 绑定**。**[已核实]**（crates.io 检索；穷尽性列 [推测/待验证]）
- **纯 Rust LLL**：
  - **feanor-math**（最新 3.6.2，2026-10-31，**MIT**，[github.com/FeanorTheElf/feanor-math](https://github.com/FeanorTheElf/feanor-math)）：`algorithms::lll` 模块提供 **`exact::lll()`（任意精度算术）** 与 **`float::lll()` / `lll_quadratic_form()`（浮点）** 两种 LLL 实现，另含 Fincke–Pohst 枚举、有理重构等。**这是目前最可用的纯 Rust、任意精度 LLL 选项**。**[已核实]**（[lll 模块文档](https://docs.rs/feanor-math/latest/feanor_math/algorithms/lll/index.html)、[algorithms 索引](https://docs.rs/feanor-math/latest/feanor_math/algorithms/index.html)）
  - **lll-rs**（0.2.0，2020，[github.com/rust-crypto-labs/lll-rs](https://github.com/rust-crypto-labs/lll-rs)）：实现 LLL 与 L²，但 README 自述"far from feature-complete, should be considered experimental"，且其 `BigVector`/`RationalVector` **依赖 rug::Integer/Rational**（即仍间接依赖 GMP）。README 建议需要稳定库者改用 fplll。**[已核实]**（[README](https://raw.githubusercontent.com/rust-crypto-labs/lll-rs/master/README.md)）
  - **lllreduce**（0.0.2，2019，陈旧）；**g6k-rs**（0.1.0，2026-08，研究导向的格筛法，面向 Metal/CUDA，非 fpminimax 所需的经典 LLL/CVP）。**[已核实]**（crates.io 检索）

**对 Rhytido 的含义**：fpminimax 需要 LLL/CVP。首选评估 **feanor-math 的 `exact::lll`（MIT、纯 Rust、任意精度）**；若性能/功能不足，再 FFI 到 fplll（LGPL-2.1+，注意 5.5.0 ABI 断裂）。lll-rs 因实验性且依赖 rug，不宜作为生产核心。

### B4. 解析器生成器：lalrpop / chumsky / pest 对比

**Sollya 语法规模（本地源码核读，作为选型输入）**
- 主语法 `parser.y`：约 **46 个非终结符规则**、约 **604 条 `|` 备选产生式**（合计约 650 条，grep 近似计数）。**[已核实]**（本地 `parser.y`）
- 另有两套辅助语法：`miniparser.y`（约 361 条备选，用于 `execute()` 字符串解析，见 `execute.c` 的 miniparser 警告）与 `internparser.y`（2266 行，内部表达式解析，`execute.c` 包含 `internparser.h`）。对应三套 lexer：`lexer.l`、`minilexer.l`、`internlexer.l`。**[已核实]**（本地文件 + `execute.c`）
  - **更正任务背景（产生式数）**：背景称"约 250 条产生式"系与节点类型数（约 250，见 execute.h）混淆；实测主语法约 46 个非终结符规则 + 604 条 `|` 备选 ≈ 650 条产生式。**[已核实]**（本地 parser.y grep 计数）
  - **三套语法分工（Lead 复核更正）**：按调用点核读——主语法 yyparse 用于交互式 REPL（general.c:2994）；internyyparse 用于 executeFile 整文件执行（execute.c:392）；miniyyparse 用于 parseStringInternal 字符串解析（execute.c:319，parse() 命令与库 API）。即"交互/文件/字符串"三分成立；本节早先把 mini/intern 分工写反，已更正。**[已核实]**（本地调用点）

**三个候选（crates.io 官方元数据）**
- **lalrpop**：最新 **0.23.2（2026-10-19）**，LR(1) 解析器生成器，[github.com/lalrpop/lalrpop](https://github.com/lalrpop/lalrpop)，下载量约 7900 万。**[已核实]**（[crates.io 检索](https://crates.io/api/v1/crates?q=lalrpop&per_page=3)）
- **chumsky**：稳定版 **0.13.0**、最新 **1.0.0-alpha.8（2026-05-06）**，解析器组合子、强错误恢复，仓库已迁至 [codeberg.org/zesterer/chumsky](https://codeberg.org/zesterer/chumsky)。**[已核实]**（[crates.io 检索](https://crates.io/api/v1/crates?q=chumsky&per_page=3)）
- **pest**：最新 **2.9.6（2026-11-10）**，PEG，**MIT OR Apache-2.0**，[pest.rs](https://pest.rs/)、[github.com/pest-parser/pest](https://github.com/pest-parser/pest)，下载量约 3.9 亿、迭代非常活跃。**[已核实]**（[crates.io 元数据](https://crates.io/api/v1/crates/pest)）

**适配性分析（推断）**
- Sollya 语法是**大型、含表达式优先级与多种命令**的文法，且需三套变体。lalrpop 的 LR(1) + 显式文法文件最贴近原 bison/flex 结构，便于逐条移植约 650 条产生式并处理优先级；pest 的 PEG 写起来简洁但需把左递归/优先级改写为 PEG 习惯，且对"三套共享子文法"的复用需自行组织；chumsky 组合子灵活、错误恢复好，但超大文法的可维护性与编译期开销需评估。**[推测/待验证]**（选型推断，未做原型基准）

**对 Rhytido 的含义**：以"贴近 bison 文法、可逐条移植、能复用子文法"为首要标准，**lalrpop 是最稳妥默认**；pest 适合想要更现代错误报告且愿意重写文法的场景；chumsky 适合交互式 REPL 的增量/容错解析。建议先用主语法的一个子集做三者原型对比再定。

### B5. 纯 Rust 大数 / 任意精度浮点替代：dashu、malachite 等

- **dashu**：最新 **0.6.2（2026-10-02）**，**MIT OR Apache-2.0**，纯 Rust、全 `no_std`、MSRV 1.68，含 `dashu-int/float/ratio/cmplx`，定位"GMP+MPFR+MPC 的 Rust 原生替代"，支持任意进制浮点。**[已核实]**（[crates.io 元数据](https://crates.io/api/v1/crates/dashu)、[用户指南 FAQ](https://zyxin.xyz/dashu/faq.html)）
  - **关键语义**：**无 NaN**（便利层 panic、上下文层返回 `Err(FpError)`）；无穷为终端值；**实超越函数（exp/ln/三角/双曲/hypot/powf 等）经 Ziv 重试保证正确舍入**；**不提供 ball/区间算术**。**[已核实]**（FAQ"Known limitations"）
  - 含义：dashu 许可证最友好（MIT/Apache）、纯 Rust 无 C 依赖，但**无 NaN、无区间**，且 MPFR 函数族覆盖与 rug 相比仍在补齐。
- **malachite**：最新 **0.14.0（2026-10-13）**，作者 Mikhail Hogrefe，[malachite.rs](https://malachite.rs/)、[github.com/mhogrefe/malachite](https://github.com/mhogrefe/malachite)。
  - **malachite-float**：任意精度 `Float`，**语义遵循 MPFR**，函数成族 `f(x)` / `f_prec(x,prec)` / `f_prec_round(x,prec,rm)`，每次舍入返回 `Ordering` 指示相对精确值的方向，**结果正确舍入（一次舍入）**；已实现全部初等函数，gamma/zeta/erf 等特殊函数尚缺；含 NaN、±∞、±0（单一 NaN、无 payload）。**许可证 LGPL-3.0-only**。**[已核实]**（[docs.rs README](https://docs.rs/crate/malachite-float/latest/source/README.md)、[crates.io 元数据](https://crates.io/api/v1/crates/malachite-float)）
  - **malachite-nz**（Natural/Integer）同为 **LGPL-3.0-only**。**[已核实]**（[crates.io 版本元数据](https://crates.io/api/v1/crates/malachite-nz/0.14.0)）
  - **更正一处常见误解**：dashu FAQ 声称 malachite"不覆盖任意精度浮点"，这与事实不符——malachite-float 自 2023 年起即提供任意精度浮点。**[已核实]**（malachite-float crates.io 历史版本）
- **性能对比参考**：[BigBench](https://github.com/BreezeWhite/BigBench)（用 π 计算基准测试各 Rust 大数库）。**[已核实]**（检索定位；具体数值本次未核读，列 [推测/待验证]）
- **新兴**：`puremp`（0.3.3，2026-11，纯 Rust、clean-room、MPFR 级浮点、带 C ABI）非常新，成熟度待观察。**[已核实]**（crates.io 检索）

**对 Rhytido 的含义**：若目标是 **MIT/Apache + 纯 Rust + 无 C 依赖**，dashu 是许可证最干净的浮点后端，但需自行补 NaN 语义与区间层；若可接受 **LGPL** 且想要最接近 MPFR 的语义与函数族，malachite-float 是强候选（且自带正确舍入 + Ordering，利于 faithful 证书）。rug 仍是函数覆盖最全者（B1）。三者取舍本质是"许可证 vs 函数覆盖 vs 纯 Rust"的三角权衡。

### B6. libsollya 兼容 C ABI 层：Rust 侧导出注意事项

- **crate 类型**：Rust Reference 明确 **`cdylib`** 生成供其他语言加载的动态系统库（Linux `.so`、macOS `.dylib`、Windows `.dll`）；**`staticlib`** 生成 `.a`/`.lib` 但会**导出所有上游依赖的公开符号**，需用链接器/符号版本脚本（Linux）、导出符号表（macOS）或 .def（Windows）限制导出，并可用 `--gc-sections`/`-dead_strip` 去除未用段。**[已核实]**（[Rust Reference — Linkage](https://doc.rust-lang.org/reference/linkage.html)）
- **Sollya 的符号版本做法（本地源码）**：`reference/sollya/sollya.map` 定义版本节点 `LIBSOLLYA_8.0.0 { global: sollya_lib_*; sollya_tool_main; local: *; }`——即只导出 `sollya_lib_*` 前缀符号，其余全部 local。**[已核实]**（本地 `sollya.map`）
- **Rust 侧对应做法（推断）**：用 `cdylib` + `#[no_mangle] extern "C"` 导出与 `sollya_lib_*` 同名同签名的函数；用链接器版本脚本复刻 `LIBSOLLYA_8.0.0` 节点以隐藏内部符号、保证 ABI 兼容；头文件可用 cbindgen 生成。**[推测/待验证]**（方案推断；cdylib/符号脚本机制本身已由 Reference 核实）
- **panic/ unwind 边界**：Reference 提示跨 FFI 的 unwind 需一致，Rust 侧应在 `extern "C"` 边界捕获 panic，避免 unwind 进入 C。**[已核实]**（Reference"Prohibited linkage and unwinding"）+ 实践推断 **[推测/待验证]**。

**对 Rhytido 的含义**：要做 libsollya 兼容层，用 `cdylib` + `extern "C"` + 版本脚本复刻 `sollya.map` 的 `sollya_lib_*` 导出面，并在边界捕获 panic；这样现有链接 libsollya 的工具（如 pythonsollya、Metalibm）可近乎无缝切换到 Rhytido。

---

## C. 许可证发现汇总（对 Rhytido 许可证选择的影响）

| 组件 | 许可证 | 核实状态 / 来源 |
|---|---|---|
| **Sollya** | **CeCILL-C**（文件级弱 copyleft，LGPL 类似），链接 LGPL 库 | [已核实] 本地 `COPYING`/`version.h` + [sollya.org](https://www.sollya.org/) |
| **Gappa** | **CeCILL v2.1（完整 CeCILL，GPL 类似）+ GNU GPL 双许可**；Coq 支持库 LGPL | [已核实] [官网](https://gappa.gitlabpages.inria.fr/) + [COPYING](https://gitlab.inria.fr/gappa/gappa/-/raw/master/COPYING)。**更正：非 CeCILL-C** |
| **pythonsollya** | **CeCILL v2.1** | [已核实] [项目页](https://gitlab.com/metalibm-dev/pythonsollya) |
| **rug** | **LGPL-3.0+** | [已核实] [README](https://docs.rs/crate/rug/latest/source/README.md) |
| **gmp-mpfr-sys** | **LGPL-3.0+**（内置 GMP 6.3.0 / MPFR 4.2.2 / MPC 1.4.1，均 LGPL） | [已核实] [README](https://docs.rs/crate/gmp-mpfr-sys/latest/source/README.md) |
| **GMP / MPFR / MPC** | LGPL（GMP 双 GPL/LGPL） | [已核实] 随 gmp-mpfr-sys 分发；[MPFR](https://www.mpfr.org/) |
| **MPFI** | 根 COPYING 为 **GPL-3 文本**；确切许可证待核读源码头（历史上 LGPL，GPL COPYING 或为 LGPL 引用基础） | [已核实 COPYING] Lead 核读 [COPYING](https://gitlab.inria.fr/mpfi/mpfi/-/raw/master/COPYING)；**对 Rhytido 无实际影响**——自研区间层不链接 MPFI |
| **fplll** | **LGPL-2.1+** | [已核实] [nixpkgs](https://github.com/NixOS/nixpkgs/blob/release-25.11/pkgs/development/libraries/fplll/default.nix) |
| **inari** | **MIT** | [已核实] [crates.io](https://crates.io/api/v1/crates/inari) |
| **dashu** | **MIT OR Apache-2.0** | [已核实] [crates.io](https://crates.io/api/v1/crates/dashu) |
| **malachite-float / -nz** | **LGPL-3.0-only** | [已核实] [float](https://crates.io/api/v1/crates/malachite-float)、[nz](https://crates.io/api/v1/crates/malachite-nz/0.14.0) |
| **pest** | **MIT OR Apache-2.0** | [已核实] [crates.io](https://crates.io/api/v1/crates/pest) |
| **feanor-math** | **MIT** | [已核实] [docs.rs](https://docs.rs/feanor-math/latest/feanor_math/algorithms/lll/index.html) |
| **CORE-MATH** | **MIT** | [已核实] [官网](https://core-math.gitlabpages.inria.fr/) |
| **lalrpop** | **Apache-2.0 OR MIT** | [已核实] Lead 核读 [crates.io 0.23.2](https://crates.io/api/v1/crates/lalrpop)（MSRV 1.86） |
| **chumsky** | **MIT** | [已核实] Lead 核读 [crates.io 0.13.0](https://crates.io/api/v1/crates/chumsky)（MSRV 1.65） |
| **lll-rs / metallic-rs** | 未逐一核读 | [推测/待验证] |

**关键结论（对 Rhytido 许可证选择）**
1. **clean-room 重实现可自选许可证**：CeCILL-C 是文件级 copyleft，仅约束** copying 了 Sollya 源码的文件**。若 Rhytido 不复制 Sollya 代码、仅 reimplement 其算法/语义，则新代码不受 CeCILL-C 传染，可自选 MIT/Apache 等。**[推测/待验证]**（法律推断，应咨询法务）
2. **真正的许可证约束来自数值后端**：
   - 选 **rug / gmp-mpfr-sys / malachite / fplll / MPFI**（均 LGPL 系）→ Rhytido 即便自身 MIT，也须满足 LGPL 的链接/重链接义务（动态链接较易合规，静态链接需提供可重链接的目标文件）。
   - 选 **dashu（MIT/Apache）+ feanor-math（MIT）+ inari（MIT）** 的纯 Rust 组合 → 可整体保持宽松许可、无 C 依赖，但需自行补 NaN/区间/部分 MPFR 函数。
3. **CeCILL-C 与 LGPL 兼容**（Sollya 即 CeCILL-C 链接 LGPL 库），但 **CeCILL-C 与 MIT 对"被复制部分"不直接兼容**；Gappa 的完整 CeCILL/GPL 比 Sollya 的 CeCILL-C copyleft 更强，若 Rhytido 复用 Gappa 证明生成逻辑需注意。**[已核实]**（各许可证文本）+ 兼容性推断 **[推测/待验证]**。
4. **建议**：若 Rhytido 定位"宽松许可、可嵌入、纯 Rust 优先"，优先 dashu + feanor-math + 自研区间层；若定位"最快达到 Sollya 功能对齐"，用 rug + FFI fplll/MPFI 并接受 LGPL 合规成本。两条路线的许可证后果应在立项时明确。

---

## D. 未能核实 / 待验证清单（供后续跟进）

1. **TOMS 2006《Computing machine-efficient polynomial approximations》作者署名**：DOI 已核实，作者未打开一手页确认（dblp 被反爬拦截）。
2. **Berz–Makino 近年（2020–2026）Taylor 模型具体论文**：本次仅核实 TaylorModels.jl 与 Joldes 论文脉络，未逐一核读 COSY INFINITY 近年原文。
3. **Lefèvre–Muller 最坏情形经典论文原始出处**：资源页/CORE-MATH 指向该脉络，但未核读原始论文。
4. **Metalibm 主仓库当前公开可访问性**：gitlab.com/metalibm-dev/metalibm 返回需登录；仅确认组织页与 Kalray 公开镜像。
5. **MPFI 确切许可证**：Lead 已核读根 COPYING = GPL-3 文本（非 LGPL）；MPFI 历史上以 LGPL 发布，确切声明需核读源码头。对 Rhytido 无实际影响（自研区间层，不链接 MPFI）。
6. **是否存在维护中的 MPFI / fplll Rust 绑定**：本次 crates.io 检索未发现，穷尽性待验证。
7. **metallic-rs、lll-rs 的许可证与成熟度细节**：未逐一核读。（lalrpop=Apache-2.0 OR MIT、chumsky=MIT 已由 Lead 核实 crates.io 官方元数据，见 C 节。）
8. **BigBench 具体性能数值**：仅定位仓库，未核读基准结果。
9. **自研 rug 区间层、解析器选型、C ABI 方案**：均为基于已核实原语的推断，未做原型验证。
10. **"无 Sollya 直接 Rust 后继"**：为本次检索范围内结论，非穷尽性断言。
11. **SOS 正性证明在当前 Sollya HEAD 的集成状态**：全部 .c 检索 "sum of squares/sos" 无匹配，showPositivity 实证走 Sturm；SOS 是否以其他命名存在或已移除，待进一步确认（Lead 复核新增）。

---

## E. 本机环境就绪度（Lead 实测，2026-11-12）

- **Rust 工具链**：rustc/cargo **1.99.0**（scoop 安装，shims 路径），**host = x86_64-pc-windows-msvc**，LLVM 23.1.1。**无 rustup**（target 管理需另装 rustup 或用 scoop）。**[已核实]**（rustc -vV）
- **链接器**：当前 shell PATH 未见 link.exe（MSVC）或 gcc（GNU）；MSVC 编译可能需 VS Build Tools 环境。**[已核实]**（Get-Command）
- **工作区路径无空格**（满足 rug 的路径约束，但 rug 仍需 GNU 工具链）。**[已核实]**
- **对后端选型的含义**：本机为 **MSVC** 环境 → **路线 A（rug）需另装 MSYS2/MinGW 并切换 GNU target，摩擦大**；**路线 B/C（纯 Rust：dashu/malachite + feanor-math）可直接用现有 MSVC 工具链**，无 C 依赖、无工具链切换。**本机环境强烈 favor 纯 Rust 路线 B/C。**
- **WSL**：沙箱内 wsl.exe 返回 E_ACCESSDENIED，未能确认发行版（需用户在本机终端核实）。

---

## F. M1 字节级输出兼容面（Lead 实测，2026-11-12）

字节级 CLI 兼容（决策 #3）的工作量与难点集中在三处：
- **消息目录**：sollya-messages.h 共 **487 条编号消息**（grep 计数），含怪癖（如 216 重号、若干跳号）。showmessagenumbers=on 时编号可见，须原样保留。M1 需把 487 条措辞+编号搬成 Rust 目录。**[已核实]**
- **定制 printf**：printf.c 是自研 vfprintf（sollyaInternalBaseSnFprintf，约 1500 行），实现 MPFR/MPFI/MPQ/MPZ 定制格式符（%v/%b/%r 等）+ 宽度/精度/%n 可移植处理。M1 需复刻。**[已核实]**
- **多模式数值显示**：fprintValue（expression.c:2596）= dyadic 记法 `<尾数>b<2指数>`；fprintValueWithPrintMode 按 display 模式分发十进制/dyadic 等；数字生成一律走 **mpfr_get_str**。**[已核实]**

**对后端选型的含义（新发现的权衡）**：字节级十进制输出依赖 mpfr_get_str 的精确数字生成 + Sollya 定制舍入警告逻辑。**rug 封装同一 MPFR，最易字节对齐**；dashu/malachite 数字生成实现不同，字节对齐需额外验证/适配（待原型）。→ 决策 #2/#3 存在张力：**路线 A（rug）字节输出易但 Windows 工具链难；路线 B/C（纯 Rust）Windows 易但字节输出难**。若坚持字节级兼容，rug 权重上升；若接受语义级兼容，纯 Rust 路线更顺。

---

*本文件为 Rhytido 项目新建文档，置于 `docs/research/sollya-port-research.md`（仓库原无文档约定，此为合理新位置）。除本文件外未创建/修改任何文件；`reference/sollya` 仅只读核读。*
