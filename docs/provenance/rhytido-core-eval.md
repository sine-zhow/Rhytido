# 溯源记录：rhytido-core / eval（M2）

- **模块 / crate**：`rhytido-core::eval`（`eval_point`）
- **里程碑**：M2（求值引擎基础）
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：表达式求值（faithful 引擎前置）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 设计思想 | 表达式树递归求值 + DAG 记忆化（公有领域技术） | 共享子树只求值一次 |
| 依赖 | 本 crate `Expr`/`Interner`/`BaseFn`、后端 `BigFloat` | 组合既有类型 |

## 2. 独立设计说明

- `eval_point(it, id, x, prec)`：自由变量绑定到 x，常量表达式忽略 x；`HashMap<NodeId, BigFloat>` 记忆化。
- 每步运算后 `with_precision(prec)` 控制工作精度；`apply_fn` 分派 15 个初等函数到 BigFloat facade。
- 本模块为**点求值**，不携带舍入质量证书；faithful/Ziv 引擎在其上构建。
- 与任何参考表达无关：独立设计。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；独立设计。

## 4. 行为验证

- 12 个四类测试：正常（常量/变量/pi/四则/除幂/函数应用/含变量复合）、极端（共享子树记忆化、除零、40 层嵌套）、异常报错（sqrt 负/asin 域外/ln0 传播）、异常中断（特殊值全组合不 panic、全 15 函数可求值）。
- 全绿（core 合计 77 单元）；`fmt --check` 无 diff；`clippy -D warnings` 零警告。

## 5. 复核

- 同伴复核：待；法务审查：待审。