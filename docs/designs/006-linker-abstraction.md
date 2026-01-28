# ADR-006: Linker Abstraction

**Status:** Accepted
**Date:** 2026-01-28

## Context

AOT compilation produces object files that must be linked into executables. The linking step needs to:
- Work across platforms (macOS, Linux, Windows)
- Support different linkers (system `cc`, `lld`, `mold`)
- Allow customization (libraries, search paths)
- Handle errors gracefully

Hardcoding a specific linker limits portability. Directly spawning `cc` without abstraction makes testing difficult.

## Decision

Use a trait-based linker abstraction with a default `cc`-based implementation:

```rust
pub trait Linker {
    fn link(
        &self,
        objects: &[&Path],
        output: &Path,
        options: &LinkOptions,
    ) -> Result<(), LinkError>;
}

pub struct CcLinker { /* ... */ }
impl Linker for CcLinker { /* ... */ }
```

### Link Options

```rust
pub struct LinkOptions {
    pub libraries: Vec<String>,      // -l flags
    pub library_paths: Vec<PathBuf>, // -L flags
    pub linker_flavor: Option<LinkerFlavor>, // -fuse-ld=<linker>
    pub extra_args: Vec<String>,     // pass-through
}

pub enum LinkerFlavor {
    Ld,      // System ld
    Lld,     // LLVM's lld
    Mold,    // Fast mold linker
    Gold,    // GNU gold
    Custom(String), // Custom linker path
}
```

## Rationale

### Why Trait Abstraction?
- **Testability**: Can mock the linker in tests
- **Extensibility**: Easy to add `LldLinker`, `MoldLinker`, etc.
- **Portability**: Linker selection per platform
- **Configuration**: Different linkers for debug vs release

### Why Default to `cc`?
- Available on all Unix-like systems
- Respects `CC` environment variable
- Handles platform-specific flags automatically
- Users can override via environment
- Supports `-fuse-ld=<linker>` for linker selection (like Rust does)

### Why Not Direct `ld`?
- `ld` requires platform-specific flags (CRT paths, etc.)
- `cc` knows how to invoke `ld` correctly
- More portable across systems

### Why Options Struct?
- Builder pattern for ergonomic API
- Clear separation of concerns
- Easy to add new options without breaking API

## Consequences

### Positive
- Portable across platforms
- Easy to test (mock linker)
- Extensible to new linkers
- Clean API for library users

### Negative
- Depends on system having a C compiler
- Extra process spawn overhead
- Limited control over exact linker flags

## Implementation

- **Linker trait**: `spl-codegen/src/link.rs`
- **CcLinker**: `spl-codegen/src/link.rs`
- **LinkOptions**: `spl-codegen/src/link.rs`
- **Helper function**: `link_object_to_executable()`

### Usage Examples

```rust
// Simple case: use defaults
compile_and_link(source, Path::new("output"))?;

// With options
let options = LinkOptions::new()
    .library("m")           // Link libm
    .library("pthread")     // Link pthread
    .library_path("/usr/local/lib");

compile_and_link_with_options(source, output, &options)?;

// With mold linker (like Rust's -C link-arg=-fuse-ld=mold)
let options = LinkOptions::new()
    .linker_flavor(LinkerFlavor::Mold);

compile_and_link_with_options(source, output, &options)?;
```

### Error Handling

```rust
pub enum LinkError {
    WriteObjectFile(io::Error),  // Failed to write temp .o
    SpawnLinker(io::Error),      // Failed to run linker
    LinkerFailed { status, stdout, stderr },  // Non-zero exit
    ReadBinary(io::Error),       // Failed to read output
    Io(io::Error),               // Generic IO error
}
```

### Environment Variables

| Variable | Effect |
|----------|--------|
| `CC` | Overrides compiler/linker command |
| `SPL_LINKER` | Linker flavor override (ld, lld, mold, gold, or path) |
| `CFLAGS` | Not directly used (pass via `extra_args`) |

### Project Configuration

Like Rust's `.cargo/config.toml`, SPL can support project-level linker configuration:

```toml
# spl.toml or .spl/config.toml
[target.'cfg(target_os = "linux")']
linker-flavor = "mold"

[target.'cfg(target_os = "macos")']
linker-flavor = "ld"  # macOS uses system ld
```

Priority order:
1. Explicit `LinkOptions` in code
2. `SPL_LINKER` environment variable
3. Project config file
4. Default (`cc` with system linker)

## References

- [GCC Linking](https://gcc.gnu.org/onlinedocs/gcc/Link-Options.html)
- [mold linker](https://github.com/rui314/mold) - Fast alternative linker
- [lld](https://lld.llvm.org/) - LLVM's linker
- [Rust linker configuration](https://doc.rust-lang.org/cargo/reference/config.html#targettriplelinker) - How Rust handles linker selection
