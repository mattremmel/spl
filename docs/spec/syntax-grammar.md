# SPL Syntax Grammar

This document defines the syntax grammar of SPL using Extended Backus-Naur Form (EBNF). It builds on the lexical grammar defined in `lexical-grammar.md`.

## Syntax Design Philosophy

SPL uses a clean, consistent syntax with several key principles:

1. **Unified path separator**: Use `.` for all paths (no `::`).
2. **Parentheses for application**: Type arguments use `()` not `<>`: `Vec(T: i32)`.
3. **Named arguments with `:`**: Struct fields and call args use `:`: `Point(x: 1, y: 2)`.
4. **Case-based disambiguation**: Uppercase identifiers are type args (`T: i32`), lowercase are value args (`x: 1`).
5. **Return type with `:`**: Functions use `:` for return type: `fn foo(): i32`.
6. **Where clauses for generics**: `fn id(x: T): T where T`.
7. **Pattern matching with `is`**: `if value is Some(x)` instead of `if let`.
8. **Explicit return/yield**: `return` for functions, `yield` for block values. Both require semicolons.
9. **Uniform semicolons**: Semicolons are statement terminators with no semantic significance.

## EBNF Notation

| Notation       | Meaning                                      |
|----------------|----------------------------------------------|
| `=`            | Definition                                   |
| `\|`           | Alternation (choice)                         |
| `( ... )`      | Grouping                                     |
| `[ ... ]`      | Optional (zero or one)                       |
| `{ ... }`      | Repetition (zero or more)                    |
| `"text"`       | Terminal literal                             |
| `UPPERCASE`    | Terminal token from lexer                    |
| `PascalCase`   | Non-terminal (grammar rule)                  |

Trailing commas are allowed in all comma-separated lists (Rust-style).

---

## 1. Program Structure

```ebnf
Program = { InnerAttribute } { Item } ;

Item = { OuterAttribute } [ Visibility ] ItemKind ;

ItemKind = FunctionDef
         | GeneratorDef
         | StructDef
         | EnumDef
         | TraitDef
         | ImplBlock
         | TypeAlias
         | ConstDef
         | StaticDef
         | ExternBlock
         | ExternFnDef
         | UseDecl
         | ModuleDecl ;

Visibility = "pub" [ "(" VisibilityScope ")" ] ;

VisibilityScope = "package" | "super" | "in" Path ;
```

### Attributes

Attributes provide metadata for items, enabling compiler directives, conditional compilation, and derive macros.

```ebnf
(* Outer attributes apply to the following item *)
OuterAttribute = "#" "[" AttrContent "]" ;

(* Inner attributes apply to the enclosing item (e.g., module) *)
InnerAttribute = "#" "!" "[" AttrContent "]" ;

AttrContent = AttrPath [ AttrArgs ] ;

AttrPath = IDENTIFIER { "." IDENTIFIER } ;

AttrArgs = "(" AttrArgList ")"
         | "=" Expression ;              (* e.g., #[doc = "..."] *)

AttrArgList = AttrArg { "," AttrArg } [ "," ] ;

AttrArg = IDENTIFIER [ "=" Expression ]  (* key or key = value *)
        | Expression ;                   (* positional value *)
```

**Common Attributes:**

| Attribute | Usage | Description |
|-----------|-------|-------------|
| `#[derive(Copy, Clone)]` | Structs/enums | Auto-implement traits |
| `#[repr(C)]` | Structs/enums | C-compatible memory layout |
| `#[repr(C, packed)]` | Structs | Packed layout (no padding) |
| `#[link(name = "foo")]` | Extern blocks | Link native library |
| `#[cfg(target_os = "linux")]` | Any item | Conditional compilation |
| `#[no_mangle]` | Functions | Preserve symbol name for FFI |
| `#[inline]` | Functions | Hint to inline |
| `#![name("...")]` | Module file | Module configuration |

### Const and Static Definitions

```ebnf
(* Compile-time constant *)
ConstDef = "const" IDENTIFIER ":" Type "=" Expression ";" ;

(* Static variable (module-level mutable state) *)
StaticDef = "static" [ "mut" ] IDENTIFIER ":" Type "=" Expression ";" ;
```

**Examples:**

```spl
const MAX_SIZE: usize = 1024;
const PI: f64 = 3.14159265359;

static COUNTER: i32 = 0;
static mut GLOBAL_STATE: i32 = 0;  // Requires unsafe to access
```

### Function Definitions

```ebnf
FunctionDef = "fn" IDENTIFIER "(" [ ParamList ] ")" [ ":" Type ] [ WhereClause ] Block ;

(* Generator functions yield multiple values lazily *)
GeneratorDef = "gen" "fn" IDENTIFIER "(" [ ParamList ] ")" ":" Type [ WhereClause ] Block ;

ParamList = Param { "," Param } [ "," ] ;

Param = SelfParam | TypedParam ;

SelfParam = [ "&" [ "mut" ] ] "self" ;

(* Parameters with optional labels *)
TypedParam = [ LabelSpec ] [ "mut" ] IDENTIFIER ":" Type ;

(* Label before parameter name: "to name" means call with "to: value" *)
(* "_" means no label required at call site *)
LabelSpec = "_" | IDENTIFIER ;

(* Where clause declares type parameters and optional constraints *)
(* Unlike Rust, `where` both introduces AND constrains type parameters *)
WhereClause = "where" TypeParam { "," TypeParam } [ "," ] ;

TypeParam = IDENTIFIER [ ":" TypeBound { "+" TypeBound } ] ;

TypeBound = TypePath [ GenericArgs ] ;
```

**Note:** Generator functions require a return type annotation (the yielded type). See [iteration.md](iteration.md) for full generator semantics.

**Examples:**

```spl
// Simple function with return type
fn add(a: i32, b: i32): i32 {
    return a + b;
}

// Generic function - `where T` declares the type parameter T
fn identity(x: T): T where T {
    return x;
}

// Named parameters (external label differs from internal name)
fn greet(to person: String) {
    // Called as: greet(to: "Alice")
}

// Omit label with underscore
fn add(_ a: i32, _ b: i32): i32 {
    // Called as: add(1, 2) instead of add(a: 1, b: 2)
    return a + b;
}

// Generic with bounds
fn clone_it(x: &T): T where T: Clone {
    return x.clone();
}
```

### Struct Definitions

```ebnf
(* Structs use parentheses, not braces *)
StructDef = "struct" IDENTIFIER "(" [ FieldList ] ")" [ WhereClause ] ;

FieldList = Field { "," Field } [ "," ] ;

Field = [ "pub" ] IDENTIFIER ":" Type ;
```

**Examples:**

```spl
// Simple struct
struct Point(x: f64, y: f64)

// Empty struct
struct Empty()

// Generic struct - `where T` declares type parameter T
struct Box(value: T) where T

// Public fields
pub struct Point(pub x: f64, pub y: f64)

// Generic with bounds - `where T: Clone` declares T and requires Clone
struct Container(items: Vec(T: T)) where T: Clone

// Multiple type parameters
struct Pair(first: T, second: U) where T, U

// Multiple type parameters with bounds
struct Map(keys: Vec(T: K), values: Vec(T: V)) where K: Hash + Eq, V
```

### Enum Definitions

```ebnf
(* Enums use parentheses for variants, consistent with struct syntax *)
(* Type parameters are used inline in variants and declared in where clause *)
EnumDef = "enum" IDENTIFIER "(" [ VariantList ] ")" [ WhereClause ] ;

VariantList = Variant { "," Variant } [ "," ] ;

(* Variants can be unit, tuple-style, or struct-style *)
Variant = IDENTIFIER [ "(" VariantFields ")" ] ;

(* Type-only = tuple variant, with : = named fields *)
VariantFields = FieldList           (* named fields: x: i32, y: i32 *)
              | TypeList ;          (* tuple fields: i32, String *)
```

**Examples:**

```spl
// Simple enum
enum Color(Red, Green, Blue)

// Enum with data (type params used inline, declared in where)
enum Option(
    Some(T),
    None,
) where T

// Enum with named fields in variants
enum Message(
    Quit,
    Move(x: i32, y: i32),     // named fields
    Write(String),             // tuple variant
    ChangeColor(u8, u8, u8),   // tuple variant
)

// Result type
enum Result(
    Ok(T),
    Err(E),
) where T, E
```

### Trait Definitions

```ebnf
(* Traits use braces for their body *)
(* Generic args on trait name for input type parameters, e.g., trait Add(RHS) *)
(* Supertraits specified with : before where clause, e.g., trait Numeric: Add + Sub *)
(* Unsafe traits have invariants the compiler cannot verify, e.g., unsafe trait Sync *)
TraitDef = [ "unsafe" ] "trait" IDENTIFIER [ GenericArgs ] [ ":" TypeBound { "+" TypeBound } ] [ WhereClause ] "{" { TraitItem } "}" ;

TraitItem = [ "pub" ] ( TraitMethod | AssociatedType ) ;

TraitMethod = "fn" IDENTIFIER "(" [ ParamList ] ")" [ ":" Type ] [ WhereClause ] ( ";" | Block ) ;

AssociatedType = "type" IDENTIFIER [ ":" TypeBound { "+" TypeBound } ] ";" ;
```

**Examples:**

```spl
// Simple trait
trait Clone {
    fn clone(&self): Self;
}

// Trait with associated type
trait Iterator {
    type Item;
    fn next(&mut self): Option(Self.Item);
}

// Trait with default implementation
trait Default {
    fn default(): Self;
}

// Trait with bounds
trait Numeric: Add + Sub + Mul + Div {
    fn zero(): Self;
    fn one(): Self;
}
```

### Implementation Blocks

```ebnf
(* Impl blocks use parentheses for generic args *)
(* Inherent impl: impl Type { ... } *)
(* Trait impl: impl Trait for Type { ... } *)
(* Unsafe impl required for implementing unsafe traits: unsafe impl Sync for MyType *)
ImplBlock = [ "unsafe" ] "impl" [ TypePath "for" ] TypePath [ GenericArgs ] [ WhereClause ] "{" { ImplItem } "}" ;

ImplItem = [ "pub" ] FunctionDef ;
```

**Examples:**

```spl
// Inherent implementation
impl Point {
    pub fn new(x: f64, y: f64): Point {
        return Point(x: x, y: y);
    }
}

// Trait implementation
impl Clone for Point {
    fn clone(&self): Self {
        return Self(x: self.x, y: self.y);
    }
}

// Generic trait implementation
impl Clone for Option(T: T) where T: Clone {
    fn clone(&self): Self {
        return match self {
            Some(v) => Some(v.clone()),
            None => None,
        };
    }
}
```

**Examples:**

```spl
// Simple impl
impl Point {
    pub fn new(x: f64, y: f64): Point {
        return Point(x: x, y: y);
    }
}

// Generic impl with where clause
impl Box(T: T) where T {
    pub fn unwrap(self): T {
        return self.value;
    }
}

// Impl with bounds
impl Container(T: T) where T: Clone {
    pub fn clone_all(&self): Vec(T: T) {
        return self.items.clone();
    }
}
```

### Type Aliases

```ebnf
TypeAlias = "type" IDENTIFIER [ WhereClause ] "=" Type ";" ;
```

### Use Declarations

Import items or modules into scope. See `module-system.md` for full details.

```ebnf
UseDecl = "use" UsePath ";" ;

UsePath = PathPrefix [ "." UseTree ] ;

PathPrefix = [ "$" | "super" | "self" ] "." IDENTIFIER { "." IDENTIFIER }
           | IDENTIFIER { "." IDENTIFIER } ;

UseTree = "*"                                    (* glob import *)
        | "{" UseTreeList "}"                    (* grouped import *)
        | IDENTIFIER [ "as" IDENTIFIER ] ;       (* item or rename *)

UseTreeList = UseTree { "," UseTree } [ "," ] ;
```

**Examples:**

| Syntax | Description |
|--------|-------------|
| `use std.vec.Vec;` | Import single item |
| `use std.io;` | Import module |
| `use std.collections.HashMap as Map;` | Import with rename |
| `use std.collections.{HashMap, HashSet};` | Grouped import |
| `use std.prelude.*;` | Glob import |
| `use $.utils.helper;` | Package-root import |
| `use super.common;` | Parent module import |

### Module Declarations

Declare inline modules for namespacing within a file, or reference submodules.

```ebnf
ModuleDecl = "module" IDENTIFIER ( ";" | "{" { Item } "}" ) ;
```

**Examples:**

| Syntax | Description |
|--------|-------------|
| `module network;` | Reference submodule in directory |
| `pub module api;` | Public submodule reference |
| `module internal { ... }` | Inline module for namespacing |

**Inline Module Example:**

```spl
// Inline module for namespacing
module internal {
    fn helper(): i32 {
        return 42;
    }

    pub fn public_helper(): i32 {
        return helper();
    }
}

fn main() {
    let x = internal.public_helper();
}
```

**Note:** Inline modules are a future feature. See the module system roadmap.

### Extern Blocks and FFI

Extern blocks declare foreign functions. Extern function definitions create SPL functions callable from foreign code.

```ebnf
(* Declare foreign functions *)
ExternBlock = "extern" AbiString "{" { ExternFnDecl } "}" ;

ExternFnDecl = "fn" IDENTIFIER "(" [ ExternParamList ] [ "," "..." ] ")" [ ":" Type ] ";" ;

ExternParamList = ExternParam { "," ExternParam } ;

ExternParam = IDENTIFIER ":" Type ;

(* Define SPL functions with C calling convention *)
ExternFnDef = "extern" AbiString "fn" IDENTIFIER "(" [ ParamList ] ")" [ ":" Type ] Block ;

AbiString = "\"C\"" ;
```

**Examples:**

```spl
// Declare foreign functions (in extern block)
#[link(name = "mylib")]
extern "C" {
    fn c_function(x: i32): i32;
    fn variadic_fn(fmt: Ptr(T: u8), ...): i32;
}

// Define SPL function callable from C (outside extern block)
#[no_mangle]
extern "C" fn my_callback(value: i32): i32 {
    return value * 2;
}

// Function pointer type for callbacks
type Callback = extern "C" fn(i32): i32;
```

**Notes:**
- Calling functions declared in extern blocks requires `unsafe`
- Functions defined with `extern "C" fn` use the C calling convention
- `#[no_mangle]` preserves the function name for FFI
- Variadic parameters (`...`) are only allowed in extern block declarations

---

## 2. Types

```ebnf
Type = BaseType [ "?" ] ;              (* Optional postfix: T? = Option(T: T) *)

BaseType = ReferenceType
         | ArrayType
         | TupleType
         | FnPointerType
         | NeverType
         | PathType ;

ReferenceType = "&" [ "mut" ] BaseType ;

ArrayType = "[" Type [ ";" Expression ] "]" ;

TupleType = "(" [ Type { "," Type } [ "," ] ] ")" ;

(* Function pointer return type uses colon *)
FnPointerType = "fn" "(" [ TypeList ] ")" [ ":" Type ] ;

TypeList = Type { "," Type } [ "," ] ;

NeverType = "!" ;

PathType = TypePath [ GenericArgs ]
         | "Self" ;

(* Paths use dot, not double-colon *)
TypePath = IDENTIFIER { "." IDENTIFIER } ;

(* Generic args use parentheses with named type arguments *)
(* Case-based disambiguation: uppercase identifier = type arg, lowercase = value arg *)
(* Use ^ sigil to force type arg with lowercase, @ sigil to force value arg with uppercase *)
GenericArgs = "(" [ TypeArg { "," TypeArg } [ "," ] ] ")" ;

TypeArg = [ "^" ] IDENTIFIER ":" Type ;       (* named type argument, e.g., T: i32 or ^t: i32 *)
```

### Type Examples

| Syntax              | Description                        |
|---------------------|------------------------------------|
| `i32`               | Simple type                        |
| `Point(T: i32)`     | Generic type with named arg        |
| `std.vec.Vec`       | Qualified path type                |
| `Self`              | Self type (in impl blocks)         |
| `&T`                | Immutable reference                |
| `&mut T`            | Mutable reference                  |
| `[T]`               | Slice type                         |
| `[T; 10]`           | Fixed-size array                   |
| `(T, U)`            | Tuple type                         |
| `()`                | Unit type                          |
| `fn(i32): bool`     | Function pointer                   |
| `fn(T, U): V`       | Generic function pointer           |
| `fn()`              | Function pointer returning unit    |
| `!`                 | Never type                         |
| `HashMap(K: String, V: i32)` | Multi-param generic type   |
| `Result(T: i32, E: Error)` | Named type arguments         |
| `i32?`              | Optional type (sugar for `Option(T: i32)`) |
| `String?`           | Optional String                    |
| `&T?`               | Reference to optional (rare)       |

---

## 3. Statements

```ebnf
Block = "{" { Statement } "}" ;

Statement = LetStatement
          | ExpressionStatement ;

LetStatement = "let" [ "mut" ] Pattern [ ":" Type ] [ "=" Expression ] ";" ;

ExpressionStatement = Expression ";"
                    | BlockExpression [ ";" ] ;
```

Block expressions (`if`, `while`, `for`, `loop`, and bare blocks) may omit the trailing semicolon when used as statements.

**Semicolon Rules:**

Unlike Rust, semicolons in SPL are purely syntactic terminators with no semantic significance:

| Context | Rule |
|---------|------|
| Regular statements | Semicolon required: `let x = 1;` |
| Block expressions as statements | Semicolon optional: `if x { ... }` or `if x { ... };` |
| `return` statement | Semicolon required: `return 42;` |
| `yield` statement | Semicolon required: `yield value;` |
| Expression in block (not yield) | Semicolon required, value discarded |

The semicolon does NOT determine whether an expression's value is used (unlike Rust). Instead, `return` and `yield` explicitly indicate intent.

**Block Values:**

Blocks containing multiple statements require explicit `yield` to produce a value:

```spl
let result = {
    let a = compute();
    let b = transform(a);
    yield a + b;
};
```

However, blocks containing a **single expression** have an implicit value—no `yield` is needed:

```spl
let doubled = if x > 0 { x * 2 } else { 0 };  // Single expression per branch
let value = { compute() };                     // Single expression block
```

Without `yield` or a single expression, a block's type is `()` (unit).

---

## 4. Expressions

Expressions are defined using layered production rules that encode operator precedence. Lower precedence operators are defined first; they call higher precedence rules.

### Precedence Table

| Precedence | Operators                    | Associativity | Production         |
|------------|------------------------------|---------------|--------------------|
| 1 (lowest) | `=` `+=` `-=` `*=` `/=` `%=` | Right         | AssignmentExpr     |
| 2          | `\|\|`                       | Left          | OrExpr             |
| 3          | `&&`                         | Left          | AndExpr            |
| 4          | `is`                         | Left          | IsExpr             |
| 5          | `==` `!=`                    | Left          | EqualityExpr       |
| 6          | `<` `>` `<=` `>=`            | Left          | ComparisonExpr     |
| 7          | `..` `..=`                   | Left          | RangeExpr          |
| 8          | `+` `-`                      | Left          | AdditiveExpr       |
| 9          | `*` `/` `%`                  | Left          | MultiplicativeExpr |
| 10         | `!` `-` `&` (unary)          | Right         | UnaryExpr          |
| 11 (highest)| `.` `()` `[]` `[:]` `?`     | Left          | PostfixExpr        |

Note: Type conversions use methods (`.widen()`, `.truncate()`, `.try_into()`) rather than a cast operator.

### Expression Grammar

```ebnf
Expression = AssignmentExpr ;

AssignmentExpr = OrExpr [ AssignOp AssignmentExpr ] ;

AssignOp = "=" | "+=" | "-=" | "*=" | "/=" | "%=" ;

OrExpr = AndExpr { "||" AndExpr } ;

AndExpr = IsExpr { "&&" IsExpr } ;

(* Pattern matching with is *)
IsExpr = EqualityExpr [ "is" Pattern ] ;

EqualityExpr = ComparisonExpr { ( "==" | "!=" ) ComparisonExpr } ;

ComparisonExpr = RangeExpr { ( "<" | ">" | "<=" | ">=" ) RangeExpr } ;

RangeExpr = AdditiveExpr [ ( ".." | "..=" ) [ AdditiveExpr ] ] ;

AdditiveExpr = MultiplicativeExpr { ( "+" | "-" ) MultiplicativeExpr } ;

MultiplicativeExpr = UnaryExpr { ( "*" | "/" | "%" ) UnaryExpr } ;

UnaryExpr = ( "!" | "-" | "&" [ "mut" ] ) UnaryExpr
          | PostfixExpr ;

(* No :: for paths - use . only *)
PostfixExpr = PrimaryExpr { PostfixOp } ;

PostfixOp = "." IDENTIFIER [ GenericArgs ]                   (* field or associated item *)
          | "." IDENTIFIER [ GenericArgs ] "(" [ ArgList ] ")" (* method call *)
          | "(" [ ArgList ] ")"                               (* function call *)
          | "[" Expression "]"                                (* index *)
          | "[" SliceExpr "]"                                 (* slice *)
          | "?" ;                                             (* error propagation *)

SliceExpr = [ IndexExpr ] ":" [ IndexExpr ] ;

IndexExpr = Expression
          | "$" [ "-" Expression ] ;  (* $ = length, $-1 = last index *)

(* Arguments can be named with : *)
(* Case-based disambiguation: uppercase identifier = type arg, lowercase = value arg *)
(* Use ^ sigil to force type arg with lowercase, @ sigil to force value arg with uppercase *)
ArgList = Arg { "," Arg } [ "," ] ;

Arg = TypeArg                               (* type argument: T: Type or ^t: Type *)
    | ValueArg                              (* value argument: name: expr or @NAME: expr *)
    | Expression ;                          (* positional argument *)

ValueArg = [ "@" ] IDENTIFIER ":" Expression ;  (* named value argument *)
```

### Primary Expressions

```ebnf
PrimaryExpr = LiteralExpr
            | EnumShorthandExpr
            | PathExpr
            | TypeExpr
            | GroupedExpr
            | TupleExpr
            | ArrayExpr
            | StructExpr
            | ClosureExpr
            | MatchExpr
            | BlockExpression
            | IfExpr
            | WhileExpr
            | ForExpr
            | LoopExpr
            | BreakExpr
            | ContinueExpr
            | ReturnExpr
            | YieldExpr ;

(* Type expressions - types used as values for associated function calls *)
(* Requires GenericArgs to distinguish from PathExpr *)
TypeExpr = TypePath GenericArgs ;

(* Closures - see closures.md for full semantics *)
ClosureExpr = [ "clone" | "move" ] ClosureParams ClosureBody ;

ClosureParams = "||"
              | "|" [ ClosureParamList ] "|" ;

ClosureParamList = ClosureParam { "," ClosureParam } [ "," ] ;

(* ~ modifier clones the capture at closure creation time *)
ClosureParam = [ "~" ] [ "mut" ] IDENTIFIER [ ":" Type ] ;

ClosureBody = Block | Expression ;

LiteralExpr = INTEGER | FLOAT | STRING | CHAR | "true" | "false" ;

(* Enum variant shorthand - type inferred from context *)
EnumShorthandExpr = "." IDENTIFIER [ "(" [ ExpressionList ] ")" ] ;

(* Paths use dot separator *)
PathExpr = IDENTIFIER { "." IDENTIFIER } ;

GroupedExpr = "(" Expression ")" ;

TupleExpr = "(" [ Expression "," [ Expression { "," Expression } ] [ "," ] ] ")" ;

ArrayExpr = "[" [ Expression { "," Expression } [ "," ] ] "]"
          | "[" Expression ";" Expression "]" ;

(* Struct instantiation uses parentheses; type args and fields in same list *)
(* Case-based disambiguation: uppercase = type arg, lowercase = field *)
StructExpr = StructExprPath "(" [ StructArgList ] ")" ;

StructExprPath = TypePath | "Self" ;

StructArgList = StructArg { "," StructArg } [ "," ] ;

StructArg = TypeArg                            (* type argument: T: Type *)
          | StructField ;                      (* value field: name: expr or shorthand *)

(* Field with optional expression; colon separates name from value *)
(* Bare identifier = shorthand: `x` means `x: x` *)
StructField = IDENTIFIER [ ":" Expression ] ;

(* Match expression *)
MatchExpr = "match" Expression "{" { MatchArm } "}" ;

(* Trailing comma optional; required if another arm follows *)
MatchArm = Pattern [ "if" Expression ] "=>" Expression [ "," ] ;
```

**Struct Expression Examples:**

```spl
// All fields with values
let p = Point(x: 1, y: 2)

// Shorthand when variable name matches field
let x = 1
let y = 2
let p = Point(x, y)  // Same as Point(x: x, y: y)

// Generic type instantiation
let b = Box(T: i32, value: 42)

// Self in impl blocks
impl Point {
    fn origin(): Self {
        return Self(x: 0, y: 0);
    }
}
```

**Match Expression Examples:**

```spl
let result = match value {
    Some(x) => x * 2,
    None => 0,
}

let description = match count {
    0 => "none",
    1 => "one",
    n if n < 10 => "few",
    _ => "many",
}
```

**Enum Shorthand Examples:**

When the enum type can be inferred from context, variants can be referenced with a leading dot without qualifying the enum type:

```spl
// In match arms (type inferred from scrutinee)
let color: Color = get_color()
match color {
    .Red => "red",
    .Green => "green",
    .Blue => "blue",
}

// In function arguments (type inferred from parameter)
fn set_color(c: Color) { ... }
set_color(.Blue)

// In variable bindings with explicit type
let c: Color = .Green

// With variant data
let msg: Message = .Move(x: 10, y: 20)
let result: Result(T: i32, E: Error) = .Ok(42)

// In return statements (type inferred from function signature)
fn default_color(): Color {
    return .Red
}

// Comparison with known enum type
if color == .Blue { ... }
```

### Control Flow Expressions

```ebnf
BlockExpression = Block
                | IfExpr
                | WhileExpr
                | ForExpr
                | LoopExpr ;

IfExpr = "if" Expression Block [ "else" ( IfExpr | Block ) ] ;

(* Loop labels (future feature) *)
Label = "'" IDENTIFIER ":" ;

WhileExpr = [ Label ] "while" Expression Block ;

ForExpr = [ Label ] "for" Pattern "in" Expression Block ;

LoopExpr = [ Label ] "loop" Block ;

(* Break/continue can target a labeled loop (future feature) *)
BreakExpr = "break" [ "'" IDENTIFIER ] [ Expression ] ;

ContinueExpr = "continue" [ "'" IDENTIFIER ] ;

(* Explicit return required for returning values from functions *)
ReturnExpr = "return" [ Expression ] ;

(* Yield provides a value for block expressions *)
YieldExpr = "yield" Expression ;
```

**Pattern Matching in Control Flow:**

The `is` operator enables pattern matching directly in conditions:

```spl
// Pattern matching with is
if value is Some(x) {
    // x is bound here
}

// Check without binding
if value.is_some() {
    // value exists
}

// Combined with other conditions
if value is Some(x) && x > 0 {
    // x is positive
}

// In while loops
while queue.pop() is Some(item) {
    process(item)
}
```

**Explicit Return and Yield:**

Functions must use `return` to return values, and block expressions must use `yield` to provide a value—**unless the block contains only a single expression**, in which case the value is implicit.

```spl
// Single-expression function: implicit return
fn double(x: i32): i32 { x * 2 }

// Multi-statement function: explicit return required
fn compute(x: i32): i32 {
    let temp = x * 2;
    return temp + 1;
}

// Single-expression block: implicit value
let result = if condition { x * 2 } else { 0 };

// Multi-statement block: yield required
let result = {
    let temp = compute();
    yield temp * 2;
};

// Error: multi-statement without return
fn bad(x: i32): i32 {
    let temp = x;
    temp * 2;  // ERROR: missing return
}

// Error: multi-statement without yield
let bad = {
    let x = 1;
    x + 1;  // Block has type (), not i32
};
```

**Why this design?**

Single-expression blocks are concise and unambiguous. Multi-statement blocks require explicit `return`/`yield` to avoid the subtle semantics where semicolon presence changes program behavior.

---

## 5. Patterns

Patterns are used in `let` bindings, `for` loops, `is` expressions, and match arms.

```ebnf
(* Or-patterns: match any of the alternatives *)
Pattern = SinglePattern { "|" SinglePattern } ;

SinglePattern = IdentifierPattern
              | WildcardPattern
              | LiteralPattern
              | RangePattern
              | TuplePattern
              | SlicePattern
              | StructPattern
              | EnumPattern
              | ReferencePattern
              | GroupedPattern ;

(* Parentheses for grouping in complex or-patterns *)
GroupedPattern = "(" Pattern ")" ;

IdentifierPattern = [ "mut" ] IDENTIFIER ;

WildcardPattern = "_" ;

LiteralPattern = [ "-" ] INTEGER
               | [ "-" ] FLOAT
               | STRING
               | CHAR
               | "true"
               | "false" ;

RangePattern = LiteralPattern ( ".." | "..=" ) LiteralPattern ;

TuplePattern = "(" [ Pattern { "," Pattern } [ "," ] ] ")" ;

SlicePattern = "[" [ SlicePatternElement { "," SlicePatternElement } [ "," ] ] "]" ;

SlicePatternElement = RestPattern | Pattern ;

RestPattern = ".." [ IDENTIFIER ] ;

(* Struct patterns use parentheses *)
StructPattern = TypePath "(" [ StructPatternFields ] ")" ;

StructPatternFields = StructPatternField { "," StructPatternField } [ "," ] [ ".." ] ;

(* Field with optional pattern binding *)
StructPatternField = IDENTIFIER [ ":" Pattern ] ;

(* Enum variant patterns *)
EnumPattern = TypePath [ "(" [ Pattern { "," Pattern } [ "," ] ] ")" ] ;

ReferencePattern = "&" [ "mut" ] Pattern ;
```

**Note:** At most one `RestPattern` (`..` or `..name`) is allowed per slice pattern. This is enforced semantically, not syntactically.

### Pattern Examples

| Syntax                | Description                          |
|-----------------------|--------------------------------------|
| `x`                   | Bind to identifier                   |
| `mut x`               | Mutable binding                      |
| `_`                   | Wildcard (ignore value)              |
| `42`                  | Match literal integer                |
| `-1`                  | Match negative literal               |
| `0..10`               | Match range 0-9 (exclusive end)      |
| `0..=10`              | Match range 0-10 (inclusive end)     |
| `'a'..'z'`            | Match characters a-y (exclusive end) |
| `'a'..='z'`           | Match characters a-z (inclusive end) |
| `(a, b)`              | Destructure tuple                    |
| `[a, b, c]`           | Destructure fixed-size array/slice   |
| `[first, ..]`         | Match first, ignore rest             |
| `[first, ..rest]`     | Match first, bind rest to `rest`     |
| `[.., last]`          | Match last element                   |
| `[first, ..middle, last]` | Match first, last, bind middle   |
| `Point(x, y)`         | Destructure struct (shorthand)       |
| `Point(x: a, y: b)`   | Destructure with rename              |
| `Point(x, ..)`        | Partial struct destructure           |
| `Some(x)`             | Match enum variant with binding      |
| `None`                | Match enum variant without payload   |
| `Ok(value)`           | Match Result Ok variant              |
| `Err(e)`              | Match Result Err variant             |
| `&x`                  | Match reference                      |
| `&mut x`              | Match mutable reference              |
| `1 \| 2 \| 3`         | Or-pattern: match any alternative    |
| `Some(x) \| None`     | Or-pattern with variants             |
| `"yes" \| "y"`        | Or-pattern with strings              |

**Or-Pattern Examples:**

```spl
match value {
    1 | 2 | 3 => "small",
    4 | 5 | 6 => "medium",
    _ => "large",
}

match option {
    Some(0) | None => "empty or zero",
    Some(n) => "has value",
}

// Bindings must be consistent across alternatives
match point {
    Point(x: 0, y) | Point(x: y, y: 0) => "on axis",  // ERROR: inconsistent bindings
    Point(x, y: 0) | Point(x: 0, y: x) => ...,        // ERROR: inconsistent bindings
    Point(x: 0, y) | Point(x, y: 0) => use_coord(y),  // OK: y bound in both
}
```

---

## 6. Literals

Terminal tokens from the lexer. See `lexical-grammar.md` for precise definitions.

```ebnf
INTEGER = (* decimal, hex, binary, or octal integer *) ;

FLOAT = (* floating-point number *) ;

STRING = (* double-quoted string with escapes *) ;

CHAR = (* single-quoted character *) ;
```

Boolean literals use the keywords `true` and `false`.

---

## Operator Precedence Summary

From lowest to highest precedence:

| Prec | Category       | Operators                       | Assoc | Example                   |
|------|----------------|--------------------------------|-------|---------------------------|
| 1    | Assignment     | `=` `+=` `-=` `*=` `/=` `%=`   | Right | `x = y = 1`               |
| 2    | Logical OR     | `\|\|`                         | Left  | `a \|\| b \|\| c`         |
| 3    | Logical AND    | `&&`                           | Left  | `a && b && c`             |
| 4    | Pattern Match  | `is`                           | Left  | `x is Some(v)`            |
| 5    | Equality       | `==` `!=`                      | Left  | `a == b != c`             |
| 6    | Comparison     | `<` `>` `<=` `>=`              | Left  | `a < b`                   |
| 7    | Range          | `..` `..=`                     | Left  | `0..10`, `0..=10`         |
| 8    | Additive       | `+` `-`                        | Left  | `a + b - c`               |
| 9    | Multiplicative | `*` `/` `%`                    | Left  | `a * b / c`               |
| 10   | Unary          | `!` `-` `&` `&mut`             | Right | `!&mut x`                 |
| 11   | Postfix        | `.` `()` `[]` `[:]` `?`        | Left  | `a.b()?.c[0]`             |

---

## Ambiguity Resolution

### 1. Struct Expression vs Function Call

When the parser sees `IDENTIFIER(`, it must determine if this is a struct instantiation or a function call.

**Rule:** Context and argument syntax disambiguate:
- Named fields with `:` indicate struct instantiation: `Point(x: 1, y: 2)`
- Positional arguments indicate a function call: `add(1, 2)`

```spl
Point(x: 1, y: 2)      // Struct instantiation (named fields)
Point(x, y)            // Struct instantiation (shorthand, variables x, y)
add(1, 2)              // Function call (positional args)
greet(to: "Alice")     // Function call (named argument)
```

### 2. Generic Arguments

Generic arguments always use parentheses, avoiding the `<`/`>` ambiguity entirely.

```spl
let v: Vec(T: i32) = ...              // Generic type with named arg
let m: HashMap(K: String, V: i32)     // Multiple named type args
Vec(T: i32).new()                     // Type application then method call
```

No turbofish needed - parentheses are unambiguous.

### 3. Explicit Type Application in Function Calls

When calling a generic function with explicit type arguments, type args use uppercase identifiers and value args use lowercase identifiers (see section 9 for details):

```spl
identity(T: i32, 42)              // T (uppercase) = type arg, 42 = positional
convert(From: i32, To: f64, value: 100)  // From, To = type args, value = value arg
parse(T: Config, input: text)     // T = type arg, input = value arg
```

**Case-based disambiguation:**
- Uppercase identifier: type argument, RHS parsed as Type
- Lowercase identifier: value argument, RHS parsed as Expression
- Use `^` to force lowercase as type arg, `@` to force uppercase as value arg

Most generic calls don't need explicit type args due to inference:

```spl
let x = identity(42);     // T inferred as i32
let v = Vec.new();        // Type inferred from later usage
v.push(1);                // Now v: Vec(T: i32)
```

### 4. Paths

All paths use `.` (dot) as the separator. No `::` exists.

```spl
std.vec.Vec              // Module path
self.field               // Field access
Point.new()              // Associated function
```

### 5. Tuple vs Grouped Expression

A parenthesized expression could be a tuple or a grouped expression.

**Rule:** A single expression in parentheses without a trailing comma is a grouped expression. With a trailing comma (or multiple elements), it is a tuple.

```spl
(1 + 2)          // Grouped expression, evaluates to 3
(1,)             // Single-element tuple
(1, 2)           // Two-element tuple
()               // Unit (empty tuple)
```

### 6. Struct Field Shorthand

When a struct field name matches a variable name, the `:` can be omitted.

```spl
let x = 1
let y = 2
Point(x, y)              // Equivalent to Point(x: x, y: y)
Point(x, y: y + 1)       // Mixed shorthand and explicit
```

### 7. Index vs Slice

A bracketed expression could be an index or a slice.

**Rule:** If `:` appears at the top level inside brackets, it is a slice expression. Otherwise, it is an index expression.

```spl
arr[0]           // Index: element at position 0
arr[i + 1]       // Index: element at computed position
arr[$-1]         // Index: last element ($ = length)
arr[$-2]         // Index: second to last element
arr[1:3]         // Slice: elements 1, 2
arr[:3]          // Slice: elements 0, 1, 2
arr[1:]          // Slice: from index 1 to end
arr[1:$]         // Slice: from index 1 to end (explicit $)
arr[1:$-1]       // Slice: all except first and last
arr[:]           // Slice: full copy
```

The `$` symbol represents the array/slice length and is valid in index and slice expressions. It enables Python-style negative indexing: `$-1` is the last element, `$-2` is second to last, etc.

### 8. Associated Functions on Types

To call associated functions on a type, use either:
- **Simple path**: `Point.new()` - for non-generic types
- **Type expression**: `Vec(T: i32).new()` - for generic types

```spl
Point.new(1.0, 2.0)              // Associated function on simple type
Vec(T: i32).new()                // Associated function on generic type
HashMap(K: String, V: i32).new() // Multiple type parameters
Option(T: T).some(value)         // Generic type with type parameter
```

**Grammar distinction:**
- `TypeExpr` requires `GenericArgs`, producing forms like `Vec(T: i32)`
- `PathExpr` is a simple identifier path like `Point` or `std.vec.Vec`

Named type arguments (`T: i32`) are syntactically distinct from value arguments, making type expressions unambiguous.

### 9. Type Arguments vs Value Arguments

SPL uses **case-based disambiguation** to distinguish type arguments from value arguments:

| Identifier Case | Interpretation | RHS Parsed As |
|-----------------|----------------|---------------|
| **Uppercase** (e.g., `T`, `Key`) | Type argument | Type |
| **Lowercase** (e.g., `x`, `name`) | Value argument | Expression |

This allows the parser to determine at parse time whether to invoke the Type or Expression grammar for the right-hand side of `Name: ...`.

**Default rule:**
```spl
// Uppercase identifier → type argument → RHS is a Type
Vec(T: i32)                    // T is uppercase, i32 parsed as Type
HashMap(K: String, V: i32)     // K, V uppercase → type args

// Lowercase identifier → value argument → RHS is an Expression
Point(x: 1, y: 2)              // x, y lowercase → value args
greet(to: "Alice")             // to lowercase → value arg
```

**Escape sigils for exceptions:**

Use `@` to force a value argument with an uppercase identifier:
```spl
// Uppercase field names (e.g., from external APIs)
Config(@URL: "https://example.com", @ID: 123, timeout: 30)
HttpRequest(@Method: "GET", @URI: path)
```

Use `^` to force a type argument with a lowercase identifier:
```spl
// Lowercase type parameters (rare, Haskell-style)
Functor(^f: Option, ^a: Int)
Monad(^m: Result)
```

**Summary table:**

| Syntax | Meaning |
|--------|---------|
| `T: i32` | Type arg (uppercase default) |
| `^t: i32` | Type arg (lowercase with `^` sigil) |
| `x: 1` | Value arg (lowercase default) |
| `@X: 1` | Value arg (uppercase with `@` sigil) |
| `x` | Positional arg or field shorthand |

**Mixed type and value arguments:**
```spl
// Generic function call with explicit type args
parse(T: Config, input: text)          // T = type arg, input = value arg
convert(From: i32, To: f64, value: 100) // From, To = type args, value = value arg

// Struct instantiation with type parameters
Container(T: i32, value: 42)           // T = type arg, value = value arg
```

**Positional arguments:**
- Always parsed as expressions (value arguments)
- Type arguments must be named

```spl
print("hello", 42)             // Positional value args
foo(T: i32, 42, 43)            // T = type arg, 42 and 43 = positional value args
```

### 10. `is` vs Other Operators

The `is` keyword binds looser than comparison but tighter than `&&`.

```spl
x > 0 && y is Some(v)     // (x > 0) && (y is Some(v))
value is Some(x) && x > 0 // (value is Some(x)) && (x > 0)
```

---

## Complete Example

The following program demonstrates key grammar constructs:

```spl
// Struct with parentheses
pub struct Point(
    pub x: T,
    pub y: T,
) where T

impl Point(T: T) where T {
    // Return type with colon
    pub fn new(x: T, y: T): Point(T: T) {
        return Point(x: x, y: y);
    }

    pub fn swap(&mut self) {
        let temp = self.x;
        self.x = self.y;
        self.y = temp;
    }
}

// Type alias with generic
type Pair(T) = (T, T)

// Named parameters with labels
fn distance(from p1: &Point(T: f64), to p2: &Point(T: f64)): f64 {
    let dx = p1.x - p2.x;
    let dy = p1.y - p2.y;
    return (dx * dx + dy * dy).sqrt();
}

fn main() {
    // Struct instantiation with parentheses
    let mut origin = Point.new(0.0, 0.0);
    let target = Point(x: 3.0, y: 4.0);

    // Associated functions on generic types
    let numbers: Vec(T: i32) = Vec(T: i32).new();
    let map = HashMap(K: String, V: i32).new();

    // Named arguments at call site
    let dist = distance(from: &origin, to: &target);

    // Control flow
    if dist > 5.0 {
        return;
    }

    // Pattern matching with is
    let maybe: Option(T: i32) = Some(42);
    if maybe is Some(x) {
        // x is bound
    }

    if maybe.is_some() {
        // value exists
    }

    // Match expression
    let doubled = match maybe {
        Some(n) => n * 2,
        None => 0,
    };

    // Loops
    for i in 0..10 {
        if i % 2 == 0 {
            continue;
        }
        // Process odd numbers
    }

    let mut count = 0;
    while count < 3 {
        count += 1;
    }

    loop {
        if count >= 10 {
            break;
        }
        count += 1;
    }

    // Expressions and operators
    let value = 10 + 5 * 2;             // 20 (multiplicative binds tighter)
    let widened = 65.widen();           // Type conversion via method
    let reference = &mut origin;        // Mutable reference
    let indexed = [1, 2, 3][0];         // Array indexing
    let range = 0..100;                 // Exclusive range (0 to 99)
    let inclusive = 0..=100;            // Inclusive range (0 to 100)

    // Slicing and indexing with $
    let arr = [1, 2, 3, 4, 5];
    let last = arr[$-1];                // 5 (last element)
    let second_last = arr[$-2];         // 4 (second to last)
    let slice1 = arr[1:3];              // [2, 3]
    let slice2 = arr[:3];               // [1, 2, 3]
    let slice3 = arr[2:];               // [3, 4, 5]
    let slice4 = arr[2:$];              // [3, 4, 5] (explicit end)
    let middle = arr[1:$-1];            // [2, 3, 4] (exclude first and last)
    let copy = arr[:];                  // full copy

    // Patterns
    let (a, b) = (1, 2);                // Tuple destructuring
    let Point(x, y) = target;           // Struct destructuring
    let [first, ..rest] = [1, 2, 3, 4]; // Slice pattern with rest
    let [head, .., tail] = [1, 2, 3];   // First and last

    // Block with yield
    let computed = {
        let a = 10;
        let b = 20;
        yield a + b;
    };
}

// Function pointer types (colon for return)
type Predicate = fn(i32): bool
type BinaryOp = fn(i32, i32): i32
type Action = fn()

// Omit labels with underscore
fn apply(_ f: fn(i32): i32, _ x: i32): i32 {
    return f(x);
}

// Self type in impl blocks
impl Point(T: T) where T {
    fn origin(): Self {
        return Self(x: 0, y: 0);
    }

    fn clone(&self): Self {
        return Self(x: self.x, y: self.y);
    }
}
```

---

## Grammar Summary

| Category    | Key Productions                                                     |
|-------------|---------------------------------------------------------------------|
| Program     | `Program`, `Item`, `FunctionDef`, `StructDef`, `EnumDef`, `TraitDef`|
| Modules     | `UseDecl`, `UsePath`, `UseTree`, `ModuleDecl`                       |
| Types       | `Type`, `ReferenceType`, `ArrayType`, `FnPointerType`, `GenericArgs`|
| Statements  | `Block`, `Statement`, `LetStatement`                                |
| Expressions | `Expression`, `TypeExpr`, `IsExpr`, `MatchExpr`, `IfExpr`, `LoopExpr`|
| Patterns    | `Pattern`, `EnumPattern`, `StructPattern`, `SlicePattern`           |
| Literals    | `INTEGER`, `FLOAT`, `STRING`, `CHAR`, `true`, `false`               |

## Key Syntax Differences from Rust

| Feature             | Rust                      | SPL                          |
|---------------------|---------------------------|------------------------------|
| Path separator      | `::`                      | `.`                          |
| Generic application | `Vec<T>`                  | `Vec(T: T)` or `Vec(T: i32)` |
| Type vs value args  | Context-dependent         | Case-based: `T:` = type, `x:` = value |
| Return type         | `-> T`                    | `: T`                        |
| Generic declaration | `fn foo<T>() {}`          | `fn foo() where T {}`        |
| Where clause        | Constrains only           | Declares AND constrains      |
| Turbofish           | `::<T>`                   | Not needed (use parentheses) |
| Struct literal      | `Point { x: 1 }`          | `Point(x: 1)`                |
| Pattern matching    | `if let Some(x) = v {}`   | `if v is Some(x) {}`         |
| Function return     | `expr` (implicit tail)    | `expr` (single) or `return` (multi-stmt) |
| Block value         | `expr` (implicit tail)    | `expr` (single) or `yield` (multi-stmt)  |
| Semicolons          | Semantic (tail vs stmt)   | Required for statements      |
| Named parameters    | Not built-in              | `fn foo(to name: T)`         |
