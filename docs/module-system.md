# SPL Module System

This document defines the module system for SPL (Simple Programming Language). SPL uses a hybrid approach inspired by Go's simplicity and Rust's explicit control.

## Overview

SPL's module system provides:

- **File-to-module mapping**: Multiple files can form a single module (Go-style)
- **Explicit control**: Optional `_module.spl` files for fine-grained module structure (Rust-style)
- **Import syntax**: Rust-style `use` declarations for importing items
- **Visibility**: Public (`pub`) and private (default) items
- **Prelude**: Common types available without explicit import

### Design Philosophy

1. **Simple by default**: All `.spl` files in a directory form one module automatically
2. **Explicit when needed**: `_module.spl` provides control over module structure
3. **Familiar syntax**: Rust-like imports are readable and well-established
4. **No boilerplate**: Prelude eliminates repetitive imports

---

## 1. Module Structure

### Default Mode (No `_module.spl`)

When no `_module.spl` file is present, SPL uses Go-style automatic module formation:

```
myproject/
├── main.spl          # Entry point, part of root module
├── utils.spl         # Part of root module
└── network/
    ├── client.spl    # Part of 'network' module
    └── server.spl    # Part of 'network' module
```

**Rules:**
- All `.spl` files in a directory form a single module
- Module name = directory name (root module = crate name)
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
fn utils_function() -> i32 {
    42
}

// In myproject/network/client.spl
pub struct Client { }

impl Client {
    pub fn new() -> Client {
        Client { }
    }
}
```

### Explicit Mode (With `_module.spl`)

The `_module.spl` file provides explicit control over module structure:

```
myproject/
├── _module.spl       # Declares module structure
├── main.spl
├── internal.spl      # Not part of module unless declared
└── network/
    ├── _module.spl   # Controls network module
    └── client.spl
```

**`_module.spl` contents:**

```spl
// Declare submodules (optional, for re-exports or explicit ordering)
mod network;        // Looks for network/_module.spl or network.spl

// Re-export items
pub use network.Client;

// All .spl files in the same directory are auto-included
```

**Rules:**
- All `.spl` files in a directory are automatically part of the module
- `_module.spl` provides re-exports and explicit ordering, not file inclusion
- `pub mod name;` declares and exports a submodule publicly
- `pub use path.item;` re-exports items
- **Scope**: `_module.spl` only affects the current directory, not children

### Mode Inheritance

`_module.spl` does **not** propagate to subdirectories. Each directory independently chooses its mode:

```
myproject/
├── _module.spl       # Explicit mode for root
├── main.spl
├── utils.spl         # Must be declared in _module.spl
└── network/          # No _module.spl here
    ├── client.spl    # Auto-included (default mode)
    └── server.spl    # Auto-included (default mode)
```

In this example:
- Root uses explicit mode (has `_module.spl`)
- `network/` uses default mode (no `_module.spl`)
- All files in `network/` are automatically part of the `network` module

This prevents the "cascade problem" where adding `_module.spl` to one directory forces you to add it to all children.

### Module Resolution

For `mod name;` declarations, SPL looks for:

1. `name.spl` in the same directory
2. `name/_module.spl` (directory with module file)
3. `name/` directory with `.spl` files (default mode for submodule)

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

### Import Examples

```spl
// Import single item
use std.vec.Vec;

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
let v = std.vec.Vec(i32).new();

// Module root reference
use module.utils.helper;

// Parent module reference
use super.common.Config;

// Current module (explicit)
use self.internal.parse;
```

### Module Declarations

```ebnf
ModDecl = [ "pub" ] "mod" IDENTIFIER ";" ;
```

Module declarations are only valid in `_module.spl` files:

```spl
// In _module.spl
mod network;          // Private submodule
pub mod utils;        // Public submodule
pub mod api;          // Public submodule
```

---

## 3. Visibility Rules

### Basic Visibility

| Modifier | Visibility |
|----------|------------|
| (none) | Private to current module |
| `pub` | Public to all |

```spl
// Private function (only accessible within this module)
fn internal_helper() -> i32 {
    42
}

// Public function (accessible from anywhere)
pub fn public_api() -> i32 {
    internal_helper()
}

// Public struct with mixed field visibility
pub struct Config {
    pub name: String,      // Public field
    secret_key: String,    // Private field
}
```

### Privacy and Modules

- Private items are visible within their module and all submodules
- Child modules can access parent's private items
- Sibling modules cannot access each other's private items

```spl
// In parent/_module.spl
fn private_helper() { }
pub fn public_fn() { }

mod child;

// In parent/child.spl
fn child_fn() {
    super::private_helper();  // OK: child can access parent's private
    super::public_fn();       // OK
}
```

### Future Extensions (v2)

Planned visibility modifiers for finer control:

| Modifier | Visibility |
|----------|------------|
| `pub(crate)` | Visible within crate only |
| `pub(super)` | Visible to parent module |
| `pub(in path)` | Visible to specific module |

```spl
// Future syntax
pub(crate) fn crate_internal() { }
pub(super) fn parent_accessible() { }
pub(in module.api) fn api_only() { }
```

---

## 4. Path Resolution

### Path Types

| Path | Description | Example |
|------|-------------|---------|
| Absolute | From module root | `module.submod.item` |
| Relative | From current module | `submodule.item` |
| Self | Current module | `self.item` |
| Super | Parent module | `super.item` |
| External | External crate | `cratename.item` |

### Resolution Rules

1. **Unqualified names** resolve in order:
   - Local scope (variables, parameters)
   - Items in current module
   - Prelude items

2. **Qualified paths** resolve:
   - `module.` from the module root
   - `self.` from the current module
   - `super.` from the parent module
   - Other identifiers from current scope or imports

```spl
use module.utils;           // Absolute: from module root
use self.internal;          // Explicit current module
use super.common;           // Parent module
use std.collections.HashMap;  // External crate (future)

fn example() {
    // Relative path
    let x = submodule.function();

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
fn example() -> Option<i32> {
    let v: Vec<i32> = Vec::new();
    let s: String = String::from("hello");

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

## 6. Keywords

The module system adds these reserved keywords:

| Keyword | Description |
|---------|-------------|
| `use` | Import declaration |
| `mod` | Module declaration |
| `module` | Root module reference |
| `super` | Parent module reference |

Note: `self` and `pub` are already keywords for other purposes.

---

## 7. Item Grammar

Items that can appear at module level:

```ebnf
Item = [ "pub" ] ( FunctionDef | StructDef | ImplBlock | TypeAlias | UseDecl | ModDecl ) ;
```

In regular `.spl` files:
- `FunctionDef`, `StructDef`, `ImplBlock`, `TypeAlias`, `UseDecl`

In `_module.spl` files only:
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
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
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

// models/_module.spl
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
// In lib/_module.spl
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
| File = module | Dir-based | File-based | Dir-based | File-based |
| Explicit mod decl | Optional | Required | No | No |
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
| Module | Collection of items (functions, types, etc.) |
| Crate | Compilation unit (root module + submodules) |
| Item | Named entity: function, struct, type, etc. |
| Path | Route to an item: `module.submod.item` |
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

// Absolute path
use module.submod.item;

// Parent module
use super.item;

// Module declaration (in _module.spl)
mod submodule;
pub mod public_submodule;

// Re-export
pub use internal.PublicApi;
```

### Design Rationale

1. **Go-style defaults**: Most projects don't need explicit module control. Automatic directory-based modules reduce boilerplate.

2. **Rust-style control**: When needed, `_module.spl` provides full control over module structure without changing the default behavior.

3. **Rust-style imports**: The `use` syntax is proven, readable, and handles complex import patterns elegantly.

4. **Minimal prelude**: Auto-importing common types reduces boilerplate while keeping the prelude small and predictable.
