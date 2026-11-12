# 05 · 测试分层规范与迁移记录

> 2026-11-12 ｜ 依据 AGENTS.md §4「测试三层分离（硬性结构）」；本文是操作细则与迁移台账。

## 1. 三层结构

| 层 | 位置 | 视角 | 内容 | 运行 |
|---|---|---|---|---|
| **单元测试** | `src/<module>.rs` 内联 `#[cfg(test)] mod tests` | crate 内部（可访问私有项） | 仅私有/内部实现细节：辅助函数、宏、不变量（如 `expr_hash`/`expr_eq`、`make_range`、`FBinExt`、`prec`） | `cargo test --lib` |
| **模块测试** | `crates/<crate>/tests/<module>.rs` | 外部 crate（仅公共 API） | 每个源模块一个文件，四类用例齐备（正常/极端/异常报错/异常中断） | `cargo test --tests` |
| **集成测试** | `crates/<crate>/tests/integration_*.rs` | 外部 crate（跨模块/跨 crate） | 端到端场景：类型协作、管线、契约一致性 | `cargo test --tests` |

- doctest 留在源码文档中，不属于三层，随 `cargo test` 一并运行。
- 全量：`cargo test --workspace`。
- 集成测试是独立 crate：**不得**对外来类型写固有 impl（E0116）；测试辅助一律自由函数；私有项不可访问（依赖私有项的测试必须留在单元层）。
- 集成测试可使用被测 crate 的常规依赖（如 core 的测试可用 `rhytido_backend`）。

## 2. 命名与覆盖

- 测试函数：`normal_*` / `extreme_*` / `error_*` / `robustness_*`（四类语义前缀）或 `test_<行为>`。
- 每个模块测试文件四类齐备；迁移**不得丢失覆盖、不得弱化断言**。
- 每个 tests 文件顶部 `//!` 注明所属层与对应源模块。

## 3. 迁移台账（2026-11-12）

**基线（迁移前，全绿）**：backend 88 内联（integer 14 / bigfloat 37 / rational 14 / interval 13 / format 10）+ core 77 内联（expr 14 / print 11 / message 9 / quality 10 / fpresult 11 / context 10 / eval 12）+ doctest 5（均在 backend）。

**迁移规则**：
- 公共 API 测试 → `tests/<module>.rs`（模块层），测试名与断言逐一保留。
- 私有细节测试留/增内联（单元层）：backend 的 `make_range`、`FBinExt`；core 的 `expr_hash`/`expr_eq`、`prec`。
- 新增集成测试：backend `integration_numeric.rs`（跨数值类型协作）；core `integration_pipeline.rs`（expr→print→eval→FpResult/Quality 管线）。
- 新增 `tests/error.rs`（backend Error 枚举此前无独立测试）。
- `powi_helper` 由固有 impl 改为自由函数（集成测试 orphan rule）。

**迁移后计数（lead 独立复核，真实工具证据）**：

- **core（已核验，`cargo test -p rhytido-core --offline` EXIT=0）**：单元 9（expr 7：expr_hash/expr_eq 一致性、NaN/符号零、私有 buckets 注入冲突判别、1000 节点幂等；print 2：prec 优先级）+ 模块 77（expr 14 / print 11 / message 9 / quality 10 / fpresult 11 / context 10 / eval 12，基线逐一保留）+ 集成 7（integration_pipeline.rs）= **93 全绿**；clippy `-D warnings` EXIT=0。
- **backend（已核验，`cargo test -p rhytido-backend --offline` EXIT=0）**：单元 9（bigfloat 3：FBinExt 谓词，含「∞ 的 significand 为零但 is_zero 必须为 false」；interval 6：make_range 向外舍入契约/最大精度/等端点/unlimited 回退/lo>hi/NaN）+ 模块 95（integer 14 / bigfloat 37 / rational 14 / interval 13 / format 10 / error 7[新增]）+ 集成 8（integration_numeric.rs[新增]）+ doctest 5（不变）= **117 全绿**；clippy `-D warnings` EXIT=0。
- **合计**：基线 165（backend 88 + core 77）**零丢失** + 新增 40（backend 24 + core 16）= 205 个三层测试 + 5 doctest = **210 全绿**。

## 4. 验收证据（集中门禁，lead 实测）

- `cargo test --workspace --offline`：**EXIT=0**，210 passed / 0 failed（19 个测试二进制，分层计数见 §3）。
- `cargo clippy --all-targets --offline -- -D warnings`：**EXIT=0**，零警告。
- `cargo fmt` + `cargo fmt --check`：**EXIT=0**，无 diff（集中 fmt 后测试复验全绿）。
- 分层门禁：backend `cargo test -p rhytido-backend` EXIT=0（117）；core `cargo test -p rhytido-core` EXIT=0（93）。

## 4b. 迁移中发现的既有问题（非本次引入）

- **BigFloat Display 精度怪癖**：`to_decimal` 按值精度渲染，53 位精度下十进制仅约 15 位有效数字——`(2^53−1).round_to_format(Double, Nearest).to_string()` 为 "9007199254740990"，但数值本身精确等于 2^53−1（数值断言通过）。列 M2 精化（显示应给出可往返的足够位数）；集成测试现将 Display 交叉核对限于 ≤15 位有效数字，数值断言覆盖全值域。

## 5. 后续约定

- 新增函数/类型/trait：先补模块测试（四类），涉及私有逻辑再补单元测试；跨模块行为补集成测试。
- 字节级语料测试（M3+）另设 `tests/corpus/` 语料目录与 diff 驱动（见 03-roadmap §4），不与本文三层冲突。