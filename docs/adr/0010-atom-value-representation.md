# ADR-10：原子值类型表示契约（M3-A · D2）

- **状态**：已接受（用户批准，2026-11-13）
- **日期**：2026-11-13
- **关联**：ADR-9（落位）、ADR-11（会话）、`docs/plan/02-architecture.md` §5、AGENTS.md §2/§3

## 决策

`rhytido-core::value::Atom` 为**不可变枚举**，字段私有，变体集合（净室自定表示，不复制
参考实现）：

- `Number(BigFloat)` / `Integer(Integer)` / `Rational(Rational)`——facade 类型，
  仅出现在 backend/core 边界内（facade 隔离不破坏）；
- `Bool(bool)`；
- `String(Rc<str>)`——内容相等；字节级渲染属 L6，本层不定义面向用户的 Display；
- `Range(Range)`：`Range { lower: Bound, upper: Bound }`；
  `Bound = Closed(BigFloat) | Open(BigFloat) | NegInfinity | PosInfinity`
  （用类型表达开闭/无穷语义，禁 bool 标志位，AGENTS C-CUSTOM-TYPE）。
  构造全域（total）：`lower > upper` 等退化组合**原样保存**，合法性判定推迟给求值层
  （M3-C/D 以参考行为夹具钉），构造不失败、不 panic；
- `Error(LangError)`——**error 一等值（行为契约，不可优化掉）**：
  `LangError { message: Rc<str>, code: Option<u32> }`，纯数据构造；消息编号（对应
  core 消息目录）与措辞由 M3-B/C 蒸馏参考公开行为后细化；求值传播规则属 M3-C，本层
  只定义形状、相等与传播的载体角色；
- `Void`——unit 语义（proc 无 return 的结果）；`Default for Atom = Atom::Void`。

## 相等 / 哈希语义

- 沿用 core interning 先例（`expr.rs` 的 `expr_hash`/`expr_eq`）：数值按
  `BigFloat::canonical_parts()`（NaN 结构相等、带符号零区分），与精度标签无关；
- Integer/Rational 用其规范相等；String 按字节内容；Range/Error/Void 结构相等。
- 目的：同一数学值在表达式与值世界得到一致的等价类。

## trait 与文档要求（AGENTS §2.5/§2.7）

- `Debug, Clone, PartialEq, Eq, Hash, Default` 齐备；`Ord` 不实现（值序语义未定，
  参考行为蒸馏后再评估，避免过早承诺）；
- 全部公共项 `///` 文档 + `# Panics`（构造全域应为「不会 panic」）；构造器静态
  `new`/具语义命名（`number()`/`string()`/`error()`…）。

## 后果与验证

- A1 范围收紧为「表示 + 构造 + 相等/哈希 + 四类测试」；数值运算、list 操作、range
  合法性判定后置（C/D 任务），不为性能牺牲语义。
- 不引入全局 intern 表（零全局）；字符串共享靠 `Rc<str>`；标识符 SymbolId 按会话
  intern，属 lang/ADR-11。
- 验证：模块测试四类覆盖（正常/极端：NaN/±∞/±0/空串/开闭与无穷组合；异常：退化
  Range 原样保存、LangError 边界消息；中断：Clone/Hash/Debug 全域不 panic），
  三门禁 EXIT=0，569 基线零回归。
