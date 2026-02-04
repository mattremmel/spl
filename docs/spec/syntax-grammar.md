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
7. **Pattern matching with `is`**: `if value is .Some(x)` instead of `if let`.
8. **Explicit return/break**: `return` for functions, `break` for block values.
9. **Optional semicolons**: Statement terminators are inferred from newlines, but can be explicitly terminated with a semicolon. Semicolons are only required when writing multiple statements on the same line.

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

Trailing commas are allowed in all comma-separated lists.

**Case-Sensitive Identifiers:**

| Token | Meaning |
|-------|---------|
| `UPPER_IDENT` | Identifier starting with A-Z (type parameters, enum variants) |
| `LOWER_IDENT` | Identifier starting with a-z or _ (value parameters, field names) |

The lexer produces a single `IDENTIFIER` token. The parser checks the first character to select the appropriate grammar alternative. This case-based disambiguation is a hard rule—no backtracking occurs.

**Semicolons in the Grammar:** Per the optional semicolons design principle, semicolons are inferred from newlines and only required when multiple statements appear on the same line. Productions that include `[ ";" ]` indicate where an explicit semicolon is syntactically permitted, not where it is required. Productions ending with clear delimiters (blocks `{}`, parenthesized fields `()`) omit `[ ";" ]` since the delimiter itself terminates the construct.

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

VisibilityScope = "$" [ "." TypePath ] | "super" ;
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
ConstDef = "const" IDENTIFIER ":" Type "=" Expression [ ";" ] ;

(* Static variable (module-level mutable state) *)
StaticDef = "static" [ "mut" ] IDENTIFIER ":" Type "=" Expression [ ";" ] ;
```

**Examples:**

```spl
const MAX_SIZE: usize = 1024
const PI: f64 = 3.14159265359

static COUNTER: i32 = 0
static mut GLOBAL_STATE: i32 = 0  // Requires unsafe to access
```

### Function Definitions

```ebnf
FunctionDef = [ "const" ] [ "unsafe" ] "fn" IDENTIFIER "(" [ ParamList ] ")" [ ":" Type ] [ ThrowsClause ] [ WhereClause ] Block ;

(* Generator functions yield multiple values lazily *)
GeneratorDef = "gen" "fn" IDENTIFIER "(" [ ParamList ] ")" ":" Type [ ThrowsClause ] [ WhereClause ] Block ;

(* Throws clause for functions returning Result.
   Note: `throws` intentionally precedes `where` in SPL's syntax order,
   keeping error handling adjacent to the return type it modifies. *)
ThrowsClause = "throws" [ Type ] ;

ParamList = Param { "," Param } [ "," ] ;

Param = SelfParam | TypedParam ;

SelfParam = [ "&" [ "mut" ] ] [ "mut" ] "self" ;

(* Parameters with optional labels and optional default values *)
TypedParam = [ LabelSpec ] [ "mut" ] IDENTIFIER ":" Type [ "=" Expression ] ;

(* Label before parameter name: "to name" means call with "to: value" *)
(* "_" means no label required at call site *)
LabelSpec = "_" | IDENTIFIER ;

(* Where clause declares type parameters and optional constraints *)
(* Unlike Rust, `where` both introduces AND constrains type parameters *)
WhereClause = "where" TypeParam { "," TypeParam } [ "," ] ;

TypeParam = IDENTIFIER [ ":" PathType { "+" PathType } ] [ "=" Type ] ;
```

**Clause Ordering:**

SPL uses a consistent ordering for function/method signatures:

| Element | Order | Example |
|---------|-------|---------|
| Parameters | 1st | `fn foo(x: T)` |
| Return type | 2nd | `: ReturnType` |
| Throws clause | 3rd | `throws Error` |
| Where clause | 4th | `where T: Clone` |

This keeps error handling adjacent to the return type it modifies, while
generic constraints come last as they apply to the entire signature.

For struct/enum definitions, `where` follows the body delimiter:
- `struct Point(x: T) where T`
- `enum Option { ... } where T`

For trait/impl definitions, `where` precedes the body:
- `trait Clone where T { ... }`
- `impl Trait for Type where T { ... }`

**Default Parameters:**

Parameters can have default values that are used when the argument is omitted at the call site. Default values are expressions evaluated at call time (not definition time).

| Rule | Description |
|------|-------------|
| Syntax | `param: Type = expr` |
| Evaluation | Expression evaluated fresh each call when argument omitted |
| Position | Allowed on any parameter |
| Calling | After first default, remaining args must use names (unless also defaulted) |
| Restriction | Default expressions cannot reference other parameters |

**Examples:**

```spl
// Simple default
fn greet(name: String = "World") {
    print("Hello, " + name)
}
greet()           // "Hello, World"
greet("Alice")    // "Hello, Alice"

// Multiple defaults
fn connect(host: String, port: i32 = 8080, timeout: i32 = 30) {
    // ...
}
connect("localhost")                      // port=8080, timeout=30
connect("localhost", 9000)                // port=9000, timeout=30
connect("localhost", timeout: 60)         // port=8080, timeout=60

// With named labels
fn send(to recipient: String, message: String = "Hello") {
    // Called as: send(to: "Alice") or send(to: "Alice", message: "Hi")
}

// Expression defaults (evaluated at call time)
fn log(message: String, timestamp: DateTime = DateTime.now()) {
    // timestamp is evaluated fresh each call, not when function is defined
}

// Non-default after default requires named argument
fn example(a: i32 = 1, b: i32) {
    // b has no default, so must be named when a is omitted
}
example(b: 5)           // a=1, b=5
example(10, 20)         // a=10, b=20
```

**Note:** Generator functions require a return type annotation (the yielded type). See [iteration.md](iteration.md) for full generator semantics.

**Generator Throws:**

Generators can declare a throws clause, with the same semantics as function throws:

| Declaration | Yield Type | Next() Returns |
|-------------|------------|----------------|
| `gen fn foo(): T` | `T` | `Option(T: T)` |
| `gen fn foo(): T throws E` | `T` | `Result(T: Option(T: T), E: E)` |
| `gen fn foo(): T throws` | `T` | `Result(T: Option(T: T), E: Error)` |

Inside a throwing generator:
- `yield value` produces a value
- `throw error` terminates iteration with an error
- The `!` operator propagates errors from fallible operations

**Examples:**

```spl
// Simple function with return type
fn add(a: i32, b: i32): i32 {
    return a + b
}

// Generic function - `where T` declares the type parameter T
fn identity(x: T): T where T {
    return x
}

// Named parameters (external label differs from internal name)
fn greet(to person: String) {
    // Called as: greet(to: "Alice")
}

// Omit label with underscore
fn add(_ a: i32, _ b: i32): i32 {
    // Called as: add(1, 2) instead of add(a: 1, b: 2)
    return a + b
}

// Generic with bounds
fn clone_it(x: &T): T where T: Clone {
    return x.clone()
}

// Function with typed throws (returns Result(T: String, E: IoError))
fn read_file(path: &str): String throws IoError {
    if !path.exists() {
        throw .NotFound(path)
    }
    return fs.read_to_string(path)
}

// Function with untyped throws (returns Result(T: Data, E: Error))
fn process(input: &str): Data throws {
    let parsed = parse(input)!
    return transform(parsed)
}

// Compile-time evaluable function
const fn square(x: i32): i32 {
    return x * x
}

const HUNDRED: i32 = square(10)  // Evaluated at compile time
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
(* Enums use braces for their variant list *)
(* Type parameters appear in variant data types and must be declared in the where clause *)
EnumDef = "enum" IDENTIFIER "{" [ VariantList ] "}" [ WhereClause ] ;

VariantList = Variant { "," Variant } [ "," ] ;

(* Variants can be unit, tuple-style, or struct-style *)
Variant = UPPER_IDENT [ "(" VariantFields ")" ] ;

(* Type-only = tuple variant, with : = named fields *)
(* Note: FieldList allows `pub` on fields syntactically, but visibility on
   enum variant fields is a semantic error — all variant fields are implicitly
   public with the variant's visibility. *)
(* IMPORTANT: A variant's fields must be consistently ALL named or ALL positional.
   Parsing strategy: If ANY field contains `:` (name: Type syntax), parse as FieldList
   for struct-style variant. Otherwise, parse as TypeList for tuple-style variant.
   The `pub` modifier is only valid in FieldList context.
   Mixing named and positional fields in a single variant is a semantic error. *)
VariantFields = FieldList           (* named fields: x: i32, y: i32 *)
              | TypeList ;          (* tuple fields: i32, String *)

(* Note: TypeList is defined in section 2 (Types). *)
```

**Examples:**

```spl
// Simple enum
enum Color { Red, Green, Blue }

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
(* In declaration context, `: Type` specifies a default: trait Add(RHS: Self) *)
(* Supertraits specified with : before where clause, e.g., trait Numeric: Add + Sub *)
(* Unsafe traits have invariants the compiler cannot verify, e.g., unsafe trait Sync *)
TraitDef = [ "unsafe" ] "trait" IDENTIFIER [ GenericArgs ] [ ":" PathType { "+" PathType } ] [ WhereClause ] "{" { TraitItem } "}" ;

TraitItem = [ "pub" ] ( TraitMethod | AssociatedType ) ;

TraitMethod = [ "const" ] [ "unsafe" ] "fn" IDENTIFIER "(" [ ParamList ] ")" [ ":" Type ] [ ThrowsClause ] [ WhereClause ] ( ";" | Block ) ;

AssociatedType = "type" IDENTIFIER [ ":" PathType { "+" PathType } ] [ ";" ] ;
```

**Examples:**

```spl
// Simple trait
trait Clone {
    fn clone(&self): Self
}

// Trait with associated type
trait Iterator {
    type Item
    fn next(&mut self): Self.Item?
}

// Trait with default implementation
trait Default {
    fn default(): Self
}

// Trait with bounds
trait Numeric: Add + Sub + Mul + Div {
    fn zero(): Self
    fn one(): Self
}

// Trait with default type parameter (RHS defaults to Self)
trait Add(RHS: Self) {
    type Output
    fn add(self, rhs: RHS): Self.Output
}

// Implementation using default (RHS = Self)
impl Add for Point {
    type Output = Point
    fn add(self, rhs: Point): Point { ... }
}

// Implementation with explicit RHS type
impl Add(RHS: Vector) for Point {
    type Output = Point
    fn add(self, rhs: Vector): Point { ... }
}
```

### Implementation Blocks

```ebnf
(* Impl blocks use parentheses for generic args *)
(* Inherent impl: impl Type { ... } *)
(* Trait impl: impl Trait for Type { ... } *)
(* Unsafe impl required for implementing unsafe traits: unsafe impl Sync for MyType *)
ImplBlock = [ "unsafe" ] "impl" [ TypePath [ GenericArgs ] "for" ] TypePath [ GenericArgs ] [ WhereClause ] "{" { ImplItem } "}" ;

ImplItem = [ "pub" ] ( FunctionDef | TypeAssignment ) ;

TypeAssignment = "type" IDENTIFIER "=" Type [ ";" ] ;
```

**Examples:**

```spl
// Inherent implementation
impl Point {
    pub fn new(x: f64, y: f64): Point {
        return Point(x: x, y: y)
    }
}

// Trait implementation
impl Clone for Point {
    fn clone(&self): Self {
        return Self(x: self.x, y: self.y)
    }
}

// Generic trait implementation
impl Clone for Option(T: T) where T: Clone {
    fn clone(&self): Self {
        return match self {
            .Some(v) => .Some(v.clone()),
            .None => .None,
        }
    }
}
```

**Impl Block Patterns:**

SPL's `where` clause both **declares** and optionally **constrains** type parameters for impl blocks. This eliminates the redundancy of Rust's `impl<T> Type<T>` pattern.

```spl
// Simple impl (non-generic type)
impl Point {
    pub fn new(x: f64, y: f64): Point {
        return Point(x: x, y: y)
    }
}

// Generic impl - `where T` declares the type parameter
impl Box(T: T) where T {
    pub fn unwrap(self): T {
        return self.value
    }
}

// Conditional impl - bounds restrict which types this impl applies to
impl Container(T: T) where T: Clone {
    pub fn clone_all(&self): Vec(T: T) {
        return self.items.clone()
    }
}

// Concrete impl - no where clause, implements for specific type
impl Box(T: u32) {
    pub fn special_u32_method(&self): u32 {
        return self.value * 2
    }
}

impl Box(T: String) {
    pub fn special_string_method(&self): usize {
        return self.value.len()
    }
}

// Different names for impl vs struct type parameters
// Emphasizes that T is the parameter NAME and R is the TYPE
struct Wrapper(val: T) where T
impl Wrapper(T: R) where R {
    fn get(&self): &R { return &self.val }
}

// Method-level generics - methods can have additional type parameters
impl Vec(T: T) where T {
    fn convert(&self): Vec(T: U) where U, T: Into(Target: U) {
        // Each element converted from T to U
    }
}

// Multiple concrete trait implementations
trait Format {
    fn display(&self): String
}

struct Amount(value: T) where T

impl Format for Amount(T: USD) {
    fn display(&self): String {
        return format("${}", self.value.cents)
    }
}

impl Format for Amount(T: EUR) {
    fn display(&self): String {
        return format("{}€", self.value.cents)
    }
}
```

**Note:** `format()` is a standard library function that performs string formatting with placeholders (`{}`). String interpolation syntax (e.g., `f"value: {x}"`) may be added in a future version of SPL.

### Type Aliases

```ebnf
TypeAlias = "type" IDENTIFIER [ GenericParams ] "=" Type [ WhereClause ] [ ";" ] ;

GenericParams = "(" TypeParamList ")" ;

TypeParamList = IDENTIFIER { "," IDENTIFIER } [ "," ] ;
```

**Note:** Type aliases use `GenericParams` (bare identifiers) rather than `GenericArgs` (named type args) because they declare new type parameters rather than instantiate existing ones. See the comparison table below.

**GenericParams vs GenericArgs:**

| Context | Syntax | Example | Purpose |
|---------|--------|---------|---------|
| `GenericParams` | Bare identifiers | `type Pair(T, U) = ...` | Declare type parameters (no defaults) |
| `GenericArgs` | Named type args | `Pair(T: i32, U: String)` | Instantiate with concrete types |
| `GenericArgs` (in trait declaration) | Named type args | `trait Add(RHS: Self)` | Declare with defaults |

`GenericParams` appears in type alias declarations to introduce type parameter names. `GenericArgs` appears in type instantiation to bind parameters to concrete types, and also in trait declarations where `: Type` specifies a default value for the parameter.

### Use Declarations

Import items or modules into scope. See `module-system.md` for full details.

```ebnf
UseDecl = "use" UsePath [ ";" ] ;

UsePath = PathPrefix [ "." UseTree ] ;

PathPrefix = ( "$" | "super" | "self" ) "." IDENTIFIER { "." IDENTIFIER }
           | IDENTIFIER { "." IDENTIFIER } ;

UseTree = "*"                                    (* glob import *)
        | "{" UseTreeList "}"                    (* grouped import *)
        | IDENTIFIER [ "as" IDENTIFIER ] ;       (* item or rename *)

UseTreeList = UseTree { "," UseTree } [ "," ] ;
```

**Examples:**

| Syntax | Description |
|--------|-------------|
| `use std.vec.Vec` | Import single item |
| `use std.io` | Import module |
| `use std.collections.HashMap as Map` | Import with rename |
| `use std.collections.{HashMap, HashSet}` | Grouped import |
| `use std.prelude.*` | Glob import |
| `use $.utils.helper` | Package-root import |
| `use super.common` | Parent module import |

### Module Declarations

Declare inline modules for namespacing within a file, or reference submodules.

```ebnf
ModuleDecl = "module" IDENTIFIER ( ";" | "{" { Item } "}" ) ;
```

**Examples:**

| Syntax | Description |
|--------|-------------|
| `module network` | Reference submodule in directory |
| `pub module api` | Public submodule reference |
| `module internal { ... }` | Inline module for namespacing |

**Inline Module Example:**

```spl
// Inline module for namespacing
module internal {
    fn helper(): i32 {
        return 42
    }

    pub fn public_helper(): i32 {
        return helper()
    }
}

fn main() {
    let x = internal.public_helper()
}
```

**Note:** Inline modules are a future feature. See the module system roadmap.

### Extern Blocks and FFI

Extern blocks declare foreign functions. Extern function definitions create SPL functions callable from foreign code.

```ebnf
(* Declare foreign functions *)
ExternBlock = "extern" AbiString "{" { ExternFnDecl } "}" ;

ExternFnDecl = "fn" IDENTIFIER "(" [ ExternParamList ] [ "," "..." ] ")" [ ":" Type ] [ ";" ] ;

ExternParamList = ExternParam { "," ExternParam } [ "," ] ;

ExternParam = IDENTIFIER ":" Type ;

(* Define SPL functions with C calling convention *)
ExternFnDef = "extern" AbiString "fn" IDENTIFIER "(" [ ParamList ] ")" [ ":" Type ] Block ;

AbiString = STRING ;

(* Note: Valid ABI strings are validated semantically. Currently supported: "C".
   Future ABIs may include "system", "stdcall", etc. *)
```

**Examples:**

```spl
// Declare foreign functions (in extern block)
#[link(name = "mylib")]
extern "C" {
    fn c_function(x: i32): i32
    fn variadic_fn(fmt: Ptr(T: u8), ...): i32
}

// Define SPL function callable from C (outside extern block)
#[no_mangle]
extern "C" fn my_callback(value: i32): i32 {
    return value * 2
}

// Function pointer type for callbacks
type Callback = extern "C" fn(i32): i32
```

**Notes:**
- Calling functions declared in extern blocks requires `unsafe`
- Functions defined with `extern "C" fn` use the C calling convention
- `#[no_mangle]` preserves the function name for FFI
- Variadic parameters (`...`) are only allowed in extern block declarations

### Macros (Deferred)

Macro syntax is deferred to a future version of SPL. The language will likely support:
- Declarative macros (pattern-based transformation)
- Procedural macros (code generation via compiler plugins)

Syntax design will be documented in a separate ADR before implementation.

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

(* Lifetime marker - optional precision for reference provenance *)
Lifetime = "'" IDENTIFIER ;

ReferenceType = "&" [ Lifetime ] [ "mut" ] BaseType ;

(* Note: Lifetime markers are optional precision, not required annotations.
   SPL uses second-class references with intersection semantics by default.
   See memory-model.md for details on when lifetime markers are useful. *)

ArrayType = "[" Type [ ";" Expression ] "]" ;

(* Tuple types support optional named fields *)
TupleType = "(" [ TupleTypeElement { "," TupleTypeElement } [ "," ] ] ")" ;

TupleTypeElement = [ IDENTIFIER ":" ] Type ;

(* Function type return uses colon.
   Note: Function types do not have throws clauses. The `throws` keyword in
   function definitions is syntactic sugar that desugars to a Result return type.
   For example, `fn foo(): String throws Error` has type `fn(): Result(T: String, E: Error)`. *)
FnType = [ "unsafe" ] [ "extern" AbiString ] "fn" [ LifetimeParams ] "(" [ TypeList ] ")" [ ":" Type ] ;

LifetimeParams = "(" Lifetime { "," Lifetime } [ "," ] ")" ;

TypeList = Type { "," Type } [ "," ] ;

NeverType = "Never" ;

PathType = TypePath [ GenericArgs ] ;

(* Note: `Self.Item?` parses as `(Self.Item)?` — the optional `?` postfix
   applies to the entire path type, not just the final identifier. *)

(* Paths use dot, not double-colon *)
TypePath = [ PathRoot ] IDENTIFIER { "." IDENTIFIER }
         | "Self" [ "." IDENTIFIER ] ;  (* Self or Self.AssociatedType *)

PathRoot = "$" "."          (* package root *)
         | "super" "."      (* parent module *)
         | "self" "." ;     (* current module *)

(* Generic args use parentheses with named type arguments *)
(* Case-based disambiguation: uppercase identifier = type arg, lowercase = value arg *)
(* This is a hard rule enforced by the parser — no backtracking or semantic reinterpretation *)
(* Shorthand: bare identifier T means T: T (type param name matches type name) *)
GenericArgs = "(" [ TypeArg { "," TypeArg } [ "," ] ] ")" ;

TypeArg = UPPER_IDENT [ ":" Type ] ;   (* T: i32 or T (shorthand for T: T) *)
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
| `fn('a)(&'a str): &'a Token` | Function type with lifetime parameter |
| `fn()`              | Function type returning unit       |
| `Never`             | Never type                         |
| `HashMap(K: String, V: i32)` | Multi-param generic type   |
| `Result(T: i32, E: Error)` | Named type arguments         |
| `Option(T)`         | Type arg shorthand (same as `Option(T: T)`) |
| `Result(T, E)`      | Multiple shorthand type args |
| `Result(T, E: Error)` | Mixed shorthand and explicit |
| `i32?`              | Optional type (sugar for `Option(T: i32)`) |
| `String?`           | Optional String                    |
| `&T?`               | Reference to optional (rare)       |

### Named Tuples

Tuples can have optional named fields, enabling anonymous record types without defining a struct:

```spl
// Named tuple type as return type
fn get_coords(x: f64): (identity: f64, square: f64) {
    return (identity: x, square: x * x)
}

// Named tuple in variable binding
let point: (x: i32, y: i32) = (x: 1, y: 2)

// Access by name
let x_val = point.x

// Positional access also works
let y_val = point.1

// Mixed positional and named (allowed)
let mixed: (i32, name: String) = (42, name: "hello")
```

**Use Cases:**
- Return multiple values with self-documenting field names
- Ad-hoc data grouping without struct definitions
- API boundaries where named fields improve clarity

**Field Access:**
- Named fields: `tuple.field_name`
- Positional fields: `tuple.0`, `tuple.1`, etc.
- Both access styles work regardless of whether fields were declared with names

### Trait Objects and Existential Types

SPL currently uses automatic boxing for trait objects rather than explicit `dyn Trait` syntax. When a value type is used where a trait bound is expected, the compiler may automatically box it.

Explicit `dyn Trait` syntax and `impl Trait` return types are deferred features. The language may add these in the future if automatic boxing proves insufficient for performance-critical code.

---

## 3. Statements

```ebnf
Block = "{" { Statement } "}" ;

Statement = LetStatement
          | ExpressionStatement
          | Item ;                  (* nested items like functions *)

LetStatement = "let" Pattern [ ":" Type ] [ "=" Expression ] [ ";" ] ;

(* Note: Mutability is specified in the pattern, not on `let` itself:
   - Simple binding: `let mut x = 5` (where `mut x` is an IdentifierPattern)
   - Destructuring: `let (mut a, b) = tuple` (mut on individual bindings)
   This avoids the double-mut issue of `let mut mut x`. *)

ExpressionStatement = Expression [ ";" ] ;
```

Block expressions (`if`, `while`, `for`, `loop`, and bare blocks) may omit the trailing semicolon when used as statements.

**Nested Items:**

Items (functions, structs, etc.) can be defined inside blocks, similar to Rust. These nested items are scoped to the containing block and can access generic type parameters from the enclosing function:

```spl
fn outer(value: T): i32 where T: Display {
    // Nested function - can use T from outer scope
    fn helper(x: T): String {
        return x.to_string()
    }

    let s = helper(value)
    return s.len()
}
```

**Semicolon Rules:**

SPL uses Swift-style optional semicolons:

| Rule | Description |
|------|-------------|
| Newline termination | Newlines act as statement terminators |
| Optional explicit | Semicolons can be added explicitly but are never required at end of line |
| Multiple statements | Required only when writing multiple statements on a single line: `let x = 1; let y = 2` |

Unlike Rust, semicolons have no semantic significance—they don't determine whether an expression's value is used.

**Block Values:**

Blocks containing multiple statements require explicit `break` to produce a value:

```spl
let result = {
    let a = compute()
    let b = transform(a)
    break a + b
}
```

However, blocks containing a **single expression** have an implicit value—no `break` is needed:

```spl
let doubled = if x > 0 { x * 2 } else { 0 }  // Single expression per branch
let value = { compute() }                     // Single expression block
```

Without `break` or a single expression, a block's type is `()` (unit).

---

## 4. Expressions

Expressions are defined using layered production rules that encode operator precedence. Lower precedence operators are defined first; they call higher precedence rules.

### Precedence Table

| Precedence | Operators                    | Associativity | Production         |
|------------|------------------------------|---------------|--------------------|
| 1 (lowest) | `=` `+=` `-=` `*=` `/=` `%=` `&=` `\|=` `^=` `<<=` `>>=` | Right | AssignmentExpr |
| 2          | `??`                         | Right         | CoalesceExpr       |
| 3          | `\|\|`                       | Left          | OrExpr             |
| 4          | `&&`                         | Left          | AndExpr            |
| 5          | `is`                         | Left          | IsExpr             |
| 6          | `==` `!=`                    | Left          | EqualityExpr       |
| 7          | `<` `>` `<=` `>=`            | Left          | ComparisonExpr     |
| 8          | `\|`                         | Left          | BitwiseOrExpr      |
| 9          | `^`                          | Left          | BitwiseXorExpr     |
| 10         | `&`                          | Left          | BitwiseAndExpr     |
| 11         | `<<` `>>`                    | Left          | ShiftExpr          |
| 12         | `..` `..=`                   | Left          | RangeExpr          |
| 13         | `+` `-`                      | Left          | AdditiveExpr       |
| 14         | `*` `/` `%`                  | Left          | MultiplicativeExpr |
| 15         | `**`                         | Right         | ExponentiationExpr |
| 16         | `!` `-` `*` `&` `~` (unary)  | Right         | UnaryExpr          |
| 17 (highest)| `.` `?.` `()` `[]` `[:]` `!` | Left          | PostfixExpr        |

Note: `&` serves as both a unary reference operator (prefix) and a binary bitwise AND operator; context disambiguates. Type conversions use methods (`.widen()`, `.truncate()`, `.try_into()`) rather than a cast operator.

### Expression Grammar

```ebnf
Expression = AssignmentExpr ;

AssignmentExpr = CoalesceExpr [ AssignOp AssignmentExpr ] ;

AssignOp = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ;

CoalesceExpr = OrExpr { "??" OrExpr } ;

OrExpr = AndExpr { "||" AndExpr } ;

AndExpr = IsExpr { "&&" IsExpr } ;

(* Pattern matching with is *)
IsExpr = EqualityExpr [ "is" Pattern ] ;

EqualityExpr = ComparisonExpr { ( "==" | "!=" ) ComparisonExpr } ;

ComparisonExpr = BitwiseOrExpr { ( "<" | ">" | "<=" | ">=" ) BitwiseOrExpr } ;

BitwiseOrExpr = BitwiseXorExpr { "|" BitwiseXorExpr } ;

BitwiseXorExpr = BitwiseAndExpr { "^" BitwiseAndExpr } ;

BitwiseAndExpr = ShiftExpr { "&" ShiftExpr } ;

ShiftExpr = RangeExpr { ( "<<" | ">>" ) RangeExpr } ;

RangeExpr = AdditiveExpr [ ( ".." | "..=" ) [ AdditiveExpr ] ] ;

AdditiveExpr = MultiplicativeExpr { ( "+" | "-" ) MultiplicativeExpr } ;

MultiplicativeExpr = ExponentiationExpr { ( "*" | "/" | "%" ) ExponentiationExpr } ;

(* Exponentiation is right-associative: 2 ** 3 ** 2 = 2 ** (3 ** 2) = 512 *)
ExponentiationExpr = UnaryExpr [ "**" ExponentiationExpr ] ;

UnaryExpr = ( "!" | "-" | "&" [ "mut" ] | "*" | "~" ) UnaryExpr
          | PostfixExpr ;

(* Dereference operator `*`:
   - Dereferences safe references: `&T` → `T`, `&mut T` → `T`
   - Example: `let x = *ref_to_int` or `*mut_ref = 10`
   - Does NOT apply to raw pointers (`Ptr`, `MutPtr`)

   Raw pointers use explicit methods instead (requires unsafe block):
   - `ptr.read()` — read value from pointer
   - `ptr.write(value)` — write value to pointer
   See unsafe.md for details. *)

(* No :: for paths - use . only *)
PostfixExpr = PrimaryExpr { PostfixOp } ;

PostfixOp = "." IDENTIFIER                                   (* field access *)
          | "." IDENTIFIER "(" [ ArgList ] ")"               (* method call *)
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
(* This is a hard rule — uppercase names are always types, lowercase are always values *)
(* Note: Type arguments in function/method calls use NamedArg with uppercase identifiers.
   For example, `collect(T: Vec(T: i32))` passes a type argument in a call context.
   This differs from GenericArgs, which is used in type instantiation contexts. *)
ArgList = Arg { "," Arg } [ "," ] ;

Arg = NamedArg                              (* named argument: case determines type vs value *)
    | Expression ;                          (* positional argument *)

(* Case of identifier determines how RHS is parsed — no backtracking *)
NamedArg = UPPER_IDENT ":" Type             (* type argument: T: i32 *)
         | LOWER_IDENT ":" Expression ;     (* value argument: x: 1 *)

(* Note: UPPER_IDENT and LOWER_IDENT are defined in the EBNF Notation section. *)
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
            | ThrowExpr
            | UnsafeExpr ;

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

(* Closure return types are always inferred from the body — no explicit
   return type annotation syntax is provided, matching Rust's behavior. *)
ClosureBody = Block | Expression ;

(* Note: `||` is disambiguated by position. In prefix position (start of an
   expression where a closure is expected), `||` begins an empty-parameter
   closure. In infix position (between two expressions), `||` is logical OR.
   The parser context determines which interpretation applies. *)

(* Note: Explicit capture lists (@[...]) control whether variables are
   captured by reference or by value. The capture expression determines
   ownership: `@[x]` captures x by value (moves), `@[x: &x]` captures by
   reference. See closures.md for details. *)

LiteralExpr = INTEGER | FLOAT | STRING | CHAR | "true" | "false" ;

(* Enum variant shorthand - type inferred from context.
   See EnumShorthandPattern in section 5 for the corresponding pattern syntax. *)
EnumShorthandExpr = "." UPPER_IDENT [ "(" [ ArgList ] ")" ] ;

(* Note: Enum variants must start with an uppercase letter (PascalCase convention). *)

(* Paths use dot separator *)
(* Note: Self is restricted to at most one component (Self or Self.AssociatedItem),
   matching TypePath and Rust's behavior. This avoids ambiguity about Self.Foo.Bar. *)
PathExpr = [ PathRoot ] IDENTIFIER { "." IDENTIFIER }
         | "Self" [ "." IDENTIFIER ] ;

GroupedExpr = "(" Expression ")" ;

(* Tuple expressions support optional named fields *)
TupleExpr = "(" [ TupleExprElement { "," TupleExprElement } [ "," ] ] ")" ;

TupleExprElement = [ IDENTIFIER ":" ] Expression ;

ArrayExpr = "[" [ Expression { "," Expression } [ "," ] ] "]"
          | "[" Expression ";" Expression "]" ;

(* Struct instantiation uses parentheses; type args and fields in same list *)
StructExpr = StructExprPath "(" [ StructArgList ] ")" ;

StructExprPath = TypePath ;

StructArgList = StructArg { "," StructArg } [ "," ] ;

(* Case-based disambiguation (see EBNF Notation section):
   - UPPER_IDENT (starts A-Z) → type argument
   - LOWER_IDENT (starts a-z or _) → value field
   This is a hard rule; no backtracking occurs. *)
StructArg = UPPER_IDENT [ ":" Type ]           (* type argument: T: Type or T shorthand *)
          | LOWER_IDENT [ ":" Expression ] ;   (* value field: name: expr or name shorthand *)

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
        return Self(x: 0, y: 0)
    }
}
```

**Match Expression Examples:**

```spl
let result = match value {
    .Some(x) => x * 2,
    .None => 0,
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
BlockExpression = [ Label ] Block
                | IfExpr
                | WhileExpr
                | ForExpr
                | LoopExpr ;

IfExpr = "if" Expression Block [ "else" ( IfExpr | Block ) ] ;

(* Labels use tick prefix and colon suffix for definition, tick prefix for reference *)
(* Definition: `'label: { ... }` or `'label: for x in ...` *)
(* Reference: `break 'label` or `continue 'label` *)
Label = "'" IDENTIFIER ":" ;

WhileExpr = [ Label ] "while" Expression Block ;

ForExpr = [ Label ] "for" Pattern "in" Expression Block ;

LoopExpr = [ Label ] "loop" Block ;

(* Break exits blocks/loops with optional value *)
(* - `break` exits immediately enclosing block/loop *)
(* - `break value` exits with value *)
(* - `break 'label` exits labeled block/loop *)
(* - `break 'label value` exits labeled block/loop with value *)
BreakExpr = "break" [ "'" IDENTIFIER ] [ Expression ] ;

ContinueExpr = "continue" [ "'" IDENTIFIER ] ;

(* Explicit return required for returning values from functions *)
ReturnExpr = "return" [ Expression ] ;

(* Yield is exclusively for generator functions - suspends and produces a value *)
YieldExpr = "yield" Expression ;

(* Throw an error in a throws function - desugars to return Err(expr) *)
ThrowExpr = "throw" Expression ;

(* Unsafe enables operations the compiler cannot verify as safe *)
UnsafeExpr = "unsafe" Block ;
```

**Unsafe Blocks:**

The `unsafe` keyword enables operations that the compiler cannot verify as safe. Like Rust, SPL only allows `unsafe` with block syntax:

```spl
// Unsafe block - enables unsafe operations within the block
let values = unsafe {
    let a = p1.read()
    let b = p2.read()
    (a, b)
}

// Single unsafe operation still uses block syntax
let value = unsafe { p.read() }

// Unsafe function definition
unsafe fn dangerous_operation(p: Ptr(T: i32)): i32 {
    return p.read()  // body is implicitly unsafe
}

// Calling unsafe function requires unsafe context
let result = unsafe { dangerous_operation(ptr) }
```

See [unsafe.md](unsafe.md) for the full list of operations that require unsafe.

**Pattern Matching in Control Flow:**

The `is` operator enables pattern matching directly in conditions:

```spl
// Pattern matching with is
if value is .Some(x) {
    // x is bound here
}

// Check without binding
if value.is_some() {
    // value exists
}

// Combined with other conditions
if value is .Some(x) && x > 0 {
    // x is positive
}

// In while loops
while queue.pop() is .Some(item) {
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
    let temp = x * 2
    return temp + 1
}

// Single-expression block: implicit value
let result = if condition { x * 2 } else { 0 }

// Multi-statement block: break required
let result = {
    let temp = compute()
    break temp * 2
}

// Error: multi-statement without return
fn bad(x: i32): i32 {
    let temp = x
    temp * 2  // ERROR: missing return
}

// Error: multi-statement without break
let bad = {
    let x = 1
    x + 1  // Block has type (), not i32
}
```

**Why this design?**

Single-expression blocks are concise and unambiguous. Multi-statement blocks require explicit `return`/`break` to avoid the subtle semantics where semicolon presence changes program behavior.

**Labeled Blocks and Break:**

Blocks, loops, and other control flow constructs can be labeled for targeted `break` or `continue`:

| Syntax | Meaning |
|--------|---------|
| `break` | Exit immediately enclosing block/loop |
| `break value` | Exit immediately enclosing with value |
| `break 'label` | Exit specific labeled scope |
| `break 'label value` | Exit specific labeled scope with value |
| `continue` | Continue immediately enclosing loop |
| `continue 'label` | Continue specific labeled loop |

Labels use tick prefix for both definition and reference, with a trailing colon for definitions (like Rust):

```spl
// Labeled block with value
let result = 'computed: {
    let a = expensive()
    let b = transform(a)
    break 'computed a + b
}

// Unlabeled block with value
let result = {
    let a = expensive()
    break a * 2
}

// Nested loops with labels
'outer: for x in items {
    'inner: for y in other {
        if done {
            break 'outer  // exit outer loop
        }
    }
}
```

**`yield` in Generators:**

The `yield` keyword is exclusively for generator functions—it suspends the generator and produces a value to the caller:

```spl
gen fn count(): i32 {
    let computed = 'block: {
        let a = 1
        break 'block a + 1  // block value via break
    }
    yield computed       // generator yield
    yield computed * 2   // generator yield
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
              | EnumShorthandPattern
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

RangePattern = RangePatternBound ( ".." | "..=" ) [ RangePatternBound ]
             | ( ".." | "..=" ) RangePatternBound ;

RangePatternBound = LiteralPattern
                  | PathExpr ;    (* const item path *)

(* Note: PathExpr in range patterns must resolve to a const item.
   This is verified semantically. *)

TuplePattern = "(" [ Pattern { "," Pattern } [ "," ] ] ")" ;

SlicePattern = "[" [ SlicePatternElement { "," SlicePatternElement } [ "," ] ] "]" ;

SlicePatternElement = RestPattern | Pattern ;

RestPattern = ".." [ IDENTIFIER ] ;

(* Struct patterns use parentheses *)
StructPattern = StructPatternPath "(" [ StructPatternFields ] ")" ;

StructPatternPath = TypePath ;

StructPatternFields = StructPatternField { "," StructPatternField } [ "," ] [ ".." ] ;

(* Field with optional pattern binding *)
StructPatternField = IDENTIFIER [ ":" Pattern ] ;

(* Enum variant patterns - supports both tuple-style and struct-style variants *)
EnumPattern = EnumPatternPath [ "(" [ EnumPatternFields ] ")" ] ;

EnumPatternPath = TypePath ;

(* Enum variant shorthand pattern - type inferred from context.
   See EnumShorthandExpr in section 4 for the corresponding expression syntax. *)
EnumShorthandPattern = "." UPPER_IDENT [ "(" [ EnumPatternFields ] ")" ] ;

(* Note: Enum variants must start with an uppercase letter (PascalCase convention). *)

(* Pattern fields for enum variants *)
EnumPatternFields = EnumPatternField { "," EnumPatternField } [ "," ] [ ".." ] ;

(* Named field (struct-style variant) or plain pattern (tuple-style or shorthand) *)
(* SEMANTIC NOTE: When the Pattern alternative is an IdentifierPattern (e.g., `x`),
   semantic analysis checks if the enum variant has a field with that name. If so,
   the pattern is interpreted as shorthand for `x: x` (bind field x to variable x).
   Otherwise, it's a positional match. This mirrors struct field shorthand in
   expressions — see "Struct Field Shorthand" in the Ambiguity Resolution section. *)
EnumPatternField = LOWER_IDENT ":" Pattern      (* explicit: field name with pattern *)
                 | Pattern ;                     (* positional or shorthand *)

ReferencePattern = "&" [ "mut" ] Pattern ;
```

**Note:** `VariantFields` (for declarations) and `EnumPatternFields` (for patterns) have different structures because they serve different purposes:
- **Declarations** specify types: `FieldList` contains `Type` or `name: Type`
- **Patterns** match values: `EnumPatternField` contains `Pattern` or `name: Pattern`

This asymmetry mirrors the distinction between struct definitions and struct patterns throughout the grammar.

**Note:** At most one `RestPattern` (`..` or `..name`) is allowed per slice pattern. This is enforced semantically, not syntactically.

**Tuple Patterns for Struct Destructuring:**

When the expected type is a struct (known from context), a tuple pattern can destructure it positionally:

```spl
let point = Point(x: 10.0, y: 20.0)
let (x, y) = point    // Fields matched positionally by declaration order
```

This works because the compiler knows `point` is a `Point` and can match tuple pattern elements to struct fields in declaration order. The struct pattern `Point(x, y)` remains available when explicit type naming is preferred.

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
| `0..MAX`              | Match range with const path as bound |
| `(a, b)`              | Destructure tuple                    |
| `[a, b, c]`           | Destructure fixed-size array/slice   |
| `[first, ..]`         | Match first, ignore rest             |
| `[first, ..rest]`     | Match first, bind rest to `rest`     |
| `[.., last]`          | Match last element                   |
| `[first, ..middle, last]` | Match first, last, bind middle   |
| `Point(x, y)`         | Destructure struct (shorthand)       |
| `Point(x: a, y: b)`   | Destructure with rename              |
| `Point(x, ..)`        | Partial struct destructure           |
| `(a, b) = struct_val` | Destructure struct via type inference |
| `Some(x)`             | Match tuple-style enum variant       |
| `None`                | Match unit enum variant              |
| `Ok(value)`           | Match Result Ok variant              |
| `Err(e)`              | Match Result Err variant             |
| `.Move(x, y)`         | Enum shorthand with implicit field bindings |
| `.Move(x: a, y: b)`   | Shorthand with explicit bindings     |
| `Message.Move(x, ..)`  | Named variant partial destructure   |
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
    .Some(0) | .None => "empty or zero",
    .Some(n) => "has value",
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

### 3. Explicit Type Application in Function and Method Calls

When calling generic functions or methods with explicit type arguments, type args use uppercase identifiers and value args use lowercase identifiers (see section 11 for details). Type arguments are mixed into `ArgList` alongside value arguments:

```spl
// Function calls
identity(T: i32, 42)              // T (uppercase) = type arg, 42 = positional
convert(From: i32, To: f64, value: 100)  // From, To = type args, value = value arg
parse(T: Config, input: text)     // T = type arg, input = value arg

// Method calls - same pattern
obj.method(T: i32, x: 1, y: 2)    // T = type arg, x/y = value args
list.collect(T: Vec(T: i32))      // explicit result type
```

**Case-based disambiguation (hard rule):**
- Uppercase identifier → RHS parsed as Type
- Lowercase identifier → RHS parsed as Expression

No backtracking or semantic reinterpretation occurs. Most generic calls don't need explicit type args due to inference:

```spl
let x = identity(42)      // T inferred as i32
let v = Vec.new()         // Type inferred from later usage
v.push(1)                 // Now v: Vec(T: i32)
```

### 4. Paths

All paths use `.` (dot) as the separator. No `::` exists. Paths can be prefixed with `$`, `super`, or `self` to specify the starting point:

```spl
std.vec.Vec              // Module path
self.field               // Field access (value)
self.helper              // Current module path
super.Parent             // Parent module path
$.utils.Config           // Package-root qualified path
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

### 7. Type Argument Shorthand

When a type parameter name matches the type name being passed, the `: Type` part can be omitted. This parallels struct field shorthand.

```spl
Option(T)                // Equivalent to Option(T: T)
Result(T, E)             // Equivalent to Result(T: T, E: E)
Result(T, E: Error)      // Mixed: T shorthand, E explicit
Vec(T: i32)              // Explicit (T ≠ i32)
```

This shorthand is particularly useful in generic contexts where type parameter names match:

```spl
// In where clauses and impl blocks
impl Clone for Option(T) where T: Clone {  // T means T: T
    fn clone(&self): Self {
        match self {
            .Some(v) => .Some(v.clone()),
            .None => .None,
        }
    }
}

// In function signatures
fn wrap(value: T): Option(T) where T {     // Return type uses shorthand
    return .Some(value)
}
```

### 8. Tuple Pattern for Struct Destructuring

A tuple pattern can match a struct when the struct's type is known from context. The pattern elements bind to struct fields in declaration order.

```spl
struct Point(x: f64, y: f64)

let point = Point(x: 10.0, y: 20.0)
let (x, y) = point              // Binds x=10.0, y=20.0

// Equivalent to explicit struct pattern:
let Point(x, y) = point
```

**When to use which:**
- `Point(x, y)` — explicit, works in any context, self-documenting
- `(x, y)` — concise, requires type to be inferrable from context

**Ambiguity:** When the expected type is ambiguous, a tuple pattern matches a tuple. Use the struct pattern form when you need to be explicit:

```spl
let value = get_value()         // Returns Point or (f64, f64)?
let (a, b) = value              // Matches based on inferred type
let Point(a, b) = value         // Explicitly matches Point
```

### 9. Index vs Slice

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

### 10. Associated Functions on Types

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

### 11. Type Arguments vs Value Arguments

SPL uses **case-based disambiguation** to distinguish type arguments from value arguments:

| Identifier Case | Parsed As |
|-----------------|-----------|
| **Uppercase** (e.g., `T`, `Key`) | Type argument |
| **Lowercase** (e.g., `x`, `name`) | Value argument |

This is a hard rule enforced by the parser. There is no backtracking or semantic reinterpretation—the case of the identifier definitively determines how it is parsed.

**Examples:**
```spl
// Uppercase identifier → type argument
Vec(T: i32)                    // T is uppercase → type arg, i32 is a Type
HashMap(K: String, V: i32)     // K, V uppercase → type args

// Lowercase identifier → value argument
Point(x: 1, y: 2)              // x, y lowercase → value args
greet(to: "Alice")             // to lowercase → value arg
```

**Naming conventions:**

Because case determines parsing, SPL enforces naming conventions:
- Type parameters must be uppercase: `T`, `K`, `V`, `Item`, `Error`
- Value parameters/fields must be lowercase: `x`, `name`, `value`, `count`

Code that violates these conventions will not parse as intended:
```spl
// These would NOT work as the author might expect:
Point(X: 1)                    // X is uppercase → parser expects a Type, not 1
vec(t: i32)                    // t is lowercase → parser expects an Expression, not i32 type
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

### 12. `is` vs Other Operators

The `is` keyword binds looser than comparison but tighter than `&&`.

```spl
x > 0 && y is .Some(v)     // (x > 0) && (y is .Some(v))
value is .Some(x) && x > 0 // (value is .Some(x)) && (x > 0)
```

### 13. Additional Disambiguation Examples

This section provides comprehensive examples for tricky cases.

#### Type vs Expression in Generic Context

When a name could be either a type or a value:

```spl
// 'String' is a type (uppercase), parsed as Type
let v: Vec(T: String) = Vec.new()

// 'string' is a value (lowercase), parsed as Expression
let s = string              // Variable reference
let p = Point(x: string)    // Value passed to field

// Type aliases must be uppercase to be usable as type arguments
type MyInt = i32                   // OK: uppercase alias
let v: Vec(T: MyInt) = Vec.new()   // T: uppercase → type arg, MyInt parsed as Type

// type myint = i32              // Discouraged: lowercase type alias
// let v: Vec(t: myint) = ...    // ERROR: t lowercase → parsed as value arg
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
let x = Option.Some(42)       // Variant constructor

// Option(T: i32) is a type
let y: Option(T: i32) = .Some(42)

// Vec(T: i32).new() - type then method
let v = Vec(T: i32).new()

// Vec.new() - path to associated function (type inferred)
let v = Vec.new()
v.push(42)  // Now Vec(T: i32)
```

#### Nested Generic Types

```spl
// Nested generics - each level uses its own type args
let nested: Vec(T: Option(T: i32)) = Vec.new()

// HashMap with complex value type
let map: HashMap(K: String, V: Vec(T: i32)) = HashMap.new()

// Result containing Option
let r: Result(T: Option(T: User), E: Error) = Ok(Some(user))
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

#### Naming Convention Requirements

Because case determines parsing without backtracking, certain naming patterns are not supported:

```spl
// NOT SUPPORTED: Uppercase value field names
struct JsonObject(
    Type: String,       // ERROR: "Type" is uppercase, parser expects a Type not String
    ID: i64,            // ERROR: "ID" is uppercase, parser expects a Type not i64
)

// SUPPORTED: Use lowercase field names
struct JsonObject(
    type_name: String,  // OK: lowercase field name
    id: i64,            // OK: lowercase field name
)

// NOT SUPPORTED: Lowercase type parameters
trait Functor where f {  // ERROR: f is lowercase, parsed as value not type
    // ...
}

// SUPPORTED: Use uppercase type parameters
trait Functor where F {  // OK: F is uppercase
    fn map(self, func: fn(A): B): F(T: B) where A, B
}
```

This design trades flexibility for simplicity—the parser never needs to backtrack or defer disambiguation to semantic analysis.

#### Grammar vs Semantic Disambiguation

SPL's grammar is designed to minimize ambiguity at the syntactic level, but some constructs require semantic analysis for full interpretation:

| Construct | Syntactic Rule | Semantic Interpretation |
|-----------|----------------|------------------------|
| Type vs value args | Case of identifier (uppercase/lowercase) | Purely syntactic—no semantic check needed |
| Struct field shorthand | `IDENTIFIER` without `:` in StructExpr | Resolved to `name: name` during type checking |
| Enum pattern shorthand | `Pattern` in EnumPatternField | If variant has matching field name, interpreted as `field: binding` |
| Tuple pattern on struct | TuplePattern matched against struct type | Positional matching by field declaration order |
| Variant field style | FieldList vs TypeList in VariantFields | Must be consistently named or positional (semantic error if mixed) |
| `pub` on variant fields | Allowed syntactically in FieldList | Semantic error—variant fields inherit variant visibility |
| Rest pattern count | Multiple `..` allowed syntactically | Semantic error if more than one per slice pattern |

**Design principle:** Where possible, disambiguation is syntactic (case-based rules, delimiter differences). Semantic checks handle consistency rules that would complicate the grammar.

#### Common Patterns

```spl
// 1. Generic function with inferred type
let items = [1, 2, 3]
let doubled = items.map(|x| x * 2)  // Types inferred

// 2. Generic function with explicit type
let parsed = parse(T: Config, input)  // Explicit T

// 3. Type annotation on binding
let config: Config = parse(input)     // Type on let, not call

// 4. Explicit type — use type annotation or type application
let v: Vec(T: i32) = Vec.new()    // Type annotation on binding
let v = Vec(T: i32).new()         // Type application on call

// 5. Return type provides context
fn load(): Result(T: Config, E: Error) {
    let data = read_file(path)!    // Result types inferred
    return parse(data)              // Return type known
}
```

#### Edge Cases with Imports

```spl
// Imported type - still uppercase
use other.module.Config
let c: Config = Config.default()

// Imported value - still lowercase
use other.module.default_config
let c = default_config()

// Aliased import - case preserved
use other.module.Config as Cfg    // Type alias
let c: Cfg = Cfg.default()

use other.module.helper as h      // Value alias
let result = h()
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

// Type arg shorthand: Point(T) means Point(T: T)
impl Point(T) where T {
    // Return type can use explicit Point(T: T) or shorthand Point(T)
    pub fn new(x: T, y: T): Point(T: T) {
        return Point(x: x, y: y)
    }

    // Self refers to Point(T) — the impl's self type
    fn clone(&self): Self {
        return Self(x: self.x, y: self.y)
    }

    pub fn swap(&mut self) {
        let temp = self.x
        self.x = self.y
        self.y = temp
    }
}

// Type alias with generic
type Pair(T) = (T, T)

// Function with default parameters
fn create_point(x: f64 = 0.0, y: f64 = 0.0): Point(T: f64) {
    return Point(x, y)
}

// Named parameters with labels
fn distance(from p1: &Point(T: f64), to p2: &Point(T: f64)): f64 {
    let dx = p1.x - p2.x
    let dy = p1.y - p2.y
    return (dx * dx + dy * dy).sqrt()
}

fn main() {
    // Struct instantiation with parentheses
    let mut origin = Point.new(0.0, 0.0)
    let target = Point(x: 3.0, y: 4.0)

    // Type alias for demonstration
    type T = i32

    // Associated functions on generic types (with shorthand and explicit forms)
    let numbers: Vec(T: i32) = Vec(T: i32).new()  // Explicit type args
    let items: Vec(T) = Vec(T).new()              // Shorthand: T means T: T (where T is a type in scope)
    let map = HashMap(K: String, V: i32).new()    // Explicit when types differ

    // Named arguments at call site
    let dist = distance(from: &origin, to: &target)

    // Control flow
    if dist > 5.0 {
        return
    }

    // Pattern matching with is
    let maybe: i32? = .Some(42)
    if maybe is .Some(x) {
        // x is bound
    }

    if maybe.is_some() {
        // value exists
    }

    // Match expression
    let doubled = match maybe {
        .Some(n) => n * 2,
        .None => 0,
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
    let value = 10 + 5 * 2             // 20 (multiplicative binds tighter)
    let widened = 65.widen()           // Type conversion via method
    let reference = &mut origin        // Mutable reference
    let indexed = [1, 2, 3][0]         // Array indexing
    let range = 0..100                 // Exclusive range (0 to 99)
    let inclusive = 0..=100            // Inclusive range (0 to 100)

    // Slicing and indexing with $
    let arr = [1, 2, 3, 4, 5]
    let last = arr[$-1]                // 5 (last element)
    let second_last = arr[$-2]         // 4 (second to last)
    let slice1 = arr[1:3]              // [2, 3]
    let slice2 = arr[:3]               // [1, 2, 3]
    let slice3 = arr[2:]               // [3, 4, 5]
    let slice4 = arr[2:$]              // [3, 4, 5] (explicit end)
    let middle = arr[1:$-1]            // [2, 3, 4] (exclude first and last)
    let copy = arr[:]                  // full copy

    // Named tuples
    let coords = (x: 3.0, y: 4.0)      // Named tuple expression
    let x_coord = coords.x              // Named field access
    let y_coord = coords.1              // Positional access also works

    // Named tuple as return type
    fn divide(a: i32, b: i32): (quotient: i32, remainder: i32) {
        return (quotient: a / b, remainder: a % b)
    }
    let result = divide(17, 5)
    let q = result.quotient             // Named access: 3
    let r = result.remainder            // Named access: 2

    // Patterns
    let (a, b) = (1, 2)                // Tuple destructuring
    let Point(x, y) = target           // Struct destructuring (explicit)
    let (px, py) = target              // Struct destructuring (inferred from type)
    let [first, ..rest] = [1, 2, 3, 4] // Slice pattern with rest
    let [head, .., tail] = [1, 2, 3]   // First and last

    // Block with break
    let computed = 'calc: {
        let a = 10
        let b = 20
        break 'calc a + b
    }
}

// Function types (colon for return)
type Predicate = fn(i32): bool
type BinaryOp = fn(i32, i32): i32
type Action = fn()

// Omit labels with underscore
fn apply(_ f: fn(i32): i32, _ x: i32): i32 {
    return f(x)
}

```

---

## Grammar Summary

| Category    | Key Productions                                                     |
|-------------|---------------------------------------------------------------------|
| Program     | `Program`, `Item`, `FunctionDef`, `StructDef`, `EnumDef`, `TraitDef`|
| Modules     | `UseDecl`, `UsePath`, `UseTree`, `ModuleDecl`                       |
| Types       | `Type`, `ReferenceType`, `ArrayType`, `FnType`, `GenericArgs`       |
| Statements  | `Block`, `Statement`, `LetStatement`                                |
| Expressions | `Expression`, `TypeExpr`, `IsExpr`, `MatchExpr`, `IfExpr`, `LoopExpr`|
| Patterns    | `Pattern`, `EnumPattern`, `StructPattern`, `SlicePattern`           |
| Literals    | `INTEGER`, `FLOAT`, `STRING`, `CHAR`, `true`, `false`               |

## Key Syntax Differences from Rust

| Feature             | Rust                      | SPL                          |
|---------------------|---------------------------|------------------------------|
| Path separator      | `::`                      | `.`                          |
| Generic application | `Vec<T>`                  | `Vec(T: T)` or `Vec(T: i32)` |
| Type vs value args  | Context-dependent         | Case-based (hard rule): `T:` = type, `x:` = value |
| Return type         | `-> T`                    | `: T`                        |
| Generic declaration | `fn foo<T>() {}`          | `fn foo() where T {}`        |
| Where clause        | Constrains only, after `<T>` | Declares AND constrains; struct/enum: after body, trait/impl: before body |
| Impl block generics | `impl<T> Vec<T>`          | `impl Vec(T: T) where T`     |
| Concrete impl       | `impl Vec<u32>`           | `impl Vec(T: u32)`           |
| Named struct decl   | `struct Point { x: i32 }` | `struct Point(x: i32)`       |
| Positional struct   | `struct Pair(i32, i32);`  | `struct Pair(i32, i32)`      |
| Struct literal      | `Point { x: 1 }`          | `Point(x: 1)` (instantiation) |
| Pattern matching    | `if let Some(x) = v {}`  | `if v is .Some(x) {}`        |
| Function return     | `expr` (implicit tail)    | `expr` (single) or `return` (multi-stmt) |
| Block value         | `expr` (implicit tail)    | `expr` (single) or `break` (multi-stmt)  |
| Semicolons          | Semantic (tail vs stmt)   | Optional (newline-terminated) |
| Named parameters    | Not built-in              | `fn foo(to name: T)`         |
| Default parameters  | Not supported             | `fn foo(x: i32 = 0)`         |
| Named tuples        | Not supported             | `(x: i32, y: i32)` type and expr |
| Type arg shorthand  | Not applicable            | `Option(T)` means `Option(T: T)` — parallels struct field shorthand for consistency |
| Tuple pattern for struct | Not supported        | `let (x, y) = point` (type inferred) |
| Concurrency model   | `async`/`await` keywords  | Go-style: `await()` is a method call, not a keyword |
