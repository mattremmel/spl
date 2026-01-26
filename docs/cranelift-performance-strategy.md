Absolutely. There's a lot of headroom before you need to reach for LLVM.

**1. Focus on the allocator**

This is often the single biggest lever. A naive malloc/free on every operation will dwarf any codegen differences. Consider:

- Bump allocators for request-scoped memory
- Arena allocation for AST nodes, IR, etc.
- Object pools for fixed-size hot types
- Generational patterns if you're building a GC

A mediocre codegen with a great allocator beats great codegen with a bad allocator.

**2. Data layout**

Cache misses cost 100x+ what instructions cost. Structure your data for access patterns:

- Struct-of-arrays vs array-of-structs depending on access patterns
- Keep hot fields together, cold fields separate
- Avoid pointer chasing—prefer indices into dense arrays
- Size your structs to avoid straddling cache lines

**3. Help Cranelift help you**

Cranelift isn't dumb, but it's less aggressive. Write IR that's easier to optimize:

- Avoid unnecessary allocas—promote to SSA values where possible
- Keep loop bodies simple and branch-free when you can
- Use explicit loop preheaders so Cranelift sees the structure
- Prefer known-size operations over dynamic ones

**4. Strategic intrinsics**

For true hot spots, expose architecture-specific operations:

- SIMD for bulk operations (Cranelift has decent vector support)
- Population count, leading zeros, byte swap
- Prefetch hints if you're doing predictable striding

**5. Algorithm selection over micro-optimization**

Cranelift will generate reasonable code. The algorithm choice matters more:

- Robin Hood or Swiss Table hashing vs naive chaining
- Radix sort for integers vs comparison sort
- B-trees over binary trees for cache density

**6. Monomorphization**

Specialize generic code at compile time. A `sort<i64>` that knows its comparison is an integer compare will beat a generic version with indirect calls, regardless of backend.

**7. Profile-guided hot/cold splitting**

Even without PGO, you can manually hint:

- Mark error paths as cold
- Keep fast paths straight-line (no branches)
- Outline rarely-executed setup code

**The honest truth:**

For most code, you'll be within 20-30% of LLVM -O2 with these techniques. The gap widens mainly on:

- Floating point heavy numeric code (vectorization)
- Tiny tight loops where instruction scheduling matters
- Code that benefits from aggressive inlining heuristics

If spl is aimed at general-purpose programming rather than number crunching, you can get very far without LLVM. And you can always add an LLVM backend later for release builds while keeping Cranelift for development—Zig does exactly this.
