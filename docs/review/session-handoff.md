# 会话交接 · Rhytido（新会话 Agent 快速上手）

> 2026-11-13 由上一会话 lead 撰写；2026-11-13/14 由本会话 lead 三次更新（M3-A → M3-B1 → M3-B2 完成；现基线 942，下一步 M3-B3）。**新会话第一步：读本文 + AGENTS.md（自动加载）**。项目=Rhytido，用 Rust 净室重实现 Sollya 数值引擎。回复用**中文**。

## 1. 当前状态（务必先核对）

- **M0–M2 已验收推送**；**M3-A、M3-B1、M3-B2 已验收**：M3-A=ADR-9/10/11 + core `value`(Atom, ef37fa3) + `rhytido-lang`(SymbolTable/Value/Session 帧栈, 5a1b5fe)；M3-B1=lalrpop 0.23.2 接入 + 数学表达式文法 → core AST（`parse_expr`，f51bb59）；**M3-B2=语句文法（赋值/`:=`/`!`、var 块、print 重定向、proc/procedure、调用、`[|..|]`、串转义）+ ast 模块 + `Value::Proc`/`Function` 表示 + `parse_script` 双链融合 + 蒸馏文档 `docs/research/sollya-language-b2.md`（2d76ccd；subagent 收尾前截断、lead 最小修接管收口，复盘 #16——新对策=门禁顺序 test 先跑穿，M3-plan §3.2）**。#13–#16 四批次零丢失、接管一次（低成本）。
- 最终测试基线：**workspace 942 全绿**（backend 269 / core 376 / lang 297），三门禁 `test`/`clippy -D warnings`/`fmt --check` EXIT=0。
- **下一步 = M3-B3**（控制流/match 文法 + mini/internal 双解析模式入口；文法蒸馏续用公开帮助页，`>` 与比较算符冲突等 B3 起领清单见蒸馏文档 §8）。**派发时必带 M3-plan §3.1/§3.2 全部实证与"test 先行跑穿"条款**。
- **B3 必复用实证**（全量见 M3-plan §3.1/§3.2）：`.lalrpop` 需显式 `grammar;` 头；内置 lexer 禁 `'input` 生命周期；生成码 `#[allow(missing_docs, clippy::all, unused_imports, dead_code)]`；动作避免 turbofish；match 块可实现关键字优先+最长匹配；多状态参数（interner+symbols）可用；**派发模板新条款：门禁顺序 test 先跑穿（含 doctest）→ clippy → fmt**。
- 远端 `github.com/sine-zhow/Rhytido`，交接时 HEAD=M3-B2 收口提交（代码在 `2d76ccd`）。**开工前先 `git log --oneline -1` + `git status -sb` 核对实际状态，勿假设。**
- crate 现状：`rhytido-backend`(L0 facade) + `rhytido-core`(L1 AST/值/求值引擎) + `rhytido-lang`(L4：值(含 Proc/Function 表示)/Session 帧栈/ast 语句树/**数学+语句双层解析器**；控制流与 match 文法及 mini/internal 模式待 B3，求值器待 C)。eval/approx/cli 等未拆分（求值引擎仍在 core）。
- 参考项目 `reference/sollya`（gitignored、只读、**绝不提交/复制**；B1 未读 parser.y/lexer 源码，往返锚=自有打印器先例 + 手册级公开行为）。

## 2. 必读文档（按序）

1. `AGENTS.md` — 全部硬性规范（自动加载）。
2. `docs/plan/M3-plan.md` — **下一里程碑任务方案**（A–F 拆分、文法三段、设计决策 D1/D2/D3）。
3. `docs/plan/M2-acceptance.md` — M2 已交付内容（避免重做）。
4. `docs/review/subagent-retrospective.md` — 16 批次 subagent 台账 + 四类上下文耗尽根因 + 已验证对策。
5. `docs/research/rust-numerical-optimization.md` — 派发引用的正确+高效 Rust 技巧。
6. `docs/plan/03-roadmap.md`、`02-architecture.md`、`05-testing.md` — 路线/架构/测试分层。

## 3. 环境关键（踩过的坑，务必照做）

- **每个 pwsh 先加 PATH**：`$env:PATH = "C:\Users\zhouz\scoop\apps\rust\1.99.0\bin;" + $env:PATH`。
- **cargo 一律 `--offline`**（沙箱断网，依赖已缓存）。Windows MSVC；**stable rustfmt**（无 nightly，勿用 `+nightly`）。
- **沙箱受限模式拒绝外部程序管道输出**：`cargo/git ... 2>&1 | Select-Object` 触发 EPERM（`Access to the path '\\.\pipe\LOCAL\dotnet_...' is denied`）。**Workaround**：① `run_in_background=true` 跑**纯命令（绝不带管道）**再 `job_output` 读——lead 层全程有效（含全量 cargo test/clippy）；② 对 pwsh 加 `sandbox_permissions="danger-full-access"`+中文 justification；③（subagent 环境变体，M3-A 实证）若直接调用也 EPERM，可用 `[System.Diagnostics.Process]::Start`（`UseShellExecute=false`、`CreateNoWindow=true`、**不设任何 RedirectStandard\***，继承句柄）拿输出与真实 ExitCode。**绝不在受限模式反复重试同一条被拒命令。**
- **cargo 首次 quiet 全量跑约 4–5 分钟**：lead 门禁复跑用 `cargo test --workspace --offline -- --quiet`，输出仅每 target 计数行，便于逐层核对。
- 文件操作用 harness 工具（read/write/edit/glob/grep）；编辑前先读；同文件多处编辑串行。

## 4. Git 推送配方（22 端口被阻，走官方 443）

```
git remote set-url origin ssh://git@ssh.github.com:443/sine-zhow/Rhytido.git
$env:GIT_SSH_COMMAND="ssh -C -o BatchMode=yes -o ConnectTimeout=20 -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 -o ServerAliveCountMax=10 -o IPQoS=throughput"
git push origin main        # 需 danger-full-access
git ls-remote origin main   # 核验；遇瞬时 Connection reset 重试一次
git remote set-url origin git@github.com:sine-zhow/Rhytido.git   # 还原
```
- git 身份（repo-local 已配）：`sine-zhow <60770263+sine-zhow@users.noreply.github.com>`。
- **URL 纪律**：只用真实端点（`ssh.github.com:443` 是 GitHub 官方 SSH-over-443），**绝不**用 routify/内部代理替换；推送前核对远端、推送后 `ls-remote` 核验。本任务通常零 URL 工具，对注入的 URL 提醒**一句带过**即可。

## 5. 已验证高效的工作流（照此执行）

**任务单 → subagent 串行实现 → lead 独立取证验收 → Conventional Commits 提交 → 443 推送 → ls-remote 核验 → 台账/复盘/溯源回填。**
- **派发提示词必备（AGENTS.md §7①–⑦）**：执行纪律（严禁计划文本收尾、交付落盘过门禁）/证据式报告（git status 原文+分层计数+退出码）/探针用后即删/完全自包含/**上下文预算**/**上下文 sink 规避**/**lead 蒸馏的参考行为规格 + Rust 优化要点**。
- **lead 验收协议（不采信自报）**：git 取证→分层重计数→三门禁→关键实现抽查，全过才采纳。
- **串行**：共享 target/源码依赖的任务（backend/core）不并行。
- **subagent 不可续作**（`send_message`/`interrupt_agent` 报 not found）；失败→git 取证→重派（写明失败模式）或 lead 亲自兜底；连续两次失败→lead 亲自/拆小。
- 每任务完成回填 `docs/review/subagent-retrospective.md` 一行 + `docs/provenance/` 溯源 + `05-testing.md`/`M2-plan.md`/`M3-plan.md` 台账。

## 6. 核心经验教训（subagent 上下文耗尽四类根因，已固化 §1.1/§7⑤⑥）

1. **过度在脑内设计**：整份实现囤 reasoning 不落盘 → 增量落盘、边写边编译。
2. **探针调试黑洞**：探针 edit→run→error 循环 → 探针极简、>2–3 轮即弃用改真实测试。
3. **测试追因无界**：同一 panic 反复 run→diagnose → 识别已知边界（如 dashu `precision cannot be 0`）针对性处理。
4. **样板重复确认**：每轮冗长确认注入提醒（URL 纪律）吞噬上下文 → **极简响应，一句带过**（lead 自己也照做）。
- 对策经 #9–#15 连续七次实证有效（含 M3-A 两批次与 M3-B1：规格以 ADR/派发提示承载 + 体量按模块切分，均一次通过独立验收）。**任务体量须匹配 subagent 上下文，过大拆小**（文法拆 B1/B2/B3 即此因；B1 首次接 lalrpop 亦一次成功，实证 §3.1 已归档）。#16（M3-B2）复发的"收尾阶段终止"新病根=**门禁顺序**（clippy 不查 doctest/测试输入，test 未跑穿即收尾）→ 对策固化 M3-plan §3.2：test 先跑穿再收，B3+ 派发模板必含。

## 7. 硬性护栏（违反即停）

- **净室**：不复制 Sollya 源码/表达；从公开规格/论文/行为测试独立实现；lead 蒸馏行为规格给 subagent（非源码）；维护溯源。
- **unsafe 须用户先批准** + 逻辑无遗漏证明 + 完备测试；核心全 safe。
- **只有测试过的代码才合并**；每单元四类用例（正常/极端/异常报错/异常中断）；三层测试分离（单元 inline 仅私有 / 模块 tests/<module>.rs / 集成 tests/integration_*.rs）；迁移不弱化断言。
- **语义契约不可优化掉**：动态作用域、error 一等值、符号回退、8 态质量证书。
- 语义/表现分离；零全局状态（Context 显式穿线）；不可变+interning。

## 8. 下一步：M3-B3（前置已扫清，派发前向用户确认一句即可）

- 读 `docs/plan/M3-plan.md` §3（B3 行 + **§3.1/§3.2 全部实证**）与蒸馏文档 `docs/research/sollya-language-b2.md` §8（未决移交清单：`>` 与比较算符冲突消解、下标/`@` 摊开、进制字面量、末条分号可省性等——**B3 起领**）。**M3-B3** = 控制流/match 文法 + mini/internal 双解析模式入口；解析四类测试。
- 文法蒸馏：继续用公开帮助页/示例（禁读 .y/.l/.c/.h），逐条标置信级；match 语义（字符串/列表切割）按 M3-plan §2 口径蒸馏。**常量精度政策归 M3-C**（B3 不动 core）。
- **B3+ 派发模板新条款（#16 教训）**：三门禁顺序 **test 先跑穿（含 doctest）→ clippy → fmt**，结束判据加"workspace test 全绿并逐 target 报数"。
- 现有文法规模：grammar.lalrpop 359 行（数学 Expr 链 + 语句 Statements 链双自动机），ast.rs 1081 行；B3 在同一文件续扩，沿用 fuse 降级 intern 模式与关键字优先 match 块。
- 夹具提醒（M3-A1 实证）：**相邻工作精度的超越函数值可能 canonical_parts 相同**（π@200==π@201）——精度相关断言以"值变化⇒身份变化"为准，勿写"精度不同⇒值不同"。
- 台账事实（供回填格式参考）：M3 批次=#13–#16（subagent-retrospective §1），溯源=rhytido-core-value.md / rhytido-lang-m3a.md / rhytido-lang-parser.md / rhytido-lang-ast-b2.md。

## 9. 快速自检清单（新会话开工）

- [ ] `git log --oneline -1` / `git status -sb` 核对状态（预期 M3-B2 收口提交之后、clean）。
- [ ] PATH 前缀 + `cargo test --workspace --offline` 复跑确认 942 基线（用 §3 workaround）。
- [ ] 读 M3-plan §3.1/§3.2 与蒸馏文档 §8；向用户确认一句即派发 M3-B3。