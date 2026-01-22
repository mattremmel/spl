# Rust Design Lessons: Technical Decisions to Reconsider

*A compilation of technical design decisions from Rust that language designers have identified as suboptimal, with detailed explanations and examples for consideration in new language projects.*

---

## Table of Contents

1. [Syntax and Grammar](#syntax-and-grammar)
   - [Angle Brackets for Generics](#angle-brackets-for-generics)
   - [Path Separator (`::`) vs Member Access (`.`)](#path-separator--vs-member-access-)
   - [Struct Initialization Syntax](#struct-initialization-syntax)
   - [Function Return Type Arrow (`->`)](#function-return-type-arrow--)
   - [Semicolon Significance](#semicolon-significance)
   - [Lifetime Annotation Syntax](#lifetime-annotation-syntax)
   - [Procedure Syntax (Implicit Unit Return)](#procedure-syntax-implicit-unit-return)
2. [Type System Design](#type-system-design)
   - [Nominal vs Structural Typing](#nominal-vs-structural-typing)
   - [First-Class vs Second-Class References](#first-class-vs-second-class-references)
   - [Explicit Lifetime Annotations](#explicit-lifetime-annotations)
3. [Numeric Types and Overflow](#numeric-types-and-overflow)
   - [Integer Overflow Behavior](#integer-overflow-behavior)
   - [Built-in Decimal Type](#built-in-decimal-type)
   - [Arbitrary Precision Integers](#arbitrary-precision-integers)
   - [The `as` Casting Operator](#the-as-casting-operator)
4. [Control Flow and Iteration](#control-flow-and-iteration)
   - [Exterior vs Interior Iteration](#exterior-vs-interior-iteration)
   - [Tail Call Optimization](#tail-call-optimization)
   - [`if-let` Design](#if-let-design)
   - [Range Syntax](#range-syntax)
5. [Module and Compilation Model](#module-and-compilation-model)
   - [Stable ABI and Compilation Units](#stable-abi-and-compilation-units)
   - [Cross-Crate Monomorphization](#cross-crate-monomorphization)
6. [Macros and Metaprogramming](#macros-and-metaprogramming)
   - [Macros as Varargs Substitute](#macros-as-varargs-substitute)
   - [Macro Invocation Syntax](#macro-invocation-syntax)
7. [Standard Library Design](#standard-library-design)
   - [Option vs Result for Checked Operations](#option-vs-result-for-checked-operations)
   - [Type Alias Naming Conventions](#type-alias-naming-conventions)
   - [Thin Standard Library Trade-offs](#thin-standard-library-trade-offs)
8. [Async and Concurrency](#async-and-concurrency)
   - [Runtime in Standard Library](#runtime-in-standard-library)
   - [Runtime-Agnostic Future Trait](#runtime-agnostic-future-trait)
   - [Async Debugging and Stack Traces](#async-debugging-and-stack-traces)
9. [Error Handling](#error-handling)
   - [Panic vs Result Duality](#panic-vs-result-duality)
   - [Error Trait Design](#error-trait-design)
10. [Trait and Implementation Syntax](#trait-and-implementation-syntax)
    - [`impl` Keyword Overloading](#impl-keyword-overloading)
    - [Index Traits vs Function Traits](#index-traits-vs-function-traits)
11. [Additional Considerations](#additional-considerations)
    - [Linear Types / Leak Prevention](#linear-types--leak-prevention)
    - [Reflection and Type Descriptors](#reflection-and-type-descriptors)

---

## Syntax and Grammar

### Angle Brackets for Generics

**The Problem:**
Rust uses `<>` for generic type parameters, which creates parsing ambiguities and requires the "turbofish" operator (`::<>`) in certain contexts.

**Graydon Hoare's Position:**
> "I lost almost every argument about this, from the angle brackets for type parameters to the pattern-binding ambiguity to the semicolon and brace rules... The grammar is not what I wanted."

> "Just don't get me started on angle brackets for type parameters and the single apostrophe for lifetimes!"

**Technical Issues:**
1. **Parsing Ambiguity:** `foo<bar>` could be a generic instantiation or a comparison expression
2. **Turbofish Requirement:** Expressions like `collect::<Vec<_>>()` require `::` to disambiguate
3. **Nested Generics:** `HashMap<String, Vec<i32>>` historically required spaces to avoid `>>` being parsed as right-shift

**Alternative: Square Brackets**

Early Rust prototypes used `[]` for generics:

```rust
// Hypothetical syntax with square brackets
fn map[T, U](vec: Vec[T], f: fn(T) -> U) -> Vec[U]

// No ambiguity - array indexing uses different context
let x = foo[Bar]      // Generic instantiation
let y = arr[0]        // Array indexing (requires integer)
```

**Design Recommendation:**
Consider using `[]` for generics, freeing `<>` exclusively for comparison operators. This eliminates parsing ambiguity entirely and removes the need for turbofish-style workarounds.

---

### Path Separator (`::`) vs Member Access (`.`)

**The Problem:**
Rust distinguishes between `::` (module paths, associated functions) and `.` (method calls, field access):

```rust
std::collections::HashMap::new()  // Path + associated function
my_map.insert(key, value)         // Method call
MyStruct::associated_fn()         // Associated function
instance.method()                 // Method
```

**Criticism:**
> "The distinction between path navigation (`::`) and member access (`.`) is not important enough to bother users at every single occasion. Instead, let the IDE use some syntax coloring and be done with it."

**Alternative Approach:**

Many languages use `.` uniformly:

```rust
// Hypothetical unified syntax
std.collections.HashMap.new()
my_map.insert(key, value)
MyStruct.associated_fn()
instance.method()
```

**Trade-offs:**
- **Pro:** Simpler mental model, less syntax to learn
- **Pro:** Consistent with most mainstream languages (Java, Python, JavaScript, C#)
- **Con:** Loses visual distinction between compile-time resolution and runtime dispatch
- **Con:** May complicate tooling that needs to distinguish these cases

**Design Recommendation:**
A unified `.` operator with IDE/tooling support for visual differentiation is likely more ergonomic for most users.

---

### Struct Initialization Syntax

**The Problem:**
Rust has distinct syntaxes for different kinds of initialization:

```rust
// Function call
let result = some_function(arg1, arg2);

// Struct with named fields (curly braces, colons)
let point = Point { x: 10, y: 20 };

// Tuple struct (parentheses)
let color = Color(255, 128, 0);

// Unit struct
let marker = Marker;
```

**Criticism:**
> "There is little reason why invoking functions, initializing structs and enums, and initializing tupled structs and enums have to follow different rules."

> "Especially considering that half the people immediately define a `::new()` function to avoid struct initialization syntax. Having the choice between both is already a net-negative on its own."

**Alternative: Unified Initialization with `()`**

```rust
// Hypothetical unified syntax
let result = some_function(arg1, arg2)
let point = Point(x: 10, y: 20)      // Named parameters
let color = Color(255, 128, 0)        // Positional parameters
let marker = Marker()                 // Explicit empty initialization
```

**Benefits:**
1. Single mental model for all "construction" operations
2. Natural path to named parameters for functions
3. No confusion about when to use `{}` vs `()`

**Design Recommendation:**
Standardize on `()` for all invocations and initializations, with `=` for named parameters (preserving `:` for type annotations):

```rust
let point = Point(x = 10, y = 20)
let result = http_request(url = "...", timeout = 30)
```

---

### Function Return Type Arrow (`->`)

**The Problem:**
Rust uses `->` for return types but `:` for all other type annotations:

```rust
let x: i32 = 5;                    // Variable: colon
fn foo(x: i32) -> i32 { x + 1 }    // Parameter: colon, Return: arrow
struct Point { x: i32, y: i32 }    // Field: colon
```

**Criticism:**
> "No reason to have two different ways to attach a type to the preceding program element. Just use `:`."

**Alternative:**

```rust
// Hypothetical unified syntax
fn foo(x: i32): i32 { x + 1 }

// Or with trailing return type
fn foo(x: i32) -> i32 { x + 1 }  // Keep arrow but make it mean "produces"
```

**Considerations:**
- The `->` syntax comes from ML/Haskell tradition and reads as "maps to"
- Using `:` everywhere creates a completely uniform type annotation syntax
- Languages like TypeScript use `:` for function returns successfully

---

### Semicolon Significance

**The Problem:**
In Rust, the presence or absence of a semicolon changes program semantics:

```rust
fn returns_value() -> i32 {
    42      // No semicolon: this IS the return value
}

fn returns_unit() {
    42;     // Semicolon: this is a statement, returns ()
}

// This is a compile error:
fn broken() -> i32 {
    42;     // Returns (), but signature says i32
}
```

**Criticism:**
> "Varying the meaning of a piece of code based on the presence of a `;` at a specific line is bad user interface design. Remove it and implement automatic semicolon inference, such that IDEs can show them, but no user has to ever type them."

**The Counter-argument:**
The expression-based nature of Rust (where `if`, `match`, and blocks are expressions) benefits from this distinction. However, it's a common source of confusion for newcomers.

**Alternative Approaches:**

1. **Explicit return everywhere:**
   ```rust
   fn foo() -> i32 {
       return 42
   }
   ```

2. **Trailing expression without semicolon significance:**
   ```rust
   fn foo() -> i32 {
       42  // Last expression is always the return value
   }
   
   fn bar() {
       42  // Discarded because return type is ()
   }
   ```

3. **Different block syntax for expressions vs statements:**
   ```rust
   fn foo() -> i32 = { 42 }      // Expression block
   fn bar() { do_stuff(); }      // Statement block
   ```

---

### Lifetime Annotation Syntax

**The Problem:**
Rust uses a single apostrophe for lifetime parameters:

```rust
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

struct ImportantExcerpt<'a> {
    part: &'a str,
}
```

**Graydon Hoare's Position:**
> "Just don't get me started on angle brackets for type parameters and the single apostrophe for lifetimes!"

**Issues:**
1. The apostrophe is unusual syntax that doesn't exist in most languages
2. Combined with angle brackets, creates visual noise: `<'a, 'b, T: 'a>`
3. Easy to miss when reading code

**Alternative Approaches:**

1. **Use a keyword or sigil:**
   ```rust
   fn longest[life a](x: &a str, y: &a str) -> &a str
   ```

2. **Make lifetimes more explicit:**
   ```rust
   fn longest<lifetime a>(x: &[a] str, y: &[a] str) -> &[a] str
   ```

3. **Second-class references (see below) - eliminate most lifetime annotations entirely**

---

### Procedure Syntax (Implicit Unit Return)

**The Problem:**
Functions returning `()` don't need an explicit return type:

```rust
fn prints_hello() {           // Implicitly returns ()
    println!("Hello");
}

fn returns_unit() -> () {     // Explicitly returns ()
    println!("Hello");
}
```

**Criticism:**
> "Functions that return no useful value enjoy special syntax privileges over functions that return a value. Drop this syntax sugar and require `-> ()` to be written down explicitly, like for every other type."

**Design Consideration:**
Requiring explicit `-> ()` makes the return type consistent and searchable, but adds verbosity to void functions.

---

## Type System Design

### Nominal vs Structural Typing

**The Problem:**
Rust uses nominal typing—types are distinct based on their names, not their structure:

```rust
struct Point { x: i32, y: i32 }
struct Vector { x: i32, y: i32 }

fn process_point(p: Point) { /* ... */ }

let v = Vector { x: 1, y: 2 };
// process_point(v);  // ERROR: expected Point, found Vector
```

**Graydon Hoare's Preference:**
> "Hoare prefers 'structural' typing (where objects have compatible types if their structure is the same — regardless of whether they've been declared with the same type name)."

**Structural Typing Example:**

```rust
// Hypothetical structural typing
struct Point { x: i32, y: i32 }
struct Vector { x: i32, y: i32 }

fn process(p: { x: i32, y: i32 }) { /* ... */ }

let point = Point { x: 1, y: 2 };
let vector = Vector { x: 1, y: 2 };

process(point);   // OK: structure matches
process(vector);  // OK: structure matches
```

**Trade-offs:**
- **Structural Pro:** More flexible, enables duck typing-like patterns
- **Structural Pro:** Better composability with anonymous types
- **Nominal Pro:** Stronger type safety (semantically different types stay different)
- **Nominal Pro:** Better error messages and IDE support
- **Nominal Pro:** Easier to reason about what types are compatible

**Hybrid Approach:**
TypeScript demonstrates a successful hybrid: structural typing for most cases, with nominal typing available via brands/tags when semantic distinction is needed.

---

### First-Class vs Second-Class References

**The Problem:**
In Rust, references are first-class types that can be stored in structs, returned from functions, and used anywhere a type is expected:

```rust
struct Container<'a> {
    data: &'a str,      // Reference stored in struct
}

fn get_ref<'a>(s: &'a str) -> &'a str {  // Reference returned
    s
}
```

This power comes with significant complexity: lifetime annotations, the borrow checker, `Pin`, and numerous edge cases.

**Graydon Hoare's Preference:**
> "I wanted `&` to be a 'second-class' parameter-passing mode, not a first-class type, and I still think this is the sweet spot for the feature. In other words I didn't think you should be able to return `&` from a function or put it in a structure. I think the cognitive load doesn't cover the benefits."

**Second-Class References Model:**

```rust
// Hypothetical second-class references
// References can only be function parameters, never stored or returned

fn process(data: &str) {  // OK: reference as parameter
    println!("{}", data);
}

// NOT ALLOWED:
// fn get_ref(s: &str) -> &str { s }  // Can't return reference
// struct Container { data: &str }     // Can't store reference
```

**Benefits of Second-Class References:**
1. **Dramatically simpler borrow checker** - no lifetime inference across function boundaries
2. **No lifetime annotations** - lifetimes are always local to a function
3. **Simpler mental model** - references are just a calling convention
4. **Easier to teach** - no lifetime parameters, no `'a` syntax

**How Iteration Works Without First-Class References:**

> "Iteration used to be by stack / non-escaping coroutines, which we also called 'interior' iteration, as opposed to 'exterior' iteration by pointer-like things that live in variables you advance. Such coroutines are now finally supported by LLVM and are actually a fairly old and reliable mechanism for a linking-friendly, not-having-to-inline-tons-of-library-code abstraction for iteration."

With interior iteration (coroutines/generators), you don't need iterators that hold references:

```rust
// Exterior iteration (current Rust) - needs first-class references
for item in vec.iter() {  // iter() returns Iterator holding &Vec
    process(item);
}

// Interior iteration (coroutine-based) - no stored references
vec.each(|item| {  // Callback receives reference, never stores it
    process(item);
});
```

**Design Recommendation:**
For a new language prioritizing simplicity, second-class references with coroutine-based iteration is worth serious consideration. You lose some expressiveness (can't build complex self-referential structures safely) but gain enormous simplicity.

---

### Explicit Lifetime Annotations

**The Problem:**
Rust requires explicit lifetime annotations in many situations:

```rust
// Without elision rules, you'd write:
fn first_word<'a>(s: &'a str) -> &'a str { /* ... */ }

// With multiple references, explicit annotations required:
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str { /* ... */ }

// Structs holding references always need annotations:
struct Parser<'input> {
    input: &'input str,
    position: usize,
}
```

**Graydon Hoare's Position:**
> "I was talked into [explicit lifetimes] as 'something that will almost always be inferred so it doesn't matter what the syntax is, nobody will ever write them'. Obviously that... didn't quite happen."

**The Reality:**
Lifetime annotations are pervasive in non-trivial Rust code, especially in:
- Library APIs
- Structs containing references
- Functions with multiple reference parameters
- Trait definitions

**Alternatives:**

1. **Second-class references (eliminate the need)**
2. **Better inference** - more sophisticated analysis to infer more lifetimes
3. **Different defaults** - make common patterns the default, annotate exceptions

---

## Numeric Types and Overflow

### Integer Overflow Behavior

**The Problem:**
Rust's integer overflow behavior differs between debug and release builds:

```rust
let x: u8 = 255;
let y = x + 1;

// Debug build: panic! (overflow check)
// Release build: wraps to 0 (no check, for performance)
```

**Graydon Hoare's Position:**
> "Integers overflow and either trap or wrap. Great. Maybe in another decade we can collectively decide this is also an important enough class of errors to catch?"

> "Swift at least traps in release by default — I wish Rust had chosen to."

**The Security Concern:**
Integer overflow has been the source of numerous security vulnerabilities:

> "Not to mention that `as` caused security vulnerabilities already."

A specific example from Mozilla's codebase:
> "Using `u16::max` instead of `u16::MAX` potentially allows very long certificates" — demonstrating how subtle integer issues cause real security bugs.

**Design Options:**

1. **Always trap (Swift's approach):**
   ```rust
   let x: u8 = 255;
   let y = x + 1;  // Always panics/traps, even in release
   ```

2. **Explicit wrapping types:**
   ```rust
   let x: u8 = 255;
   let y = x + 1;           // Compile error: overflow possible
   let z = x.wrapping_add(1); // Explicit: wraps to 0
   let w = x.saturating_add(1); // Explicit: stays at 255
   ```

3. **Arbitrary precision by default (see below)**

**Design Recommendation:**
For a safety-focused language, trap on overflow by default in all builds, with explicit wrapping/saturating operations for performance-critical code that needs different behavior.

---

### Built-in Decimal Type

**The Problem:**
Rust has no built-in decimal floating-point type for financial calculations:

```rust
// Binary floating point is unsuitable for money
let price = 0.1 + 0.2;  // 0.30000000000000004

// Must use external crates like `rust_decimal`
use rust_decimal::Decimal;
let price = Decimal::from_str("0.10").unwrap() + Decimal::from_str("0.20").unwrap();
```

**Graydon Hoare's Position:**
> "Basically every language discovers the long way that financial math is special and, at great length, eventually adds a decimal type. I wanted Rust to do this upfront, but it was perpetually deferred to libraries."

**Design Recommendation:**
Include a decimal type in the standard library or as a language primitive:

```rust
// Hypothetical built-in decimal
let price: decimal = 0.10d + 0.20d;  // Exactly 0.30
let tax = price * 0.0825d;            // Precise decimal arithmetic
```

---

### Arbitrary Precision Integers

**The Problem:**
Rust's integer types have fixed sizes that can overflow:

```rust
let big: i64 = 9_223_372_036_854_775_807;
let overflow = big + 1;  // Overflow!

// Must use external crates for big integers
use num_bigint::BigInt;
```

**Graydon Hoare's Position:**
> "Another thing that's great to have a compiler open-code is an integer type that overflows to an owned or refcounted bignum type: shipping enough stuff to let this happen efficiently in libraries is a huge pain (even if you get as far as stable inline assembly it won't go as fast as doing it in the compiler)."

**Design Options:**

1. **Python-style unlimited integers by default:**
   ```rust
   let x = 999999999999999999999999999999;  // Just works
   ```

2. **Automatic promotion:**
   ```rust
   let x: i32 = 2_000_000_000;
   let y = x * x;  // Automatically promotes to BigInt
   ```

3. **Explicit bigint with compiler support:**
   ```rust
   let x: bigint = 2_000_000_000;
   let y = x * x;  // Compiler generates optimal code
   ```

**Design Recommendation:**
Having compiler-native arbitrary precision integers enables optimizations that libraries cannot achieve, and eliminates an entire class of overflow bugs.

---

### The `as` Casting Operator

**The Problem:**
Rust's `as` operator performs multiple different operations:

```rust
let x: i32 = 256;

// Type coercion (safe)
let y: i64 = x as i64;

// Truncation (potentially lossy)
let z: i8 = x as i8;  // Silently truncates to 0!

// Pointer casts
let ptr = &x as *const i32;

// Numeric to boolean... wait, that's not allowed
// let b = x as bool;  // Error
```

**Criticism:**
> "Drop `as`... or at least make it make sense: it should *either* do type conversions *or* value conversions, but not both."

> "`as` caused security vulnerabilities already."

**Design Recommendation:**
Separate different casting operations:

```rust
// Hypothetical separated casting
let widened: i64 = x.widen();           // Safe widening
let truncated: i8 = x.truncate();       // Explicit lossy
let saturated: i8 = x.saturate();       // Clamp to range
let checked: Option<i8> = x.try_into(); // Fallible conversion
let bits: u32 = x.reinterpret();        // Bit reinterpretation
```

---

## Control Flow and Iteration

### Exterior vs Interior Iteration

**The Problem:**
Rust uses exterior iteration with iterator objects:

```rust
// Exterior iteration - iterator is a value you manipulate
let mut iter = vec.iter();
while let Some(item) = iter.next() {
    process(item);
}

// Syntactic sugar
for item in vec.iter() {
    process(item);
}
```

This requires iterators to hold references, which necessitates first-class references and lifetime complexity.

**Graydon Hoare's Preference:**
> "Iteration used to be by stack / non-escaping coroutines, which we also called 'interior' iteration... Such coroutines are now finally supported by LLVM and are actually a fairly old and reliable mechanism for a linking-friendly, not-having-to-inline-tons-of-library-code abstraction for iteration. They're in, like, BLISS and Modula-2 and such. Really normal thing to have, early Rust had them, and they got ripped out for a bunch of reasons that, again, mostly just form 'an argument I lost' rather than anything I disagree with today. I wish Rust still had them."

**Interior Iteration Model:**

```rust
// Interior iteration - control inverted, callback receives items
vec.each(|item| {
    process(item);
});

// The collection controls iteration, not the caller
// No iterator object needed, no stored references

// Early exit with special return
vec.each(|item| {
    if condition {
        return Break(result);
    }
    Continue
});
```

**Benefits:**
1. No need for first-class references for iteration
2. No iterator invalidation problems
3. Collection can optimize traversal internally
4. Works naturally with async/generators

**Implementation:**
Interior iteration is naturally implemented with stackful coroutines/generators:

```rust
// Generator-based iteration (hypothetical)
gen fn iterate<T>(vec: &Vec<T>) yields &T {
    for i in 0..vec.len() {
        yield &vec[i];
    }
}
```

---

### Tail Call Optimization

**The Problem:**
Rust does not guarantee tail call optimization (TCO):

```rust
// This might overflow the stack for large n
fn factorial(n: u64, acc: u64) -> u64 {
    if n <= 1 {
        acc
    } else {
        factorial(n - 1, n * acc)  // Tail position, but not guaranteed TCO
    }
}
```

**Graydon Hoare's Position:**
> "Tail calls. I actually wanted them! I think they're great. And I got argued into not having them because the project in general got argued into the position of 'compete to win with C++ on performance' and so I wound up writing a sad post rejecting them which is one of the saddest things ever written on the subject."

**Why TCO Matters:**
1. Enables functional programming patterns without stack overflow
2. Allows state machines to be expressed as mutual recursion
3. Makes certain algorithms natural to express

**Design Options:**

1. **Guaranteed TCO for tail calls:**
   ```rust
   fn factorial(n: u64, acc: u64) -> u64 {
       if n <= 1 { acc }
       else { factorial(n - 1, n * acc) }  // Guaranteed: no stack growth
   }
   ```

2. **Explicit `tailcall` keyword:**
   ```rust
   fn factorial(n: u64, acc: u64) -> u64 {
       if n <= 1 { acc }
       else { tailcall factorial(n - 1, n * acc) }  // Compiler enforces tail position
   }
   ```

3. **`become` keyword (proposed for Rust):**
   ```rust
   fn factorial(n: u64, acc: u64) -> u64 {
       if n <= 1 { acc }
       else { become factorial(n - 1, n * acc) }  // Explicit TCO request
   }
   ```

---

### `if-let` Design

**The Problem:**
Rust's `if let` has spawned multiple extension proposals:

```rust
// Basic if let
if let Some(x) = option {
    use(x);
}

// Proposed extensions:
// if let ... && condition { }
// if let ... else if let ... { }
// let ... else { }
// if let chains
```

**Criticism:**
> "You know a feature is not well-thought-out if it has spawned 4 extension proposals already."

**Alternative: `is` Pattern Matching:**

A more unified approach using an `is` operator:

```rust
// Hypothetical `is` syntax
if option is Some(x) {
    use(x);
}

if value is Some(x) && x > 0 {
    // Natural combination with boolean conditions
}

// Works uniformly in expressions
let result = if input is Valid(data) { process(data) } else { default };
```

**Benefits:**
- Single unified syntax for pattern matching in conditions
- Natural combination with boolean expressions
- No need for multiple extension proposals

---

### Range Syntax

**The Problem:**
Rust has dedicated syntax for ranges:

```rust
0..10      // Range, exclusive end
0..=10     // RangeInclusive
..10       // RangeTo
0..        // RangeFrom
..         // RangeFull
```

**Criticism:**
> "Range syntax takes up way too much language footprint for very little actual benefit, is a source of language expansion proposals and the actual implementation in Rust suffers from quite a few other problems."

**Issues:**
1. Multiple range types with subtle differences
2. The `..` vs `..=` distinction is a common source of off-by-one errors
3. Iterator behavior differences between range types

**Alternative:**
Make ranges library types with clear constructors:

```rust
// Hypothetical library-based ranges
Range.exclusive(0, 10)
Range.inclusive(0, 10)
Range.from(0)
Range.to(10)

// Or with named parameters
Range(start: 0, end: 10, inclusive: false)
```

---

## Module and Compilation Model

### Stable ABI and Compilation Units

**The Problem:**
Rust lacks a stable ABI (Application Binary Interface), meaning:
- Rust libraries must be recompiled with each compiler version
- Dynamic linking between Rust crates is impractical
- No stable C-compatible ABI for Rust types

**Graydon Hoare's Position:**
> "I wanted crates to allow inlining inside but present stable entrypoints to the outside. Swift wound up close to here, it's a huge technical headache but failure to do so is also a big part of Rust's terrible compile times and lack of a stable ABI. I resisted this at the time and have objected to the choice ever since."

**The Swift Model:**
Swift has `@inlinable` and module stability, allowing:
- Stable ABI for frameworks
- Selective cross-module inlining
- Binary-compatible library evolution

**Design Consideration:**
A stable ABI enables:
1. Pre-compiled system libraries (faster builds)
2. Plugin systems
3. Dynamic loading
4. Forward-compatible libraries

Trade-off: Some optimizations (like cross-crate monomorphization) become opt-in rather than automatic.

---

### Cross-Crate Monomorphization

**The Problem:**
Rust monomorphizes generic code at each use site:

```rust
// In library crate
pub fn process<T: Display>(value: T) {
    println!("{}", value);
}

// In user crate - generates new machine code for each T
process(42i32);
process("hello");
process(MyType);
```

This means:
- Generic code is compiled multiple times
- All generic code must be in headers (like C++ templates)
- Slow compile times for generic-heavy code

**Alternative: Type-erased generics with optional specialization:**

```rust
// Hypothetical: default is type-erased (one copy of machine code)
fn process<T: Display>(value: T) {
    println!("{}", value);
}

// Explicit monomorphization when performance needed
#[specialize]
fn fast_process<T: Numeric>(values: &[T]) -> T {
    // This gets monomorphized for each T
}
```

---

## Macros and Metaprogramming

### Macros as Varargs Substitute

**The Problem:**
Rust uses macros for variadic functions because the language lacks varargs:

```rust
// println! is a macro because Rust lacks varargs
println!("x = {}, y = {}", x, y);

// vec! is a macro for the same reason
let v = vec![1, 2, 3, 4, 5];

// format! too
let s = format!("{} + {} = {}", a, b, a + b);
```

**Criticism:**
> "Macros are largely used to work around the lack of varargs in Rust. All language designers hate varargs, but handing out macros as a replacement is considerably worse."

> "Macros are not very good. They are over-used due to the fact that Rust lacks varargs and abused due to the fact that they require special syntax at call-site (`some_macro!()`). Pattern matching in macros is also weird."

**Alternative: True Variadic Functions:**

```rust
// Hypothetical varargs syntax
fn println(fmt: &str, args: ...Display) {
    // args is a heterogeneous tuple/list of Display implementors
}

// Call without macro syntax
println("x = {}, y = {}", x, y)

fn vec<T>(items: ...T) -> Vec<T> {
    // items is a compile-time list
}

let v = vec(1, 2, 3, 4, 5)  // No exclamation mark!
```

**Design Recommendation:**
Support variadic generics/functions to eliminate the most common macro use cases, reserving macros for true metaprogramming.

---

### Macro Invocation Syntax

**The Problem:**
Rust macros require `!` at the call site:

```rust
println!("Hello");
vec![1, 2, 3];
format!("{}", x);
```

**Issues:**
1. Visual noise
2. Users must know whether something is a macro or function
3. Prevents transparent abstraction (can't replace macro with function)

**Criticism:**
> "Macros... require special syntax at call-site (`some_macro!()`)"

**Alternative Approaches:**

1. **No special syntax (hygienic macros look like functions):**
   ```rust
   println("Hello")  // Macro or function? Doesn't matter!
   ```

2. **Special syntax only for macro definitions, not calls:**
   ```rust
   macro println(fmt, ..args) { /* ... */ }
   
   println("Hello")  // Call looks like regular function
   ```

---

## Standard Library Design

### Option vs Result for Checked Operations

**The Problem:**
Rust's checked arithmetic returns `Option`:

```rust
let result: Option<i32> = 100i32.checked_add(200);

match result {
    Some(value) => println!("Sum: {}", value),
    None => println!("Overflow!"),
}
```

**The Issue:**
`Option` doesn't work with the `?` operator for error propagation without conversion:

```rust
fn calculate() -> Result<i32, Error> {
    let a = get_value()?;
    let b = get_other()?;
    
    // Can't use ? directly:
    // let sum = a.checked_add(b)?;  // Type mismatch!
    
    // Must convert:
    let sum = a.checked_add(b).ok_or(Error::Overflow)?;
}
```

**Proposed Fix:**
> "The `checked_*` methods in primitive integers should return `Result` so they are usable with `?`."

**Counter-argument:**
> "There has been some existing desire to make `?` work with `Option` as well, which would achieve this without any breakage."

**Design Recommendation:**
Either:
1. Make `?` work uniformly with `Option` and `Result`
2. Have checked operations return `Result<T, OverflowError>`
3. Provide both: `checked_add() -> Option`, `try_add() -> Result`

---

### Type Alias Naming Conventions

**The Problem:**
Rust's `std::io` module redefines `Result`:

```rust
// In std::io
pub type Result<T> = std::result::Result<T, std::io::Error>;

// This shadows the standard Result in io code
fn read_file() -> Result<String> {  // This is io::Result, not std::Result!
    // ...
}
```

**Criticism:**
> "Type alias misuse: In e.g. io crate: `type Result<T> = Result<T, io::Error>` … just call it `IoResult`."

**The Confusion:**
- `Result` means different things in different modules
- Error messages reference the alias, not the concrete type
- Newcomers are confused about which `Result` is which

**Design Recommendation:**
Use distinct names for specialized type aliases:

```rust
pub type IoResult<T> = Result<T, IoError>;
pub type ParseResult<T> = Result<T, ParseError>;
pub type FmtResult = Result<(), FmtError>;
```

---

### Thin Standard Library Trade-offs

**The Problem:**
Rust's standard library is intentionally minimal:

```rust
// No async runtime
// Must use external crate
use tokio::runtime::Runtime;

// No random numbers  
// Must use external crate
use rand::Rng;

// No HTTP client
// Must use external crate
use reqwest::Client;

// No argument parsing
// Must use external crate
use clap::Parser;
```

**Criticism:**
> "Rust's std is so thin that it doesn't even come with an async runtime. You have to pull in something like Tokio. Even random numbers require an external package!"

> "People have been making jokes about node_modules for a decade now, but this problem is just as bad in Rust codebases I've seen."

**Trade-offs:**

**Thin stdlib (Rust's approach):**
- Pro: Best-of-breed solutions can emerge
- Pro: Faster language evolution
- Con: Dependency hell
- Con: Inconsistent APIs across ecosystem
- Con: Security/audit burden

**Thick stdlib (Python/Go approach):**
- Pro: Batteries included
- Pro: Consistent, well-tested APIs
- Pro: Easier onboarding
- Con: Harder to improve stdlib
- Con: May not fit all use cases

**Design Recommendation:**
Consider a layered approach:
1. Core language (minimal, stable)
2. Extended stdlib (curated, versioned)
3. Ecosystem packages

---

## Async and Concurrency

### Runtime in Standard Library

**The Problem:**
Rust has async/await syntax but no runtime to execute futures:

```rust
async fn fetch_data() -> Data { /* ... */ }

fn main() {
    // This doesn't work! No runtime.
    // let data = fetch_data().await;
    
    // Must use external runtime
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let data = runtime.block_on(fetch_data());
}
```

**Criticism:**
> "Rust doesn't provide a runtime for its futures in the language, delegating instead to libraries like Tokio."

**Issues:**
1. **Ecosystem fragmentation:** Code written for Tokio may not work with async-std
2. **Learning curve:** Users must understand runtime concepts immediately  
3. **Incompatibility:** Libraries often hard-code runtime dependencies

> "An inconvenient truth about async Rust is that libraries still need to be written against individual runtimes. Writing your async code in a runtime-agnostic fashion requires conditional compilation, compatibility layers and handling edge-cases."

**Alternative Approaches:**

1. **Built-in runtime (Go model):**
   ```rust
   async fn main() {
       let data = fetch_data().await;  // Just works
   }
   ```

2. **Pluggable runtime with default:**
   ```rust
   // Default runtime provided, but swappable
   #[runtime(tokio)]  // Optional: specify non-default
   async fn main() { /* ... */ }
   ```

---

### Runtime-Agnostic Future Trait

**The Problem:**
Rust's `Future` trait doesn't encode runtime requirements:

```rust
// This future secretly requires Tokio
async fn needs_tokio() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}

// Runtime mismatch = runtime panic!
async_std::task::block_on(needs_tokio());  // Panics!
```

**Proposed Solution:**
> "Future trait which is generic on the context being passed in, such that you can get a compile error if you try to run a future dependent on one runtime inside of another, instead of the current approach where you get a runtime panic."

**Hypothetical Design:**

```rust
// Future parameterized by runtime
trait Future<R: Runtime> {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<R>) -> Poll<Self::Output>;
}

// Compile-time enforcement
async fn needs_tokio() -> impl Future<TokioRuntime> { /* ... */ }
async fn needs_async_std() -> impl Future<AsyncStdRuntime> { /* ... */ }

// Type error, not runtime panic:
// async_std::block_on(needs_tokio())  // Compile error!
```

---

### Async Debugging and Stack Traces

**The Problem:**
Async Rust produces poor stack traces:

```rust
// Sync code - clear stack trace
fn sync_chain() {
    a();  // Stack: main -> sync_chain -> a
}

// Async code - fragmented "stack"
async fn async_chain() {
    a().await;  // "Stack" is scattered across task states
}
```

**Criticism:**
> "Rust Async is hard to debug, because there is no usable stack anymore. Only task with their current state, but we lose the context/lineage of its execution. Tokio console is doing a lot to help on that point, but it does not come close to the effectiveness of a meaningful stacktrace/core-dump."

> "Stack traces in async Rust typically contain details from these state machines, as well as function calls from the runtime. As such, interpreting stack traces can be a bit more involved than it would be in synchronous Rust."

**Design Considerations:**

1. **Structured concurrency:** Track parent-child relationships between tasks
2. **Async stack reconstruction:** Runtime support for logical stack traces
3. **Tracing integration:** First-class support for distributed tracing

---

## Error Handling

### Panic vs Result Duality

**The Problem:**
Rust has two error handling mechanisms:

```rust
// Recoverable errors - Result
fn parse(s: &str) -> Result<i32, ParseError> {
    s.parse().map_err(|_| ParseError)
}

// Unrecoverable errors - panic
fn index(arr: &[i32], i: usize) -> i32 {
    arr[i]  // Panics if out of bounds!
}
```

**Criticism:**
> "Panic is a little more bothersome, because Rust libraries go to great pains (with many syntactic tricks like `?` and auto-conversions from smaller error types to larger ones) to handle errors explicitly, but then panics unwind the stack to the top of the process, and panics inside a panic don't run destructors, etc. The overall effect is that, like my three questions of language design, the answer to 'how you handle errors' is 'at least two, incompatible ways.'"

**Issues:**
1. Panics are invisible in function signatures
2. Panic in one thread can poison mutexes in other threads
3. Double-panic aborts without cleanup
4. `catch_unwind` exists but is awkward

**Alternative Approaches:**

1. **Effects/Capabilities system:**
   ```rust
   fn index(arr: &[i32], i: usize) -> i32 throws Panic {
       arr[i]  // Caller knows this might panic
   }
   ```

2. **Everything is Result:**
   ```rust
   fn index(arr: &[i32], i: usize) -> Result<i32, IndexError> {
       arr.get(i).ok_or(IndexError)
   }
   ```

3. **Checked exceptions:**
   ```rust
   fn index(arr: &[i32], i: usize) throws IndexError -> i32 {
       if i >= arr.len() { throw IndexError }
       arr[i]
   }
   ```

---

## Trait and Implementation Syntax

### `impl` Keyword Overloading

**The Problem:**
Rust's `impl` keyword serves multiple purposes:

```rust
// 1. Inherent implementations (methods on a type)
impl Point {
    fn new(x: i32, y: i32) -> Point { Point { x, y } }
}

// 2. Trait implementations
impl Display for Point {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result { /* ... */ }
}

// 3. Extension methods on generic types
impl<T> Option<Option<T>> {
    fn flatten(self) -> Option<T> { /* ... */ }
}
```

**Criticism:**
> "There are roughly three different purposes for which `impl` is used. Disentangle these purposes, drop the `impl` keyword, and make the replacements feel more cohesive with the rest of the language."

**Proposed Alternative:**

```rust
// 1. Methods in the type body
struct Point(x: i32, y: i32) {
    fn new(x: i32, y: i32) -> Point { Point(x, y) }
}

// 2. Trait implementation with `trait` keyword
trait Display for Point {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result { /* ... */ }
}

// 3. Extension traits (explicit)
trait OptionExt<T> for Option<Option<T>> {
    fn flatten(self) -> Option<T> { /* ... */ }
}
```

---

### Index Traits vs Function Traits

**The Problem:**
Rust has separate traits for indexing and function calls:

```rust
// Indexing
impl Index<usize> for MyVec {
    type Output = i32;
    fn index(&self, i: usize) -> &i32 { &self.data[i] }
}

// Function calls
impl Fn<(i32,)> for Adder {
    type Output = i32;
    extern "rust-call" fn call(&self, (x,): (i32,)) -> i32 { self.0 + x }
}

// Usage looks similar but uses different traits
my_vec[0]    // Uses Index
adder(5)     // Uses Fn
```

**Criticism:**
> "Providing traits to let people decide how round they want their function call parentheses to be is not a useful feature. Fold `Index` and `IndexMut` into `Fn` trait family."

**Consideration:**
The separation exists because indexing and calling have different semantics (indexing typically returns references, calling returns owned values). But the distinction may not be worth the complexity.

---

## Additional Considerations

### Linear Types / Leak Prevention

**The Problem:**
Rust allows values to be leaked without running destructors:

```rust
use std::mem::forget;

let file = File::open("important.txt")?;
forget(file);  // File handle leaked! Never closed.

// Or via Rc cycles
let a = Rc::new(RefCell::new(None));
let b = Rc::new(RefCell::new(Some(a.clone())));
*a.borrow_mut() = Some(b.clone());
// Both leaked via reference cycle
```

**Proposed Solution:**
> "`Leak`/`Forget` auto-trait restricting the ability to leak values without calling the destructor."

**Linear Types Concept:**

```rust
// Hypothetical linear types
linear struct MustClose {
    handle: RawHandle,
}

impl MustClose {
    // Must be consumed by one of these:
    fn close(self) { /* ... */ }
    fn into_handle(self) -> RawHandle { /* ... */ }
}

let file = MustClose::open("file.txt")?;
// forget(file);  // Compile error! Linear type must be consumed
file.close();     // OK: explicitly consumed
```

**Benefits:**
- Guarantee cleanup code runs
- Prevent resource leaks by construction
- Enable safe APIs for things like scoped threads

---

### Reflection and Type Descriptors

**The Problem:**
Rust has very limited runtime reflection:

```rust
// Can get TypeId, but not much else
let id = TypeId::of::<MyStruct>();

// No runtime field access
// No runtime method invocation
// No runtime type construction
```

**Graydon Hoare's Original Vision:**
> "The language initially had (and I hoped it would have again) compiler-emitted 'type descriptors' that the user could invoke a reflection operator on."

**Use Cases:**
1. Serialization without procedural macros
2. ORM/database mapping
3. Dependency injection
4. Testing frameworks
5. Debug printing

**Trade-offs:**
- Reflection adds runtime overhead
- Conflicts with monomorphization (no single type representation)
- Security/privacy concerns (accessing private fields)

**Potential Design:**

```rust
// Hypothetical reflection
let desc = TypeDescriptor::of::<MyStruct>();

for field in desc.fields() {
    println!("{}: {:?}", field.name(), field.get(&instance));
}

// Opt-in reflection to control overhead
#[reflect]
struct MyStruct {
    pub name: String,
    private_field: i32,  // Not reflected
}
```

---

## Summary Table

| Category | Current Rust | Alternative Approach |
|----------|--------------|---------------------|
| Generic syntax | `<T>` with turbofish | `[T]` square brackets |
| Path separator | `::` vs `.` | Unified `.` |
| Struct init | `Point { x: 1 }` | `Point(x = 1)` |
| Return type | `-> T` | `: T` |
| Semicolons | Significant | Optional/inferred |
| Lifetimes | `'a` syntax | Keywords or second-class refs |
| Unit return | Implicit | Explicit `-> ()` |
| Typing | Nominal | Structural or hybrid |
| References | First-class | Second-class |
| Integer overflow | Debug: trap, Release: wrap | Always trap |
| Decimal type | Library only | Built-in |
| Bigints | Library only | Built-in |
| Casting | `as` (multi-purpose) | Separate operations |
| Iteration | Exterior (iterators) | Interior (coroutines) |
| Tail calls | Not guaranteed | Guaranteed or explicit |
| Pattern matching | `if let` | Unified `is` syntax |
| Ranges | `..` syntax | Library types |
| ABI | Unstable | Stable with opt-in inlining |
| Varargs | Macros | True variadic functions |
| Async runtime | External crate | Built-in or standard |
| Error handling | Result + panic | Unified system |
| Reflection | Minimal | Compiler-emitted descriptors |

---

## Sources

1. Graydon Hoare, "The Rust I Wanted Had No Future" (2023)
2. Graydon Hoare, Reddit comments and discussions
3. Rust Internals Forum, "What if the transition to Rust 2.0 can be fully machine applicable?"
4. "Language Design: Fixing Rust's mistakes" - soc.me
5. "Async Rust Is A Bad Language" - bitbashing.io
6. "The State of Async Rust" - Rust Async Book
7. "Common Mistakes with Rust Async" - Qovery Blog
8. "Second-Class References" - Fernando Borretti
9. The New Stack, "Graydon Hoare Remembers the Early Days of Rust"
10. Various Hacker News and Lobsters discussions
11. Rust GitHub issues with `rust-2-breakage-wishlist` label
