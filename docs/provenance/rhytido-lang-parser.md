# 溯源记录：rhytido-lang / parse（M3-B1）

- **模块 / crate**：`rhytido-lang::parse`（`parse_expr`、`ParseError`）+ `src/grammar.lalrpop`
  + `build.rs`；workspace 引入 lalrpop/lalrpop-util 0.23.2
- **里程碑**：M3-B1 ｜ **作者 / 日期**：subagent 59d7001e（lead 独立验收）/ 2026-11-13
- **关联需求**：03-roadmap M3、M3-plan §3 B1 行、ADR-9（数学 AST 仍 core NodeId）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 行为规格 | lead 蒸馏派发规格（优先级层/结合性/字面量/函数名表/log 别名） | 以**自有已验收打印器** `core::print` 为往返锚（print.rs + tests/print.rs 先例），非参考实现 |
| 文法技术 | lalrpop 0.23.2 官方公开用法（build.rs/lalrpop_mod!/grammar 状态参数） | 探针实证先行（grammar; 头等） |
| 明确禁区 | 参考项目 parser.y/lexer 文法源码 | **未阅读、未复制产生式**；文法独立编写 |

## 2. 独立设计说明

- 分层文法 Sum(1,左) → Product(2,左) → Unary(3) → Power(4,右；基=Atom、指数=Unary)
  → Atom(5：数字/pi/_x_/NaN/应用/括号)，与打印器 prec 层精确互逆；内置 lexer，无
  extern lexer、无 catch-all 标识符（未知标识符=语法错误，符号回退属 B2+）。
- `parse_expr(source, &mut Interner) -> Result<NodeId, ParseError>`：Interner 以
  grammar 状态参数穿线（零全局语义状态；thread_local 仅缓存无状态生成解析器的
  regex 编译，文档注明）。ParseError=thiserror 五变体均带字节位置。
- 30 基函数名逐一对照 print.rs base_fn_name + `log`→Ln 别名（M2 验收 §5 指定）。
- 常量 `.expect` 依 regex-totality 注释不变量（AGENTS §2.6 豁免）；`lexeme` 切片
  防御退化，解析全域不 panic。

## 3. 不复制声明

净室：文法从行为规格独立编写；未参照 Sollya parser.y/lexer 或任何参考源码表达。

## 4. 行为验证

- +48 测试：模块 39（正常 17/极端 6/异常报错 12/异常中断 4）+ 集成 7（规范化
  fixpoint 语料、典范串字节恒等、AST 往返、左嵌套幂非单射 carve-out、非二进小数
  值级往返）+ 内联 1 + doctest 1；workspace 774→**822** 零回归，三门禁 lead 独立
  复跑 EXIT=0。
- **已裁决边界**：非二进十进制字面量（如 1.5e-7）经 print→parse 的 NodeId 不恒等
  ——根因 backend FromStr 精度策略 `max(165, digits·10/3+1)`（lead 核源码确认），
  值级（with_precision(DEFAULT_PRECISION)）往返成立并已测；**常量字面量精度政策
  归 M3-C 求值器蒸馏**，B1 不擅改 core。

## 5. 复核

- lead 独立验收（git 取证/分层计数/三门禁/文法与往返测试通读）通过。同伴复核：待；
  法务审查：待审。
