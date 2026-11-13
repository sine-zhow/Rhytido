# 溯源记录：rhytido-backend / erf·erfc（M2-B）

- **模块 / crate**：`rhytido-backend::erf`（src/erf.rs，tests/erf.rs，lib.rs +1 声明）
- **里程碑**：M2-B
- **执行 / 日期**：subagent 7c3bfe6a 实现，lead 独立验收 / 2026-11-13
- **关联**：docs/research/erf-erfc-design.md（lead 编写的算法规格）；docs/plan/M2-stage2-tasks.md 任务 B

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 数学定义 | erf/erfc 积分定义、交错幂级数、几何余项界、上界 erfc(x)≤e^{−x²}/(x√π)（公有领域数学，DLMF/A&S） | 级数递推与界推导全部独立复现 |
| 设计文档 | docs/research/erf-erfc-design.md | 算法选型（统一级数+相消精度 P=p+3⌈a²⌉+guard）由 lead 设计，subagent 实现并深化 |

## 2. 独立设计说明

- **Bracket 证书核心**：区间即证书——两端点 nearest 同值（Ziv Test1）⇒ 正确舍入，界=舍入容差 ∈[½ulp,1ulp)；未证得 ⇒ 界=宽+容差（诚实退化）；erf/erfc 两侧同时判定（erfc 相消侧是真瓶颈）。
- 两类误差**分项相加**：精确数学余项（几何级数控制，n+2>x² 起）+ 舍入预算 (8(n+1)+16)·2^-prec·mag，BOUND_SLACK_BITS=6（约束 slack≤0.114p+20 推导）。
- 阈值证明链：a²≥p+2 ⇒ erfc<2^{-(p+2)}<½ulp(1) ⇒ plain 值精确为极限 0/±1/2；checked 版同时给可证严格界。
- π/√π 界全部经正确舍入基元 + 显式裕量 + 定向舍入构造，注释含推导（L219–251 等）。
- 与任何参考表达无关：未读/未复制 reference/sollya。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；源自公开数学定义 + lead 设计文档 + subagent 独立实现与深化。

## 4. 行为验证（lead 独立实测）

- `cargo test -p rhytido-backend` EXIT=0：**226 全绿**（基线 178 零丢失 + erf 单元 18 + erf 模块 27 + doctest 3）；`cargo test --workspace` 319 全绿；`cargo clippy --workspace --all-targets -D warnings` EXIT=0；`cargo fmt --check` EXIT=0。
- 6 个公开常数（≤15 位，DLMF/A&S）首跑全过，未改常数迁就输出；另用**独立算法**（Simpson+Richardson 数值积分 oracle，临时文件用后删除）交叉验证内核至 ~19–22 位，erf(1) 与设计文档 50 位逐位一致。
- 自曝并修复的真实缺陷：series_bound 曾把精确数学余项误乘舍入预算（界虚小）——分项相加修正；界宽 ~1.5ulp 与升级循环漏 erfc 侧两项设计缺陷同轮重构。

## 5. 复核

- lead 独立复核：已完成（git 取证、三门禁、分层计数、报告一致性核对）。
- 建议抽查点：erf.rs L410–467（误差合成）、L164–180（舍入容差）、L598–640（阈值链）、tests/erf.rs L86–110（常数断言与 checked 界的关系）。
- 同伴复核 / 法务审查：待。