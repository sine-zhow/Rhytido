# 溯源记录：rhytido-lang / ast + parse_script（M3-B2）

- **模块 / crate**：`rhytido-lang`：新 `ast`（Stmt/ProgramExpr/BinaryOp/UnaryOp/Proc/Formal/
  Redirect/Script 等）；`parse::parse_script`；`value` 增 `Proc`/`Function` 变体；
  `grammar.lalrpop` 语句层扩展（145→359 行）
- **里程碑**：M3-B2 ｜ **作者 / 日期**：workflow subagent（job workflow-dcdeda05935e49ff）
  + lead 接管收口 / 2026-11-14
- **关联需求**：ADR-9（语言 AST/复合值落位）、M3-plan §3 B2 行、§2（proc/parse 行为口径）

## 1. 规格来源

| 类型 | 来源 | 说明 |
|---|---|---|
| 行为蒸馏 | `reference/sollya/doc/` **公开帮助页与示例工件**（proc/return/print/parse/var/default/display/prec/range/string/boolean/listof/keywords.def/introExample*.sollya 等） | 逐条结论 + 置信级落 `docs/research/sollya-language-b2.md`；未写清处取最小子集并列未决项 |
| 落位约束 | `docs/adr/0009`（语言 AST 归 lang、数学子树 core NodeId） | Rc 不可变树、不 hash-cons（§3 理由） |
| 禁区 | parser.y / lexer.l / mini*/intern* / *.c/*.h | **全程未读**；文法产生式独立编写 |

## 2. 独立设计说明

- 语句层：赋值 `=`/`:=` + 可选静默 `!`（AST 保留算符与消息策略，语义归 C/E）；`var`
  序言块（文法位置强制"声明先于语句"）；块 `{ }`；print + `>`/`>>` 文件重定向；
  proc 字面量（形参 `name = ...` 表变长）；`procedure` 缩写保持独立节点（等价性判定
  归求值层）；return 限过程体尾（据 return 帮助页 + 示例推断）。
- 程序表达式链（PExpr）与数学链（Expr）**双链独立 LR 自动机**：`BinaryOp/UnaryOp::fuse`
  在子树全为数学时降级 intern（无名字子表达式与 parse_expr 得同一 NodeId），含名字
  者留在程序层——"设置名/变量名是标识符引用"的最小表示选择，值身份判定归 C。
- 字面量：list `[|..|]`、string（C99 转义清单 + 溢出 wrapping 记未决）。
- 未决项（B3/C/D/E 交接清单）见蒸馏文档 §8（`>` 与比较算符冲突、末语句分号可省性、
  `!` 适用范围、下标/摊开、类型关键字/externalproc、进制字面量等）。

## 3. 不复制声明

净室：语句语法事实转述自公开用户文档（帮助页/示例），逐条标注来源与置信级；未复制
参考实现的文法产生式、代码或散文表达。

## 4. 行为验证

- 新增 lang 120 测试（ast 模块 37、parse 模块 +32、integration_parse_scripts 25、
  value +16、单元 +6、doctest +4）；workspace 822→**942**，基线零回归；三门禁 lead
  复跑 EXIT=0。
- B1 适配（预授权范围内）：3 处错误变体等强度对齐（标识符成 token 后 InvalidToken→
  UnexpectedToken，位置+词素仍钉）；**用例零删除**。
- lead 接管修复（subagent 在跑测试前截断终止，见复盘 #16）：1 处单元断言过限定
  （UTF-8 字节长→字符数+值）、2 处测试输入笔误（游离 `]`；未闭合列表形态）、
  `proc_lit` 放宽为 `impl Into<Rc<Proc>>`（doctest 暴露；文法未动）。

## 5. 复核

- lead 独立验收（git 取证 / 分层重计数 / 三门禁 / 文法与 AST 与蒸馏文档抽查）通过。
  同伴复核：待；法务审查：待审。
