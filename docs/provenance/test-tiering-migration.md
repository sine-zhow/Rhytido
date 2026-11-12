# 溯源记录：测试体系三层分离重构

- **范围**：`crates/rhytido-backend`、`crates/rhytido-core` 的测试组织；AGENTS.md §4/§1.1；`docs/plan/05-testing.md`
- **里程碑**：M2 期间的质量基础设施重构（不改任何非测试源码逻辑）
- **执行 / 日期**：lead 派发两个写作用域互不重叠的 subagent（backend / core），lead 独立复核并集中跑门禁 / 2026-11-12
- **关联规范**：AGENTS.md §4「测试三层分离（硬性结构）」、§1.1 工作纪律、§7 Subagent 规范

## 1. 变更内容

- **单元层**（src 内联，仅私有项）：backend 9（bigfloat FBinExt 3 + interval make_range 6）；core 9（expr expr_hash/expr_eq 7 + print prec 2）。
- **模块层**（tests/<module>.rs，公共 API，四类用例）：backend 6 文件 95（integer 14 / bigfloat 37 / rational 14 / interval 13 / format 10 / error 7[新增]）；core 7 文件 77（expr 14 / print 11 / message 9 / quality 10 / fpresult 11 / context 10 / eval 12）。
- **集成层**（tests/integration_*.rs，跨模块/跨 crate）：backend integration_numeric 8（新增）；core integration_pipeline 7（新增）。
- 基线 165 个内联测试（backend 88 + core 77）**逐一保留名称与断言**迁至模块层；doctest 5 不变。
- 技术要点：`powi_helper` 由固有 impl 改自由函数（集成测试 orphan rule，E0116 规避）；`use super::*` 改显式公共导入；测试辅助随迁。

## 2. 验证证据（lead 实测，非仅 subagent 报告）

- `cargo test --workspace --offline` EXIT=0：**210 passed / 0 failed**（= 基线 165 + 新增 40 + doctest 5）。
- `cargo clippy --all-targets --offline -- -D warnings` EXIT=0，零警告。
- `cargo fmt` + `cargo fmt --check` EXIT=0，无 diff；fmt 后测试复验全绿。
- 分层计数经 lead 独立运行核对，与 subagent 报告一致（backend 117 / core 93）。

## 3. 偏差与发现（如实记录）

- backend 单元测初版用最近舍入构造 1/3 的 20/100 位端点失败（两精度均向上舍入），改用定向舍入构造，测试意图不变。
- 发现既有 **BigFloat Display 精度怪癖**（53 位精度下十进制仅约 15 位有效数字），非本次引入；数值断言保留全值域，Display 交叉核对限于 ≤15 位有效数字；列 M2 精化（见 05-testing.md §4b）。
- 新测试文件按 §6 加 SPDX 头；既有 src 文件为保持「仅测试改动」未顺手添加。
- core 两处等价改写（std 导入提升顶层、Rounding 用导入名），断言未弱化。

## 4. 净室与护栏遵守

- 未读/未复制 reference/sollya 任何内容；无 unsafe；未做许可证/架构决策；非测试源码逻辑零改动。
- subagent 遵循 AGENTS.md（任务提示内嵌护栏与文件路径）；lead 复核后采纳（§7）。

## 5. 复核

- lead 独立复核：已完成（计数、门禁、抽查 integration_pipeline.rs 与 expr.rs 单元层）。
- 同伴复核 / 法务审查：待。