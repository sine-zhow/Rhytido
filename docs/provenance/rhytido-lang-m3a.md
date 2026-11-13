# 溯源记录：rhytido-lang（M3-A2）

- **模块 / crate**：`rhytido-lang`（`symbol::SymbolTable/SymbolId`、`value::Value`、
  `context::{Settings, Frame, Session, DisplayMode}`）
- **里程碑**：M3-A2 ｜ **作者 / 日期**：subagent 63e143f2（lead 独立验收）/ 2026-11-13
- **关联需求**：ADR-9（落位）、ADR-11（帧栈/零全局）、02-architecture §3（lang 行）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 行为规格 | `docs/adr/0011-session-frame-stack.md` | 查找最内优先、赋值命中最内已绑定帧否则当前帧、压/弹帧 guard、多实例 |
| 表示契约 | `docs/adr/0009`（lang 只包不造）+ `rhytido-core::value` | Value=Atom 透传 + List；proc/function 延后 |
| 参考行为 | M3-plan §2 蒸馏（动态作用域语义） | 精确语义由 M3-C 夹具钉；本层只交付 ADR 已定契约 |

## 2. 独立设计说明

- 新 crate `rhytido-lang`（L4），依赖 backend+core facade（分层无环）；SPDX/missing_docs/
  unsafe_code=deny 全继承 workspace。
- `SymbolTable` 按 Session 持有（零全局）；id 自 0 起 ⇒ 跨表 id 可能别名，以测试如实钉住。
- `Value::List(Rc<[Value]>)` 不可变，`append/prepend` 返回新值（原地列表修改→重建，
  行为等价，§3 架构）；非列表接收者返回 `None`（不预判语言类型规则）。
- `Session`：Settings（6 平铺字段 + `to_core_context()`，镜像 MIN_PRECISION 钳制）+
  复用 core `MessageCatalog` 抑制集 + 帧栈。`pop_frame` 于全局帧返回 false 零改动；
  `with_frame` 闭包 guard 单出口弹帧；**panic 展开不弹帧**为文档化边界（M3-E CLI
  以重建 Session 处置，ADR-11 §5），有 catch_unwind 测试钉住。
- `DisplayMode` 四态占位；verbosity 与 catalog 两旋钮统一口径待 M3-E。

## 3. 不复制声明

净室：未参照 Sollya 解析器/对象层源码；帧栈/符号表/值表示由 ADR 自研推导。

## 4. 行为验证

- lang 新增 129（模块 symbol 20 / value 30 / context 40 + 单元 10 + 集成 19 + doc 10）；
  workspace 645→774 零回归；三门禁 lead 独立复跑 EXIT=0。
- 契约抽查：`Session::assign/lookup/with_frame/pop_frame` 与 ADR-11 逐句对应。

## 5. 复核

- lead 独立验收（git 取证/分层计数/三门禁/关键实现抽查）通过。同伴复核：待；法务：待审。
