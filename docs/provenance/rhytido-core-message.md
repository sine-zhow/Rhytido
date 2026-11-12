# 溯源记录：rhytido-core / message（M1）

- **模块 / crate**：`rhytido-core::message`（`Message`、`MessageCatalog`）
- **里程碑**：M1
- **作者 / 日期**：lead agent / 2026-11-12
- **关联需求**：消息目录（编号消息 + verbosity + 抑制）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 设计思想 | 诊断分级/抑制（公有领域日志/诊断模式） | 稳定编号 + verbosity 门控 + 按码抑制 |
| 行为约定 | Sollya `verbosity`/`suppresswarnings`/`showmessagenumbers` 的**公开行为** | 仅行为语义，非复制措辞 |

## 2. 独立设计说明

- `Message{code, level, text}` + `MessageCatalog{verbosity, suppressed, show_numbers}`；`should_emit`/`render` 门控。
- M1 用**自拟措辞**的少量代表消息验证机制。
- **净室注意点（已标记，待法务审查）**：字节级 CLI 兼容要求消息措辞与 Sollya 一致；该措辞属功能性兼容输出，其逐条复现作为独立的字节级兼容任务处理，并在复现前经净室/法务评估（避免直接搬运表达）。
- 与任何参考表达无关：机制为独立设计。

## 3. 不复制声明

未复制 Sollya 源码/注释/命名表达；机制源自公有领域诊断模式 + 独立设计。消息措辞的兼容复现单列并待法务评估。

## 4. 行为验证

- 9 个四类测试：正常（默认 verbosity 发射/门控/显示编号）、极端（verbosity=0 全不发射/高 verbosity 全发射）、异常报错（抑制阻断/取消抑制恢复）、异常中断（抑制/取消幂等、未知码不误抑制）。
- 全绿；`fmt --check` 无 diff；`clippy -D warnings` 零警告。

## 5. 复核

- 同伴复核：待；法务审查：**待审（消息措辞兼容复现需评估）**。