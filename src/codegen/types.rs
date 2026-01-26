//! Type mapping from SPL types to Cranelift types.
//!
//! This module handles the translation of SPL semantic types (`TypeId`) to
//! Cranelift IR types. Some types map directly to Cranelift types, while
//! others (like compound types) require stack allocation.

use cranelift_codegen::ir::AbiParam;
use cranelift_codegen::ir::Type as ClifType;
use cranelift_codegen::ir::types;

use crate::sema::types::{PrimitiveKind, Type, TypeId, TypeInterner};

/// How a type is represented at ABI boundaries.
///
/// This abstraction generalizes handling of different type categories
/// for function signatures, call lowering, and argument passing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbiRepr {
    /// Single register value (primitives, thin pointers).
    Scalar(ClifType),
    /// Zero-sized type, no runtime representation.
    Zst,
    /// Fat pointer: multiple pointer-sized fields (`StrRef`, slices).
    /// `num_fields` indicates how many pointer-sized values make up this type.
    FatPointer { num_fields: usize },
    /// Passed by reference (large structs, arrays).
    /// Not yet implemented, reserved for future use.
    Indirect,
}

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

            // References, raw pointers, and function pointers are pointer-sized
            Type::Ref(_, _) | Type::RawPtr(_, _) | Type::FnPtr { .. } => Some(self.pointer_type),

            // All other types return None:
            // - Infer types should be resolved before codegen
            // - Compound types (Array, Slice, Tuple, Struct, Alias) require stack allocation
            // - Type parameters should be monomorphized before codegen
            // - Self type should be resolved before codegen
            // - StrRef is a compound type (ptr + len), like Rust's &str
            // - Error types should not reach codegen
            // - Module types are for namespace access, not runtime values
            Type::Infer(_, _)
            | Type::Array(_, _)
            | Type::Slice(_)
            | Type::Tuple(_)
            | Type::Struct(_, _)
            | Type::Alias(_, _)
            | Type::Param(_)
            | Type::SelfType
            | Type::StrRef
            | Type::Error
            | Type::Module(_) => None,
        }
    }

    /// Map a primitive type to a Cranelift type.
    fn map_primitive(&self, prim: PrimitiveKind) -> Option<ClifType> {
        match prim {
            // 1-byte types
            PrimitiveKind::I8 | PrimitiveKind::U8 | PrimitiveKind::Bool => Some(types::I8),

            // 2-byte types
            PrimitiveKind::I16 | PrimitiveKind::U16 => Some(types::I16),

            // 4-byte types (Char is Unicode scalar value)
            PrimitiveKind::I32 | PrimitiveKind::U32 | PrimitiveKind::Char => Some(types::I32),

            // 8-byte types
            PrimitiveKind::I64 | PrimitiveKind::U64 => Some(types::I64),

            // 16-byte types
            PrimitiveKind::I128 | PrimitiveKind::U128 => Some(types::I128),

            // Pointer-sized types
            PrimitiveKind::Isize | PrimitiveKind::Usize => Some(self.pointer_type),

            // Floating point
            PrimitiveKind::F32 => Some(types::F32),
            PrimitiveKind::F64 => Some(types::F64),

            // ZSTs have no runtime representation; Str is unsized
            PrimitiveKind::Unit | PrimitiveKind::Never | PrimitiveKind::Str => None,
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
            // Unit, Never, and zero-length array are all ZSTs
            Type::Primitive(PrimitiveKind::Unit | PrimitiveKind::Never) | Type::Array(_, 0) => true,

            // Empty tuple is a ZST
            Type::Tuple(elems) if elems.is_empty() => true,

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

    /// Get the ABI representation for a type.
    ///
    /// This determines how a type should be passed at function call boundaries:
    /// - `Scalar`: passed in a single register
    /// - `Zst`: not passed at all (zero-sized)
    /// - `FatPointer`: passed as multiple pointer-sized values
    /// - `Indirect`: passed by reference (not yet implemented)
    pub fn abi_repr(&self, type_id: TypeId, interner: &TypeInterner) -> AbiRepr {
        // Check if it maps to a scalar type first
        if let Some(clif_ty) = self.map_type(type_id, interner) {
            return AbiRepr::Scalar(clif_ty);
        }

        // Check if it's a ZST
        if self.is_zst(type_id, interner) {
            return AbiRepr::Zst;
        }

        // Check for fat pointer types
        let ty = interner.get(type_id);
        match ty {
            Type::StrRef | Type::Slice(_) => AbiRepr::FatPointer { num_fields: 2 },
            // Everything else that's not scalar or ZST is passed indirectly
            _ => AbiRepr::Indirect,
        }
    }

    /// Get ABI parameters for a type in function signatures.
    ///
    /// Returns the list of `AbiParam` values needed to represent this type
    /// in a Cranelift function signature.
    pub fn abi_params(&self, type_id: TypeId, interner: &TypeInterner) -> Vec<AbiParam> {
        match self.abi_repr(type_id, interner) {
            AbiRepr::Scalar(clif_ty) => vec![AbiParam::new(clif_ty)],
            AbiRepr::Zst => vec![],
            AbiRepr::FatPointer { num_fields } => {
                // Each field is pointer-sized
                (0..num_fields)
                    .map(|_| AbiParam::new(self.pointer_type))
                    .collect()
            }
            AbiRepr::Indirect => {
                // Passed as a pointer to the value
                vec![AbiParam::new(self.pointer_type)]
            }
        }
    }

    /// Check if a type is a fat pointer (`StrRef`, slice, etc.).
    ///
    /// Fat pointers are compound types that are passed as multiple
    /// pointer-sized values at the ABI level.
    pub fn is_fat_pointer(&self, type_id: TypeId, interner: &TypeInterner) -> bool {
        matches!(self.abi_repr(type_id, interner), AbiRepr::FatPointer { .. })
    }
}

/// Build a Cranelift signature for a MIR body.
///
/// This is the shared implementation used by both JIT and AOT compilers.
/// It handles all type representations including fat pointers.
pub fn build_signature(
    call_conv: cranelift_codegen::isa::CallConv,
    type_mapper: &TypeMapper,
    body: &crate::mir::body::Body,
    types: &TypeInterner,
) -> cranelift_codegen::ir::Signature {
    let mut sig = cranelift_codegen::ir::Signature::new(call_conv);

    // Add return type parameters
    for param in type_mapper.abi_params(body.return_ty(), types) {
        sig.returns.push(param);
    }

    // Add argument parameters
    for arg in body.args() {
        for param in type_mapper.abi_params(body.local_decl(arg).ty, types) {
            sig.params.push(param);
        }
    }

    sig
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
        let struct_ty = interner.mk_struct(DefId::new(0), vec![]);
        assert_eq!(mapper.map_type(struct_ty, &interner), None);
    }

    #[test]
    fn map_str_ref_is_none() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(mapper.map_type(interner.str_ref(), &interner), None);
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

    // ABI representation tests

    #[test]
    fn abi_repr_scalar() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(
            mapper.abi_repr(interner.i32(), &interner),
            AbiRepr::Scalar(types::I32)
        );
        assert_eq!(
            mapper.abi_repr(interner.bool(), &interner),
            AbiRepr::Scalar(types::I8)
        );
    }

    #[test]
    fn abi_repr_zst() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(mapper.abi_repr(interner.unit(), &interner), AbiRepr::Zst);
        assert_eq!(mapper.abi_repr(interner.never(), &interner), AbiRepr::Zst);
    }

    #[test]
    fn abi_repr_fat_pointer_strref() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert_eq!(
            mapper.abi_repr(interner.str_ref(), &interner),
            AbiRepr::FatPointer { num_fields: 2 }
        );
    }

    #[test]
    fn abi_repr_fat_pointer_slice() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let slice = interner.mk_slice(interner.i32());
        assert_eq!(
            mapper.abi_repr(slice, &interner),
            AbiRepr::FatPointer { num_fields: 2 }
        );
    }

    #[test]
    fn abi_repr_indirect_array() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let arr = interner.mk_array(interner.i32(), 10);
        assert_eq!(mapper.abi_repr(arr, &interner), AbiRepr::Indirect);
    }

    #[test]
    fn abi_params_scalar() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        let params = mapper.abi_params(interner.i32(), &interner);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].value_type, types::I32);
    }

    #[test]
    fn abi_params_zst_empty() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        let params = mapper.abi_params(interner.unit(), &interner);
        assert!(params.is_empty());
    }

    #[test]
    fn abi_params_strref_two_pointers() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        let params = mapper.abi_params(interner.str_ref(), &interner);
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].value_type, types::I64);
        assert_eq!(params[1].value_type, types::I64);
    }

    #[test]
    fn abi_params_strref_32bit() {
        let mapper = mapper_32();
        let interner = TypeInterner::new();

        let params = mapper.abi_params(interner.str_ref(), &interner);
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].value_type, types::I32);
        assert_eq!(params[1].value_type, types::I32);
    }

    #[test]
    fn is_fat_pointer_strref() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert!(mapper.is_fat_pointer(interner.str_ref(), &interner));
    }

    #[test]
    fn is_fat_pointer_slice() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        let slice = interner.mk_slice(interner.i32());
        assert!(mapper.is_fat_pointer(slice, &interner));
    }

    #[test]
    fn is_not_fat_pointer_scalar() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert!(!mapper.is_fat_pointer(interner.i32(), &interner));
        assert!(!mapper.is_fat_pointer(interner.bool(), &interner));
    }

    #[test]
    fn is_not_fat_pointer_zst() {
        let mapper = mapper_64();
        let interner = TypeInterner::new();

        assert!(!mapper.is_fat_pointer(interner.unit(), &interner));
    }

    #[test]
    fn is_not_fat_pointer_reference() {
        let mapper = mapper_64();
        let mut interner = TypeInterner::new();

        use crate::sema::types::Mutability;
        let ref_ty = interner.mk_ref(Mutability::Shared, interner.i32());
        assert!(!mapper.is_fat_pointer(ref_ty, &interner));
    }
}
