# M0 验收报告 · 数值地基

- **日期**：2026-11-12
- **状态**：**M0 达成**（核心数值地基完成；精化项明确划入 M2）
- **后端**：dashu（ADR-8，见 `docs/adr/0008-backend-selection.md`）

## 1. 对照验收标准

| 验收项 | 结论 | 证据 |
|---|---|---|
| 数值原语正确性 | ✅ | Integer/BigFloat/Rational/Interval/Format 五模块，四类测试全绿 |
| 定向舍入 | ✅ | `BigFloat::round_to`（6 模式）+ `round_to_format`（5 格式） |
| 区间语义 | ✅ | Interval：NaN/空集/有界三态 + 向外舍入 + 区间四则 |
| 后端决策记录 | ✅ | ADR-8：MSVC 环境选 dashu，facade 可切换 |
| workspace/CI/许可证 | ✅ | workspace + CI（Win/Linux/macOS）+ Apache-2.0 LICENSE/NOTICE |
| 基准 harness | ✅ | `examples/bench.rs`（std 计时），基线已采集 |

## 2. 交付清单

- **代码**（`crates/rhytido-backend`）：`integer` / `bigfloat` / `rational` / `interval` / `format` / `error`，facade 隔离后端。
- **测试**：**67 单元测试 + 5 doctest 全绿**；每模块覆盖四类（正常/极端/异常报错/异常中断）。
- **门禁**：`cargo fmt --check` 无 diff；`cargo clippy --all-targets -- -D warnings` 零警告。
- **溯源**：`docs/provenance/rhytido-backend-{integer,bigfloat,rational,interval,format}.md`。
- **基准基线**（release，165-bit，示例量级）：BigFloat add ~0.2µs、mul ~0.2µs、div ~0.3µs、sqrt ~0.2µs、exp ~17µs、ln ~56µs；Integer mul ~6ns；Rational add ~27ns。

## 3. 关键设计

- **facade 隔离**：后端类型（dashu）不出现在公共 API；其余层仅依赖 facade 抽象。
- **BigFloat**：base-2 bit 精度（匹配 Sollya/MPFR），facade 补齐 dashu 缺失的 NaN，特殊值显式处理，内部 base-2 + 显示转十进制。
- **Interval**：三态枚举显式表达 NaN/空集/有界，向外舍入保证包含性。
- **内存安全**：全 safe Rust（无 unsafe）；所有权 + 值语义，无手工引用计数。

## 4. 明确划入 M2 的精化项（非 M0 缺口）

- 格式舍入的溢出/下溢/次正规（subnormal）行为；DD/TD 展开格式。
- 全定向舍入区间内核；除以含零区间的完整处理（M0 返回 NaN）。
- 其余基本函数（三角/双曲/erf 等，FR-EVAL-4 的完整 32 函数）。
- 十六进制/二进制字面量解析；求值质量证书（`Approximation` 信息）。

## 5. 环境备注

- 沙箱阻断 schannel TLS：拉新依赖需提权，缓存后 `--offline`。
- rustfmt/clippy 随 scoop rust 安装，需加入 PATH（CI 自动装好）。

## 6. 结论

M0 数值地基达成，可进入 **M1（表达式 AST + interning + 字节级打印器 + 消息目录）**。待用户确认后启动 M1。