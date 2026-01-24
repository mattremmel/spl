//! Semantic type representation for SPL.
//!
//! This module defines the internal type representation used by type inference
//! and type checking. Unlike AST types which contain source spans and represent
//! syntax, these semantic types are resolved and interned for efficient comparison.
//!
//! # Contract Checking Philosophy
//!
//! The `TypeInterner` uses `debug_assert!()` for bounds checking on `TypeId` lookups.
//! This is intentional: `TypeId` values are always created by the interner itself
//! via `intern()`, `fresh_type_var()`, etc. If an invalid `TypeId` is passed to
//! `get()`, it indicates a bug in the compiler (using a stale ID, mixing IDs from
//! different interners, etc.), not a user error.
//!
//! In release builds, these assertions are disabled and invalid IDs would cause
//! a panic from out-of-bounds indexing. The `debug_assert!()` provides better
//! diagnostics during development without runtime cost in production.

use std::collections::HashMap;

use super::symbol::DefId;

/// A unique identifier for an interned type.
///
/// Types are interned so that type equality can be checked via ID comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

/// A type inference variable.
///
/// During type inference, unknown types are represented as variables that
/// get unified with concrete types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeVar(pub u32);

/// Primitive (built-in) type kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveKind {
    // Signed integers
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    // Unsigned integers
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    // Floating point
    F32,
    F64,
    // Other primitives
    Bool,
    Char,
    /// The unit type `()`
    Unit,
    /// The never type `!`
    Never,
    /// The unsized string slice type `str`
    Str,
}

impl PrimitiveKind {
    /// Validate that an integer literal value is in range for this primitive type.
    /// Returns `Ok(())` if in range, or `Err(message)` with a descriptive error.
    pub fn validate_int_literal_range(self, value: i128) -> Result<(), String> {
        let (min, max): (i128, i128) = match self {
            Self::I8 => (i8::MIN as i128, i8::MAX as i128),
            Self::I16 => (i16::MIN as i128, i16::MAX as i128),
            Self::I32 => (i32::MIN as i128, i32::MAX as i128),
            Self::I64 => (i64::MIN as i128, i64::MAX as i128),
            Self::I128 => (i128::MIN, i128::MAX),
            Self::Isize => (isize::MIN as i128, isize::MAX as i128),
            Self::U8 => (0, u8::MAX as i128),
            Self::U16 => (0, u16::MAX as i128),
            Self::U32 => (0, u32::MAX as i128),
            Self::U64 => (0, u64::MAX as i128),
            Self::U128 => (0, i128::MAX), // Can't represent u128::MAX in i128
            Self::Usize => (0, usize::MAX as i128),
            // Non-integer types
            _ => return Ok(()),
        };

        if value < min || value > max {
            Err(format!(
                "literal `{}` is out of range for `{}`",
                value,
                self.as_str()
            ))
        } else {
            Ok(())
        }
    }

    /// Returns the primitive kind for a given type name, if it exists.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "i128" => Some(Self::I128),
            "isize" => Some(Self::Isize),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "u128" => Some(Self::U128),
            "usize" => Some(Self::Usize),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "bool" => Some(Self::Bool),
            "char" => Some(Self::Char),
            "str" => Some(Self::Str),
            _ => None,
        }
    }

    /// Returns the type name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::Isize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::Usize => "usize",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Char => "char",
            Self::Unit => "()",
            Self::Never => "!",
            Self::Str => "str",
        }
    }
}

/// Reference mutability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mutability {
    /// Shared reference `&T`
    Shared,
    /// Mutable reference `&mut T`
    Mutable,
}

/// Classification of inference variables for constrained unification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InferKind {
    /// General type variable - unifies with anything.
    General,
    /// Integer type variable - unifies only with integers, defaults to i32.
    Int,
    /// Float type variable - unifies only with floats, defaults to f64.
    Float,
}

/// A semantic type representation.
///
/// This enum represents all possible types in the SPL type system after
/// name resolution. Unlike AST types, these are fully resolved and ready
/// for type inference and checking.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    /// A primitive built-in type.
    Primitive(PrimitiveKind),

    /// A type inference variable with kind classification.
    /// - General: unifies with any type
    /// - Int: unifies only with integer types, defaults to i32
    /// - Float: unifies only with float types, defaults to f64
    Infer(TypeVar, InferKind),

    /// A reference type `&T` or `&mut T`.
    Ref(Mutability, TypeId),

    /// A raw pointer type `*T` (immutable) or `*mut T` (mutable).
    RawPtr(Mutability, TypeId),

    /// An array type `[T; N]` with a compile-time known size.
    Array(TypeId, u64),

    /// A slice type `[T]` (unsized).
    Slice(TypeId),

    /// A tuple type `(T, U, ...)`.
    Tuple(Vec<TypeId>),

    /// A user-defined struct type with generic arguments.
    Struct(DefId, Vec<TypeId>),

    /// A type alias with generic arguments.
    Alias(DefId, Vec<TypeId>),

    /// A function pointer type `fn(T, U) -> R`.
    FnPtr { params: Vec<TypeId>, ret: TypeId },

    /// A generic type parameter `T` (refers to a type parameter definition).
    Param(DefId),

    /// The `Self` type in impl blocks.
    SelfType,

    /// A string reference type (fat pointer: ptr + len).
    /// Equivalent to Rust's `&str`. Points to UTF-8 data without owning it.
    StrRef,

    /// An error type for error recovery during type checking.
    /// This type unifies with anything to prevent cascading errors.
    Error,
}

/// An interner for semantic types.
///
/// Types are interned so that type equality can be checked via TypeId comparison.
/// Primitive types are pre-interned with stable IDs.
#[derive(Debug)]
pub struct TypeInterner {
    /// All interned types.
    types: Vec<Type>,
    /// Map from type to its ID for deduplication.
    type_to_id: HashMap<Type, TypeId>,
    /// Counter for generating fresh type variables.
    next_type_var: u32,

    // Pre-interned primitive type IDs
    unit_id: TypeId,
    bool_id: TypeId,
    i32_id: TypeId,
    i64_id: TypeId,
    f32_id: TypeId,
    f64_id: TypeId,
    never_id: TypeId,
    error_id: TypeId,
    str_ref_id: TypeId,
    char_id: TypeId,
    str_id: TypeId,
}

impl Default for TypeInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeInterner {
    /// Create a new type interner with pre-interned primitive types.
    pub fn new() -> Self {
        let mut interner = Self {
            types: Vec::new(),
            type_to_id: HashMap::new(),
            next_type_var: 0,
            // These will be filled in below
            unit_id: TypeId(0),
            bool_id: TypeId(0),
            i32_id: TypeId(0),
            i64_id: TypeId(0),
            f32_id: TypeId(0),
            f64_id: TypeId(0),
            never_id: TypeId(0),
            error_id: TypeId(0),
            str_ref_id: TypeId(0),
            char_id: TypeId(0),
            str_id: TypeId(0),
        };

        // Pre-intern common types for fast access
        interner.unit_id = interner.intern(Type::Primitive(PrimitiveKind::Unit));
        interner.bool_id = interner.intern(Type::Primitive(PrimitiveKind::Bool));
        interner.i32_id = interner.intern(Type::Primitive(PrimitiveKind::I32));
        interner.i64_id = interner.intern(Type::Primitive(PrimitiveKind::I64));
        interner.f32_id = interner.intern(Type::Primitive(PrimitiveKind::F32));
        interner.f64_id = interner.intern(Type::Primitive(PrimitiveKind::F64));
        interner.never_id = interner.intern(Type::Primitive(PrimitiveKind::Never));
        interner.error_id = interner.intern(Type::Error);
        interner.str_ref_id = interner.intern(Type::StrRef);
        interner.char_id = interner.intern(Type::Primitive(PrimitiveKind::Char));
        interner.str_id = interner.intern(Type::Primitive(PrimitiveKind::Str));

        // Pre-intern all other primitives for consistency
        for prim in [
            PrimitiveKind::I8,
            PrimitiveKind::I16,
            PrimitiveKind::I64,
            PrimitiveKind::I128,
            PrimitiveKind::Isize,
            PrimitiveKind::U8,
            PrimitiveKind::U16,
            PrimitiveKind::U32,
            PrimitiveKind::U64,
            PrimitiveKind::U128,
            PrimitiveKind::Usize,
        ] {
            interner.intern(Type::Primitive(prim));
        }

        interner
    }

    /// Intern a type, returning its unique ID.
    ///
    /// If the type has already been interned, returns the existing ID.
    pub fn intern(&mut self, ty: Type) -> TypeId {
        if let Some(&id) = self.type_to_id.get(&ty) {
            return id;
        }

        debug_assert!(
            self.types.len() < u32::MAX as usize,
            "precondition: type interner overflow - {} types exceeds u32::MAX",
            self.types.len()
        );

        let id = TypeId(self.types.len() as u32);
        self.types.push(ty.clone());
        self.type_to_id.insert(ty, id);
        id
    }

    /// Get the type for a given ID.
    pub fn get(&self, id: TypeId) -> &Type {
        debug_assert!(
            (id.0 as usize) < self.types.len(),
            "precondition: TypeId {} must be valid (< {})",
            id.0,
            self.types.len()
        );
        &self.types[id.0 as usize]
    }

    // ===== Fresh Type Variables =====

    /// Create a fresh general type variable.
    pub fn fresh_type_var(&mut self) -> TypeId {
        debug_assert!(
            self.next_type_var < u32::MAX,
            "precondition: type variable ID overflow"
        );
        let var = TypeVar(self.next_type_var);
        self.next_type_var += 1;
        self.intern(Type::Infer(var, InferKind::General))
    }

    /// Create a fresh integer type variable (defaults to i32 if unconstrained).
    pub fn fresh_int_var(&mut self) -> TypeId {
        debug_assert!(
            self.next_type_var < u32::MAX,
            "precondition: type variable ID overflow"
        );
        let var = TypeVar(self.next_type_var);
        self.next_type_var += 1;
        self.intern(Type::Infer(var, InferKind::Int))
    }

    /// Create a fresh float type variable (defaults to f64 if unconstrained).
    pub fn fresh_float_var(&mut self) -> TypeId {
        debug_assert!(
            self.next_type_var < u32::MAX,
            "precondition: type variable ID overflow"
        );
        let var = TypeVar(self.next_type_var);
        self.next_type_var += 1;
        self.intern(Type::Infer(var, InferKind::Float))
    }

    // ===== Type Construction Helpers =====

    /// Create a reference type.
    pub fn mk_ref(&mut self, mutability: Mutability, inner: TypeId) -> TypeId {
        self.intern(Type::Ref(mutability, inner))
    }

    /// Create a raw pointer type.
    pub fn mk_raw_ptr(&mut self, mutability: Mutability, pointee: TypeId) -> TypeId {
        self.intern(Type::RawPtr(mutability, pointee))
    }

    /// Create an array type.
    pub fn mk_array(&mut self, elem: TypeId, len: u64) -> TypeId {
        self.intern(Type::Array(elem, len))
    }

    /// Create a slice type.
    pub fn mk_slice(&mut self, elem: TypeId) -> TypeId {
        self.intern(Type::Slice(elem))
    }

    /// Create a tuple type.
    pub fn mk_tuple(&mut self, elems: Vec<TypeId>) -> TypeId {
        self.intern(Type::Tuple(elems))
    }

    /// Create a struct type.
    pub fn mk_struct(&mut self, def_id: DefId, type_args: Vec<TypeId>) -> TypeId {
        self.intern(Type::Struct(def_id, type_args))
    }

    /// Create a type alias.
    pub fn mk_alias(&mut self, def_id: DefId, type_args: Vec<TypeId>) -> TypeId {
        self.intern(Type::Alias(def_id, type_args))
    }

    /// Create a function pointer type.
    pub fn mk_fn_ptr(&mut self, params: Vec<TypeId>, ret: TypeId) -> TypeId {
        self.intern(Type::FnPtr { params, ret })
    }

    /// Create a type parameter reference.
    pub fn mk_param(&mut self, def_id: DefId) -> TypeId {
        self.intern(Type::Param(def_id))
    }

    /// Intern a primitive type by kind.
    pub fn primitive(&mut self, kind: PrimitiveKind) -> TypeId {
        self.intern(Type::Primitive(kind))
    }

    // ===== Convenience Accessors =====

    /// Get the unit type `()`.
    pub fn unit(&self) -> TypeId {
        self.unit_id
    }

    /// Get the bool type.
    pub fn bool(&self) -> TypeId {
        self.bool_id
    }

    /// Get the i32 type.
    pub fn i32(&self) -> TypeId {
        self.i32_id
    }

    /// Get the i64 type.
    pub fn i64(&self) -> TypeId {
        self.i64_id
    }

    /// Get the f32 type.
    pub fn f32(&self) -> TypeId {
        self.f32_id
    }

    /// Get the f64 type.
    pub fn f64(&self) -> TypeId {
        self.f64_id
    }

    /// Get the never type `!`.
    pub fn never(&self) -> TypeId {
        self.never_id
    }

    /// Get the error type.
    pub fn error(&self) -> TypeId {
        self.error_id
    }

    /// Get the StrRef type (fat pointer to UTF-8 data, like Rust's `&str`).
    pub fn str_ref(&self) -> TypeId {
        self.str_ref_id
    }

    /// Get the char type.
    pub fn char(&self) -> TypeId {
        self.char_id
    }

    /// Get the str type.
    pub fn str(&self) -> TypeId {
        self.str_id
    }

    /// Get the Self type.
    pub fn self_type(&mut self) -> TypeId {
        self.intern(Type::SelfType)
    }

    // ===== Contract Helpers =====

    /// Returns the number of interned types.
    /// Used for contract assertions to validate TypeId bounds.
    pub fn types_len(&self) -> usize {
        self.types.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_interner_creation() {
        let interner = TypeInterner::new();
        // Should have pre-interned types
        assert!(!interner.types.is_empty());
    }

    #[test]
    fn test_primitive_pre_interned() {
        let interner = TypeInterner::new();

        // Check that commonly used primitives are pre-interned
        let unit = interner.unit();
        let bool_ty = interner.bool();
        let i32_ty = interner.i32();
        let f64_ty = interner.f64();

        assert_eq!(interner.get(unit), &Type::Primitive(PrimitiveKind::Unit));
        assert_eq!(interner.get(bool_ty), &Type::Primitive(PrimitiveKind::Bool));
        assert_eq!(interner.get(i32_ty), &Type::Primitive(PrimitiveKind::I32));
        assert_eq!(interner.get(f64_ty), &Type::Primitive(PrimitiveKind::F64));
    }

    #[test]
    fn test_type_equality_via_id() {
        let mut interner = TypeInterner::new();

        let arr1 = interner.mk_array(interner.i32(), 10);
        let arr2 = interner.mk_array(interner.i32(), 10);
        let arr3 = interner.mk_array(interner.i32(), 20);

        // Same type should return same ID
        assert_eq!(arr1, arr2);
        // Different type should return different ID
        assert_ne!(arr1, arr3);
    }

    #[test]
    fn test_intern_same_type_returns_same_id() {
        let mut interner = TypeInterner::new();

        let ty1 = interner.intern(Type::Primitive(PrimitiveKind::I32));
        let ty2 = interner.intern(Type::Primitive(PrimitiveKind::I32));

        assert_eq!(ty1, ty2);
    }

    #[test]
    fn test_fresh_type_var() {
        let mut interner = TypeInterner::new();

        let var1 = interner.fresh_type_var();
        let var2 = interner.fresh_type_var();
        let var3 = interner.fresh_type_var();

        // Each fresh variable should be unique
        assert_ne!(var1, var2);
        assert_ne!(var2, var3);
        assert_ne!(var1, var3);

        // Check the underlying types
        match interner.get(var1) {
            Type::Infer(TypeVar(0), InferKind::General) => {}
            other => panic!("expected Infer(0, General), got {other:?}"),
        }
        match interner.get(var2) {
            Type::Infer(TypeVar(1), InferKind::General) => {}
            other => panic!("expected Infer(1, General), got {other:?}"),
        }
    }

    #[test]
    fn test_fresh_int_var() {
        let mut interner = TypeInterner::new();

        let var = interner.fresh_int_var();

        match interner.get(var) {
            Type::Infer(_, InferKind::Int) => {}
            other => panic!("expected Infer(_, Int), got {other:?}"),
        }
    }

    #[test]
    fn test_fresh_float_var() {
        let mut interner = TypeInterner::new();

        let var = interner.fresh_float_var();

        match interner.get(var) {
            Type::Infer(_, InferKind::Float) => {}
            other => panic!("expected Infer(_, Float), got {other:?}"),
        }
    }

    #[test]
    fn test_mk_ref() {
        let mut interner = TypeInterner::new();

        let shared_ref = interner.mk_ref(Mutability::Shared, interner.i32());
        let mut_ref = interner.mk_ref(Mutability::Mutable, interner.i32());

        assert_ne!(shared_ref, mut_ref);

        match interner.get(shared_ref) {
            Type::Ref(Mutability::Shared, inner) => {
                assert_eq!(*inner, interner.i32());
            }
            other => panic!("expected Ref(Shared, _), got {other:?}"),
        }

        match interner.get(mut_ref) {
            Type::Ref(Mutability::Mutable, inner) => {
                assert_eq!(*inner, interner.i32());
            }
            other => panic!("expected Ref(Mutable, _), got {other:?}"),
        }
    }

    #[test]
    fn test_mk_array() {
        let mut interner = TypeInterner::new();

        let arr = interner.mk_array(interner.i32(), 42);

        match interner.get(arr) {
            Type::Array(elem, len) => {
                assert_eq!(*elem, interner.i32());
                assert_eq!(*len, 42);
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_slice() {
        let mut interner = TypeInterner::new();

        let slice = interner.mk_slice(interner.i32());

        match interner.get(slice) {
            Type::Slice(elem) => {
                assert_eq!(*elem, interner.i32());
            }
            other => panic!("expected Slice, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_tuple() {
        let mut interner = TypeInterner::new();

        let tuple = interner.mk_tuple(vec![interner.i32(), interner.bool(), interner.f64()]);

        match interner.get(tuple) {
            Type::Tuple(elems) => {
                assert_eq!(elems.len(), 3);
                assert_eq!(elems[0], interner.i32());
                assert_eq!(elems[1], interner.bool());
                assert_eq!(elems[2], interner.f64());
            }
            other => panic!("expected Tuple, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_tuple_empty() {
        let mut interner = TypeInterner::new();

        let empty_tuple = interner.mk_tuple(vec![]);

        match interner.get(empty_tuple) {
            Type::Tuple(elems) => {
                assert!(elems.is_empty());
            }
            other => panic!("expected empty Tuple, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_struct() {
        let mut interner = TypeInterner::new();

        let struct_ty = interner.mk_struct(DefId(5), vec![interner.i32()]);

        match interner.get(struct_ty) {
            Type::Struct(def_id, args) => {
                assert_eq!(*def_id, DefId(5));
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], interner.i32());
            }
            other => panic!("expected Struct, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_alias() {
        let mut interner = TypeInterner::new();

        let alias_ty = interner.mk_alias(DefId(7), vec![interner.bool()]);

        match interner.get(alias_ty) {
            Type::Alias(def_id, args) => {
                assert_eq!(*def_id, DefId(7));
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], interner.bool());
            }
            other => panic!("expected Alias, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_fn_ptr() {
        let mut interner = TypeInterner::new();

        let fn_ptr = interner.mk_fn_ptr(vec![interner.i32(), interner.bool()], interner.f64());

        match interner.get(fn_ptr) {
            Type::FnPtr { params, ret } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], interner.i32());
                assert_eq!(params[1], interner.bool());
                assert_eq!(*ret, interner.f64());
            }
            other => panic!("expected FnPtr, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_param() {
        let mut interner = TypeInterner::new();

        let param_ty = interner.mk_param(DefId(3));

        match interner.get(param_ty) {
            Type::Param(def_id) => {
                assert_eq!(*def_id, DefId(3));
            }
            other => panic!("expected Param, got {other:?}"),
        }
    }

    #[test]
    fn test_primitive_from_name() {
        assert_eq!(PrimitiveKind::from_name("i32"), Some(PrimitiveKind::I32));
        assert_eq!(PrimitiveKind::from_name("u64"), Some(PrimitiveKind::U64));
        assert_eq!(PrimitiveKind::from_name("bool"), Some(PrimitiveKind::Bool));
        assert_eq!(PrimitiveKind::from_name("f64"), Some(PrimitiveKind::F64));
        assert_eq!(PrimitiveKind::from_name("str"), Some(PrimitiveKind::Str));
        assert_eq!(PrimitiveKind::from_name("char"), Some(PrimitiveKind::Char));
        assert_eq!(
            PrimitiveKind::from_name("isize"),
            Some(PrimitiveKind::Isize)
        );
        assert_eq!(
            PrimitiveKind::from_name("usize"),
            Some(PrimitiveKind::Usize)
        );
        assert_eq!(PrimitiveKind::from_name("unknown"), None);
        assert_eq!(PrimitiveKind::from_name("String"), None); // String is not a primitive
    }

    #[test]
    fn test_primitive_as_str() {
        assert_eq!(PrimitiveKind::I32.as_str(), "i32");
        assert_eq!(PrimitiveKind::U64.as_str(), "u64");
        assert_eq!(PrimitiveKind::Bool.as_str(), "bool");
        assert_eq!(PrimitiveKind::Unit.as_str(), "()");
        assert_eq!(PrimitiveKind::Never.as_str(), "!");
        assert_eq!(PrimitiveKind::Str.as_str(), "str");
    }

    #[test]
    fn test_error_type() {
        let interner = TypeInterner::new();

        let error = interner.error();
        assert_eq!(interner.get(error), &Type::Error);
    }

    #[test]
    fn test_never_type() {
        let interner = TypeInterner::new();

        let never = interner.never();
        assert_eq!(interner.get(never), &Type::Primitive(PrimitiveKind::Never));
    }

    #[test]
    fn test_string_type() {
        let interner = TypeInterner::new();

        let string_ty = interner.str_ref();
        assert_eq!(interner.get(string_ty), &Type::StrRef);
    }

    #[test]
    fn test_self_type() {
        let mut interner = TypeInterner::new();

        let self_ty = interner.self_type();
        assert_eq!(interner.get(self_ty), &Type::SelfType);
    }

    #[test]
    fn test_nested_types() {
        let mut interner = TypeInterner::new();

        // Create &[i32]
        let slice = interner.mk_slice(interner.i32());
        let ref_slice = interner.mk_ref(Mutability::Shared, slice);

        match interner.get(ref_slice) {
            Type::Ref(Mutability::Shared, inner) => {
                assert_eq!(*inner, slice);
                match interner.get(*inner) {
                    Type::Slice(elem) => {
                        assert_eq!(*elem, interner.i32());
                    }
                    other => panic!("expected Slice, got {other:?}"),
                }
            }
            other => panic!("expected Ref, got {other:?}"),
        }
    }

    #[test]
    fn test_complex_fn_ptr() {
        let mut interner = TypeInterner::new();

        // fn(&i32, &mut bool) -> (i32, f64)
        let ref_i32 = interner.mk_ref(Mutability::Shared, interner.i32());
        let mut_ref_bool = interner.mk_ref(Mutability::Mutable, interner.bool());
        let ret_tuple = interner.mk_tuple(vec![interner.i32(), interner.f64()]);

        let fn_ptr = interner.mk_fn_ptr(vec![ref_i32, mut_ref_bool], ret_tuple);

        match interner.get(fn_ptr) {
            Type::FnPtr { params, ret } => {
                assert_eq!(params.len(), 2);
                assert_eq!(*ret, ret_tuple);
            }
            other => panic!("expected FnPtr, got {other:?}"),
        }
    }

    #[test]
    fn test_type_interner_default() {
        let interner = TypeInterner::default();
        // Should work the same as new()
        assert_eq!(
            interner.get(interner.i32()),
            &Type::Primitive(PrimitiveKind::I32)
        );
    }

    #[test]
    fn test_all_primitives_from_name() {
        let cases = [
            ("i8", PrimitiveKind::I8),
            ("i16", PrimitiveKind::I16),
            ("i32", PrimitiveKind::I32),
            ("i64", PrimitiveKind::I64),
            ("i128", PrimitiveKind::I128),
            ("isize", PrimitiveKind::Isize),
            ("u8", PrimitiveKind::U8),
            ("u16", PrimitiveKind::U16),
            ("u32", PrimitiveKind::U32),
            ("u64", PrimitiveKind::U64),
            ("u128", PrimitiveKind::U128),
            ("usize", PrimitiveKind::Usize),
            ("f32", PrimitiveKind::F32),
            ("f64", PrimitiveKind::F64),
            ("bool", PrimitiveKind::Bool),
            ("char", PrimitiveKind::Char),
            ("str", PrimitiveKind::Str),
        ];

        for (name, expected) in cases {
            assert_eq!(
                PrimitiveKind::from_name(name),
                Some(expected),
                "failed for {name}"
            );
        }
    }

    #[test]
    fn test_type_var_ids_are_sequential() {
        let mut interner = TypeInterner::new();

        let v1 = interner.fresh_type_var();
        let v2 = interner.fresh_int_var();
        let v3 = interner.fresh_float_var();
        let v4 = interner.fresh_type_var();

        // Extract the TypeVar IDs
        let id1 = match interner.get(v1) {
            Type::Infer(TypeVar(id), InferKind::General) => *id,
            _ => panic!("expected Infer(_, General)"),
        };
        let id2 = match interner.get(v2) {
            Type::Infer(TypeVar(id), InferKind::Int) => *id,
            _ => panic!("expected Infer(_, Int)"),
        };
        let id3 = match interner.get(v3) {
            Type::Infer(TypeVar(id), InferKind::Float) => *id,
            _ => panic!("expected Infer(_, Float)"),
        };
        let id4 = match interner.get(v4) {
            Type::Infer(TypeVar(id), InferKind::General) => *id,
            _ => panic!("expected Infer(_, General)"),
        };

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
        assert_eq!(id4, 3);
    }

    #[test]
    fn test_primitive_method() {
        let mut interner = TypeInterner::new();

        let i8_ty = interner.primitive(PrimitiveKind::I8);
        let u128_ty = interner.primitive(PrimitiveKind::U128);

        assert_eq!(interner.get(i8_ty), &Type::Primitive(PrimitiveKind::I8));
        assert_eq!(interner.get(u128_ty), &Type::Primitive(PrimitiveKind::U128));
    }

    // ===== Additional comprehensive tests =====

    #[test]
    fn test_all_primitives_as_str() {
        // Test every single variant of PrimitiveKind::as_str()
        let cases = [
            (PrimitiveKind::I8, "i8"),
            (PrimitiveKind::I16, "i16"),
            (PrimitiveKind::I32, "i32"),
            (PrimitiveKind::I64, "i64"),
            (PrimitiveKind::I128, "i128"),
            (PrimitiveKind::Isize, "isize"),
            (PrimitiveKind::U8, "u8"),
            (PrimitiveKind::U16, "u16"),
            (PrimitiveKind::U32, "u32"),
            (PrimitiveKind::U64, "u64"),
            (PrimitiveKind::U128, "u128"),
            (PrimitiveKind::Usize, "usize"),
            (PrimitiveKind::F32, "f32"),
            (PrimitiveKind::F64, "f64"),
            (PrimitiveKind::Bool, "bool"),
            (PrimitiveKind::Char, "char"),
            (PrimitiveKind::Unit, "()"),
            (PrimitiveKind::Never, "!"),
            (PrimitiveKind::Str, "str"),
        ];

        for (kind, expected) in cases {
            assert_eq!(kind.as_str(), expected, "failed for {kind:?}");
        }
    }

    #[test]
    fn test_primitive_from_name_returns_none_for_special_syntax() {
        // Unit and Never use special syntax, not names
        assert_eq!(PrimitiveKind::from_name("()"), None);
        assert_eq!(PrimitiveKind::from_name("!"), None);
        assert_eq!(PrimitiveKind::from_name("unit"), None);
        assert_eq!(PrimitiveKind::from_name("never"), None);
    }

    #[test]
    fn test_primitive_from_name_case_sensitive() {
        assert_eq!(PrimitiveKind::from_name("I32"), None);
        assert_eq!(PrimitiveKind::from_name("Bool"), None);
        assert_eq!(PrimitiveKind::from_name("CHAR"), None);
    }

    #[test]
    fn test_all_convenience_accessors() {
        let interner = TypeInterner::new();

        // Test all pre-interned accessors return correct types
        assert_eq!(
            interner.get(interner.unit()),
            &Type::Primitive(PrimitiveKind::Unit)
        );
        assert_eq!(
            interner.get(interner.bool()),
            &Type::Primitive(PrimitiveKind::Bool)
        );
        assert_eq!(
            interner.get(interner.i32()),
            &Type::Primitive(PrimitiveKind::I32)
        );
        assert_eq!(
            interner.get(interner.i64()),
            &Type::Primitive(PrimitiveKind::I64)
        );
        assert_eq!(
            interner.get(interner.f32()),
            &Type::Primitive(PrimitiveKind::F32)
        );
        assert_eq!(
            interner.get(interner.f64()),
            &Type::Primitive(PrimitiveKind::F64)
        );
        assert_eq!(
            interner.get(interner.never()),
            &Type::Primitive(PrimitiveKind::Never)
        );
        assert_eq!(
            interner.get(interner.char()),
            &Type::Primitive(PrimitiveKind::Char)
        );
        assert_eq!(
            interner.get(interner.str()),
            &Type::Primitive(PrimitiveKind::Str)
        );
        assert_eq!(interner.get(interner.error()), &Type::Error);
        assert_eq!(interner.get(interner.str_ref()), &Type::StrRef);
    }

    #[test]
    fn test_all_primitives_pre_interned() {
        let mut interner = TypeInterner::new();

        // All 19 primitives should already be interned
        let all_prims = [
            PrimitiveKind::I8,
            PrimitiveKind::I16,
            PrimitiveKind::I32,
            PrimitiveKind::I64,
            PrimitiveKind::I128,
            PrimitiveKind::Isize,
            PrimitiveKind::U8,
            PrimitiveKind::U16,
            PrimitiveKind::U32,
            PrimitiveKind::U64,
            PrimitiveKind::U128,
            PrimitiveKind::Usize,
            PrimitiveKind::F32,
            PrimitiveKind::F64,
            PrimitiveKind::Bool,
            PrimitiveKind::Char,
            PrimitiveKind::Unit,
            PrimitiveKind::Never,
            PrimitiveKind::Str,
        ];

        let initial_count = interner.types.len();

        for prim in all_prims {
            interner.primitive(prim);
        }

        // No new types should have been added
        assert_eq!(
            interner.types.len(),
            initial_count,
            "primitives were not pre-interned"
        );
    }

    #[test]
    fn test_mk_array_zero_length() {
        let mut interner = TypeInterner::new();

        let arr = interner.mk_array(interner.i32(), 0);

        match interner.get(arr) {
            Type::Array(elem, len) => {
                assert_eq!(*elem, interner.i32());
                assert_eq!(*len, 0);
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_array_large_length() {
        let mut interner = TypeInterner::new();

        let u8_ty = interner.primitive(PrimitiveKind::U8);
        let arr = interner.mk_array(u8_ty, u64::MAX);

        match interner.get(arr) {
            Type::Array(_, len) => {
                assert_eq!(*len, u64::MAX);
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_fn_ptr_no_params() {
        let mut interner = TypeInterner::new();

        let fn_ptr = interner.mk_fn_ptr(vec![], interner.unit());

        match interner.get(fn_ptr) {
            Type::FnPtr { params, ret } => {
                assert!(params.is_empty());
                assert_eq!(*ret, interner.unit());
            }
            other => panic!("expected FnPtr, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_struct_no_type_args() {
        let mut interner = TypeInterner::new();

        let struct_ty = interner.mk_struct(DefId(10), vec![]);

        match interner.get(struct_ty) {
            Type::Struct(def_id, args) => {
                assert_eq!(*def_id, DefId(10));
                assert!(args.is_empty());
            }
            other => panic!("expected Struct, got {other:?}"),
        }
    }

    #[test]
    fn test_mk_alias_no_type_args() {
        let mut interner = TypeInterner::new();

        let alias_ty = interner.mk_alias(DefId(20), vec![]);

        match interner.get(alias_ty) {
            Type::Alias(def_id, args) => {
                assert_eq!(*def_id, DefId(20));
                assert!(args.is_empty());
            }
            other => panic!("expected Alias, got {other:?}"),
        }
    }

    #[test]
    fn test_self_type_interning_idempotent() {
        let mut interner = TypeInterner::new();

        let self1 = interner.self_type();
        let self2 = interner.self_type();
        let self3 = interner.self_type();

        // Should all be the same interned type
        assert_eq!(self1, self2);
        assert_eq!(self2, self3);
    }

    #[test]
    fn test_tuple_interning_same_elements() {
        let mut interner = TypeInterner::new();

        let tuple1 = interner.mk_tuple(vec![interner.i32(), interner.bool()]);
        let tuple2 = interner.mk_tuple(vec![interner.i32(), interner.bool()]);

        // Same tuple should return same ID
        assert_eq!(tuple1, tuple2);
    }

    #[test]
    fn test_tuple_interning_different_order() {
        let mut interner = TypeInterner::new();

        let tuple1 = interner.mk_tuple(vec![interner.i32(), interner.bool()]);
        let tuple2 = interner.mk_tuple(vec![interner.bool(), interner.i32()]);

        // Different order should be different types
        assert_ne!(tuple1, tuple2);
    }

    #[test]
    fn test_different_struct_same_def_different_args() {
        let mut interner = TypeInterner::new();

        let s1 = interner.mk_struct(DefId(1), vec![interner.i32()]);
        let s2 = interner.mk_struct(DefId(1), vec![interner.bool()]);
        let s3 = interner.mk_struct(DefId(1), vec![interner.i32()]);

        // Same def, different args -> different types
        assert_ne!(s1, s2);
        // Same def, same args -> same type
        assert_eq!(s1, s3);
    }

    #[test]
    fn test_different_struct_different_def_same_args() {
        let mut interner = TypeInterner::new();

        let s1 = interner.mk_struct(DefId(1), vec![interner.i32()]);
        let s2 = interner.mk_struct(DefId(2), vec![interner.i32()]);

        // Different def -> different types even with same args
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_ref_different_mutability_same_inner() {
        let mut interner = TypeInterner::new();

        let shared = interner.mk_ref(Mutability::Shared, interner.i32());
        let mutable = interner.mk_ref(Mutability::Mutable, interner.i32());

        assert_ne!(shared, mutable);
    }

    #[test]
    fn test_ref_same_mutability_different_inner() {
        let mut interner = TypeInterner::new();

        let ref1 = interner.mk_ref(Mutability::Shared, interner.i32());
        let ref2 = interner.mk_ref(Mutability::Shared, interner.bool());

        assert_ne!(ref1, ref2);
    }

    #[test]
    fn test_slice_vs_array_same_element() {
        let mut interner = TypeInterner::new();

        let slice = interner.mk_slice(interner.i32());
        let array = interner.mk_array(interner.i32(), 1);

        // Slice and array are different types even with same element
        assert_ne!(slice, array);
    }

    #[test]
    fn test_fn_ptr_different_return_same_params() {
        let mut interner = TypeInterner::new();

        let fn1 = interner.mk_fn_ptr(vec![interner.i32()], interner.bool());
        let fn2 = interner.mk_fn_ptr(vec![interner.i32()], interner.i32());

        assert_ne!(fn1, fn2);
    }

    #[test]
    fn test_fn_ptr_same_return_different_params() {
        let mut interner = TypeInterner::new();

        let fn1 = interner.mk_fn_ptr(vec![interner.i32()], interner.bool());
        let fn2 = interner.mk_fn_ptr(vec![interner.bool()], interner.bool());

        assert_ne!(fn1, fn2);
    }

    #[test]
    fn test_fn_ptr_different_param_count() {
        let mut interner = TypeInterner::new();

        let fn1 = interner.mk_fn_ptr(vec![interner.i32()], interner.unit());
        let fn2 = interner.mk_fn_ptr(vec![interner.i32(), interner.i32()], interner.unit());

        assert_ne!(fn1, fn2);
    }

    #[test]
    fn test_deeply_nested_type() {
        let mut interner = TypeInterner::new();

        // Create &mut [&[i32; 5]]
        let inner_array = interner.mk_array(interner.i32(), 5);
        let inner_ref = interner.mk_ref(Mutability::Shared, inner_array);
        let slice = interner.mk_slice(inner_ref);
        let outer_ref = interner.mk_ref(Mutability::Mutable, slice);

        // Verify structure
        match interner.get(outer_ref) {
            Type::Ref(Mutability::Mutable, slice_id) => match interner.get(*slice_id) {
                Type::Slice(ref_id) => match interner.get(*ref_id) {
                    Type::Ref(Mutability::Shared, arr_id) => match interner.get(*arr_id) {
                        Type::Array(elem, 5) => {
                            assert_eq!(*elem, interner.i32());
                        }
                        other => panic!("expected Array, got {other:?}"),
                    },
                    other => panic!("expected Ref(Shared), got {other:?}"),
                },
                other => panic!("expected Slice, got {other:?}"),
            },
            other => panic!("expected Ref(Mutable), got {other:?}"),
        }
    }

    #[test]
    fn test_tuple_single_element() {
        let mut interner = TypeInterner::new();

        let tuple = interner.mk_tuple(vec![interner.i32()]);

        match interner.get(tuple) {
            Type::Tuple(elems) => {
                assert_eq!(elems.len(), 1);
                assert_eq!(elems[0], interner.i32());
            }
            other => panic!("expected Tuple, got {other:?}"),
        }
    }

    #[test]
    fn test_struct_multiple_type_args() {
        let mut interner = TypeInterner::new();

        let struct_ty = interner.mk_struct(
            DefId(1),
            vec![interner.i32(), interner.bool(), interner.f64()],
        );

        match interner.get(struct_ty) {
            Type::Struct(def_id, args) => {
                assert_eq!(*def_id, DefId(1));
                assert_eq!(args.len(), 3);
                assert_eq!(args[0], interner.i32());
                assert_eq!(args[1], interner.bool());
                assert_eq!(args[2], interner.f64());
            }
            other => panic!("expected Struct, got {other:?}"),
        }
    }

    #[test]
    fn test_param_different_def_ids() {
        let mut interner = TypeInterner::new();

        let p1 = interner.mk_param(DefId(1));
        let p2 = interner.mk_param(DefId(2));
        let p3 = interner.mk_param(DefId(1));

        assert_ne!(p1, p2);
        assert_eq!(p1, p3);
    }

    #[test]
    fn test_error_type_interning_idempotent() {
        let interner = TypeInterner::new();

        // Error should always return the same pre-interned ID
        let e1 = interner.error();
        let e2 = interner.error();

        assert_eq!(e1, e2);
    }

    #[test]
    fn test_string_type_interning_idempotent() {
        let interner = TypeInterner::new();

        let s1 = interner.str_ref();
        let s2 = interner.str_ref();

        assert_eq!(s1, s2);
    }

    #[test]
    fn test_infer_kinds_are_different() {
        let mut interner = TypeInterner::new();

        // Even with same underlying TypeVar id, different InferKind = different type
        let ty1 = interner.intern(Type::Infer(TypeVar(100), InferKind::General));
        let ty2 = interner.intern(Type::Infer(TypeVar(100), InferKind::Int));
        let ty3 = interner.intern(Type::Infer(TypeVar(100), InferKind::Float));

        assert_ne!(ty1, ty2);
        assert_ne!(ty2, ty3);
        assert_ne!(ty1, ty3);
    }

    #[test]
    fn test_mutability_equality() {
        assert_eq!(Mutability::Shared, Mutability::Shared);
        assert_eq!(Mutability::Mutable, Mutability::Mutable);
        assert_ne!(Mutability::Shared, Mutability::Mutable);
    }

    #[test]
    fn test_primitive_kind_equality() {
        assert_eq!(PrimitiveKind::I32, PrimitiveKind::I32);
        assert_ne!(PrimitiveKind::I32, PrimitiveKind::I64);
        assert_ne!(PrimitiveKind::Bool, PrimitiveKind::Char);
    }

    #[test]
    fn test_interner_get_after_many_interns() {
        let mut interner = TypeInterner::new();

        // Intern many types and verify we can get them all back
        let mut ids = Vec::new();
        for i in 0..100 {
            let arr = interner.mk_array(interner.i32(), i);
            ids.push((arr, i));
        }

        for (id, expected_len) in ids {
            match interner.get(id) {
                Type::Array(_, len) => {
                    assert_eq!(*len, expected_len);
                }
                other => panic!("expected Array, got {other:?}"),
            }
        }
    }
}
