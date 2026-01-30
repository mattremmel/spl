# SPL Compilation Unit Strategy

This document evaluates compilation unit strategies and provides a recommendation for SPL.

## Background

A **compilation unit** is the smallest piece of code that can be compiled independently. This choice affects:

- **Compile times**: Smaller units enable more incremental builds
- **ABI stability**: Can pre-compiled units be reused across compiler versions?
- **Optimization**: What scope does the optimizer have?
- **Developer experience**: Error messages, refactoring, IDE support

## Survey of Approaches

### Rust: Crate as Compilation Unit

Rust's crate is a collection of modules compiled together.

**How it works:**
- A crate is compiled into a single `.rlib` (for libraries) or binary
- Generic code is monomorphized at each use site, even across crates
- No stable ABI - crates must be recompiled when the compiler changes

**Pros:**
- Clear compilation boundaries
- Parallel compilation of independent crates
- Aggressive monomorphization = fast runtime code

**Cons:**
- No stable ABI - can't ship pre-compiled Rust libraries
- Slow compile times (cross-crate monomorphization recompiles generic code)
- Orphan rules needed for trait coherence
- Large binary sizes from duplicated monomorphized code

From [Rust's Huge Compilation Units (TiDB)](https://www.pingcap.com/blog/rust-huge-compilation-units/):

> "The monomorphization-compile-time problem is compounded in Rust because Rust translates generic functions in every crate that instantiates them. If crate A calls `print("hello")`, and crate B also calls `print("world")`, then both crate A and B will contain the monomorphized `print_str` function — the compiler does all the type-checking and translation work twice."

> "Generics in Rust can lead to accidentally-quadratic compilation times across many crates!"

From [rust-design-lessons.md](./rust-design-lessons.md):

> **Graydon Hoare's Position:** "I wanted crates to allow inlining inside but present stable entrypoints to the outside. Swift wound up close to here, it's a huge technical headache but failure to do so is also a big part of Rust's terrible compile times and lack of a stable ABI. I resisted this at the time and have objected to the choice ever since."

### Swift: Module with ABI Stability

Swift modules have selective ABI stability with witness tables for generics.

**How it works:**
- Modules compile to `.swiftmodule` + object files
- `@inlinable` marks functions that can be inlined across module boundaries
- ABI stability for system frameworks enables binary compatibility
- Generics use witness tables (dictionary passing) by default

**Pros:**
- Pre-compiled system libraries (fast builds)
- Selective inlining preserves optimization where needed
- True binary compatibility for frameworks

**Cons:**
- Complex implementation
- Must carefully choose what to make `@inlinable`
- ABI stability constrains language evolution

From [How Swift Achieved Dynamic Linking Where Rust Couldn't (Faultlore)](https://faultlore.com/blah/swift-abi/):

> "Unlike Rust and C++, which generate separate code copies for each generic specialization, Swift compiles generic functions into single implementations that handle all type substitutions dynamically."

> "Swift employs 'witness tables' (essentially vtables) to expose type metadata at runtime. For resilient types, the application queries the dynamic library at runtime for a type's value witness table, which contains essential properties: size, alignment, stride, and extra inhabitants."

From [Swift ABI Stability and More](https://www.swift.org/blog/abi-stability-and-more/):

> "Swift 5 provides binary compatibility for apps: a guarantee that going forward, an app built with one version of the Swift compiler will be able to talk to a library built with another version."

### Go: Package with GCShape Stenciling (Go 1.18+)

Go packages compile independently with a hybrid generics approach.

**How it works:**
- Each package compiles to a `.a` archive
- Go 1.18 added generics using "GCShape stenciling" with dictionaries
- Types with the same "GC shape" (size, alignment, pointer layout) share instantiations
- All pointer types share a single instantiation

**Pros:**
- Very fast incremental builds
- Simple mental model
- Pre-compiled stdlib improves build times
- Smaller binary sizes than full monomorphization

**Cons:**
- Less optimization than full monomorphization
- Method calls on generics can't be inlined
- Escape analysis must be conservative

From [Go 1.18 Generics Implementation Design](https://github.com/golang/proposal/blob/master/design/generics-implementation-dictionaries-go1.18.md):

> "In order to avoid creating a different function instantiation for each invocation of a generic function with distinct type arguments, we pass a dictionary along with every call to a generic function. The dictionary provides relevant information about the type arguments that allows a single function instantiation to run correctly for many distinct type arguments."

From [Go GCShape Design](https://github.com/golang/proposal/blob/master/design/generics-implementation-gcshape.md):

> "A gcshape grouping is a collection of types that can all share the same instantiation of a generic function in our implementation. Two concrete types are in the same gcshape grouping if and only if they have the same underlying type or they are both pointer types."

### OCaml: File as Compilation Unit

OCaml compiles individual `.ml` files separately with powerful module system.

**How it works:**
- Each file compiles to `.cmo` (bytecode) or `.cmx` (native)
- Interface files (`.mli`) declare module signatures
- Functors provide compile-time polymorphism
- Cross-module optimization via flambda

**Pros:**
- Very fine-grained incremental compilation
- Separate compilation is simple and fast
- Powerful module abstraction (functors)

**Cons:**
- Complex module system (functors, first-class modules)
- Limited optimization without whole-program analysis
- Recursive modules require special handling

From [OCaml Manual - The Module System](https://ocaml.org/manual/5.3/moduleexamples.html):

> "Compilation units are special cases of structures and signatures. A compilation unit A comprises two files: the implementation file A.ml and the interface file A.mli."

From [OCamlPro's Compiler Team Work](https://ocamlpro.com/blog/2019_08_30_ocamlpros_compiler_team_work_update/):

> "One patch allows compiling several different files as mutually recursive modules... This will allow developers using recursive modules extensively to properly separate not only the different modules from each other, but also the implementation and interfaces into .ml and .mli files."

### Zig: Comptime with Monomorphization

Zig uses compile-time execution for generics, similar to C++ templates.

**How it works:**
- `comptime` keyword allows compile-time code execution
- `anytype` parameters trigger monomorphization
- Duck-typed generics (no trait bounds)
- Full compile-time reflection

**Pros:**
- Powerful metaprogramming without macros
- "Zig all the way down" - no separate macro language
- Fast runtime code (full monomorphization)

**Cons:**
- C++-style template error messages
- Function signatures don't document type requirements
- Same compile-time costs as Rust/C++

From [Assorted Thoughts on Zig and Rust](https://www.scattered-thoughts.net/writing/assorted-thoughts-on-zig-and-rust/):

> "Like templates, one of things you can use comptime for is duck typed generics, as opposed to Rust's early-checked generics. Arguably this combination is more complex than Rust's. It has no way to constrain a comptime type argument (traits), so you get error messages like C++ templates."

## Trade-off Analysis

### Summary Table

| Factor | Rust (Crate) | Swift (Module+ABI) | Go (Package+GCShape) | OCaml (File) | Zig (Comptime) |
|--------|--------------|-------------------|---------------------|--------------|----------------|
| Incremental build speed | Slow | Fast | Fast | Very Fast | Slow |
| Runtime performance | Excellent | Very Good | Good | Good | Excellent |
| Pre-compiled libraries | No | Yes | Partial | Yes | No |
| Generic code sharing | No | Yes | Partial | N/A | No |
| Implementation complexity | Medium | High | Medium | Medium | Low |
| Binary size | Large | Medium | Small | Small | Large |

### The Generics Trade-off Triangle

From [Models of Generics and Metaprogramming (Tristan Hume)](https://thume.ca/2019/07/14/a-tour-of-metaprogramming-models-for-generics/):

> "Boxing and monomorphization form the basis of the two major classes of solutions to generics. Boxing is where we put everything in uniform 'boxes' so they all act the same way... Monomorphization is where we copy the code multiple times for the different types of data."

> "Swift makes the interesting realization that by using dictionary passing and also putting the size of types and how to move, copy and free them into the tables, they can provide all the information required to work with any type in a uniform way without boxing."

**Three approaches:**
1. **Full Monomorphization** (Rust, C++, Zig): Best performance, worst compile times
2. **Boxing/Type Erasure** (Java, Go interfaces): Fast compiles, runtime overhead
3. **Witness Tables** (Swift, Haskell): Middle ground - no boxing allocation, some dynamic dispatch

### Key Insight: Monomorphization is the Pain Point

From [Generics and Compile-Time in Rust (TiDB)](https://www.pingcap.com/blog/generics-and-compile-time-in-rust/):

> "Besides just duplication, generics add one more problem — they shift the blame for compile times to consumers. Most of the compile time cost of generic functions is borne out by the crates that use the functionality, while the defining crate just typechecks the code without doing any code generation."

> "Downstream monomorphization means generics are only translated once they are instantiated, so even if all crates are perfectly equally sized for parallel compilation, their generic types will not be translated until later stages in the crate graph."

Swift's solution from [Doug Gregor's Swift for C++ Practitioners](https://www.douggregor.net/posts/swift-for-cxx-practitioners-generics/):

> "With Swift's separate compilation of generics, monomorphization is an optimization: the compiler can choose to monomorphize uses of generics when it can see both the use and the definition. This effectively lets the optimizer decide between having a single implementation (slower due to dynamic dispatch, but shared) and having many monomorphized implementations (faster because each is specialized for a type, but can lead to 'template bloat')."

## Recommendation for SPL

### Decision: Package = Compilation Unit with Stable ABI

SPL should use **packages** as compilation units with **stable ABI by default** and **type-erased generics** with opt-in specialization.

```
myproject/
├── main.spl           ─┐
└── utils.spl           │── One compilation unit: "myproject" package
                       ─┘
├── network/
│   ├── client.spl     ─┐
│   └── server.spl      │── Modules within the same package
│                      ─┘
```

This aligns with:
- SPL's module system (Go-style directory = module, package = compilation unit)
- Graydon Hoare's preferred design for Rust
- Swift's proven approach to ABI stability

### Design Choices

**Phased Approach:** The recommendations below are architected to keep options open, not commitments to ship immediately:

- **Phases 1-2:** Compile from source, focus on language features and correctness
- **Phase 3+:** Add binary stdlib/library support *if* compile times become a pain point
- **Key insight:** Witness table architecture enables stable ABI without requiring it upfront

This means we design for ABI stability (using witness tables, defining clear package boundaries) but defer the maintenance burden of actually shipping and supporting pre-compiled binaries until there's a demonstrated need.

#### 1. Design for Stable ABI

The architecture supports stable calling conventions and layouts:
- Packages *can* be pre-compiled into `.splpkg` or `.a` files
- The standard library *could* ship pre-compiled
- Incremental builds *would* only recompile changed packages

**Current reality:** In early phases, we compile everything from source. The witness table design keeps the pre-compiled library option open for later.

**Rationale:** This directly addresses Rust's compile time issues while deferring complexity. From rust-design-lessons.md:

> "A stable ABI enables: (1) Pre-compiled system libraries (faster builds), (2) Plugin systems, (3) Dynamic loading, (4) Forward-compatible libraries."

#### 2. Witness Tables for Generics by Default

Generic functions use witness tables (like Swift):

```spl
// This compiles to ONE function that works with any T
fn print_all<T: Display>(items: &[T]) {
    for item in items {
        println(item.display());  // Dynamic dispatch via witness table
    }
}
```

The witness table contains:
- Size and alignment of T
- How to copy, move, and drop T
- Function pointers for trait methods

**Rationale:** Enables separate compilation of generic code. From Swift's approach:

> "Swift is able to compile a generic function into a single implementation that can handle every substitution dynamically."

#### 3. Opt-in Monomorphization via `#[specialize]`

For performance-critical code, allow specialization:

```spl
#[specialize]  // Monomorphize for each T
fn sum<T: Numeric>(items: &[T]) -> T {
    // This generates specialized code for i32, i64, f64, etc.
}
```

**Rationale:** Preserves ability to optimize hot paths. The optimizer can also auto-specialize within a package.

#### 4. Package Interface Files

Each package produces an interface file (like Swift's `.swiftinterface` or OCaml's `.mli`):

```
network.splpkg      # Binary interface (for fast loading)
network.spli        # Text interface (for debugging/tooling)
network.a           # Compiled code
```

**Rationale:** Enables incremental compilation and tooling without exposing implementation details.

### Implementation Phases

**Phase 1: Current (Whole Program)** - Already implemented
- Compile everything together
- No separate compilation
- Good for bootstrapping

**Phase 2: Package Boundaries**
- Define package interface format
- Implement separate compilation of packages
- Generics still use whole-program monomorphization

**Phase 3: Witness Tables**
- Define stable calling convention
- Implement witness tables for generic types
- Type-erased generics by default

**Phase 4: Selective Specialization**
- Add `#[specialize]` attribute
- Implement monomorphization for marked functions
- Auto-specialization heuristics within packages

### Implementation Priority

Binary library support (pre-compiled stdlib, `.splpkg` files) is **"nice to have"** not **"must have"**:

1. **Language features and correctness come first** - Phases 1-2 focus on getting the language right
2. **Compile-from-source is the initial strategy** - Simple, debuggable, no ABI maintenance burden
3. **The architecture enables the optimization path** - Witness tables let us add binary libraries later without redesign
4. **Trigger for Phase 3+** - Only pursue binary libraries when/if compile times become a demonstrated pain point

This prioritization avoids premature optimization of build times while ensuring we don't paint ourselves into a corner architecturally.

### What This Means in Practice

```spl
// In std/vec.spl (pre-compiled, ships with SPL)
pub struct Vec<T> { ... }

impl<T> Vec<T> {
    // Uses witness table for T - one compiled version
    pub fn push(&mut self, item: T) { ... }

    // Marked for specialization - monomorphized at use site
    #[specialize]
    pub fn sort(&mut self) where T: Ord { ... }
}

// In user code
use std::vec::Vec;

fn main() {
    let mut v: Vec<i32> = Vec::new();
    v.push(1);   // Calls pre-compiled push, no recompilation
    v.sort();    // Monomorphized for i32, compiled here
}
```

### Trade-offs

**Accepted:**
1. Slight runtime overhead for non-specialized generic code (witness table indirection)
2. Complexity of implementing witness tables
3. Decision burden on when to use `#[specialize]`

**Avoided:**
1. No orphan rules needed - coherence is simpler with type erasure
2. No recompilation cascade - changing a generic function doesn't rebuild the world
3. No unstable ABI - pre-compiled libraries work across versions

## Comparison to Task Questions

From the original task (spl-bkf):

> 1. Rust's crate model - benefits/drawbacks?

**Benefits:** Clear boundaries, aggressive optimization, parallel crate compilation.
**Drawbacks:** No stable ABI, slow incremental builds due to cross-crate monomorphization, orphan rules complexity.

> 2. Could packages be compiled independently?

**Yes**, with:
- Stable ABI for non-generic code
- Witness tables for generic code
- Interface files for type information

This is exactly what Swift does successfully.

> 3. Cross-package optimization / LTO trade-offs?

With witness tables as default:
- Cross-package optimization is less critical (generic code is pre-compiled)
- `#[specialize]` provides escape hatch for performance-critical paths
- LTO remains available for release builds
- Profile-guided optimization can inform auto-specialization

## Summary

| Choice | Decision | Rationale |
|--------|----------|-----------|
| Compilation unit | Package | Matches SPL's module system |
| ABI | Design for stability (implement later) | Keeps option open without upfront maintenance burden |
| Generics | Witness tables by default | Enables separate compilation |
| Optimization | Opt-in monomorphization | Preserves performance where needed |

This positions SPL to have:
- **Fast compile times** (Go-like, due to pre-compiled generics)
- **Good runtime performance** (Swift-like, with specialization for hot paths)
- **Simple mental model** (one package = one compilation unit)
- **Pre-compiled standard library** (major build time reduction)

## References

### Project Documentation
- [SPL Module System](./spec/module-system.md)
- [Rust Design Lessons](./rust-design-lessons.md)

### Rust Compilation
- [Rust's Huge Compilation Units (TiDB)](https://www.pingcap.com/blog/rust-huge-compilation-units/)
- [Generics and Compile-Time in Rust (TiDB)](https://www.pingcap.com/blog/generics-and-compile-time-in-rust/)
- [Fast Rust Builds (matklad)](https://matklad.github.io/2021/09/04/fast-rust-builds.html)
- [Monomorphization - Rust Compiler Dev Guide](https://rustc-dev-guide.rust-lang.org/backend/monomorph.html)

### Swift ABI and Generics
- [How Swift Achieved Dynamic Linking (Faultlore)](https://faultlore.com/blah/swift-abi/)
- [Swift ABI Stability Manifesto](https://github.com/apple/swift/blob/main/docs/ABIStabilityManifesto.md)
- [ABI Stability and More (Swift.org)](https://www.swift.org/blog/abi-stability-and-more/)
- [Swift for C++ Practitioners: Generics (Doug Gregor)](https://www.douggregor.net/posts/swift-for-cxx-practitioners-generics/)

### Go Generics
- [Go 1.18 Generics Implementation](https://github.com/golang/proposal/blob/master/design/generics-implementation-dictionaries-go1.18.md)
- [GCShape Stenciling Design](https://github.com/golang/proposal/blob/master/design/generics-implementation-gcshape.md)
- [Generics Can Make Your Go Code Slower (PlanetScale)](https://planetscale.com/blog/generics-can-make-your-go-code-slower)

### Other Languages
- [Models of Generics and Metaprogramming (Tristan Hume)](https://thume.ca/2019/07/14/a-tour-of-metaprogramming-models-for-generics/)
- [OCaml Manual - Module System](https://ocaml.org/manual/5.3/moduleexamples.html)
- [Assorted Thoughts on Zig and Rust](https://www.scattered-thoughts.net/writing/assorted-thoughts-on-zig-and-rust/)

### Graydon Hoare on Rust
- [The Rust I Wanted Had No Future (via Michael Tsai)](https://mjtsai.com/blog/2023/06/08/the-rust-i-wanted-had-no-future/)
- [Graydon Hoare Remembers Early Rust (The New Stack)](https://thenewstack.io/graydon-hoare-remembers-the-early-days-of-rust/)
