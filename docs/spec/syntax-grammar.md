# SPL Syntax Grammar

This document defines the syntax grammar of SPL using Extended Backus-Naur Form (EBNF). It builds on the lexical grammar defined in `lexical-grammar.md`.

## Syntax Design Philosophy

SPL uses a clean, consistent syntax with several key principles:

1. **Unified path separator**: Use `.` for all paths (no `::`).
2. **Parentheses for application**: Type arguments use `()` not `<>`: `Vec(i32)`.
3. **Named arguments with `:`**: Struct fields and call args use `:`: `Point(x: 1, y: 2)`.
4. **Return type with `:`**: Functions use `:` for return type: `fn foo(): i32`.
5. **Where clauses for generics**: `fn id(x: T): T where T`.
6. **Pattern matching with `is`**: `if value is Some(x)` instead of `if let`.
7. **Explicit return/yield**: `return` for functions, `yield` for block values. Both require semicolons.
8. **Uniform semicolons**: Semicolons are statement terminators with no semantic significance.

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

ParamList = Param { "," Param } [ "," ] ;

Param = SelfParam | TypedParam ;

SelfParam = [ "&" [ "mut" ] ] "self" ;

(* Parameters with optional labels *)
TypedParam = [ LabelSpec ] [ "mut" ] IDENTIFIER ":" Type ;

(* Label before parameter name: "to name" means call with "to: value" *)
(* "_" means no label required at call site *)
LabelSpec = "_" | IDENTIFIER ;

(* Where clause for generic constraints *)
WhereClause = "where" TypeParam { "," TypeParam } ;

TypeParam = IDENTIFIER [ ":" TypeBound { "+" TypeBound } ] ;

TypeBound = TypePath [ GenericArgs ] ;
```

**Examples:**

```spl
// Simple function with return type
fn add(a: i32, b: i32): i32 {
    return a + b;
}

// Generic function with where clause
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

// Generic struct with where clause
struct Box(value: T) where T

// Public fields
struct Point(pub x: f64, pub y: f64)

// Generic with bounds
struct Container(items: Vec(T)) where T: Clone
```

### Enum Definitions

```ebnf
(* Enums use parentheses for variants, consistent with struct syntax *)
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

// Enum with data
enum Option(T)(
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
enum Result(T, E)(
    Ok(T),
    Err(E),
) where T, E
```

### Trait Definitions

```ebnf
(* Traits use braces for their body *)
TraitDef = "trait" IDENTIFIER [ WhereClause ] "{" { TraitItem } "}" ;

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
ImplBlock = "impl" [ TypePath "for" ] TypePath [ GenericArgs ] [ WhereClause ] "{" { ImplItem } "}" ;

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
impl Clone for Option(T) where T: Clone {
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
impl Box(T) where T {
    pub fn unwrap(self): T {
        return self.value;
    }
}

// Impl with bounds
impl Container(T) where T: Clone {
    pub fn clone_all(&self): Vec(T) {
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
    fn variadic_fn(fmt: Ptr(u8), ...): i32;
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
Type = BaseType [ "?" ] ;              (* Optional postfix: T? = Option(T) *)

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

(* Generic args use parentheses, support named args *)
GenericArgs = "(" [ TypeArg { "," TypeArg } [ "," ] ] ")" ;

TypeArg = Type                       (* positional type argument *)
        | IDENTIFIER "=" Type ;       (* named type argument *)
```

### Type Examples

| Syntax              | Description                        |
|---------------------|------------------------------------|
| `i32`               | Simple type                        |
| `Point(T)`          | Generic type                       |
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
| `HashMap(K, V)`     | Multi-param generic type           |
| `Result(T, E = Error)` | Named type argument             |
| `i32?`              | Optional type (sugar for `Option(i32)`) |
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
ArgList = Arg { "," Arg } [ "," ] ;

Arg = Expression
    | IDENTIFIER ":" Expression ;  (* named argument *)
```

### Primary Expressions

```ebnf
PrimaryExpr = LiteralExpr
            | EnumShorthandExpr
            | PathExpr
            | GroupedExpr
            | TupleExpr
            | ArrayExpr
            | StructExpr
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

LiteralExpr = INTEGER | FLOAT | STRING | CHAR | "true" | "false" ;

(* Enum variant shorthand - type inferred from context *)
EnumShorthandExpr = "." IDENTIFIER [ "(" [ ExpressionList ] ")" ] ;

(* Paths use dot separator *)
PathExpr = IDENTIFIER { "." IDENTIFIER } ;

GroupedExpr = "(" Expression ")" ;

TupleExpr = "(" [ Expression "," [ Expression { "," Expression } ] [ "," ] ] ")" ;

ArrayExpr = "[" [ Expression { "," Expression } [ "," ] ] "]"
          | "[" Expression ";" Expression "]" ;

(* Struct instantiation uses parentheses with = for fields *)
StructExpr = StructExprPath "(" [ StructFieldList ] ")" ;

StructExprPath = TypePath [ GenericArgs ]
               | "Self" ;

StructFieldList = StructField { "," StructField } [ "," ] ;

(* Field with optional expression; colon separates name from value *)
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
let b = Box(i32)(value: 42)

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
let result: Result(i32, Error) = .Ok(42)

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

WhileExpr = "while" Expression Block ;

ForExpr = "for" Pattern "in" Expression Block ;

LoopExpr = "loop" Block ;

BreakExpr = "break" [ Expression ] ;

ContinueExpr = "continue" ;

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
Pattern = IdentifierPattern
        | WildcardPattern
        | LiteralPattern
        | RangePattern
        | TuplePattern
        | SlicePattern
        | StructPattern
        | EnumPattern
        | ReferencePattern ;

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
let v: Vec(i32) = ...       // Generic type
let m: HashMap(String, i32) // Multiple type args
Vec(i32).new()              // Type application then method call
```

No turbofish needed - parentheses are unambiguous.

### 3. Paths

All paths use `.` (dot) as the separator. No `::` exists.

```spl
std.vec.Vec              // Module path
self.field               // Field access
Point.new()              // Associated function
```

### 4. Tuple vs Grouped Expression

A parenthesized expression could be a tuple or a grouped expression.

**Rule:** A single expression in parentheses without a trailing comma is a grouped expression. With a trailing comma (or multiple elements), it is a tuple.

```spl
(1 + 2)          // Grouped expression, evaluates to 3
(1,)             // Single-element tuple
(1, 2)           // Two-element tuple
()               // Unit (empty tuple)
```

### 5. Struct Field Shorthand

When a struct field name matches a variable name, the `:` can be omitted.

```spl
let x = 1
let y = 2
Point(x, y)              // Equivalent to Point(x: x, y: y)
Point(x, y: y + 1)       // Mixed shorthand and explicit
```

### 6. Index vs Slice

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

### 7. `is` vs Other Operators

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

impl Point(T) where T {
    // Return type with colon
    pub fn new(x: T, y: T): Point(T) {
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
fn distance(from p1: &Point(f64), to p2: &Point(f64)): f64 {
    let dx = p1.x - p2.x;
    let dy = p1.y - p2.y;
    return (dx * dx + dy * dy).sqrt();
}

fn main() {
    // Struct instantiation with parentheses
    let mut origin = Point.new(0.0, 0.0);
    let target = Point(x: 3.0, y: 4.0);

    // Named arguments at call site
    let dist = distance(from: &origin, to: &target);

    // Control flow
    if dist > 5.0 {
        return;
    }

    // Pattern matching with is
    let maybe: Option(i32) = Some(42);
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
impl Point(T) where T {
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
| Expressions | `Expression`, `IsExpr`, `MatchExpr`, `IfExpr`, `LoopExpr`           |
| Patterns    | `Pattern`, `EnumPattern`, `StructPattern`, `SlicePattern`           |
| Literals    | `INTEGER`, `FLOAT`, `STRING`, `CHAR`, `true`, `false`               |

## Key Syntax Differences from Rust

| Feature             | Rust                      | SPL                          |
|---------------------|---------------------------|------------------------------|
| Path separator      | `::`                      | `.`                          |
| Generic application | `Vec<T>`                  | `Vec(T)`                     |
| Return type         | `-> T`                    | `: T`                        |
| Generic declaration | `fn foo<T>() {}`          | `fn foo() where T {}`        |
| Turbofish           | `::<T>`                   | Not needed (use parentheses) |
| Struct literal      | `Point { x: 1 }`          | `Point(x: 1)`                |
| Pattern matching    | `if let Some(x) = v {}`   | `if v is Some(x) {}`         |
| Function return     | `expr` (implicit tail)    | `expr` (single) or `return` (multi-stmt) |
| Block value         | `expr` (implicit tail)    | `expr` (single) or `yield` (multi-stmt)  |
| Semicolons          | Semantic (tail vs stmt)   | Required for statements      |
| Named parameters    | Not built-in              | `fn foo(to name: T)`         |
