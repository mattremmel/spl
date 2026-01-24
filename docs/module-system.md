# SPL Module System

This document defines the module system for SPL (Simple Programming Language).

## Terminology

| Term | Definition |
|------|------------|
| **Module** | The whole project/compilation unit (like Go's module, Rust's crate) |
| **Package** | A directory of source files (like Go's package) |
| **Source file** | A single `.spl` file; part of a package, not a named unit itself |
| **Item** | A named entity: function, struct, type alias, etc. |

```
Module (project root)
└── Package (directory)
    └── Source files (.spl)
        └── Items (fn, struct, etc.)
```

---

## Overview

SPL's module system provides:

- **Directory-based packages**: All `.spl` files in a directory form one package (Go-style)
- **Explicit control**: Optional `_package.spl` files for fine-grained structure
- **Unified imports**: `use` declarations for all symbols (internal and external)
- **Visibility**: Public (`pub`) and private (default) items
- **Prelude**: Common types available without explicit import

### Design Philosophy

1. **Simple by default**: All `.spl` files in a directory form one package automatically
2. **Explicit when needed**: `_package.spl` provides control without changing defaults
3. **Unified `use`**: Single keyword for all imports—no distinction between internal and external
4. **Minimal prelude**: Common types auto-imported, keeping the list small and predictable

---

## 1. Package Structure

### Default Mode (No `_package.spl`)

When no `_package.spl` file is present, SPL uses Go-style automatic package formation:

```
myproject/
├── main.spl          # Part of root package
├── utils.spl         # Part of root package
└── network/
    ├── client.spl    # Part of 'network' package
    └── server.spl    # Part of 'network' package
```

**Rules:**
- All `.spl` files in a directory form a single package
- Package name = directory name (root package = module name)
- Items in the same package can reference each other freely
- Subdirectories are subpackages automatically
- No declaration needed for subpackages

```spl
// In myproject/main.spl
fn main() {
    let helper = utils_function();  // Same package, direct access
    let client = network.Client.new();  // Subpackage access
}

// In myproject/utils.spl
fn utils_function(): i32 {
    42
}

// In myproject/network/client.spl
pub struct Client { }

impl Client {
    pub fn new(): Client {
        Client { }
    }
}
```

### Explicit Mode (With `_package.spl`)

The `_package.spl` file provides explicit control over package structure using compiler directives and re-exports:

```
myproject/
├── _package.spl      # Controls root package
├── main.spl
├── internal.spl
└── network/
    ├── _package.spl  # Controls network package
    └── client.spl
```

**`_package.spl` contents:**

```spl
// Compiler directives (build-time configuration)
#![name("mylib")]              // Override package name

// Re-export items (affects public API)
pub use network.Client;
pub use internal.{Parser, Lexer};

// All .spl files in the same directory are still auto-included by default
```

**Rules:**
- All `.spl` files in a directory are automatically part of the package (unless disabled)
- `_package.spl` uses `#![...]` directives for build-time configuration
- `pub use path.item;` re-exports items to simplify the public API
- **Scope**: `_package.spl` only affects the current directory, not children

### `_package.spl` Directives

Directives use `#![...]` syntax to indicate they are compiler/build-time configuration, not runtime code.

#### Package Name Override

```spl
#![name("httptest")]  // Override package name (default is directory name)
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
├── _package.spl
├── common.spl        # Shared interface and types
├── fs_linux.spl      # Linux implementation
├── fs_windows.spl    # Windows implementation
└── fs_macos.spl      # macOS implementation
```

```spl
// filesystem/_package.spl
#![include_if(os = "linux", "fs_linux.spl")]
#![include_if(os = "windows", "fs_windows.spl")]
#![include_if(os = "macos", "fs_macos.spl")]

// Re-export the common API
pub use self.{read_file, write_file, FileHandle};
```

Each platform file implements the same interface:

```spl
// fs_linux.spl
pub fn read_file(path: &str): Result(Vec(u8), IoError) {
    // Linux-specific implementation using syscalls
}

pub fn write_file(path: &str, data: &[u8]): Result((), IoError) {
    // Linux-specific implementation
}

pub struct FileHandle { fd: i32 }
```

### Configuration Inheritance

`_package.spl` does **not** propagate to subdirectories. Each directory independently manages its configuration:

```
myproject/
├── _package.spl      # Configuration for root package
├── main.spl
├── utils.spl
└── network/          # No _package.spl here
    ├── client.spl    # Auto-included (default behavior)
    └── server.spl    # Auto-included (default behavior)
```

In this example:
- Root has `_package.spl` with custom configuration
- `network/` uses default behavior (no `_package.spl`)
- All files in `network/` are automatically part of the `network` package

### Subpackage Discovery

Subdirectories are automatically discovered as subpackages. No explicit declaration is needed:

```
myproject/
├── main.spl
├── utils/
│   └── helpers.spl   # Automatically part of 'utils' package
└── network/
    └── client.spl    # Automatically part of 'network' package
```

```spl
// In main.spl - subpackages are accessible without declaration
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
| `module.` | Root of current module (project) |
| `self.` | Current package (directory) |
| `super.` | Parent package |

### Examples

The `use` keyword works uniformly for internal packages and external modules:

```spl
// From current module (project)
use module.utils.helper;      // Module root → utils package → helper
use super.common.Config;      // Parent package
use self.internal.parse;      // Current package

// External modules (same syntax)
use std.vec.Vec;              // Standard library
use std.collections.HashMap;  // Standard library
use serde.Serialize;          // External module (future)

// Import with rename
use std.collections.HashMap as Map;

// Import package (use qualified access)
use std.io;
let reader = io.BufReader.new(file);

// Grouped imports
use std.collections.{HashMap, HashSet};

// Glob import (use sparingly)
use std.prelude.*;

// Nested groups
use std.{vec.Vec, io.{Read, Write}};

// Full qualified path (no import needed)
let v = std.vec.Vec(i32).new();
```

---

## 3. Visibility Rules

### Basic Visibility

| Modifier | Visibility |
|----------|------------|
| (none) | Private to current package |
| `pub` | Public to all |

```spl
// Private function (only accessible within this package)
fn internal_helper(): i32 {
    42
}

// Public function (accessible from anywhere)
pub fn public_api(): i32 {
    internal_helper()
}

// Public struct with mixed field visibility
pub struct Config {
    pub name: String,      // Public field
    secret_key: String,    // Private field
}
```

### Privacy and Packages

- Private items are visible within their package and all subpackages
- Child packages can access parent's private items
- Sibling packages cannot access each other's private items

```spl
// In parent/helpers.spl
fn private_helper() { }
pub fn public_fn() { }

// In parent/child/impl.spl (child is a subdirectory)
fn child_fn() {
    super.private_helper();  // OK: child can access parent's private
    super.public_fn();       // OK
}
```

### Future Extensions (v2)

Planned visibility modifiers for finer control:

| Modifier | Visibility |
|----------|------------|
| `pub(module)` | Visible within module only |
| `pub(super)` | Visible to parent package |
| `pub(in path)` | Visible to specific package |

```spl
// Future syntax
pub(module) fn module_internal() { }
pub(super) fn parent_accessible() { }
pub(in module.api) fn api_only() { }
```

---

## 4. Path Resolution

### Path Types

| Path | Description | Example |
|------|-------------|---------|
| Absolute | From module root | `module.utils.item` |
| Relative | From current package | `subpkg.item` |
| Self | Current package | `self.item` |
| Super | Parent package | `super.item` |
| External | External module | `std.item` |

### Resolution Rules

1. **Unqualified names** resolve in order:
   - Local scope (variables, parameters)
   - Items in current package
   - Prelude items

2. **Qualified paths** resolve:
   - `module.` from the module root
   - `self.` from the current package
   - `super.` from the parent package
   - Other identifiers from current scope or imports

```spl
use module.utils;             // Absolute: from module root
use self.internal;            // Explicit current package
use super.common;             // Parent package
use std.collections.HashMap;  // External module

fn example() {
    // Relative path
    let x = subpkg.function();

    // Absolute path
    let y = module.other.function();

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
let x: std.option.Option(i32) = Some(42);
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

// Functions
print                   // Print without newline
println                 // Print with newline

// Future additions
// Clone, Copy, Drop    // Marker traits
// Default              // Default values
// Iterator             // Iteration trait
```

### Using the Prelude

```spl
// No imports needed for prelude items
fn example(): Option(i32) {
    let v: Vec(i32) = Vec.new();
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
// In _package.spl (future syntax)
#![prelude(minimal)]      // Use minimal prelude
#![prelude(none)]         // No prelude
```

---

## 6. Keywords

The module system adds these reserved keywords:

| Keyword | Description |
|---------|-------------|
| `use` | Import declaration |
| `module` | Module root reference |
| `super` | Parent package reference |

Note: `self` and `pub` are already keywords for other purposes.

---

## 7. Item Grammar

Items that can appear at package level:

```ebnf
Item = [ "pub" ] ( FunctionDef | StructDef | ImplBlock | TypeAlias | UseDecl ) ;
```

All item types can appear in any `.spl` file, including `_package.spl`.

In `_package.spl` files, directives (`#![...]`) can also appear at the top of the file before any items.

---

## 8. Examples

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

### Structured Project with Subpackages

```
webapp/
├── _package.spl
├── main.spl
├── handlers/
│   ├── auth.spl
│   └── api.spl
└── models/
    ├── _package.spl
    ├── user.spl
    └── session.spl
```

```spl
// _package.spl (root)
// Re-export commonly used items for convenience
pub use models.{User, Session};

// main.spl
use module.handlers.auth;
use module.models.User;

fn main() {
    let user = User.new("alice");
    auth.login(&user);
}

// handlers/auth.spl
use module.models.User;

pub fn login(user: &User) {
    // ...
}

// models/_package.spl
// Re-export public API from this package
pub use self.user.User;
pub use self.session.Session;

// models/user.spl
pub struct User {
    pub name: String,
}

impl User {
    pub fn new(name: &str): User {
        User { name: String.from(name) }
    }
}
```

### Re-exports for API Simplification

```spl
// In lib/_package.spl

// Re-export public API from subpackages
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

## 9. Comparison with Other Languages

| Feature | SPL | Rust | Go | Python |
|---------|-----|------|----|----|
| Directory unit | Package | Module | Package | Package |
| Project unit | Module | Crate | Module | Distribution |
| Import syntax | `use path.item` | `use path::item` | `import "path"` | `from x import y` |
| Glob import | `use path.*` | `use path::*` | `.` import | `from x import *` |
| Visibility default | Private | Private | Capitalization | Public |
| Re-exports | `pub use` | `pub use` | No | `__all__` |
| Prelude | Yes | Yes | No | builtins |

---

## 10. Summary

### Key Concepts

| Concept | Description |
|---------|-------------|
| Module | The whole project/compilation unit |
| Package | A directory of source files |
| Item | Named entity: function, struct, type, etc. |
| Path | Route to an item: `module.pkg.item` |
| Visibility | Who can access an item: `pub` or private |

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

// From module root
use module.utils.item;

// Parent package
use super.item;

// Current package
use self.item;

// Re-export (in _package.spl)
pub use internal.PublicApi;
pub use subpkg.{TypeA, TypeB};
```

### `_package.spl` Directives Reference

```spl
// Override package name
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

1. **Go-style packages**: Directories are packages. All files in a directory share a namespace. Simple and intuitive.

2. **Directive-based configuration**: `_package.spl` uses `#![...]` syntax to clearly distinguish build-time configuration from runtime code. No special file naming conventions needed.

3. **Fine-grained control**: `include_if` and `exclude_if` enable platform-specific builds without cluttering the codebase with conditional compilation in every file.

4. **Unified `use` keyword**: Single keyword for all imports—internal packages and external modules use the same syntax.

5. **Clear terminology**: Module = project, Package = directory. Aligned with Go's terminology.
