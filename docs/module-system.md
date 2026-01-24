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

The `_package.spl` file provides explicit control over package structure:

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
// Declare subpackages (optional, for re-exports or explicit ordering)
mod network;        // Looks for network/_package.spl or network.spl

// Re-export items
pub use network.Client;

// All .spl files in the same directory are still auto-included
```

**Rules:**
- All `.spl` files in a directory are automatically part of the package
- `_package.spl` provides re-exports and explicit ordering, not file inclusion
- `pub mod name;` declares and re-exports a subpackage publicly
- `pub use path.item;` re-exports items
- **Scope**: `_package.spl` only affects the current directory, not children

### Mode Inheritance

`_package.spl` does **not** propagate to subdirectories. Each directory independently chooses its mode:

```
myproject/
├── _package.spl      # Explicit mode for root
├── main.spl
├── utils.spl
└── network/          # No _package.spl here
    ├── client.spl    # Auto-included (default mode)
    └── server.spl    # Auto-included (default mode)
```

In this example:
- Root uses explicit mode (has `_package.spl`)
- `network/` uses default mode (no `_package.spl`)
- All files in `network/` are automatically part of the `network` package

### Package Resolution

For `mod name;` declarations, SPL looks for:

1. `name.spl` in the same directory
2. `name/_package.spl` (directory with package file)
3. `name/` directory with `.spl` files (default mode)

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

### Package Declarations

```ebnf
ModDecl = [ "pub" ] "mod" IDENTIFIER ";" ;
```

Package declarations are only valid in `_package.spl` files:

```spl
// In _package.spl
mod network;          // Private subpackage
pub mod utils;        // Public subpackage
pub mod api;          // Public subpackage
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
// In parent/_package.spl
fn private_helper() { }
pub fn public_fn() { }

mod child;

// In parent/child.spl
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
| `mod` | Package declaration |
| `module` | Module root reference |
| `super` | Parent package reference |

Note: `self` and `pub` are already keywords for other purposes.

---

## 7. Item Grammar

Items that can appear at package level:

```ebnf
Item = [ "pub" ] ( FunctionDef | StructDef | ImplBlock | TypeAlias | UseDecl | ModDecl ) ;
```

In regular `.spl` files:
- `FunctionDef`, `StructDef`, `ImplBlock`, `TypeAlias`, `UseDecl`

In `_package.spl` files only:
- All of the above, plus `ModDecl`

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
pub mod handlers;
pub mod models;

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
mod user;
mod session;

pub use user.User;
pub use session.Session;

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
mod internal;
mod details;

// Re-export public API
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

// Package declaration (in _package.spl only)
mod subpackage;
pub mod public_subpackage;

// Re-export
pub use internal.PublicApi;
```

### Design Rationale

1. **Go-style packages**: Directories are packages. All files in a directory share a namespace. Simple and intuitive.

2. **Explicit control when needed**: `_package.spl` provides re-exports and explicit structure without changing the default auto-include behavior.

3. **Unified `use` keyword**: Single keyword for all imports—internal packages and external modules use the same syntax.

4. **Clear terminology**: Module = project, Package = directory. Aligned with Go's terminology.
