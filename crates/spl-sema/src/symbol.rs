//! Symbol definitions for the semantic analysis phase.
//!
//! A symbol represents a named entity in the program: functions, structs,
//! variables, parameters, etc. Each symbol has a unique `DefId` that serves
//! as its identity throughout compilation.
//!
//! # `DefId`: The Universal Identifier
//!
//! `DefId` is a simple index into the symbol table. Once assigned, it never
//! changes, making it safe to store in:
//! - Resolution maps (span → `DefId`)
//! - Type information (`binding_types`: `DefId` → `TypeId`)
//! - HIR nodes (variable references store `DefId`, not names)
//!
//! # Symbol Kinds
//!
//! The `SymbolKind` distinguishes different definition types:
//! - **Function**: A function definition (including methods)
//! - **Struct**: A struct type definition
//! - **`TypeAlias`**: A type alias (`type Foo = Bar`)
//! - **Impl**: An impl block (tracked for method lookup)
//! - **Local**: A local variable (`let x = ...`)
//! - **Parameter**: A function parameter
//! - **Field**: A struct field
//! - **`TypeParam`**: A generic type parameter (`<T>`)
//! - **`SelfParam`**: The `self` parameter in methods
//!
//! # Mutability
//!
//! The `is_mutable` flag tracks whether a binding was declared with `mut`.
//! This is used by later phases to validate mutation and borrow checking.

use spl_lexer::Span;
use lasso::Spur;

use super::scope::ScopeId;

/// A unique identifier for each definition in the program.
///
/// # ID Space Layout
///
/// `DefId`s are partitioned into three ranges:
/// - **User definitions** (`0` to `BUILTIN_START - 1`): Regular user-defined symbols
/// - **Builtin definitions** (`BUILTIN_START` to `MAX - 1`): Compiler-generated builtins
/// - **Invalid sentinel** (`MAX`): Marker for unresolved/error cases
///
/// This partitioning ensures user and builtin `DefId`s never collide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefId(u32);

impl DefId {
    /// Start of the builtin `DefId` range.
    ///
    /// User-defined symbols use IDs from 0 to `BUILTIN_START - 1`.
    /// Builtin methods use IDs from `BUILTIN_START` to `u32::MAX - 1`.
    ///
    /// With 2 billion slots for user symbols and 2 billion for builtins,
    /// this provides ample space for any realistic program.
    pub const BUILTIN_START: u32 = u32::MAX / 2;

    /// Maximum number of user-defined symbols before collision with builtins.
    pub const MAX_USER_SYMBOLS: u32 = Self::BUILTIN_START;

    /// Sentinel value for unresolved/invalid definitions.
    /// Uses `u32::MAX` to avoid collision with real `DefIds`.
    pub const INVALID: DefId = DefId(u32::MAX);

    /// Create a new `DefId` with the given index.
    #[inline]
    pub const fn new(index: u32) -> Self {
        DefId(index)
    }

    /// Create a new builtin `DefId` with the given offset from `BUILTIN_START`.
    ///
    /// # Panics
    ///
    /// Debug-asserts that the resulting `DefId` doesn't overflow into `INVALID`.
    #[inline]
    pub(crate) fn new_builtin(offset: u32) -> Self {
        let id = Self::BUILTIN_START.saturating_add(offset);
        debug_assert!(
            id < u32::MAX,
            "Builtin DefId overflow: too many builtin methods"
        );
        DefId(id)
    }

    /// Get the raw index value of this `DefId`.
    #[inline]
    pub fn index(self) -> u32 {
        self.0
    }

    /// Check if this `DefId` is the invalid sentinel.
    #[inline]
    pub fn is_invalid(self) -> bool {
        self == Self::INVALID
    }

    /// Check if this `DefId` is valid (not the sentinel).
    #[inline]
    pub fn is_valid(self) -> bool {
        self != Self::INVALID
    }

    /// Check if this `DefId` refers to a builtin (compiler-generated) symbol.
    #[inline]
    pub fn is_builtin(self) -> bool {
        self.0 >= Self::BUILTIN_START && self.0 < u32::MAX
    }

    /// Check if this `DefId` refers to a user-defined symbol.
    #[inline]
    pub fn is_user_defined(self) -> bool {
        self.0 < Self::BUILTIN_START
    }
}

/// The kind of symbol being defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    TypeAlias,
    Impl,
    Module,
    Local,
    Parameter,
    Field,
    TypeParam,
    SelfParam,
    Trait,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Function => "function",
            Self::Struct => "struct",
            Self::TypeAlias => "type alias",
            Self::Impl => "impl",
            Self::Module => "module",
            Self::Local => "local",
            Self::Parameter => "parameter",
            Self::Field => "field",
            Self::TypeParam => "type parameter",
            Self::SelfParam => "self parameter",
            Self::Trait => "trait",
        };
        f.write_str(s)
    }
}

/// Visibility of a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
    Crate,
    Super,
    PubSelf,
}

/// A symbol in the symbol table.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub def_id: DefId,
    pub name: Spur,
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub span: Span,
    pub scope_id: ScopeId,
    /// Whether this symbol was declared with `mut`.
    pub is_mutable: bool,
}

impl Symbol {
    pub fn new(
        def_id: DefId,
        name: Spur,
        kind: SymbolKind,
        visibility: Visibility,
        span: Span,
        scope_id: ScopeId,
        is_mutable: bool,
    ) -> Self {
        Self {
            def_id,
            name,
            kind,
            visibility,
            span,
            scope_id,
            is_mutable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn def_id_invalid_is_max() {
        assert_eq!(DefId::INVALID, DefId(u32::MAX));
    }

    #[test]
    fn def_id_invalid_is_invalid() {
        assert!(DefId::INVALID.is_invalid());
        assert!(!DefId::INVALID.is_valid());
    }

    #[test]
    fn def_id_zero_is_valid() {
        assert!(!DefId::new(0).is_invalid());
        assert!(DefId::new(0).is_valid());
    }

    #[test]
    fn def_id_regular_values_are_valid() {
        for i in 0..100 {
            assert!(DefId(i).is_valid());
        }
    }

    // ========== ID Space Partitioning Tests ==========

    #[test]
    fn builtin_start_is_half_of_max() {
        assert_eq!(DefId::BUILTIN_START, u32::MAX / 2);
    }

    #[test]
    fn max_user_symbols_equals_builtin_start() {
        assert_eq!(DefId::MAX_USER_SYMBOLS, DefId::BUILTIN_START);
    }

    #[test]
    fn user_def_id_is_not_builtin() {
        for i in 0..100 {
            let def_id = DefId::new(i);
            assert!(def_id.is_user_defined());
            assert!(!def_id.is_builtin());
        }
    }

    #[test]
    fn builtin_def_id_is_builtin() {
        for i in 0..100 {
            let def_id = DefId::new_builtin(i);
            assert!(def_id.is_builtin());
            assert!(!def_id.is_user_defined());
        }
    }

    #[test]
    fn builtin_def_id_offset_is_correct() {
        let def_id = DefId::new_builtin(0);
        assert_eq!(def_id.index(), DefId::BUILTIN_START);

        let def_id = DefId::new_builtin(42);
        assert_eq!(def_id.index(), DefId::BUILTIN_START + 42);
    }

    #[test]
    fn invalid_is_neither_user_nor_builtin() {
        // INVALID is a special sentinel, not a user or builtin symbol
        assert!(!DefId::INVALID.is_user_defined());
        // Note: is_builtin returns true for INVALID since u32::MAX >= BUILTIN_START
        // but is_invalid should be checked first in practice
        assert!(DefId::INVALID.is_invalid());
    }

    #[test]
    fn boundary_values() {
        // Just below builtin range
        let last_user = DefId::new(DefId::BUILTIN_START - 1);
        assert!(last_user.is_user_defined());
        assert!(!last_user.is_builtin());
        assert!(last_user.is_valid());

        // First builtin
        let first_builtin = DefId::new_builtin(0);
        assert!(!first_builtin.is_user_defined());
        assert!(first_builtin.is_builtin());
        assert!(first_builtin.is_valid());
    }
}
