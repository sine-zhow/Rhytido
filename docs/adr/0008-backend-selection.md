# ADR-8：数值后端选型（M0）

- **状态**：已接受（M0）
- **日期**：2026-11-12
- **关联**：`docs/plan/02-architecture.md`（facade 隔离 / ADR-2）、`docs/plan/04-license-and-cleanroom.md`

## 背景

计划要求"统一封装可信、成熟、健壮的 Rust 库，facade 可切换 rug/dashu"，M0 需原型定夺默认后端。

## 环境事实（本会话已核实）

- 本机 rustc/cargo **1.99.0**，host = **x86_64-pc-windows-msvc**；VS Build Tools 已安装（MSVC 链接器可用）。
- **无 MSYS2/MinGW、无 rustup**。rug / gmp-mpfr-sys 在 Windows 需 GNU 工具链，当前**不可构建**。
- **dashu 纯 Rust，在 MSVC 成功构建**：dashu-float 0.6.2 / dashu-int 0.6.2 / dashu-ratio 0.6.1。
- 沙箱阻断 schannel TLS；cargo 拉取依赖需提权或 `--offline`（依赖缓存后）。rustfmt/clippy 已随 scoop rust 安装，仅需加入 PATH。

## 决策

**M0 默认后端 = dashu**（纯 Rust、MIT/Apache、本机 MSVC 可构建、许可纯净）。

- 经 `rhytido-backend` facade 隔离；其余层不见 dashu 类型。
- **rug 保留为未来选项**（需先装 GNU 工具链）；facade 保证可切换。

## 权衡与后果

- **优点**：立即可构建、全宽松许可、无 C 依赖、纯 safe Rust（内存安全）。
- **代价**：dashu 无 NaN（facade 的 BigFloat 需补 NaN 语义）；部分特殊函数待补；字节级十进制数字生成与 MPFR 可能不同（M1 表现层需适配/验证）。
- "字节级兼容倾向 rug"的权衡仍成立；若后续装 GNU 工具链，可评估切换 rug 以简化字节级保真。

## 验证

- `cargo build` / `cargo test` / `cargo fmt --check` / `cargo clippy -D warnings` 全绿（Integer 模块：14 单元测试 + 1 doctest）。