# Subagent 工作评估与经验总结（Retrospective）

> 2026-11-13 建立，随任务滚动更新 ｜ 依据 AGENTS.md §7；评估一律以 lead 独立取证为准（git 状态、分层计数、门禁退出码），不采信自报。

## 1. 智能体台账

| # | ID（前 8 位） | 任务 | 写作用域 | 结果 | 证据与评估 |
|---|---|---|---|---|---|
| 1 | 8c731654 | backend 测试三层分离 | crates/rhytido-backend/** | ✅ 成功 | 117 全绿，lead 独立复核计数逐一吻合；正确规避集成测试 orphan rule（E0116，powi_helper 改自由函数）；**主动发现既有 Display 精度怪癖并如实上报**（未掩盖、未弱化断言）。评级：优秀 |
| 2 | 5559ecc0 | core 测试三层分离 | crates/rhytido-core/** | ✅ 成功 | 93 全绿；超出任务要求：用私有 buckets 字段注入同桶异哈希候选，确定性验证 hash-consing 冲突回退判别。评级：优秀 |
| 3 | 694f7fbc | M2-A 函数族补齐（8 BigFloat + 11 Interval） | crates/rhytido-backend/** | ✅ 成功 | +61 测试、workspace 271 全绿；dashu panic 边界逐行实证（log2/log10/ln_1p 域外 panic、unwrap_fp 语义）；三角极值判定用严格 π 界 + 精确有理格点算术（保守不漏纳）；唯一测试设计修正如实记录且未弱化断言。评级：优秀 |
| 4 | 455ec467 | M2-B erf/erfc（初派） | crates/rhytido-backend/** | ❌ 失败 | **零落盘静默终止**：收尾语为"现在我将编写实现模块"，git status 证实 crates/ 零改动。lead 以 git 取证识破，未采信其"进行中"叙述。损失：一轮时间。评级：失败（检测与遏制及时） |
| 5 | 7c3bfe6a | M2-B erf/erfc（重派，含纪律条款） | crates/rhytido-backend/** | ✅ 成功 | 纪律条款全面见效：探针按约全部删除；报告为证据式（git status 原文+分层计数+退出码），与 lead 独立复核逐一吻合（backend 226、workspace 319 零丢失）；6 个公开常数首跑全过且**未改常数迁就输出**；自查发现并修复 series_bound 真实缺陷（精确余项误乘舍入预算）+ 两处设计缺陷；独立 Simpson oracle 交叉验证至 ~19–22 位。评级：优秀 |

| 6 | 9e1eec2b | M2-C 实现主体（faithful/interval_eval/C1/Erf 分派） | core/** + backend bigfloat 加法 | ⚠️ 部分成功后终止 | 实现主体落盘且 `cargo check` 全绿，但在修一个探针改名时以**残缺工具调用文本**结束回合，core 测试零落盘、遗留 examples/zz_probe.rs。lead 取证识破、清理探针。评级：部分成功（实现可用，收尾失败） |
| 7 | 3913e2d0 | M2-C 收尾接管（补测试+契约核验） | core/** | ⚠️ 近完成后静默终止 | 补齐全部 core 测试（单元 9→50、faithful 27、interval_eval 24、integration_engine 15）与修正，测试全绿；但静默终止于门禁修复前，遗留 baseline_test.txt + 8 处 approx_constant + 1 处 fmt。lead 按 §7「连续两次失败亲自完成」收尾：值恒等替换 std 常量（断言不弱化）、fmt、删游离文件，三门禁全绿（workspace 459）。评级：近成功（lead 兜底） |

| 8 | a7320a9c | M2-D 实现主体（diff/taylor/context） | core/** | ⚠️ 上下文耗尽终止 | 落盘 diff.rs（17 变体导数表）、taylor.rs（中值形式，严谨）、context.rs（taylor_recursions）、tests/diff.rs 14、tests/context.rs +6，workspace 459→481 全绿；但**每条 assistant 消息都冗长确认 URL 纪律**，逐轮吞噬上下文，叠加任务体量致 "ran out of room"。遗留 clippy（taylor.rs `xI` non_snake_case）/fmt（diff.rs:118）/tests/taylor.rs 缺失/faithful 集成缺失。lead 事件流取证确诊根因（非 cargo 重试——D 早在 14:19 已自行解决管道问题）。评级：部分成功（根因=样板吞噬上下文） |

| 9 | e8489784 | M2-D 收尾接管（带根因修复条款） | core/** | ✅ 成功 | **根因修复实证有效**：极简响应注入提醒、增量落盘、无探针，一次通过 lead 验收。补齐 tests/taylor.rs 15 + faithful 集成 + clippy/fmt，workspace 481→509 零丢失；**实测发现并修复前任真实包含性漏洞**（x²@[−1,1] k=2 三阶导 0·x⁻¹ 跨零→NaN，逐层回退 naive，专项测试钉住，非弱化乃加强）；证据式报告与 lead 独立复核逐项吻合。评级：优秀 |

| 10 | fb43f2c7 | M2-E1 Display 往返修复 | backend/** | ✅ 成功 | 带根因修复+参考规格+Rust 优化条款一次成功；Display ceil(p·log10(2))+1 位修复 (2^53−1) 怪癖、FromStr 配套 max(165,D·10/3+1) 向后兼容（lead 复核采纳）；+6 往返属性（p∈{11..300}含非规格化）、解除 ≤15 位限制恢复全精度交叉核对（加强非弱化）；workspace 509→515 零丢失、core 无扰；证据式报告与 lead 复核逐项吻合。评级：优秀 |

| 11 | 56e7b9bd | M2-E2 BaseFn 收齐 | core/** | ✅ 成功 | 带根因修复+参考规格+Rust 优化条款一次成功；BaseFn 17→30 + is_smooth 穷尽分类 + 非光滑 Taylor 回退（contains_non_smooth 记忆化，clone/求导前判定）；+40 测试（非光滑回退专门包含性 9 函数×跨断点×33 点×prec×k、30 变体导数恒等式+中心差分）；workspace 515→555 零丢失、backend 无扰；**诚实标记两处格式语义缺口**（backend round_to_format 无溢出/次正规；format_interval 定向端点致值未必最近——后者源于 lead 的 Down/Up 指示），留 M2-E2b 补全。评级：优秀 |

| 12 | 32ac1684 | M2-E2b 格式语义补全 | backend+core | ✅ 成功 | 带根因修复条款一次成功；round_to_format 完整 IEEE（canonical_parts 精确整数量化、舍入后溢出判定覆盖 emax 进位、6 模式溢出/下溢规则、次正规网格）+ Format::exponent_range；format_interval 最近端点修复值未必最近（点折叠→Exact/CorrectlyRounded）；+13 溢出/次正规测试+单调性/幂等/包围属性；修正原 Down/Up 弱证书断言为加强（非弱化）；workspace 555→569 零丢失；证据式报告与 lead 复核逐项吻合。评级：优秀 |

| 13 | 14ddff4b | M3-A1 core 原子值 Atom（ADR-10） | core value.rs + tests/value.rs + lib.rs 两行 | ✅ 成功 | +76 测试（模块 64/单元 7/doc 5），workspace 569→645 零回归，三门禁 lead 独立复跑 EXIT=0；抽查与 ADR-10 逐句吻合；规格外 6 决策全部取"保持表示/不预测语义"最小路线；**主动更正自报计数偏差并复跑门禁**（22/21/10/11→23/20/10/11），诚实机制见效；实证发现归档：相邻精度 π 的 canonical_parts 重合（"精度不同⇒值不同"不成立，M3-C/D 夹具注意）。评级：优秀 |
| 14 | 63e143f2 | M3-A2 lang crate + Session 帧栈（ADR-9/11） | crates/rhytido-lang/** + 根 Cargo.toml/Cargo.lock | ✅ 成功 | +129 测试（模块 90/单元 10/集成 19/doc 10），workspace 645→774 零回归，三门禁 lead 独立复跑 EXIT=0；assign/with_frame/pop_frame 契约抽查逐句对 ADR-11；已知边界**如实文档化并钉测试**（panic 展开不弹帧→M3-E 重建处置；跨表 SymbolId 别名）；proc/function 表示延后 B1 的理由写入模块文档与台账（body 需语法树，不预判）。评级：优秀 |

| 15 | 59d7001e | M3-B1 lalrpop 数学表达式文法 | crates/rhytido-lang/** + 根两份 manifest | ✅ 成功 | +48 测试（parse 模块 39 四类齐备/集成往返 7/内联 1/doc 1），workspace 774→**822** 零回归，lead 独立重计数逐项吻合、三门禁 EXIT=0；**首次 lalrpop 0.23.2 接入一次成功**（grammar; 头、内置 lexer 禁 'input、lalrpop-util 需 lexer feature、grammar 状态参数可穿 &mut Interner、生成处 #[allow(missing_docs,…)]、动作避免 turbofish——实证全部录入 M3-plan §3.1 供 B2/B3 复用）；**发现真实跨层边界并升级裁决而非擅改 core**：非二进十进制字面量经 print→parse 的 NodeId 不恒等（lead 核 backend FromStr 源码确认根因=精度策略 max(165, digits·10/3+1)）→ lead 裁决值级往返为 B1 契约、常量精度政策归 M3-C；打印器左嵌套幂非单射如实 carve-out；报告含一次编译时长自纠（冷编译分钟级/增量秒级）。评级：优秀 |

| 16 | workflow-dcdeda05 | M3-B2 语句文法+AST+proc/function 值+蒸馏文档 | crates/rhytido-lang/** + docs/research | ⚠️ 近完成终止（lead 接管收口） | 产物近乎完整且质量高：ast.rs 1081 行、文法 145→359 行、蒸馏文档 25KB（三级置信+§8 未决移交清单）、新增 120 测试、B1 适配 3 处等强度零删除经 lead 逐处核对；但**复现 #6 收尾阶段失败模式**——停在 clippy 后、test 门禁从未跑穿即以截断工具调用文本终止，遗漏 3 处测试侧低级错（1 单元断言过限定=UTF-8 字节长、2 测试输入笔误）+1 处 doctest 编译错（clippy 不查 test/doctest）。lead 按 §7 接管仅最小修（文法/AST 零重写）+proc_lit 放宽 impl Into<Rc<Proc>>；**822→942 零回归**，三门禁 lead 复跑 EXIT=0。对策固化 M3-plan §3.2：**门禁顺序 test 先行且必含 doctest；完成判据加"跑穿 workspace test 并逐 target 报数"**。评级：良好（产物优秀、收尾失守、接管成本低） |

## 2. 经验（经证实有效的做法，保持）

- **详尽契约**：API 签名、算法参数（P=p+3ceil(x²)+guard）、测试清单（含具体常数值与"禁改常数迁就输出"条款）、门禁命令、报告格式全部写入提示词 → 3/3 成功任务一次通过 lead 复核。
- **作用域隔离 + 串行执行**：共享 target 目录与后端源码的任务（backend/core）不并行，杜绝门禁互相污染 → 零冲突事故。
- **lead 独立复核协议**：git 取证 → 分层重计数 → 三门禁 → 关键实现抽查（如 trig_range/pi_bounds）→ 才采纳 → 全部数字与自报吻合，且识破一次零输出。
- **诚实条款**：契约中写明"偏差如实记录、断言不得弱化、失败须调查根因" → 各 subagent 均如实报告失败—调查—修复过程（含 M2-A 的一次测试设计修正）。
- **环境细节入契约**：PATH 前缀、--offline、禁 fmt/禁 git → 零环境类失败、零越权操作。

## 3. 教训（失败与根因 → 已落实的改进）

- **L1 零输出静默终止**（#4）：根因——提示词缺少"未落盘不得结束回合"的硬约束，subagent 以计划文本收尾即终止。改进：① 派发模板加入执行纪律条款（从第一个工具调用起顺序推进、严禁以计划/意图文本结束回合、结束前交付物必须落盘并过门禁、无法完成须列"已落盘/未完成"清单）；② 验收第一步永远是 git 取证。已写入 AGENTS.md §7。
- **L2 临时探针文件滞留 tests/**（#5 现状）：探针本身是好实践（先实证后端语义），但会混入测试集。改进：契约要求"探针/临时文件须在最终报告前删除"；lead 验收时 git status 检查无游离文件。已写入 AGENTS.md §7。
- **L3 subagent 会话不可续作**：`send_message` 报 active teammate not found——subagent 结束后无法唤醒续作，失败任务只能重派。改进：派发提示词必须完全自包含（规格路径、契约、门禁、报告格式），不依赖任何后续对话补充。已写入 AGENTS.md §7。

- **L4 样板吞噬上下文致 "ran out of room"**（#8）：根因——对反复注入的纪律提醒（URL/网络），subagent 每轮都写冗长确认样板，逐轮消耗上下文，叠加任务体量致上下文耗尽终止（事件流取证：D 并非卡在 cargo 重试，它早已自行解决管道问题）。改进：① AGENTS.md §1.1 增"注入提醒极简响应"（最多一句带过、立即继续实质工作）；② §7 派发条款⑤"上下文预算"（任务体量匹配上下文、过大拆小、禁每轮冗长样板、cargo/git 被拒管道改 run_in_background 或提权而非受限模式反复重试）；③ lead 自身同样极简响应注入提醒。重派收尾 subagent 时把此根因修复置于提示词最高优先级。
- **L5 沙箱管道边界**：受限模式拒绝外部程序管道输出（`cargo ... 2>&1 | Select-Object` 触发 EPERM）；workaround=run_in_background 或提权，不在受限模式反复重试。D 自行发现并解决（seq 88），lead 已固化入 §7⑤。

## 3.1 异常终止 subagent 的思考循环分类（事件流取证，2026-11-13）

> 共同终末机制：**上下文耗尽（"ran out of room"）**。差异在于"什么吞噬了上下文"。四类互不相同的 sink，均已固化为 AGENTS.md §7⑥。

| # | ID | 循环类型 | 事件流证据 | 思考为何重复 |
|---|---|---|---|---|
| 4 | 455ec467 | **过度在脑内设计**（零落盘） | 末条 seq 302 reasoning 达 **13051 output tokens**、上下文 261894：在思考里完整设计 erf.rs 全部辅助函数+界推导，反复"Hmm/Wait/careful/重新推导"，随后"Now I'll write"即耗尽 | 任务要求严格界推导→完美主义在 reasoning 内反复精化/复核设计，**不增量落盘**，设计全囤在上下文直至撑爆 |
| 6 | 9e1eec2b | **探针调试黑洞** | zz_probe 57 事件：backend 探针 edit→cargo run→编译错→edit…（02:36–02:52），删后又建 core 探针重演（03:22–03:23），终止于残缺改名 edit | 探针本为一次性实证，却自身编译错不断→陷入 edit→run→error 循环，**两轮**（后端+核心）重复同一模式，非交付物吞噬上下文 |
| 7 | 3913e2d0 | **测试追因无界** | cargo 70 事件：反复 `cargo test --lib/--test faithful/--test integration_engine`，追 dashu `precision cannot be 0` panic（seq 597/601/638），加 diag_probe_tmp/probe_rounded_zero 诊断，04:41 刚跑 clippy 即终止 | 同一 panic 反复出现未被识别为已知边界→run→diagnose→run 循环，诊断探针+重复测试吞噬上下文 |
| 8 | a7320a9c | **样板重复确认** | 每条 assistant 消息以冗长"URL 纪律收悉…"开头（seq 130/142/154/…/221）；user/message 含 URL 仅 seq 10 一条 | 对注入纪律提醒每轮冗长确认，样板逐轮累积吞噬上下文（非 cargo 重试——D 早已自行解决管道问题） |

**共性根因**：(1) 工作不增量落盘、中间态囤在上下文（#4 设计、#6/#7 探针/诊断）；(2) 无收敛判据的重复循环（#6 edit-run-error、#7 run-diagnose-run、#8 样板）；(3) 无上下文预算意识，未在耗尽前收尾/报告。**对策**=§7⑥(a 增量落盘 / b 探针极简 / c 追因有界 / d 样板极简)+§7⑤ 上下文预算+lead 兜底。

## 4. 流程改进落地清单

- [x] AGENTS.md §7 增补：派发必备条款 / 验收协议（硬性）/ 失败处理（静默终止→git 取证→重派并写明失败模式；同一任务连续两次失败→lead 亲自实现或拆小）。
- [x] 任务单模板固化：docs/plan/M2-stage2-tasks.md（任务 C 派发时直接复用其契约结构）。
- [x] 本台账建立，此后每个 subagent 任务完成即更新一行。
- [x] #5 已回填（2026-11-13 验收）：纪律条款实际效果经证实——探针文件按约全部删除、报告完全证据式、常数断言首跑全过且未迁就输出、自曝真实内核缺陷并修复。重派策略有效。
- [ ] 观察项：AGENTS.md 工具链仍写 "nightly rustfmt"，但本机 scoop rust 无 nightly（`+nightly` 不可用）、rustfmt.toml 也已按 stable 选项配置——规则与事实源漂移，待以一次 docs 提交修正表述（改为"stable 可运行；nightly 选项仅当 rustfmt.toml 启用时才要求"）。

## 5. 评估结论

16 个任务批次：**11 优秀**（#1/#2/#3/#5/#9/#10/#11/#12/#13/#14/#15——计数与 lead 复核逐一吻合、超要求的测试深度、主动发现既有缺陷与自曝修复真实内核缺陷）、**1 失败**（#4 零输出静默终止，已遏制并把根因修复为 §7 制度）、**4 部分/近完成后终止**（#6/#7 M2-C 收尾终止由 lead 兜底至 workspace 459；#8 M2-D 上下文耗尽，落盘 481 全绿；#16 M3-B2 产物近乎完整且高质量、test 门禁未跑穿即截断终止，lead 最小修接管至 942 全绿）。**根因修复（§1.1 极简响应 + §7⑤⑥ 上下文预算/sink 规避 + §7⑦ 参考规格+Rust 优化）经 #9–#15 连续七次实证有效**——收尾/补全任务均一次成功、证据式报告与 lead 复核逐项吻合，且多次实测发现并修复真实缺陷（非弱化乃加强）；#16 复发的"收尾阶段终止"新病根=门禁顺序（clippy 不含 doctest/测试输入），对策固化为 **test 先行跑穿再收**（M3-plan §3.2）。**M2 全里程碑验收通过（569 全绿）**；**M3-A、M3-B1、M3-B2 验收通过（942 全绿）**。

**新增教训（M2-C）**：即便实现主体正确，subagent 仍可能在"最后收尾"阶段静默终止——纪律条款降低了但未消除该风险。对策经实证有效：**lead 验收协议 + 连续失败亲自兜底**，使终止未损失实质成果。

**新增根因（M2-D，L4/L5）**：subagent "ran out of room" 的实证根因是**每轮冗长确认注入的纪律提醒（样板）吞噬上下文**，而非任务本身不可完成。修复已制度化（§1.1 极简响应 + §7⑤ 上下文预算），并在收尾重派提示词中置于最高优先级。委派模式收益明确；当前两大薄弱环节=**收尾阶段**与**上下文预算**，对策=拆小任务 + 极简样板 + lead 兜底。