# Sollya 语言行为蒸馏 · M3-B2（语句/过程/字面量语法）

> 状态：M3-B2 蒸馏结论（2026-11-14）｜ 用途：B2 文法与 AST 的**唯一外部行为依据**，B3/C/E 继续消费
> 净室纪律：本文只记录**行为事实的自有表述**（语法形态、参数个数、类型标注、示例转述），不复制参考实现的任何表达（无产生式原文、无帮助文字逐句摘录）。
> 一手来源：`reference/sollya/doc/` 公开帮助页与公开示例脚本。每条结论标注来源与置信级：
> 【已核实】= 公开文档明确写明；【推断】= 由文档示例/用法行合理推出，未见明文；【待验证】= 文档未写清，需 C/D/E 用行为夹具钉住。
> 禁止阅读范围（`*.y *.l *.c *.h *.map *.am` 与 tests-lib/tests-tool C 源）全程未打开。

---

## 0. 蒸馏来源清单（本次实际读取）

| 来源 | 用途 |
|---|---|
| `doc/proc.tex` | proc 形态、形参、return、void、quit/restart 禁令、示例 1–10 |
| `doc/procedure.tex` | `procedure` 定义并赋值缩写形态 |
| `doc/return.tex` | return 位置与个数约束、隐式 `return void` |
| `doc/var.tex` | var 声明位置、作用域、未赋值解引用 |
| `doc/print.tex` | print 参数序列、重定向、display/midpointmode 影响 |
| `doc/parse.tex` | parse(string) 的接受面与拒绝面（B2 入口语义参考） |
| `doc/equal.tex` | `==` 与 `error`/NaN 比较性质（B3/C 用） |
| `doc/string.tex`、`doc/boolean.tex`、`doc/range.tex`、`doc/listof.tex` | 类型关键字仅在 externalproc 上下文使用 |
| `doc/verbosity.tex`、`doc/prec.tex`、`doc/precision.tex`、`doc/display.tex`、`doc/points.tex`、`doc/dieonerrormode.tex`、`doc/roundingwarnings.tex` | 设置语句形态与 `!` 静默形式 |
| `doc/nop.tex`、`doc/quit.tex`、`doc/restart.tex`、`doc/void.tex`、`doc/error.tex` | 语句型命令 |
| `doc/keywords.def` | 关键字/记号拼写事实（含 `@ :. .: == != && ||`） |
| `doc/sollya.tex`（公开用户手册）§General principles / §Variables / §Data types / §Blocks / §Assignments / §Procedures / §Strings / §Lists | 标识符字符集、块、两个赋值算符、字符串转义、列表字面量 |
| `doc/introExample9/12/20/24/25/26/27/28/29/31/35/36/37/44/45.sollya`（公开示例脚本） | 行为用例：脚本形态、赋值、列表、过程 |

---

## 1. 语句与分隔（B2 核心）

1. **语句以 `;` 终止**（不是"分隔"）：示例脚本中每条语句（含脚本最后一条、含过程体内 `return expr;`、含块语句 `{ ... };`）都以 `;` 结尾。
   来源：`introExample25.sollya`、`introExample31.sollya`、`proc.tex` 例 1–10、`var.tex` 例 1。置信级【已核实】。
2. **最后一条语句的 `;` 是否可省**：所有示例都写了 `;`，未见省略用例。置信级【待验证】（B2 取"必须有 `;`"，宽松度留给 B3/E 用行为夹具裁决）。
3. **空脚本 / 空块**：未见明文。`proc.tex` 例 1 的**打印输出**把空过程体渲染为 `nop;`，说明"体可以为空"这一状态存在（打印时用 nop 占位）。B2 接受 `{}`（0 条语句、无 return）与 0 条语句的脚本，属【推断】。
4. **表达式可以单独成为语句**：`L;`、`a;`、`succ(5);`、`3 + succ(0);` 都是脚本中的一行，交互式使用时其值被回显（回显属 REPL 表现层，M3-E）。
   来源：`introExample24.sollya`、`introExample25.sollya`、`proc.tex` 例 1/3、`var.tex` 例 1。置信级【已核实】。
5. **块**：`{` `}` 把若干语句组合成**一条**语句；块可嵌套；块内可声明局部变量。
   来源：手册 §Blocks、§Variables；`var.tex` 例 1（`{var a, b; a=5; b=3; {var a; var b; b = true; a = 1; a; b;}; a; b; };`）。置信级【已核实】。
6. **块内语句顺序约束**：`var` 声明**只能出现在块开头**；一旦出现其他语句，就不能再有 `var`；可以有多条 `var`。
   来源：`var.tex` Description。置信级【已核实】（B2 在文法层强制该顺序）。
7. **`return` 位置约束**：`return` 只允许出现在**用作过程体的块的最末尾**，且**每个块最多一条**。缺失时隐式补 `return void;`（打印时会显式打印出这条隐式 return）。
   来源：`return.tex` Description、`proc.tex` 例 4（`hey` 打印为 `... print("Hello world."); return void;`）。置信级【已核实】。
   → 推论（【推断】）：普通块语句（非过程体）内的 `return` 非法；嵌套块内也不允许 `return`。B2 文法据此把 `return` 限制在过程体尾部。
8. **语句型命令**：`nop`、`quit`、`restart` 的用法行是无参命令（`nop` 另支持 `nop()` 与 `nop(n)`）。`proc.tex` 例 7 显示 `quit;` 作为体内一条语句；含 `quit`/`restart` 的过程**不可被应用**（应用时先检查、报 warning 并求值为 `error`）——检查属 C/D。
   来源：`nop.tex`、`quit.tex`、`restart.tex`、`proc.tex` 例 7。置信级【已核实】（语法），检查语义【B3/C 归属】。
   → B2 未实现裸 `nop;`/`quit;`/`restart;` 语句（见 §8 未决项 4）。

---

## 2. 赋值与设置语句

1. **两个赋值算符**（手册 §Assignments）：
   - `=`：把右值**原样**赋给左值（不求值函数表达式）。例：`i = 1000;` 后 `i = i + 1;` 得到的是表达式 `1000 + 1` 而非 `1001`。
   - `:=`：**先求值常数函数表达式**再赋值；其他表达式上行为同 `=`。例：`i := i + 1;` 真的赋 `1001`。
   置信级【已核实】。→ B2 必须在 AST 保留算符区别（求值语义归 C/D）。
2. **左值形态**：文档同时展示 `标识符`（`i = 1000;`）与**带下标的左值**（`L[3] = L[3] + 1;`、`L[4] := L[4] + 1;`、`s[1] = "a";`）。下标赋值只支持一层（列表的列表多重下标赋值不支持；右侧可多重下标）。字符串下标赋值要求右值求值为长度 1 的字符串。
   来源：手册 §Assignments、`introExample25.sollya`、`introExample45`（引用于手册）。置信级【已核实】。
   → B2 未实现下标左值（见 §8 未决项 1），保持 lead 规格 `Assign(标识符, Rhs)`。
3. **设置语句 = 对全局标识符的赋值**：帮助页用法行形如
   `prec = <常量>`、`verbosity = n`、`display = decimal|binary|dyadic|powers|hexadecimal`、`points = n`、`dieonerrormode = on|off`、`autosimplify = on|off`、`roundingwarnings = on|off`、`fullparentheses = on|off`、`midpointmode = on|off`、`taylorrecursions = ...`、`hopitalrecursions = ...`、`honorcoeffprec = ...`、`rationalmode = ...`。
   同时"裸读取"形式就是标识符引用（用法行 `verbosity : integer`、`display : decimal|...`）。
   置信级【已核实】。→ B2 **不需要独立的 Set 语句类型**：设置就是 `Assign(设置名, 值)`，其副作用（打印消息、精度生效）属 C/E。
4. **`!` 静默赋值**：赋值给全局变量总会打印一条消息，即使 `verbosity = 0`；要静默赋值就在右值后加 `!`。用法行明确给出 `verbosity = n !`、`display = v !`、`points = n !`、`dieonerrormode = v !`；示例 `prec=30!;`、`verbosity = 1!;`。
   来源：手册 §General principles、`introExample9.sollya`（`prec=30;` / `prec=30!;`）、上述帮助页用法行。置信级【已核实】。
   → B2 在 AST 保留"静默"标记（消息抑制属 C/E 与 core 消息目录）。
   【待验证】`!` 是否对**任意**赋值都合法（文档只在设置命令用法行与 `prec` 示例中出现；`prec` 本身也是全局变量，故按"对赋值普遍可用"实现）。
5. **全局变量无需声明**：赋值即存在；只能在"非赋值左值"的表达式中使用时才确定自由变量名（§3.1）。局部变量必须 `var` 声明。
   来源：手册 §Variables。置信级【已核实】。

---

## 3. 标识符与名字

1. **字符集**（手册 §Variables）：标识符至少 1 个字符，**以 ASCII 字母开头**，后续为 ASCII 字母或数字。
   置信级【已核实】，但**与同节矛盾**：手册同节列出预定义变量 `__program_name`、`__argv`、`__unique_id`（下划线开头且含下划线），且 `doc/keywords.def` 有 `XFREEVARIABLE="_x_"`。→ B2 取实用规则 `[A-Za-z_][A-Za-z0-9_]*`（覆盖 `_x_`、`__argv` 等文档内出现的名字），并把"字母开头、无下划线"这条更严说法列为【待验证】。
2. **关键字优先**：`proc`、`procedure`、`return`、`var`、`print`、`nop`、`quit`、`restart`、`in`、`pi`、`true`、`false`、`void`、`error`、`default`、`on`、`off` 等在 `keywords.def` 中登记为关键字；基函数名（`sqrt`/`exp`/…）也是关键字。→ 词法上必须"最长匹配 + 同长时关键字优先"（否则 `printer`、`sine` 这类名字会被切碎）。置信级【已核实】（记号存在）/【推断】（优先规则，与 B1 既有最长匹配实证一致）。
3. **自由变量名 = 首个未绑定名字**（手册 §Functions、§Variables）：一个会话中，第一次在"非赋值左值"位置使用一个未绑定的名字，就确定了自由变量的名字；`_x_` 是写过程时引用自由变量的关键字形式（手册 §Procedures、`introExample37bis`）。
   置信级【已核实】。→ **判定语义归 C**；B2 的 AST 只保留名字（`Var(SymbolId)`），不区分"程序变量"与"自由变量"。
4. **`==` 与 error/NaN**：`error`、`NaN`、`[NaN,NaN]` 与任何值（含自身）比较都返回 false；`error != error` 也是 false；区分三者要用 `match ... with`。置信级【已核实】（B3/C 消费，B2 不做比较算符）。

---

## 4. 过程（proc / procedure）

1. **proc 形态**（`proc.tex` 用法行）：
   `proc(形参1, ..., 形参n) { 体 }`、`proc(形参1, ..., 形参n) { 体 return expr; }`、
   `proc(形参列表参数 = ...) { 体 }`、`proc(形参列表参数 = ...) { 体 return expr; }`。
   置信级【已核实】。
2. **形参**：普通形参是标识符；变长参数用**唯一的"形参列表参数"**，写法 `标识符 = ...`（示例 `proc(L = ...) { var acc, i; acc = 0; for i in L do acc = acc + i; return acc; }`，应用 `sumall()`/`sumall(2)`/`sumall(2,5,7,9,16)` 都合法，实参个数**不检查**，而是收集成列表绑定到该形参）。
   置信级【已核实】。
   【待验证】普通形参与 `= ...` 形参能否混用（文档把两种写法并列呈现，未见混合示例）。B2 文法接受任意位置的 `Ident = ...`，语义留给 C/D。
3. **0 形参**：`proc() { print("Hello world."); }` 合法，调用 `hey()`。置信级【已核实】。
4. **定义时只解析、不解释、不做类型检查**；应用时才检查实参个数/类型（个数不符 → warning + `error`；类型不符 → warning + `error`）。
   来源：`proc.tex` Description 与例 2（`add(3)`、`add(true,false)` 均得 `error`）。置信级【已核实】（语义归 C/D）。
5. **无 return → void**：`hey()` 求值为 void；`print(hey())` 先输出 `Hello world.` 再输出 `void`。置信级【已核实】。
6. **过程是匿名一等对象**：可赋给标识符、放进列表（`[| proc(m,n){return m+n;}, proc(m,n){return m-n;} |]`）、作实参（`applier(succ,5)`）、由过程返回（`makeadd` 返回 `add`）、在过程体内定义（`printsucc` 内 `succ = proc(n){...}`）。置信级【已核实】。
7. **`procedure` 缩写**：`procedure 标识符(形参…) { 体 [return expr;] }` = `标识符 = proc(形参…){…};`，示例 `procedure succ(n) { return n + 1; };`（注意块后仍有 `;`）。置信级【已核实】。
8. **递归**：靠 `var` 声明局部变量 + 把过程对象赋给标识符实现（`fac` 例）；`var` 声明的变量在赋值前解引用为 `error`。置信级【已核实】（语义归 C/D）。
9. **过程体内可含 `if/then/else`、`for ... in ... do`、`while ... do`**（`fac`、`sumall` 例；手册 §Conditional statements、§Loops）。→ **B3 范围**，B2 不实现（见 §8 未决项 2）。
10. **应用语法**：`过程表达式(实参, ...)`；实参可为过程对象。`proc.tex` 例 6 显示 `(myprocs[0])(5,6)`（对列表元素取用后应用）与 `sumall @ [|1,...,10|]`（`@` 把列表摊开成实参）。
    置信级【已核实】（语法）；`@` 摊开语义与下标表达式归 B3/C。
11. **调用实参列表**：逗号分隔；`nop()`、`hey()` 表明**允许 0 实参**。置信级【已核实】。

---

## 5. print 语句

1. **形态**：`print(expr1, ..., exprn)`：把各表达式**以空格分隔**输出并换行。字符串实参输出其字符序列本身；列表实参输出其表达式形式（其中的字符串带双引号）；定义字符串时用的转义序列立即被解释。
   置信级【已核实】。
2. **重定向**：`print(...) > "文件名"`（新建/覆盖）、`print(...) >> "文件名"`（追加）。
   置信级【已核实】。→ B2 在 AST 保留重定向目标与模式（文件写出属 C/E + L6）。
   【待验证】重定向目标能否是任意求值为 string 的表达式（用法行标注为 `string` 类型；示例只用字符串字面量）。B2 只接受字符串字面量。
   【待验证】`>` 与"大于"比较算符（`gt.tex`：`expr1 > expr2 -> boolean`）的词法/语法冲突如何消解；B2 无比较算符故无冲突，B3 引入比较算符时必须处理（LR 移进优先即"print 后的 `>` 一律当重定向"）。
3. **输出格式受全局设置影响**：`display`、`midpointmode`、`fullparentheses`（以及 `verbosity` 决定 warning）。属表现层（L6/M1 打印器）与 E。置信级【已核实】。
4. **`0` 实参**：用法行写作 `print(expr1,...,exprn)`，未见 `print()` 用例。B2 文法允许（与 `nop()`/`hey()` 的 0 实参先例一致）。置信级【推断】。

---

## 6. 字面量

1. **字符串字面量**：双引号包裹；`@` 为字符串连接中缀算符；`length` 取长度；`s[i]` 取第 i 个字符（返回长度 1 的字符串，无字符类型）。
   来源：手册 §Strings、`introExample20`。置信级【已核实】。
2. **转义序列**（手册 §Strings，指向 C99 语义）：`\\`、`\"`、`\?`、`\'`、`\n`、`\t`、`\a`、`\b`、`\f`、`\r`、`\v`、`\x<十六进制>`、`\<八进制>`。
   置信级【已核实】（清单）；具体数值边界（`\x` 位数上限、超范围如何处理）【待验证】——B2 按 C99 常见实现取值并对溢出做 wrapping，记入未决项 3。
   `print.tex` 例 2 佐证：`t = "Hello\tyou"` 打印为 `Hello<tab>you`，而 `[|t|]` 打印为 `[|"Hello\tyou"|]`（列表内字符串按转义形式回显）→ 转义在**定义时**即被解释。
3. **列表字面量**：`[| 元素, ... |]`，元素类型可混合（`[|1,2,true,3...|]`）；`@` 连接两个列表；`.:` 前插、`.:` 后插；`length` 取长度。
   来源：手册 §Lists、`introExample24.sollya`。置信级【已核实】。
4. **末端省略（end-elliptic）列表**：`[|1,...,5|]`、`[|1,2,true,3...|]`、`[|true,1,...,5,9...|]`——`a,...,b` 表示从 a 到 b 的整数递增序列，`b...` 表示无限延伸（下标访问会自动展开到相应长度）。
   来源：`introExample24.sollya`、`introExample25.sollya`、手册 §Lists/§Assignments。置信级【已核实】（语法存在）。
   → B2 **未实现**（需要独立的元素表示与整数步进语义；见 §8 未决项 5）。
5. **区间（range）字面量**：`[下界; 上界]` 或 `[下界, 上界]`；点区间可写 `[a]`；midpoint 形式另有语法；界可为常数表达式（立即按当前精度求值为浮点）、也可为无穷/NaN。`sup`/`inf`/`mid` 取端点与中点。
   来源：手册 §Intervals、`introExample16/17/38/40/41`。置信级【已核实】。
   → B2 **未实现**（`[` 记号与下标访问/下标赋值耦合，须与 B3/C 一并设计；见 §8 未决项 1）。
6. **数值字面量记法**（B1 已实现十进制子集；本节为 B2 蒸馏补全，供 B3/C）：`0x` 前缀表示十六进制尾数（到 `p` 或空白为止），`p` 与 `b` 均表示 `×2^`，`e` 表示 `×10^`，尾缀 `_2` 表示前面的数字是二进制；内存转储式十六进制（无 `p`）对应 IEEE754 double/single 位模式；这些记法在任意 `display` 模式下都能被解析。
   来源：手册 §General principles、`introExample10/11`、`print.tex` 例 5。置信级【已核实】。
   → 与 M3-plan §3.1 的 B1 未决项一致（`0x/0b/0o`、`@` 比例、下划线分组、`inf/infinity` 拼写）；B2 未扩数值文法（保持 B1 典范行为），列入 §8 未决项 6。
7. **特殊值/开关字面量**：`on`/`off`、`dyadic`/`powers`/`binary`/`decimal`/`hexadecimal`、`file`/`postscript`/`postscriptfile`、`RU`/`RD`/`RN`/`RZ`、`absolute`/`relative`、`floating`/`fixed`、`halfprecision`/`single`/`double`/`doubleextended`/`doubledouble`/`quad`/`tripledouble` 与缩写 `HP`/`SG`/`D`/`DE`/`DD`/`QD`/`TD`、`perturb`、`honorcoeffprec`、`default`、`error`、`void`、`true`、`false`、`pi`。
   来源：手册 §Particular values、`keywords.def`。置信级【已核实】（名字存在）。
   → B2 只把 `pi`/`NaN`/`_x_` 与基函数名作为保留记号（B1 既有），其余一律按**标识符引用**入 AST（`Var`），其"值身份"由 C 判定；这样不新增记号即可让 `display = decimal;`、`return void;`、`autosimplify = off;` 解析成功。【推断】（表示选择），行为归属【待验证 → C】。
8. **类型关键字**：`string`/`boolean`/`constant`/`function`/`integer`/`range`/`object`/`list of X`/`void` 只用于 `externalproc` 声明上下文，**不能**被赋值或在其他地方使用。置信级【已核实】。→ `externalproc` 属后续任务，B2 不实现（§8 未决项 7）。

---

## 7. parse / execute（入口语义，供 B2 入口设计与 C）

1. `parse(string)`：解析字符串中的**表达式**，返回 function 或 `error`；语法错 → warning + `error`（例 `parse("5 + * 3")`）。置信级【已核实】。
2. 字符串里**可以**含"返回表达式的命令"（含 `parse` 自身），解析后执行并返回其结果。置信级【已核实】。
3. `parse` **不能**用于只求副作用的命令或指令序列；要那样做需把副作用包进匿名过程并立即应用：`(parse("proc () { i = 42; }"))();` → `i` 变 42，而 `parse("i = 42")` 返回 `error`。
   来源：`parse.tex` 例 4。置信级【已核实】。
   → 对 B2 的启示：**"表达式入口"与"脚本入口"是两个不同的接受面**。B2 因此保留 `parse_expr`（纯数学表达式，标识符非法）并新增 `parse_script`（语句序列）；把两者合并成一个入口属 C/E 决策。
4. `parse` 用的是"miniparser"（warning 文本 "could not be parsed by the miniparser"）；三种解析模式（主/迷你/内部对象）的确切入口划分归 B3（M3-plan §2）。置信级【已核实】（存在 miniparser 概念）。

---

## 8. 未决项（B2 未实现，显式移交给 B3/C/D/E）

1. **下标（索引）语法**：表达式位 `L[2]`、`s[1]`，赋值左值 `L[3] = ...`、`L[4] := ...`、`s[1] = "a"`；以及 `(myprocs[0])(5,6)` 这种"对表达式结果再应用"。与区间字面量 `[a;b]`、点区间 `[a]` 共用 `[` 记号，必须一并设计。
   证据：手册 §Assignments/§Strings/§Lists、`introExample24/25.sollya`、`proc.tex` 例 6。归属：B3（文法）+ C（求值）+ D（列表/字符串切割语义）。
2. **控制流**：`if (c) then s [else s]`（`else` 前不能有 `;`）、`while (c) do s`、`for v from a to b [by step] do s`、`for v in L do s`、`match e with 模式: 表达式 ... default: 表达式;`。
   证据：手册 §Conditional statements/§Loops/§Pattern matching、`proc.tex` 例 5/10、`procedure.tex` 例 2。归属：B3。
3. **字符串转义数值边界**：`\x` 的位数上限、`\` 八进制位数上限、超出 0xFF 时的行为（截断/报错/多字节）。B2 按 C99 常见实现（`u8` wrapping）并只在私有辅助函数内实现，记为待验证。
4. **裸命令语句 `nop;` / `quit;` / `restart;`**：B2 未实现（`quit`/`restart` 涉及会话生命周期与"含 quit/restart 的过程不可应用"的前置检查，属 E/C；`nop;` 是空过程体的**打印**形式，属表现层）。B2 用 `{}` 表示空体，其打印形式（`nop;`）留给 L6/E 的字节级兼容工作。
   证据：`nop.tex`/`quit.tex`/`restart.tex` 用法行、`proc.tex` 例 1/7 的打印输出。
5. **末端省略列表 `[|1,...,5|]` / `[|1,2,true,3...|]`**：需要"元素 = 显式值 | 区间步进 | 无限尾"的表示与整数步进语义。B2 只接受显式逗号元素列表。
   证据：`introExample24/25.sollya`、手册 §Lists。归属：B3（表示）+ C/D（展开语义）。
6. **数值字面量扩展**：`0x…p…`、`_2` 尾缀、内存转储十六进制、`@` 比例记法、数字下划线分组、`inf`/`infinity`/小写 `nan` 拼写。B2 保持 B1 十进制子集不变（典范串往返契约优先）。
   证据：手册 §General principles、`print.tex` 例 5；与 M3-plan §3.1 一致。归属：B3/C（含"常量字面量精度政策"这一 M3-C 裁决项）。
7. **`externalproc` 声明与类型关键字**（`string`/`boolean`/`list of constant`/`->` 等）、**结构体**（`{...}` 字面量与点记法 `a.b.c = 3`）、`@`/`.:`/`.:`/`==`/`!=`/`<`/`<=`/`>`/`>=`/`&&`/`||`/`!`/`~` 等算符。
   证据：`externalproc.tex`、手册 §Structures、`keywords.def`。归属：B3（算符/模式）与后续（结构体、外部过程）。
8. **`print` 重定向的通用性**（见 §5.2 两条【待验证】）。归属：B3（`>` 冲突消解）+ E（文件写出与字节级输出）。
9. **过程体内 `return` 之外的早退路径**、以及"应用前检查 quit/restart"的确切 warning 文本与消息编号。归属：C/D + core 消息目录。

---

## 9. 本次蒸馏对 B2 规格的影响（升级记录）

以下为"公开文档与 lead 规格不一致处以文档为准"的落点，均只改**表示**、不预测语义：

| # | lead 规格 | 文档事实 | B2 落地 |
|---|---|---|---|
| 1 | `Stmt::Print(参数序列)` | `print(...) > "f"` / `>> "f"` 重定向 | `Stmt::Print` 带 `Option<Redirect>`，`Redirect{ file: Rc<str>, mode: Create\|Append }`；只接受字符串字面量文件名 |
| 2 | `Stmt::Assign(标识符, Rhs)` | 两个赋值算符 `=` / `:=` 语义不同；`!` 静默形式 | `Stmt::Assign{ target: SymbolId, op: AssignOp(Plain\|Evaluated), quiet: bool, value: ProgramExpr }` |
| 3 | `Stmt` 五种 | `var id1, id2;` 声明是块/过程体语法的一部分，且位置受限 | 新增 `Stmt::Declare(Rc<[SymbolId]>)`，文法强制"只在块开头" |
| 4 | `Stmt` 五种 | `procedure name(args){...}` 是"定义并赋值"的独立语句形态 | 新增 `Stmt::Procedure{ name: SymbolId, proc: Rc<Proc> }` |
| 5 | `Rhs/ProgramExpr` = 数学子树 ∪ Call ∪ ProcLit ∪ StringLit ∪ ListLit ∪ 标识符 | 过程体内 `return n + 1;`、`res := m + n;`、`acc = acc + i;`、`n * fac(n - 1)` 都要求**算符作用于含标识符的表达式** | 新增 `ProgramExpr::Binary(BinaryOp, Rc<P>, Rc<P>)` 与 `ProgramExpr::Unary(UnaryOp, Rc<P>)`（`UnaryOp = Neg \| Apply(BaseFn)`）；**纯数学子树仍融合回 core `NodeId`**（见 §10） |
| 6 | `Proc{ formals, body }` | 形参有"普通"与"列表参数 `= ...`"两种 | `Formal::Single(SymbolId)` / `Formal::List(SymbolId)`（C-CUSTOM-TYPE，不用 bool） |
| 7 | `Script` 为语句序列 | 标识符需 intern 成 `SymbolId`（规格 A），而入口签名只给 `&mut Interner` | `Script` 同时携带 `statements: Rc<[Stmt]>` 与 `symbols: SymbolTable`（解析期新建），签名保持 `parse_script(source, interner)`；C/E 再把名字并入 `Session` |

---

## 10. B2 的"数学子树融合"策略（实现约定，供 C 消费）

程序层表达式与 B1 数学文法**共用同一条优先级链**（`+ -` < `* /` < 一元 `-` < `^` < 原子，结合性与 B1 完全一致），归约时按如下规则决定产物：

- 两侧（或单侧）都是 `ProgramExpr::Math(NodeId)` → 直接 `interner.intern(Expr::Add/Sub/Mul/Div/Pow/Neg/Apply(...))`，结果仍是 `Math(NodeId)`；
- 任一侧含程序层节点（`Var`/`Call`/`ProcLit`/`StringLit`/`ListLit`/`Binary`/`Unary`）→ 产出 `Binary`/`Unary` 程序节点，**不**进 core。

后果（对 C 有意义）：
1. `parse_expr` 的典范输入集结果与 B1 逐字节一致（同一 interner 状态下 NodeId 相同、`interner.len()` 相同），因为纯数学路径的 intern 顺序与 B1 相同；
2. 含标识符的算符树暂存于程序层，C 在"自由变量名判定/代入"时再决定是否折叠回 core（core 的 `Expr` 无名字节点，B2 不改 core）；
3. 词法层新增标识符记号后，`parse_expr` 对**未知标识符**的错误变体从 `InvalidToken`（词法失败）变为解析拒绝类（`UnexpectedToken`/`ExtraToken`），失败与字节位置不变——这是 lead 规格 D 明确允许的变体级适配。
