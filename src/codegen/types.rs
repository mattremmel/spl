//! Type mapping from SPL types to Cranelift types.
//!
//! This module handles the translation of SPL semantic types (`TypeId`) to
//! Cranelift IR types. Some types map directly to Cranelift types, while
//! others (like compound types) require stack allocation.

use cranelift_codegen::ir::Type as ClifType;
use cranelift_codegen::ir::types;

use crate::sema::types::{PrimitiveKind, Type, TypeId, TypeInterner};

/// Maps SPL types to Cranelift types.
///
/// Not all SPL types have direct Cranelift representations:
/// - Primitive scalar types map to Cranelift integer/float types
/// - ZSTs (unit, never) have no runtime representation
/// - Compound types (arrays, structs, tuples) are handled via stack slots
pub struct TypeMapper {
    /// The pointer type for the current target (I32 or I64).
    pointer_type: ClifType,
}

impl TypeMapper {
    /// Create a new type mapper for the given pointer type.
    pub fn new(pointer_type: ClifType) -> Self {
        Self { pointer_type }
    }

    /// Map an SPL type to a Cranelift type.
    ///
    /// Returns `Some(type)` for types that can be represented in registers,
    /// or `None` for ZSTs and compound types that require stack allocation.
    pub fn map_type(&self, type_id: TypeId, interner: &TypeInterner) -> Option<ClifType> {
        let ty = interner.get(type_id);
        self.map_type_inner(ty, interner)
    }

    fn map_type_inner(&self, ty: &Type, _interner: &TypeInterner) -> Option<ClifType> {
        match ty {
            Type::Primitive(prim) => self.map_primitive(*prim),

            // References and function pointers are pointer-sized
            Type::Ref(_, _) => Some(self.pointer_type),
            Type::FnPtr { .. } => Some(self.pointer_type),

            // Infer types should be resolved before codegen
            Type::Infer(_, _) => None,

            // Compound types - require stack allocation
            Type::Array(_, _) => None,
            Type::Slice(_) => None,
            Type::Tuple(elems) => {
                // Empty tuple is unit (ZST)
                if elems.is_empty() {
                    None
                } else {
                    // Non-empty tuples are compound
                    None
                }
            }
            Type::Struct(_, _) => None,
            Type::Alias(_, _) => None,

            // Type parameters should be monomorphized before codegen
            Type::Param(_) => None,

            // Self type should be resolved before codegen
            Type::SelfType => None,

            // String is a compound type (ptr + len)
            Type::String => None,

            // Error types should not reach codegen
            Type::Error => None,
        }
    }

    /// Map a primitive type to a Cranelift type.
    fn map_primitive(&self, prim: PrimitiveKind) -> Option<ClifType> {
        match prim {
            // Signed integers
            PrimitiveKind::I8 => Some(types::I8),
            PrimitiveKind::I16 => Some(types::I16),
            PrimitiveKind::I32 => Some(types::I32),
            PrimitiveKind::I64 => Some(types::I64),
            PrimitiveKind::I128 => Some(types::I128),
            PrimitiveKind::Isize => Some(self.pointer_type),

            // Unsigned integers (same Cranelift types as signed)
            PrimitiveKind::U8 => Some(types::I8),
            PrimitiveKind::U16 => Some(types::I16),
            PrimitiveKind::U32 => Some(types::I32),
            PrimitiveKind::U64 => Some(types::I64),
            PrimitiveKind::U128 => Some(types::I128),
            PrimitiveKind::Usize => Some(self.pointer_type),

            // Floating point
            PrimitiveKind::F32 => Some(types::F32),
            PrimitiveKind::F64 => Some(types::F64),

            // Bool is represented as I8 (0 or 1)
            PrimitiveKind::Bool => Some(types::I8),

            // Char is represented as I32 (Unicode scalar value)
            PrimitiveKind::Char => Some(types::I32),

            // ZSTs have no runtime representation
            PrimitiveKind::Unit => None,
            PrimitiveKind::Never => None,

            // Str is unsized, cannot be mapped directly
            PrimitiveKind::Str => None,
        }
    }

    /// Check if a type is a zero-sized type (ZST).
    ///
    /// ZSTs have no runtime representation and don't need storage.
    pub fn is_zst(&self, type_id: TypeId, interner: &TypeInterner) -> bool {
        let ty = interner.get(type_id);
        self.is_zst_inner(ty, interner)
    }

    fn is_zst_inner(&self, ty: &Type, interner: &TypeInterner) -> bool {
        match ty {
            Type::Primitive(PrimitiveKind::Unit) => true,
            Type::Primitive(PrimitiveKind::Never) => true,

            // Empty tuple is ZST
            Type::Tuple(elems) if elems.is_empty() => true,

            // Zero-length array is ZST
            Type::Array(_, 0) => true,

            // Array of ZSTs is ZST
            Type::Array(elem, _) => self.is_zst(*elem, interner),

            // Tuple of all ZSTs is ZST
            Type::Tuple(elems) => elems.iter().all(|e| self.is_zst(*e, interner)),

            // Everything else is not a ZST
            _ => false,
        }
    }

    /// Get the pointer type for the current target.
    pub fn pointer_type(&self) -> ClifType {
        self.pointer_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapper_64() -> TypeMapper {
        TypeMapper::new(types::I64)
    }

    fn mapper_32() -> TypeMapper {
        TypeMapper::new(types::I32)
    }

    #[test]
    fn map_signed_integers() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::I8), &interner),
            Some(types::I8)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::I16), &interner),
            Some(types::I16)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::I32), &interner),
            Some(types::I32)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::I64), &interner),
            Some(types::I64)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::I128), &interner),
            Some(types::I128)
        );
    }

    #[test]
    fn map_unsigned_integers() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::U8), &interner),
            Some(types::I8)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::U16), &interner),
            Some(types::I16)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::U32), &interner),
            Some(types::I32)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::U64), &interner),
            Some(types::I64)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::U128), &interner),
            Some(types::I128)
        );
    }

    #[test]
    fn map_isize_usize_64bit() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::Isize), &interner),
            Some(types::I64)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::Usize), &interner),
            Some(types::I64)
        );
    }

    #[test]
    fn map_isize_usize_32bit() {
        let mapper = mapper_32();
        let mut interner = TypeInterner::new();

        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::Isize), &interner),
            Some(types::I32)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::Usize), &interner),
            Some(types::I32)
        );
    }

    #[test]
    fn map_floats() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::F32), &interner),
            Some(types::F32)
        );
        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::F64), &interner),
            Some(types::F64)
        );
    }

    #[test]
    fn map_bool() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::Bool), &interner),
            Some(types::I8)
        );
    }

    #[test]
    fn map_char() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        assert_eq!(
            mapper.map_type(interner.primitive(PrimitiveKind::Char), &interner),
            Some(types::I32)
        );
    }

    #[test]
    fn map_unit_is_none() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(mapper.map_type(interner.unit(), &interner), None);
    }

    #[test]
    fn map_never_is_none() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(mapper.map_type(interner.never(), &interner), None);
    }

    #[test]
    fn map_reference_is_pointer() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        use crate::sema::types::Mutability;
        let ref_ty = interner.mk_ref(Mutability::Shared, interner.i32());
        let mut_ref_ty = interner.mk_ref(Mutability::Mutable, interner.i32());

        assert_eq!(mapper.map_type(ref_ty, &interner), Some(types::I64));
        assert_eq!(mapper.map_type(mut_ref_ty, &interner), Some(types::I64));
    }

    #[test]
    fn map_fn_ptr_is_pointer() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let fn_ptr = interner.mk_fn_ptr(vec![interner.i32()], interner.bool());
        assert_eq!(mapper.map_type(fn_ptr, &interner), Some(types::I64));
    }

    #[test]
    fn map_array_is_none() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let arr = interner.mk_array(interner.i32(), 10);
        assert_eq!(mapper.map_type(arr, &interner), None);
    }

    #[test]
    fn map_slice_is_none() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let slice = interner.mk_slice(interner.i32());
        assert_eq!(mapper.map_type(slice, &interner), None);
    }

    #[test]
    fn map_tuple_is_none() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let tuple = interner.mk_tuple(vec![interner.i32(), interner.bool()]);
        assert_eq!(mapper.map_type(tuple, &interner), None);
    }

    #[test]
    fn map_empty_tuple_is_none() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let unit_tuple = interner.mk_tuple(vec![]);
        assert_eq!(mapper.map_type(unit_tuple, &interner), None);
    }

    #[test]
    fn map_struct_is_none() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        use crate::sema::DefId;
        let struct_ty = interner.mk_struct(DefId(0), vec![]);
        assert_eq!(mapper.map_type(struct_ty, &interner), None);
    }

    #[test]
    fn map_string_is_none() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(mapper.map_type(interner.string(), &interner), None);
    }

    #[test]
    fn map_str_is_none() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(mapper.map_type(interner.str(), &interner), None);
    }

    #[test]
    fn map_error_is_none() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(mapper.map_type(interner.error(), &interner), None);
    }

    // ZST tests

    #[test]
    fn is_zst_unit() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert!(mapper.is_zst(interner.unit(), &interner));
    }

    #[test]
    fn is_zst_never() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert!(mapper.is_zst(interner.never(), &interner));
    }

    #[test]
    fn is_zst_empty_tuple() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let empty_tuple = interner.mk_tuple(vec![]);
        assert!(mapper.is_zst(empty_tuple, &interner));
    }

    #[test]
    fn is_zst_zero_len_array() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let arr = interner.mk_array(interner.i32(), 0);
        assert!(mapper.is_zst(arr, &interner));
    }

    #[test]
    fn is_zst_array_of_zst() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let arr_of_unit = interner.mk_array(interner.unit(), 100);
        assert!(mapper.is_zst(arr_of_unit, &interner));
    }

    #[test]
    fn is_zst_tuple_of_zsts() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let tuple = interner.mk_tuple(vec![interner.unit(), interner.never()]);
        assert!(mapper.is_zst(tuple, &interner));
    }

    #[test]
    fn is_not_zst_i32() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert!(!mapper.is_zst(interner.i32(), &interner));
    }

    #[test]
    fn is_not_zst_nonempty_array() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let arr = interner.mk_array(interner.i32(), 10);
        assert!(!mapper.is_zst(arr, &interner));
    }

    #[test]
    fn is_not_zst_tuple_with_nonzst() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let tuple = interner.mk_tuple(vec![interner.unit(), interner.i32()]);
        assert!(!mapper.is_zst(tuple, &interner));
    }

    #[test]
    fn pointer_type_accessor() {
        let mapper_64 = mapper_64();
        let mapper_32 = mapper_32();

        assert_eq!(mapper_64.pointer_type(), types::I64);
        assert_eq!(mapper_32.pointer_type(), types::I32);
    }
}
