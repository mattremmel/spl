# SPL Syntax Grammar

This document defines the syntax grammar of SPL using Extended Backus-Naur Form (EBNF). It builds on the lexical grammar defined in `lexical-grammar.md`.

## Syntax Design Philosophy

SPL uses a clean, consistent syntax with several key principles:

1. **Unified path separator**: Use `.` for all paths (no `::`).
2. **Parentheses for application**: Type arguments use `()` not `<>`: `Vec(i32)`.
3. **Named arguments with `=`**: Struct fields and call args use `=`: `Point(x = 1, y = 2)`.
4. **Return type with `:`**: Functions use `:` for return type: `fn foo(): i32`.
5. **Where clauses for generics**: `fn id(x: T): T where T`.
6. **Pattern matching with `is`**: `if value is Some(x)` instead of `if let`.
7. **Explicit return**: `return` keyword required for returning values.

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
Program = { Item } ;

Item = [ "pub" ] ( FunctionDef | StructDef | ImplBlock | TypeAlias | UseDecl | ModDecl ) ;
```

### Function Definitions

```ebnf
FunctionDef = "fn" IDENTIFIER "(" [ ParamList ] ")" [ ":" Type ] [ WhereClause ] Block ;

ParamList = Param { "," Param } [ "," ] ;

Param = SelfParam | TypedParam ;

SelfParam = [ "&" [ "mut" ] ] "self" ;

(* Parameters with optional labels *)
TypedParam = [ LabelSpec ] [ "mut" ] IDENTIFIER ":" Type ;

(* Label before parameter name: "to name" means call with "to = value" *)
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
    return a + b
}

// Generic function with where clause
fn identity(x: T): T where T {
    return x
}

// Named parameters (external label differs from internal name)
fn greet(to person: String) {
    // Called as: greet(to = "Alice")
}

// Omit label with underscore
fn add(_ a: i32, _ b: i32): i32 {
    // Called as: add(1, 2) instead of add(a = 1, b = 2)
    return a + b
}

// Generic with bounds
fn clone_it(x: &T): T where T: Clone {
    return x.clone()
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

### Implementation Blocks

```ebnf
(* Impl blocks use parentheses for generic args *)
ImplBlock = "impl" TypePath [ GenericArgs ] [ WhereClause ] "{" { ImplItem } "}" ;

ImplItem = [ "pub" ] FunctionDef ;
```

**Examples:**

```spl
// Simple impl
impl Point {
    pub fn new(x: f64, y: f64): Point {
        return Point(x = x, y = y)
    }
}

// Generic impl with where clause
impl Box(T) where T {
    pub fn unwrap(self): T {
        return self.value
    }
}

// Impl with bounds
impl Container(T) where T: Clone {
    pub fn clone_all(&self): Vec(T) {
        return self.items.clone()
    }
}
```

### Type Aliases

```ebnf
TypeAlias = "type" IDENTIFIER [ GenericParams ] "=" Type ";" ;
```

### Use Declarations

Import items or modules into scope. See `module-system.md` for full details.

```ebnf
UseDecl = "use" UsePath ";" ;

UsePath = PathPrefix [ "." UseTree ] ;

PathPrefix = [ "crate" | "super" | "self" ] "." IDENTIFIER { "." IDENTIFIER }
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
| `use crate.utils.helper;` | Crate-relative import |
| `use super.common;` | Parent module import |

### Module Declarations

Declare submodules. Only valid in `_module.spl` files.

```ebnf
ModDecl = "mod" IDENTIFIER ";" ;
```

**Examples:**

| Syntax | Description |
|--------|-------------|
| `mod network;` | Private submodule |
| `pub mod api;` | Public submodule |

---

## 2. Types

```ebnf
Type = ReferenceType
     | ArrayType
     | TupleType
     | FnPointerType
     | NeverType
     | PathType ;

ReferenceType = "&" [ "mut" ] Type ;

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

---

## 3. Statements

```ebnf
Block = "{" { Statement } [ Expression ] "}" ;

Statement = LetStatement
          | ExpressionStatement ;

LetStatement = "let" [ "mut" ] Pattern [ ":" Type ] [ "=" Expression ] ";" ;

ExpressionStatement = Expression ";"
                    | BlockExpression [ ";" ] ;
```

Block expressions (`if`, `while`, `for`, `loop`, and bare blocks) may omit the trailing semicolon when used as statements. The optional trailing expression in a `Block` becomes the block's value.

---

## 4. Expressions

Expressions are defined using layered production rules that encode operator precedence. Lower precedence operators are defined first; they call higher precedence rules.

### Precedence Table

| Precedence | Operators                    | Associativity | Production         |
|------------|------------------------------|---------------|--------------------|
| 1 (lowest) | `=` `+=` `-=` `*=` `/=` `%=` | Right         | AssignmentExpr     |
| 2          | `\|\|`                       | Left          | OrExpr             |
| 3          | `&&`                         | Left          | AndExpr            |
| 4          | `is` `is not`                | Left          | IsExpr             |
| 5          | `==` `!=`                    | Left          | EqualityExpr       |
| 6          | `<` `>` `<=` `>=`            | Left          | ComparisonExpr     |
| 7          | `..`                         | Left          | RangeExpr          |
| 8          | `+` `-`                      | Left          | AdditiveExpr       |
| 9          | `*` `/` `%`                  | Left          | MultiplicativeExpr |
| 10         | `as`                         | Left          | CastExpr           |
| 11         | `!` `-` `&` (unary)          | Right         | UnaryExpr          |
| 12 (highest)| `.` `()` `[]` `[:]`         | Left          | PostfixExpr        |

### Expression Grammar

```ebnf
Expression = AssignmentExpr ;

AssignmentExpr = OrExpr [ AssignOp AssignmentExpr ] ;

AssignOp = "=" | "+=" | "-=" | "*=" | "/=" | "%=" ;

OrExpr = AndExpr { "||" AndExpr } ;

AndExpr = IsExpr { "&&" IsExpr } ;

(* Pattern matching with is/is not *)
IsExpr = EqualityExpr [ "is" [ "not" ] Pattern ] ;

EqualityExpr = ComparisonExpr { ( "==" | "!=" ) ComparisonExpr } ;

ComparisonExpr = RangeExpr { ( "<" | ">" | "<=" | ">=" ) RangeExpr } ;

RangeExpr = AdditiveExpr [ ".." [ AdditiveExpr ] ] ;

AdditiveExpr = MultiplicativeExpr { ( "+" | "-" ) MultiplicativeExpr } ;

MultiplicativeExpr = CastExpr { ( "*" | "/" | "%" ) CastExpr } ;

(* Cast only allows safe conversions *)
CastExpr = UnaryExpr { "as" Type } ;

UnaryExpr = ( "!" | "-" | "&" [ "mut" ] ) UnaryExpr
          | PostfixExpr ;

(* No :: for paths - use . only *)
PostfixExpr = PrimaryExpr { PostfixOp } ;

PostfixOp = "." IDENTIFIER [ GenericArgs ]                   (* field or associated item *)
          | "." IDENTIFIER [ GenericArgs ] "(" [ ArgList ] ")" (* method call *)
          | "(" [ ArgList ] ")"                               (* function call *)
          | "[" Expression "]"                                (* index *)
          | "[" SliceExpr "]" ;                               (* slice *)

SliceExpr = [ Expression ] ":" [ Expression | "$" ] ;

(* Arguments can be named with = *)
ArgList = Arg { "," Arg } [ "," ] ;

Arg = Expression
    | IDENTIFIER "=" Expression ;  (* named argument *)
```

### Primary Expressions

```ebnf
PrimaryExpr = LiteralExpr
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
            | ReturnExpr ;

LiteralExpr = INTEGER | FLOAT | STRING | CHAR | "true" | "false" ;

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

(* Field with optional expression; no colon needed *)
StructField = IDENTIFIER [ "=" Expression ] ;

(* Match expression *)
MatchExpr = "match" Expression "{" { MatchArm } "}" ;

MatchArm = Pattern [ "if" Expression ] "=>" Expression "," ;
```

**Struct Expression Examples:**

```spl
// All fields with values
let p = Point(x = 1, y = 2)

// Shorthand when variable name matches field
let x = 1
let y = 2
let p = Point(x, y)  // Same as Point(x = x, y = y)

// Generic type instantiation
let b = Box(T = i32)(value = 42)

// Self in impl blocks
impl Point {
    fn origin(): Self {
        return Self(x = 0, y = 0)
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

(* Explicit return required for returning values *)
ReturnExpr = "return" [ Expression ] ;
```

**Pattern Matching in Control Flow:**

The `is` operator enables pattern matching directly in conditions:

```spl
// Pattern matching with is
if value is Some(x) {
    // x is bound here
}

// Negated pattern
if value is not None {
    // value is Some(_)
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

**Explicit Return:**

Functions must use `return` to return values. The last expression in a block does NOT implicitly return.

```spl
// Correct: explicit return
fn double(x: i32): i32 {
    return x * 2
}

// Error: missing return statement
fn bad(x: i32): i32 {
    x * 2  // This does NOT return!
}
```

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

RangePattern = LiteralPattern ".." LiteralPattern ;

TuplePattern = "(" [ Pattern { "," Pattern } [ "," ] ] ")" ;

SlicePattern = "[" [ SlicePatternElement { "," SlicePatternElement } [ "," ] ] "]" ;

SlicePatternElement = RestPattern | Pattern ;

RestPattern = ".." [ IDENTIFIER ] ;

(* Struct patterns use parentheses *)
StructPattern = TypePath "(" [ StructPatternFields ] ")" ;

StructPatternFields = StructPatternField { "," StructPatternField } [ "," ] [ ".." ] ;

(* Field with optional pattern binding *)
StructPatternField = IDENTIFIER [ "=" Pattern ] ;

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
| `0..10`               | Match range (inclusive start, exclusive end) |
| `'a'..'z'`            | Match character range                |
| `(a, b)`              | Destructure tuple                    |
| `[a, b, c]`           | Destructure fixed-size array/slice   |
| `[first, ..]`         | Match first, ignore rest             |
| `[first, ..rest]`     | Match first, bind rest to `rest`     |
| `[.., last]`          | Match last element                   |
| `[first, ..middle, last]` | Match first, last, bind middle   |
| `Point(x, y)`         | Destructure struct (shorthand)       |
| `Point(x = a, y = b)` | Destructure with rename              |
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
| 4    | Pattern Match  | `is` `is not`                  | Left  | `x is Some(v)`            |
| 5    | Equality       | `==` `!=`                      | Left  | `a == b != c`             |
| 6    | Comparison     | `<` `>` `<=` `>=`              | Left  | `a < b`                   |
| 7    | Range          | `..`                           | Left  | `0..10`                   |
| 8    | Additive       | `+` `-`                        | Left  | `a + b - c`               |
| 9    | Multiplicative | `*` `/` `%`                    | Left  | `a * b / c`               |
| 10   | Cast           | `as`                           | Left  | `x as i32 as f64`         |
| 11   | Unary          | `!` `-` `&` `&mut`             | Right | `!&mut x`                 |
| 12   | Postfix        | `.` `()` `[]` `[:]`            | Left  | `a.b().c[0]`              |

---

## Ambiguity Resolution

### 1. Struct Expression vs Function Call

When the parser sees `IDENTIFIER(`, it must determine if this is a struct instantiation or a function call.

**Rule:** Context and argument syntax disambiguate:
- Named arguments with `=` indicate struct instantiation: `Point(x = 1, y = 2)`
- Positional arguments indicate a function call: `add(1, 2)`

```spl
Point(x = 1, y = 2)    // Struct instantiation (named fields)
Point(x, y)            // Struct instantiation (shorthand, variables x, y)
add(1, 2)              // Function call (positional args)
greet(to = "Alice")    // Function call (named argument)
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

When a struct field name matches a variable name, the `=` can be omitted.

```spl
let x = 1
let y = 2
Point(x, y)              // Equivalent to Point(x = x, y = y)
Point(x, y = y + 1)      // Mixed shorthand and explicit
```

### 6. Index vs Slice

A bracketed expression could be an index or a slice.

**Rule:** If `:` appears at the top level inside brackets, it is a slice expression. Otherwise, it is an index expression.

```spl
arr[0]           // Index: element at position 0
arr[i + 1]       // Index: element at computed position
arr[1:3]         // Slice: elements 1, 2
arr[:3]          // Slice: elements 0, 1, 2
arr[1:]          // Slice: from index 1 to end
arr[1:$]         // Slice: from index 1 to end (explicit $)
arr[:]           // Slice: full copy
```

The `$` symbol represents the end of the array and is only valid in slice expressions.

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
        return Point(x = x, y = y)
    }

    pub fn swap(&mut self) {
        let temp = self.x
        self.x = self.y
        self.y = temp
    }
}

// Type alias with generic
type Pair(T) = (T, T)

// Named parameters with labels
fn distance(from p1: &Point(f64), to p2: &Point(f64)): f64 {
    let dx = p1.x - p2.x
    let dy = p1.y - p2.y
    return (dx * dx + dy * dy).sqrt()
}

fn main() {
    // Struct instantiation with parentheses
    let mut origin = Point.new(0.0, 0.0)
    let target = Point(x = 3.0, y = 4.0)

    // Named arguments at call site
    let dist = distance(from = &origin, to = &target)

    // Control flow
    if dist > 5.0 {
        return
    }

    // Pattern matching with is
    let maybe: Option(i32) = Some(42)
    if maybe is Some(x) {
        // x is bound
    }

    if maybe is not None {
        // value exists
    }

    // Match expression
    let doubled = match maybe {
        Some(n) => n * 2,
        None => 0,
    }

    // Loops
    for i in 0..10 {
        if i % 2 == 0 {
            continue
        }
        // Process odd numbers
    }

    let mut count = 0
    while count < 3 {
        count += 1
    }

    loop {
        if count >= 10 {
            break
        }
        count += 1
    }

    // Expressions and operators
    let value = 10 + 5 * 2              // 20 (multiplicative binds tighter)
    let cast = 65 as f64                // Type cast (safe only)
    let reference = &mut origin         // Mutable reference
    let indexed = [1, 2, 3][0]          // Array indexing
    let range = 0..100                  // Range

    // Slicing
    let arr = [1, 2, 3, 4, 5]
    let slice1 = arr[1:3]               // [2, 3]
    let slice2 = arr[:3]                // [1, 2, 3]
    let slice3 = arr[2:]                // [3, 4, 5]
    let slice4 = arr[2:$]               // [3, 4, 5] (explicit end)
    let copy = arr[:]                   // full copy

    // Patterns
    let (a, b) = (1, 2)                 // Tuple destructuring
    let Point(x, y) = target            // Struct destructuring
    let [first, ..rest] = [1, 2, 3, 4]  // Slice pattern with rest
    let [head, .., tail] = [1, 2, 3]    // First and last
}

// Function pointer types (colon for return)
type Predicate = fn(i32): bool
type BinaryOp = fn(i32, i32): i32
type Action = fn()

// Omit labels with underscore
fn apply(_ f: fn(i32): i32, _ x: i32): i32 {
    return f(x)
}

// Self type in impl blocks
impl Point(T) where T {
    fn origin(): Self {
        return Self(x = 0, y = 0)
    }

    fn clone(&self): Self {
        return Self(x = self.x, y = self.y)
    }
}
```

---

## Grammar Summary

| Category    | Key Productions                                                     |
|-------------|---------------------------------------------------------------------|
| Program     | `Program`, `Item`, `FunctionDef`, `StructDef`, `WhereClause`        |
| Modules     | `UseDecl`, `UsePath`, `UseTree`, `ModDecl`                          |
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
| Struct literal      | `Point { x: 1 }`          | `Point(x = 1)`               |
| Pattern matching    | `if let Some(x) = v {}`   | `if v is Some(x) {}`         |
| Implicit return     | Last expression           | Explicit `return` required   |
| Named parameters    | Not built-in              | `fn foo(to name: T)`         |
