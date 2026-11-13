# ADR-11：会话 Session 帧栈 Context（M3-A · D3，零全局）

- **状态**：已接受（用户批准，2026-11-13）
- **日期**：2026-11-13
- **关联**：ADR-9/10、`docs/plan/02-architecture.md` §3（lang 行）/§6、ADR-4、AGENTS.md §3

## 决策

`rhytido-lang::Session` 为语言层会话状态的唯一载体（L4；core 的 `Context` 保持
`Copy` 不可变设置包，二者组合而非互相替换）：

1. **字段**：
   - 设置：内嵌/派生 `core::Context`（precision/rounding/taylor_recursions），另加
     语言设置（verbosity、display 模式、points、cutoff 等，随参考行为蒸馏增量添加）；
   - `frames: Vec<Frame>`：动态作用域帧栈，`Frame = Vec<(SymbolId, Value)>` 线性小表
     （帧内名字少，O(n) 查找可接受；如成热点再以基准驱动优化，先正确后快）；
   - 消息抑制集：复用 core 消息目录的抑制机制，不另造。
2. **零全局、显式穿线、多实例**（AGENTS §3 / ADR-4）：所有可变状态经 `&mut Session`
   传递；每实例独立可测；引擎求值入口从 Session 派生 `core::Context` 传入，core 不
   感知 lang。
3. **动态作用域契约（行为，不可优化掉）**，本 ADR 先定结构、语义细节由 M3-C 参考
   行为夹具钉住：
   - 查找：最内帧优先，逐帧向外；未绑定 → 按「符号回退」契约处理（M3-C 定夺）；
   - 赋值：写入最内层已含该名的帧；不存在则定义在当前帧；
   - proc 应用：压帧；返回/出错/早退路径**必须弹帧**。
4. **异常中断安全**：压/弹帧用作用域辅助（闭包式 with-frame 或 guard）实现，保证
   控制流无论走哪条路径帧栈都被还原、不泄漏、不变量不破坏（测试第四类）。
5. **restart = 重建 Session**（M3-E 落地语义与输出）；void / error 值作为普通栈项
   存储，不特殊化帧栈。

## 后果

- (+) 多实例、可测试、错误路径自动清理；语义契约有独立的结构落点可验收。
- (−) `&mut Session` 穿线样板增多——接受（可测试性优先）。
- (−) 帧表线性查找在深作用域下可能退化——基准驱动再优化，禁止提前引入
  `HashMap`+随机序导致行为差异。

## 验证

M3-A2 交付 lang crate 骨架 + `Session`（含 SymbolId 表、帧栈原语 push/lookup/assign、
with-frame guard）与四类测试；动态作用域语义夹具测试在 M3-C 补齐。三门禁 EXIT=0，
569 基线零回归。
