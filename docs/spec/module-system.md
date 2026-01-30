# SPL Module System

This document defines the module system for SPL (Simple Programming Language).

## Terminology

| Term | Definition |
|------|------------|
| **Package** | The whole project/compilation unit (like Rust's crate, Go's module) |
| **Module** | A directory of source files within a package (organizational unit) |
| **Source file** | A single `.spl` file; part of a module, not a named unit itself |
| **Item** | A named entity: function, struct, type alias, etc. |

```
Package (project root, also the root module)
└── Module (directory)
    └── Source files (.spl)
        └── Items (fn, struct, etc.)
```

**Note:** The root package directory is also the "root module" - packages and modules share the same structure, with "package" referring to the top-level compilation unit.

---

## Overview

SPL's module system provides:

- **Directory-based modules**: All `.spl` files in a directory form one module (Go-style)
- **Explicit control**: Optional `_module.spl` files for fine-grained structure
- **Unified imports**: `use` declarations for all symbols (internal and external)
- **Visibility**: Public (`pub`), package-private (`pub(package)`), and private (default) items
- **Prelude**: Common types available without explicit import

### Design Philosophy

1. **Simple by default**: All `.spl` files in a directory form one module automatically
2. **Explicit when needed**: `_module.spl` provides control without changing defaults
3. **Unified `use`**: Single keyword for all imports—no distinction between internal and external
4. **Minimal prelude**: Common types auto-imported, keeping the list small and predictable

---

## 1. Module Structure

### Default Mode (No `_module.spl`)

When no `_module.spl` file is present, SPL uses Go-style automatic module formation:

```
myproject/
├── main.spl          # Part of root module
├── utils.spl         # Part of root module
└── network/
    ├── client.spl    # Part of 'network' module
    └── server.spl    # Part of 'network' module
```

**Rules:**
- All `.spl` files in a directory form a single module
- Module name = directory name (root module = package name)
- Items in the same module can reference each other freely
- Subdirectories are submodules automatically
- No declaration needed for submodules

```spl
// In myproject/main.spl
fn main() {
    let helper = utils_function();  // Same module, direct access
    let client = network.Client.new();  // Submodule access
}

// In myproject/utils.spl
fn utils_function(): i32 {
    42
}

// In myproject/network/client.spl
pub struct Client()

impl Client {
    pub fn new(): Client {
        Client()
    }
}
```

### Explicit Mode (With `_module.spl`)

The `_module.spl` file provides explicit control over module structure using compiler directives and re-exports:

```
myproject/
├── _module.spl       # Controls root module
├── main.spl
├── internal.spl
└── network/
    ├── _module.spl   # Controls network module
    └── client.spl
```

**`_module.spl` contents:**

```spl
// Compiler directives (build-time configuration)
#![name("mylib")]              // Override module name

// Re-export items (affects public API)
pub use network.Client;
pub use internal.{Parser, Lexer};

// All .spl files in the same directory are still auto-included by default
```

**Rules:**
- All `.spl` files in a directory are automatically part of the module (unless disabled)
- `_module.spl` uses `#![...]` directives for build-time configuration
- `pub use path.item;` re-exports items to simplify the public API
- **Scope**: `_module.spl` only affects the current directory, not children

### `_module.spl` Directives

Directives use `#![...]` syntax to indicate they are compiler/build-time configuration, not runtime code.

#### Module Name Override

```spl
#![name("httptest")]  // Override module name (default is directory name)
```

#### File Inclusion Control

```spl
// Disable auto-include entirely (must explicitly include all files)
#![no_auto_include]

// Explicitly include a file
#![include("parser.spl")]
#![include("lexer.spl")]

// Exclude a file from auto-include
#![exclude("old_impl.spl")]
#![exclude("experimental.spl")]
```

#### Conditional Inclusion

For platform-specific or feature-gated code:

```spl
// Include only when condition is true
// Note: include_if implicitly excludes the file from auto-include
#![include_if(os = "linux", "fs_linux.spl")]
#![include_if(os = "windows", "fs_windows.spl")]
#![include_if(os = "macos", "fs_macos.spl")]

// Exclude when condition is true
#![exclude_if(feature = "no_std", "std_helpers.spl")]
```

#### Directive Grammar

```ebnf
Directive = "#![" DirectiveBody "]" ;

DirectiveBody = "name" "(" STRING ")"
              | "no_auto_include"
              | "include" "(" STRING ")"
              | "exclude" "(" STRING ")"
              | "include_if" "(" Condition "," STRING ")"
              | "exclude_if" "(" Condition "," STRING ")" ;
```

#### Condition Syntax

```ebnf
Condition = Comparison
          | "not" Condition
          | Condition "and" Condition
          | Condition "or" Condition
          | "(" Condition ")" ;

Comparison = Key "=" Value ;

Key = "os" | "arch" | "feature" ;
Value = STRING ;
```

**Available condition keys:**

| Key | Values |
|-----|--------|
| `os` | `"linux"`, `"windows"`, `"macos"`, `"freebsd"`, etc. |
| `arch` | `"x86_64"`, `"aarch64"`, `"arm"`, `"wasm32"`, etc. |
| `feature` | User-defined feature flags |

**Complex conditions:**

```spl
#![include_if(os = "linux" and arch = "x86_64", "linux_x64_optimized.spl")]
#![include_if(os = "windows" or os = "macos", "desktop_ui.spl")]
#![exclude_if(not feature = "std", "std_io.spl")]
```

### Platform-Specific Files Example

```
filesystem/
├── _module.spl
├── common.spl        # Shared interface and types
├── fs_linux.spl      # Linux implementation
├── fs_windows.spl    # Windows implementation
└── fs_macos.spl      # macOS implementation
```

```spl
// filesystem/_module.spl
#![include_if(os = "linux", "fs_linux.spl")]
#![include_if(os = "windows", "fs_windows.spl")]
#![include_if(os = "macos", "fs_macos.spl")]

// Re-export the common API
pub use self.{read_file, write_file, FileHandle};
```

Each platform file implements the same interface:

```spl
// fs_linux.spl
pub fn read_file(path: &str): Result(T: Vec(T: u8), E: IoError) {
    // Linux-specific implementation using syscalls
}

pub fn write_file(path: &str, data: &[u8]): Result(T: (), E: IoError) {
    // Linux-specific implementation
}

pub struct FileHandle(fd: i32)
```

### Configuration Inheritance

`_module.spl` does **not** propagate to subdirectories. Each directory independently manages its configuration:

```
myproject/
├── _module.spl       # Configuration for root module
├── main.spl
├── utils.spl
└── network/          # No _module.spl here
    ├── client.spl    # Auto-included (default behavior)
    └── server.spl    # Auto-included (default behavior)
```

In this example:
- Root has `_module.spl` with custom configuration
- `network/` uses default behavior (no `_module.spl`)
- All files in `network/` are automatically part of the `network` module

### Submodule Discovery

Subdirectories are automatically discovered as submodules. No explicit declaration is needed:

```
myproject/
├── main.spl
├── utils/
│   └── helpers.spl   # Automatically part of 'utils' module
└── network/
    └── client.spl    # Automatically part of 'network' module
```

```spl
// In main.spl - submodules are accessible without declaration
fn main() {
    let helper = utils.some_function();
    let client = network.Client.new();
}
```

---

## 2. Import Syntax

### Use Declarations

```ebnf
UseDecl = "use" UsePath ";" ;

UsePath = PathPrefix [ "." UseTree ] ;

PathPrefix = IDENTIFIER { "." IDENTIFIER } ;

UseTree = "*"                                    (* glob import *)
        | "{" UseTreeList "}"                    (* grouped import *)
        | IDENTIFIER [ "as" IDENTIFIER ] ;       (* item or rename *)

UseTreeList = UseTree { "," UseTree } [ "," ] ;
```

### Path Keywords

| Keyword | Meaning |
|---------|---------|
| `$` | Root of current package (like Rust's `crate`) |
| `self` | Current module (directory) |
| `super` | Parent module |

**Note:** These keywords are followed by `.` (the path separator) when accessing items. For example, `$.utils.helper` is the token `$`, then `.`, then `utils`, then `.`, then `helper`. The `.` is always the path separator—there is no `$.` or `self.` token.

### Examples

The `use` keyword works uniformly for internal modules and external packages:

```spl
// From current package root (using $ prefix)
use $.utils.helper;           // Package root → utils module → helper
use super.common.Config;      // Parent module
use self.internal.parse;      // Current module

// External packages (same syntax)
use std.vec.Vec;              // Standard library
use std.collections.HashMap;  // Standard library
use serde.Serialize;          // External package (future)

// Import with rename
use std.collections.HashMap as Map;

// Import module (use qualified access)
use std.io;
let reader = io.BufReader.new(file);

// Grouped imports
use std.collections.{HashMap, HashSet};

// Glob import (use sparingly)
use std.prelude.*;

// Nested groups
use std.{vec.Vec, io.{Read, Write}};

// Full qualified path (no import needed)
let v = std.vec.Vec(T: i32).new();
```

---

## 3. Visibility Rules

### Basic Visibility

| Modifier | Visibility |
|----------|------------|
| (none) | Private to current module |
| `pub` | Public to all |
| `pub(package)` | Visible within current package only |

```spl
// Private function (only accessible within this module)
fn internal_helper(): i32 {
    42
}

// Package-private function (accessible within this package, not external)
pub(package) fn package_internal(): i32 {
    internal_helper()
}

// Public function (accessible from anywhere, including external packages)
pub fn public_api(): i32 {
    package_internal()
}

// Public struct with mixed field visibility
pub struct Config(
    pub name: String,          // Public field
    pub(package) id: u64,      // Package-private field
    secret_key: String,        // Private field (module only)
)
```

### Privacy and Modules

- Private items are visible within their module and all submodules
- Child modules can access parent's private items
- Sibling modules cannot access each other's private items
- `pub(package)` items are visible anywhere within the same package

```spl
// In parent/helpers.spl
fn private_helper() { }
pub(package) fn package_fn() { }
pub fn public_fn() { }

// In parent/child/impl.spl (child is a subdirectory)
fn child_fn() {
    super.private_helper();  // OK: child can access parent's private
    super.package_fn();      // OK: same package
    super.public_fn();       // OK
}

// In a different package
fn external_fn() {
    other_pkg.private_helper();  // ERROR: private
    other_pkg.package_fn();      // ERROR: pub(package) not visible
    other_pkg.public_fn();       // OK: pub is visible
}
```

### Future Extensions (v2)

Additional visibility modifiers for finer control:

| Modifier | Visibility |
|----------|------------|
| `pub(super)` | Visible to parent module only |
| `pub(in path)` | Visible to specific module |

```spl
// Future syntax
pub(super) fn parent_accessible() { }
pub(in $.api) fn api_only() { }
```

---

## 4. Path Resolution

### Path Types

| Path | Description | Example |
|------|-------------|---------|
| Absolute | From package root | `$.utils.item` |
| Relative | From current module | `submod.item` |
| Self | Current module | `self.item` |
| Super | Parent module | `super.item` |
| External | External package | `std.item` |

### Resolution Rules

1. **Unqualified names** resolve in order:
   - Local scope (variables, parameters)
   - Items in current module
   - Prelude items

2. **Qualified paths** resolve:
   - `$` from the package root
   - `self.` from the current module
   - `super.` from the parent module
   - Other identifiers from current scope or imports

```spl
use $.utils;                  // Absolute: from package root
use self.internal;            // Explicit current module
use super.common;             // Parent module
use std.collections.HashMap;  // External package

fn example() {
    // Relative path
    let x = submod.function();

    // Absolute path
    let y = $.other.function();

    // Super path
    let z = super.sibling();
}
```

### Name Shadowing

Imports can shadow prelude items:

```spl
// Option and Result are in prelude, but can be shadowed
use custom_types.Option;  // Now 'Option' refers to custom_types.Option

// Original still accessible via full path
let x: i32? = Some(42);
```

---

## 5. Prelude

The prelude contains commonly used items automatically available without import.

### Standard Prelude Contents

```spl
// Types
Option, Some, None      // Optional values
Result, Ok, Err         // Error handling
Vec                     // Dynamic array
String                  // Owned string
decimal                 // Exact decimal type

// Functions
print                   // Print without newline
println                 // Print with newline

// Future additions
// Clone, Copy, Drop    // Marker traits
// Default              // Default values
// Try                   // Error propagation trait
```

### Using the Prelude

```spl
// No imports needed for prelude items
fn example(): i32? {
    let v: Vec(T: i32) = Vec.new();
    let s: String = String.from("hello");

    if v.is_empty() {
        None
    } else {
        Some(v[0])
    }
}
```

### Custom Prelude (Future)

```spl
// In _module.spl (future syntax)
#![prelude(minimal)]      // Use minimal prelude
#![prelude(none)]         // No prelude
```

---

## 6. Inline Modules (Future)

SPL will support inline module declarations using the `module` keyword:

```spl
// Inline module for namespacing
module internal {
    fn private_impl() { ... }

    pub fn helper() {
        private_impl()
    }
}

// Usage
fn main() {
    internal.helper();
}
```

**Note:** This feature is planned for a future release. See the module system roadmap for details.

---

## 7. Keywords

The module system uses these reserved keywords:

| Keyword | Description |
|---------|-------------|
| `use` | Import declaration |
| `module` | Inline module declaration (future) |
| `super` | Parent module reference |
| `$` | Package root reference (operator) |

Note: `self` and `pub` are already keywords for other purposes.

---

## 8. Item Grammar

Items that can appear at module level:

```ebnf
Item = [ Visibility ] ( FunctionDef | StructDef | ImplBlock | TypeAlias | UseDecl | ModuleDecl ) ;

Visibility = "pub" [ "(" VisibilityScope ")" ] ;

VisibilityScope = "package" | "super" | "in" Path ;
```

All item types can appear in any `.spl` file, including `_module.spl`.

In `_module.spl` files, directives (`#![...]`) can also appear at the top of the file before any items.

---

## 9. Examples

### Simple Project Structure

```
calculator/
├── main.spl
├── math.spl
└── display.spl
```

```spl
// main.spl
fn main() {
    let result = add(2, 3);
    show(result);
}

// math.spl
pub fn add(a: i32, b: i32): i32 {
    a + b
}

pub fn multiply(a: i32, b: i32): i32 {
    a * b
}

// display.spl
pub fn show(value: i32) {
    println(value);
}
```

### Structured Project with Submodules

```
webapp/
├── _module.spl
├── main.spl
├── handlers/
│   ├── auth.spl
│   └── api.spl
└── models/
    ├── _module.spl
    ├── user.spl
    └── session.spl
```

```spl
// _module.spl (root)
// Re-export commonly used items for convenience
pub use models.{User, Session};

// main.spl
use $.handlers.auth;
use $.models.User;

fn main() {
    let user = User.new("alice");
    auth.login(&user);
}

// handlers/auth.spl
use $.models.User;

pub fn login(user: &User) {
    // ...
}

// models/_module.spl
// Re-export public API from this module
pub use self.user.User;
pub use self.session.Session;

// models/user.spl
pub struct User(
    pub name: String,
)

impl User {
    pub fn new(name: &str): User {
        User(name: String.from(name))
    }
}
```

### Re-exports for API Simplification

```spl
// In lib/_module.spl

// Re-export public API from submodules
pub use internal.Parser;
pub use internal.Lexer;
pub use details.Config;

// Users can now write:
// use mylib.{Parser, Lexer, Config};
// instead of:
// use mylib.internal.Parser;
// use mylib.internal.Lexer;
// use mylib.details.Config;
```

---

## 10. Comparison with Other Languages

| Feature | SPL | Rust | Go | Python |
|---------|-----|------|----|----|
| Directory unit | Module | Module | Package | Package |
| Project unit | Package | Crate | Module | Distribution |
| Import syntax | `use path.item` | `use path::item` | `import "path"` | `from x import y` |
| Package root | `$` | `crate` | N/A | N/A |
| Glob import | `use path.*` | `use path::*` | `.` import | `from x import *` |
| Visibility default | Private | Private | Capitalization | Public |
| Package-private | `pub(package)` | `pub(crate)` | N/A | `_prefix` |
| Re-exports | `pub use` | `pub use` | No | `__all__` |
| Prelude | Yes | Yes | No | builtins |

---

## 11. Summary

### Key Concepts

| Concept | Description |
|---------|-------------|
| Package | The whole project/compilation unit |
| Module | A directory of source files within a package |
| Item | Named entity: function, struct, type, etc. |
| Path | Route to an item: `$.mod.item` |
| Visibility | Who can access an item: `pub`, `pub(package)`, or private |

### Quick Reference

```spl
// Import single item
use std.vec.Vec;

// Import with rename
use std.collections.HashMap as Map;

// Grouped import
use std.{vec.Vec, io.{Read, Write}};

// Glob import
use std.prelude.*;

// From package root
use $.utils.item;

// Parent module
use super.item;

// Current module
use self.item;

// Re-export (in _module.spl)
pub use internal.PublicApi;
pub use submod.{TypeA, TypeB};
```

### `_module.spl` Directives Reference

```spl
// Override module name
#![name("custom_name")]

// Disable automatic file inclusion
#![no_auto_include]

// Explicitly include files
#![include("file.spl")]

// Exclude files from auto-include
#![exclude("old_impl.spl")]

// Conditional inclusion (implicitly excludes from auto-include)
#![include_if(os = "linux", "linux_impl.spl")]
#![include_if(os = "windows", "windows_impl.spl")]
#![include_if(arch = "aarch64", "arm64_optimized.spl")]
#![include_if(feature = "async", "async_support.spl")]

// Conditional exclusion
#![exclude_if(feature = "minimal", "extras.spl")]

// Complex conditions
#![include_if(os = "linux" and arch = "x86_64", "linux_x64.spl")]
#![include_if(os = "macos" or os = "ios", "apple.spl")]
```

### Design Rationale

1. **Go-style modules**: Directories are modules. All files in a directory share a namespace. Simple and intuitive.

2. **Directive-based configuration**: `_module.spl` uses `#![...]` syntax to clearly distinguish build-time configuration from runtime code. No special file naming conventions needed.

3. **Fine-grained control**: `include_if` and `exclude_if` enable platform-specific builds without cluttering the codebase with conditional compilation in every file.

4. **Unified `use` keyword**: Single keyword for all imports—internal modules and external packages use the same syntax.

5. **Clear terminology**: Package = compilation unit, Module = directory. Aligned with Rust's terminology.

6. **`$` for package root**: Short, visually distinct, familiar from TypeScript ecosystem path aliases.
