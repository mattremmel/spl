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
8. **Explicit return/break**: `return` for functions, `break` for block values. Both require semicolons.
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

VisibilityScope = "$" [ "." Path ] | "super" | "in" Path ;
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
FunctionDef = "fn" IDENTIFIER "(" [ ParamList ] ")" [ ":" Type ] [ ThrowsClause ] [ WhereClause ] Block ;

(* Generator functions yield multiple values lazily *)
GeneratorDef = "gen" "fn" IDENTIFIER "(" [ ParamList ] ")" ":" Type [ WhereClause ] Block ;

(* Throws clause for functions returning Result *)
ThrowsClause = "throws" [ Type ] ;

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

// Function with typed throws (returns Result(T: String, E: IoError))
fn read_file(path: &str): String throws IoError {
    if !path.exists() {
        throw .NotFound(path);
    }
    return fs.read_to_string(path);
}

// Function with untyped throws (returns Result(T: Data, E: Error))
fn process(input: &str): Data throws {
    let parsed = parse(input)!;
    return transform(parsed);
}
```

### Struct Definitions

```ebnf
(* All structs use parentheses - named vs positional distinguished by : *)
StructDef = "struct" IDENTIFIER "(" [ FieldList ] ")" [ WhereClause ] ;

FieldList = Field { "," Field } [ "," ] ;

(* Field with optional name - named: `x: i32`, positional: `i32` *)
Field = [ "pub" ] ( IDENTIFIER ":" Type | Type ) ;
```

**Examples:**

```spl
// Named struct - fields accessed by name
struct Point(x: f64, y: f64)

// Positional struct - fields accessed by position (.0, .1, etc.)
struct Pair(i32, i32)

// Empty structs
struct Empty()
struct Unit()

// Generic named struct
struct Box(value: T) where T

// Generic positional struct (wrapper type)
struct Wrapper(T) where T

// Public fields
pub struct Point(pub x: f64, pub y: f64)
pub struct Newtype(pub i32)

// Generic with bounds
struct Container(items: Vec(T: T)) where T: Clone

// Multiple type parameters
struct Pair(first: T, second: U) where T, U

// Multiple type parameters with bounds
struct Map(keys: Vec(T: K), values: Vec(T: V)) where K: Hash + Eq, V
```

**Syntax Rationale:**

SPL uses a consistent delimiter philosophy:
- **Braces `{}`** = code blocks and item lists (enum body, trait body, impl body, function body)
- **Parentheses `()`** = data shapes (struct fields, enum variant data, tuples, function params, generic args)

All structs use parentheses because declaration mirrors usage:
- Declaration: `struct Point(x: i32, y: i32)`
- Instantiation: `Point(x: 1, y: 2)`
- Pattern: `let Point(x, y) = p`

Named vs positional fields are distinguished by the presence of `:` after an identifier:
- Named: `Point(x: i32, y: i32)` - has `name: type`
- Positional: `Pair(i32, i32)` - just types

### Enum Definitions

```ebnf
(* Enums use braces for declarations, like structs *)
(* Type parameters are used inline in variants and declared in where clause *)
EnumDef = "enum" IDENTIFIER "{" [ VariantList ] "}" [ WhereClause ] ;

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
enum Color{Red, Green, Blue}

// Enum with data (type params used inline, declared in where)
enum Option{
    Some(T),
    None,
} where T

// Enum with named fields in variants
enum Message{
    Quit,
    Move(x: i32, y: i32),     // named fields
    Write(String),             // tuple variant
    ChangeColor(u8, u8, u8),   // tuple variant
}

// Result type
enum Result{
    Ok(T),
    Err(E),
} where T, E
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
    fn next(&mut self): Self.Item?;
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

**Impl Block Patterns:**

SPL's `where` clause both **declares** and optionally **constrains** type parameters for impl blocks. This eliminates the redundancy of Rust's `impl<T> Type<T>` pattern.

```spl
// Simple impl (non-generic type)
impl Point {
    pub fn new(x: f64, y: f64): Point {
        return Point(x: x, y: y);
    }
}

// Generic impl - `where T` declares the type parameter
impl Box(T: T) where T {
    pub fn unwrap(self): T {
        return self.value;
    }
}

// Conditional impl - bounds restrict which types this impl applies to
impl Container(T: T) where T: Clone {
    pub fn clone_all(&self): Vec(T: T) {
        return self.items.clone();
    }
}

// Concrete impl - no where clause, implements for specific type
impl Box(T: u32) {
    pub fn special_u32_method(&self): u32 {
        return self.value * 2;
    }
}

impl Box(T: String) {
    pub fn special_string_method(&self): usize {
        return self.value.len();
    }
}

// Different names for impl vs struct type parameters
// Emphasizes that T is the parameter NAME and R is the TYPE
struct Wrapper(val: T) where T
impl Wrapper(T: R) where R {
    fn get(&self): &R { return &self.val; }
}

// Method-level generics - methods can have additional type parameters
impl Vec(T: T) where T {
    fn convert(&self): Vec(T: U) where U, T: Into(Target: U) {
        // Each element converted from T to U
    }
}

// Multiple concrete trait implementations
trait Format {
    fn display(&self): String;
}

struct Amount(value: T) where T

impl Format for Amount(T: USD) {
    fn display(&self): String {
        return format("${}", self.value.cents);
    }
}

impl Format for Amount(T: EUR) {
    fn display(&self): String {
        return format("{}€", self.value.cents);
    }
}
```

### Type Aliases

```ebnf
TypeAlias = "type" IDENTIFIER [ GenericParams ] "=" Type [ WhereClause ] ";" ;

GenericParams = "(" TypeParamList ")" ;

TypeParamList = IDENTIFIER { "," IDENTIFIER } [ "," ] ;
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
         | FnType
         | NeverType
         | PathType ;

ReferenceType = "&" [ "mut" ] BaseType ;

ArrayType = "[" Type [ ";" Expression ] "]" ;

(* Tuple types support optional named fields *)
TupleType = "(" [ TupleTypeElement { "," TupleTypeElement } [ "," ] ] ")" ;

TupleTypeElement = [ IDENTIFIER ":" ] Type ;

(* Function type return uses colon *)
FnType = "fn" "(" [ TypeList ] ")" [ ":" Type ] ;

TypeList = Type { "," Type } [ "," ] ;

NeverType = "!" ;

PathType = TypePath [ GenericArgs ]
         | SelfType ;

SelfType = "Self" [ "." IDENTIFIER ] ;  (* Self or Self.AssociatedType *)

(* Paths use dot, not double-colon *)
TypePath = IDENTIFIER { "." IDENTIFIER } ;

(* Generic args use parentheses with named type arguments *)
(* Case-based disambiguation: uppercase identifier = type arg, lowercase = value arg *)
(* Parser uses case to choose initial parse path, backtracks on failure *)
(* Semantic analysis can reinterpret nodes when resolution reveals the opposite was intended *)
GenericArgs = "(" [ TypeArg { "," TypeArg } [ "," ] ] ")" ;

TypeArg = IDENTIFIER ":" Type ;       (* named type argument, e.g., T: i32 *)
```

### Type Examples

| Syntax              | Description                        |
|---------------------|------------------------------------|
| `i32`               | Simple type                        |
| `Point(T: i32)`     | Generic type with named arg        |
| `std.vec.Vec`       | Qualified path type                |
| `Self`              | Self type (in impl blocks)         |
| `Self.Item`         | Associated type on Self             |
| `&T`                | Immutable reference                |
| `&mut T`            | Mutable reference                  |
| `[T]`               | Slice type                         |
| `[T; 10]`           | Fixed-size array                   |
| `(T, U)`            | Tuple type (positional)            |
| `(x: T, y: U)`      | Tuple type (named fields)          |
| `(T, name: U)`      | Tuple type (mixed)                 |
| `()`                | Unit type                          |
| `fn(i32): bool`     | Function type                      |
| `fn(T, U): V`       | Generic function type              |
| `fn()`              | Function type returning unit       |
| `!`                 | Never type                         |
| `HashMap(K: String, V: i32)` | Multi-param generic type   |
| `Result(T: i32, E: Error)` | Named type arguments         |
| `i32?`              | Optional type (sugar for `Option(T: i32)`) |
| `String?`           | Optional String                    |
| `&T?`               | Reference to optional (rare)       |

### Named Tuples

Tuples can have optional named fields, enabling anonymous record types without defining a struct:

```spl
// Named tuple type as return type
fn get_coords(x: f64): (identity: f64, square: f64) {
    return (identity: x, square: x * x);
}

// Named tuple in variable binding
let point: (x: i32, y: i32) = (x: 1, y: 2);

// Access by name
let x_val = point.x;

// Positional access also works
let y_val = point.1;

// Mixed positional and named (allowed)
let mixed: (i32, name: String) = (42, name: "hello");
```

**Use Cases:**
- Return multiple values with self-documenting field names
- Ad-hoc data grouping without struct definitions
- API boundaries where named fields improve clarity

**Field Access:**
- Named fields: `tuple.field_name`
- Positional fields: `tuple.0`, `tuple.1`, etc.
- Both access styles work regardless of whether fields were declared with names

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
| `break` statement | Semicolon required: `break value;` |
| `yield` statement (generators) | Semicolon required: `yield value;` |
| Expression in block (not break) | Semicolon required, value discarded |

The semicolon does NOT determine whether an expression's value is used (unlike Rust). Instead, `return` and `yield` explicitly indicate intent.

**Block Values:**

Blocks containing multiple statements require explicit `break` to produce a value:

```spl
let result = {
    let a = compute();
    let b = transform(a);
    break a + b;
};
```

However, blocks containing a **single expression** have an implicit value—no `break` is needed:

```spl
let doubled = if x > 0 { x * 2 } else { 0 };  // Single expression per branch
let value = { compute() };                     // Single expression block
```

Without `break` or a single expression, a block's type is `()` (unit).

---

## 4. Expressions

Expressions are defined using layered production rules that encode operator precedence. Lower precedence operators are defined first; they call higher precedence rules.

### Precedence Table

| Precedence | Operators                    | Associativity | Production         |
|------------|------------------------------|---------------|--------------------|
| 1 (lowest) | `=` `+=` `-=` `*=` `/=` `%=` | Right         | AssignmentExpr     |
| 2          | `??`                         | Right         | CoalesceExpr       |
| 3          | `\|\|`                       | Left          | OrExpr             |
| 4          | `&&`                         | Left          | AndExpr            |
| 5          | `is`                         | Left          | IsExpr             |
| 6          | `==` `!=`                    | Left          | EqualityExpr       |
| 7          | `<` `>` `<=` `>=`            | Left          | ComparisonExpr     |
| 8          | `..` `..=`                   | Left          | RangeExpr          |
| 9          | `+` `-`                      | Left          | AdditiveExpr       |
| 10         | `*` `/` `%`                  | Left          | MultiplicativeExpr |
| 11         | `!` `-` `&` (unary)          | Right         | UnaryExpr          |
| 12 (highest)| `.` `?.` `()` `[]` `[:]` `!` | Left          | PostfixExpr        |

Note: Type conversions use methods (`.widen()`, `.truncate()`, `.try_into()`) rather than a cast operator.

### Expression Grammar

```ebnf
Expression = AssignmentExpr ;

AssignmentExpr = CoalesceExpr [ AssignOp AssignmentExpr ] ;

AssignOp = "=" | "+=" | "-=" | "*=" | "/=" | "%=" ;

CoalesceExpr = OrExpr { "??" OrExpr } ;

OrExpr = AndExpr { "||" AndExpr } ;

AndExpr = IsExpr { "&&" IsExpr } ;

(* Pattern matching with is *)
IsExpr = EqualityExpr [ "is" Pattern ] ;

EqualityExpr = ComparisonExpr { ( "==" | "!=" ) ComparisonExpr } ;

ComparisonExpr = RangeExpr { ( "<" | ">" | "<=" | ">=" ) RangeExpr } ;

RangeExpr = AdditiveExpr [ ( ".." | "..=" ) [ AdditiveExpr ] ] ;

AdditiveExpr = MultiplicativeExpr { ( "+" | "-" ) MultiplicativeExpr } ;

MultiplicativeExpr = UnaryExpr { ( "*" | "/" | "%" ) UnaryExpr } ;

UnaryExpr = ( "!" | "-" | "&" [ "mut" ] | "*" ) UnaryExpr
          | PostfixExpr ;

(* Note: The `*` operator dereferences references (`*r = 10`). Raw pointers
   (`Ptr`, `MutPtr`) use explicit `.read()` and `.write()` methods instead—
   see unsafe.md. *)

(* No :: for paths - use . only *)
PostfixExpr = PrimaryExpr { PostfixOp } ;

PostfixOp = "." IDENTIFIER [ GenericArgs ]                   (* field or associated item *)
          | "." IDENTIFIER [ GenericArgs ] "(" [ ArgList ] ")" (* method call *)
          | "?." IDENTIFIER                                   (* optional chain field *)
          | "?." IDENTIFIER "(" [ ArgList ] ")"               (* optional chain method *)
          | "(" [ ArgList ] ")"                               (* function call *)
          | "[" Expression "]"                                (* index *)
          | "[" SliceExpr "]"                                 (* slice *)
          | "!" ;                                             (* try/propagate *)

SliceExpr = [ IndexExpr ] ":" [ IndexExpr ] ;

IndexExpr = Expression
          | "$" [ "-" Expression ] ;  (* $ = length, $-1 = last index *)

(* Arguments can be named with : *)
(* Case-based disambiguation: uppercase identifier = type arg, lowercase = value arg *)
(* Parser backtracks if initial parse (based on case) fails *)
ArgList = Arg { "," Arg } [ "," ] ;

Arg = NamedArg                              (* named argument: case determines type vs value *)
    | Expression ;                          (* positional argument *)

NamedArg = IDENTIFIER ":" TypeOrExpr ;      (* case disambiguates, parser backtracks if needed *)
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
            | YieldExpr
            | ThrowExpr ;

(* Type expressions - types used as values for associated function calls *)
(* Requires GenericArgs to distinguish from PathExpr *)
TypeExpr = TypePath GenericArgs ;

(* Closures - see closures.md for full semantics *)
ClosureExpr = [ "@" CaptureList ] ClosureParams ClosureBody ;

CaptureList = "[" [ Capture { "," Capture } [ "," ] ] "]" ;

Capture = IDENTIFIER                       (* shorthand: x means x: x *)
        | IDENTIFIER ":" Expression ;      (* explicit: name: expr *)

ClosureParams = "||"
              | "|" [ ParamList ] "|" ;

ClosureBody = Block | Expression ;

LiteralExpr = INTEGER | FLOAT | STRING | CHAR | "true" | "false" ;

(* Enum variant shorthand - type inferred from context *)
EnumShorthandExpr = "." IDENTIFIER [ "(" [ ExpressionList ] ")" ] ;

(* Paths use dot separator *)
PathExpr = IDENTIFIER { "." IDENTIFIER } ;

GroupedExpr = "(" Expression ")" ;

(* Tuple expressions support optional named fields *)
TupleExpr = "(" [ TupleExprElement { "," TupleExprElement } [ "," ] ] ")" ;

TupleExprElement = [ IDENTIFIER ":" ] Expression ;

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
let msg: Message = .Move(x: 10, y: 20);
let result: Result(T: i32, E: Error) = .Ok(42);

// In return statements (type inferred from function signature)
fn default_color(): Color {
    return .Red;
}

// Comparison with known enum type
if color == .Blue { ... }
```

### Control Flow Expressions

```ebnf
BlockExpression = [ Label ] Block
                | IfExpr
                | WhileExpr
                | ForExpr
                | LoopExpr ;

IfExpr = "if" Expression Block [ "else" ( IfExpr | Block ) ] ;

(* Labels use postfix colon for definition, prefix colon for reference *)
(* Definition: `label: { ... }` or `label: for x in ...` *)
(* Reference: `break :label` or `continue :label` *)
Label = IDENTIFIER ":" ;

WhileExpr = [ Label ] "while" Expression Block ;

ForExpr = [ Label ] "for" Pattern "in" Expression Block ;

LoopExpr = [ Label ] "loop" Block ;

(* Break exits blocks/loops with optional value *)
(* - `break;` exits immediately enclosing block/loop *)
(* - `break value;` exits with value *)
(* - `break :label;` exits labeled block/loop *)
(* - `break :label value;` exits labeled block/loop with value *)
BreakExpr = "break" [ ":" IDENTIFIER ] [ Expression ] ;

ContinueExpr = "continue" [ ":" IDENTIFIER ] ;

(* Explicit return required for returning values from functions *)
ReturnExpr = "return" [ Expression ] ;

(* Yield is exclusively for generator functions - suspends and produces a value *)
YieldExpr = "yield" Expression ;

(* Throw an error in a throws function - desugars to return Err(expr) *)
ThrowExpr = "throw" Expression ;
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

**Explicit Return and Break:**

Functions must use `return` to return values, and block expressions must use `break` to provide a value—**unless the block contains only a single expression**, in which case the value is implicit.

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

// Multi-statement block: break required
let result = {
    let temp = compute();
    break temp * 2;
};

// Error: multi-statement without return
fn bad(x: i32): i32 {
    let temp = x;
    temp * 2;  // ERROR: missing return
}

// Error: multi-statement without break
let bad = {
    let x = 1;
    x + 1;  // Block has type (), not i32
};
```

**Why this design?**

Single-expression blocks are concise and unambiguous. Multi-statement blocks require explicit `return`/`break` to avoid the subtle semantics where semicolon presence changes program behavior.

**Labeled Blocks and Break:**

Blocks, loops, and other control flow constructs can be labeled for targeted `break` or `continue`:

| Syntax | Meaning |
|--------|---------|
| `break;` | Exit immediately enclosing block/loop |
| `break value;` | Exit immediately enclosing with value |
| `break :label;` | Exit specific labeled scope |
| `break :label value;` | Exit specific labeled scope with value |
| `continue;` | Continue immediately enclosing loop |
| `continue :label;` | Continue specific labeled loop |

Labels use postfix colon for definition and prefix colon for reference—the colon "points toward" what it refers to:

```spl
// Labeled block with value
let result = computed: {
    let a = expensive();
    let b = transform(a);
    break :computed a + b;
};

// Unlabeled block with value
let result = {
    let a = expensive();
    break a * 2;
};

// Nested loops with labels
outer: for x in items {
    inner: for y in other {
        if done {
            break :outer;  // exit outer loop
        }
    }
}
```

**`yield` in Generators:**

The `yield` keyword is exclusively for generator functions—it suspends the generator and produces a value to the caller:

```spl
gen fn count(): i32 {
    let computed = {
        let a = 1;
        break a + 1;  // block value via break
    };
    yield computed;       // generator yield
    yield computed * 2;   // generator yield
}
```

See [iteration.md](iteration.md) for generator semantics.

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
| 2    | Coalesce       | `??`                           | Right | `a ?? b ?? c`             |
| 3    | Logical OR     | `\|\|`                         | Left  | `a \|\| b \|\| c`         |
| 4    | Logical AND    | `&&`                           | Left  | `a && b && c`             |
| 5    | Pattern Match  | `is`                           | Left  | `x is Some(v)`            |
| 6    | Equality       | `==` `!=`                      | Left  | `a == b != c`             |
| 7    | Comparison     | `<` `>` `<=` `>=`              | Left  | `a < b`                   |
| 8    | Range          | `..` `..=`                     | Left  | `0..10`, `0..=10`         |
| 9    | Additive       | `+` `-`                        | Left  | `a + b - c`               |
| 10   | Multiplicative | `*` `/` `%`                    | Left  | `a * b / c`               |
| 11   | Unary          | `!` `-` `&` `&mut`             | Right | `!&mut x`                 |
| 12   | Postfix        | `.` `?.` `()` `[]` `[:]` `!`   | Left  | `a.b()!.c[0]`             |

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

**Case-based disambiguation with backtracking:**
- Uppercase identifier: parser first tries Type grammar for RHS
- Lowercase identifier: parser first tries Expression grammar for RHS
- On parse failure, parser backtracks and tries the alternate grammar
- If both succeed, case determines which AST to use
- Semantic analysis may reinterpret if resolution reveals the opposite was intended

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

**Rule:** A single expression in parentheses without a trailing comma is a grouped expression. With a trailing comma, multiple elements, or named fields, it is a tuple.

```spl
(1 + 2)          // Grouped expression, evaluates to 3
(1,)             // Single-element tuple (positional)
(1, 2)           // Two-element tuple (positional)
(x: 1)           // Single-element tuple (named) - NOT grouped!
(x: 1, y: 2)     // Two-element tuple (named)
(1, name: 2)     // Two-element tuple (mixed positional and named)
()               // Unit (empty tuple)
```

**Named tuples** are always tuples, never grouped expressions, because the `name:` syntax is unambiguous.

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

SPL uses **case-based disambiguation with backtracking** to distinguish type arguments from value arguments:

| Identifier Case | Initial Parse | Fallback |
|-----------------|---------------|----------|
| **Uppercase** (e.g., `T`, `Key`) | Type grammar | Expression grammar |
| **Lowercase** (e.g., `x`, `name`) | Expression grammar | Type grammar |

The parser uses the identifier's case to choose which grammar to try first. If parsing fails, it backtracks and tries the alternate grammar. When both grammars would succeed (ambiguous cases), the case determines the AST node type.

**Default behavior:**
```spl
// Uppercase identifier → try Type grammar first
Vec(T: i32)                    // T is uppercase, i32 parsed as Type
HashMap(K: String, V: i32)     // K, V uppercase → type args

// Lowercase identifier → try Expression grammar first
Point(x: 1, y: 2)              // x, y lowercase → value args
greet(to: "Alice")             // to lowercase → value arg
```

**Backtracking examples:**

When the default parse fails, the parser automatically backtracks:
```spl
// T is uppercase, but if `Config` is a value (not a type), parser backtracks
// and reparses as Expression
parse(T: Config, data: input)  // If Config resolves to value, T becomes value arg

// x is lowercase, but if it's followed by something only valid as Type,
// parser backtracks and parses as Type
some_call(x: &SomeType)        // Backtrack if & indicates reference type
```

**Semantic reinterpretation:**

When both Type and Expression grammars succeed (common for simple paths), the parser uses case to build the AST. If semantic analysis later determines the opposite was intended, it reinterprets the node:

| Parsed As | Resolved To | Reinterpreted As |
|-----------|-------------|------------------|
| `PathType("Foo")` | value | `PathExpr("Foo")` |
| `PathExpr("foo")` | type | `PathType("foo")` |
| `ReferenceType(&Foo)` | value | `AddressOf(Foo)` |
| `AddressOf(&x)` | type | `ReferenceType(x)` |

This allows natural usage without forcing sigils:
```spl
// Works even if naming conventions differ from SPL defaults
Config(URL: url_value)         // URL uppercase but refers to value field
functor(f: Option)             // f lowercase but refers to type parameter
```

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

### 11. Additional Disambiguation Examples

This section provides comprehensive examples for tricky cases.

#### Type vs Expression in Generic Context

When a name could be either a type or a value:

```spl
// 'String' is a type (uppercase), parsed as Type
let v: Vec(T: String) = Vec.new();

// 'string' is a value (lowercase), parsed as Expression
let s = string;              // Variable reference
let p = Point(x: string);    // Value passed to field

// Lowercase type alias - case suggests value, but semantic analysis
// reinterprets when `myint` resolves to a type
type myint = i32;
let v: Vec(t: myint) = Vec.new();  // t lowercase, but myint is a type
                                    // Semantic analysis reinterprets as type arg
```

#### Distinguishing Calls from Instantiation

```spl
// These look similar but are different:
Result(T: Data, E: Error)      // Type instantiation (both uppercase)
Result(ok: data, err: error)   // Would be struct fields if Result were a struct

// Function returning generic:
fn make_result(): Result(T: i32, E: String) { ... }

// Calling with type args:
parse(T: Config, "input")      // T is type arg (uppercase)
parse(config: cfg, "input")    // config is value arg (lowercase)
```

#### Enum Variant vs Type

```spl
// Option.Some is a path to variant, not a type
let x = Option.Some(42);       // Variant constructor

// Option(T: i32) is a type
let y: Option(T: i32) = Some(42);

// Vec(T: i32).new() - type then method
let v = Vec(T: i32).new();

// Vec.new() - path to associated function (type inferred)
let v = Vec.new();
v.push(42);  // Now Vec(T: i32)
```

#### Nested Generic Types

```spl
// Nested generics - each level uses its own type args
let nested: Vec(T: Option(T: i32)) = Vec.new();

// HashMap with complex value type
let map: HashMap(K: String, V: Vec(T: i32)) = HashMap.new();

// Result containing Option
let r: Result(T: Option(T: User), E: Error) = Ok(Some(user));
```

#### Method Chains with Types

```spl
// Type application, then method chain
Vec(T: i32).new().push(42)     // Create, then push

// Parentheses for clarity
(Vec(T: i32).new()).push(42)   // Same as above

// Multiple method calls
Vec(T: String).with_capacity(10).push("hello".to_string())
```

#### Ambiguous-Looking but Unambiguous

```spl
// This is ALWAYS a struct instantiation (named fields)
Point(x: 1, y: 2)

// This is ALWAYS a function call (positional args)
add(1, 2)

// This is ALWAYS type instantiation (uppercase = type args)
Vec(T: i32)

// Mixed: type arg + value arg (unambiguous due to case)
Container(T: i32, value: 42)   // T: type, value: value
```

#### Uppercase Value Fields and Lowercase Type Parameters

The backtracking and semantic reinterpretation approach handles edge cases naturally:

```spl
// JSON-like API with uppercase field names
// Parser initially tries Type grammar due to uppercase, but backtracks
// when it can't parse the string literal as a type
struct JsonObject(
    Type: String,       // Field named "Type" - value field
    ID: i64,            // Field named "ID" - value field
    data: Vec(T: u8),   // Normal field
)

// Creating instance - uppercase but semantic analysis knows these are value fields
let obj = JsonObject(
    Type: "user",       // Uppercase, but string literal forces value interpretation
    ID: 12345,          // Uppercase, but integer literal forces value interpretation
    data: bytes,        // Value arg (lowercase default)
);

// Haskell-style lowercase type parameters
// Parser initially tries Expression grammar due to lowercase, but backtracks
// when semantic analysis can't find a value named `f`
trait Functor where f {
    fn map(self, func: fn(A): B): f(b: B) where A, B, a, b;
}
// Semantic analysis reinterprets f, a, b as type parameters when they
// resolve to types rather than values
```

#### Common Patterns

```spl
// 1. Generic function with inferred type
let items = vec![1, 2, 3];
let doubled = items.map(|x| x * 2);  // Types inferred

// 2. Generic function with explicit type
let parsed = parse(T: Config, input);  // Explicit T

// 3. Type annotation on binding
let config: Config = parse(input);     // Type on let, not call

// 4. Turbofish not needed - use type annotation instead
// Rust: let v = Vec::<i32>::new();
// SPL:  let v: Vec(T: i32) = Vec.new();  // Or...
// SPL:  let v = Vec(T: i32).new();       // Type application

// 5. Return type provides context
fn load(): Result(T: Config, E: Error) {
    let data = read_file(path)!;    // Result types inferred
    return parse(data);              // Return type known
}
```

#### Edge Cases with Imports

```spl
// Imported type - still uppercase
use other.module.Config;
let c: Config = Config.default();

// Imported value - still lowercase
use other.module.default_config;
let c = default_config();

// Aliased import - case preserved
use other.module.Config as Cfg;    // Type alias
let c: Cfg = Cfg.default();

use other.module.helper as h;      // Value alias
let result = h();
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
type Pair(T) = (T, T);

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
    let maybe: i32? = Some(42);
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

    // Named tuples
    let coords = (x: 3.0, y: 4.0);      // Named tuple expression
    let x_coord = coords.x;              // Named field access
    let y_coord = coords.1;              // Positional access also works

    // Named tuple as return type
    fn divide(a: i32, b: i32): (quotient: i32, remainder: i32) {
        return (quotient: a / b, remainder: a % b);
    }
    let result = divide(17, 5);
    let q = result.quotient;             // Named access: 3
    let r = result.remainder;            // Named access: 2

    // Patterns
    let (a, b) = (1, 2);                // Tuple destructuring
    let Point(x, y) = target;           // Struct destructuring
    let [first, ..rest] = [1, 2, 3, 4]; // Slice pattern with rest
    let [head, .., tail] = [1, 2, 3];   // First and last

    // Block with break
    let computed = {
        let a = 10;
        let b = 20;
        break a + b;
    };
}

// Function types (colon for return)
type Predicate = fn(i32): bool;
type BinaryOp = fn(i32, i32): i32;
type Action = fn();

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
| Impl block generics | `impl<T> Vec<T>`          | `impl Vec(T: T) where T`     |
| Concrete impl       | `impl Vec<u32>`           | `impl Vec(T: u32)`           |
| Turbofish           | `::<T>`                   | Not needed (use parentheses) |
| Named struct decl   | `struct Point { x: i32 }` | `struct Point(x: i32)`       |
| Positional struct   | `struct Pair(i32, i32);`  | `struct Pair(i32, i32)`      |
| Struct literal      | `Point { x: 1 }`          | `Point(x: 1)` (instantiation) |
| Pattern matching    | `if let Some(x) = v {}`   | `if v is Some(x) {}`         |
| Function return     | `expr` (implicit tail)    | `expr` (single) or `return` (multi-stmt) |
| Block value         | `expr` (implicit tail)    | `expr` (single) or `break` (multi-stmt)  |
| Semicolons          | Semantic (tail vs stmt)   | Required for statements      |
| Named parameters    | Not built-in              | `fn foo(to name: T)`         |
| Named tuples        | Not supported             | `(x: i32, y: i32)` type and expr |
