//! Type layout computation for code generation.
//!
//! This module computes the memory layout of SPL types, including:
//! - Size and alignment for all types
//! - Field offsets for structs and tuples
//! - Element strides for arrays
//!
//! Uses simple C-like layout rules: fields laid out sequentially with alignment padding.

use cranelift_codegen::ir::Type as ClifType;

use crate::sema::types::{PrimitiveKind, Type, TypeId, TypeInterner};

/// The memory layout of a type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeLayout {
    /// Size in bytes.
    pub size: u32,
    /// Alignment in bytes (always a power of 2).
    pub align: u32,
}

impl TypeLayout {
    /// Create a new type layout.
    pub const fn new(size: u32, align: u32) -> Self {
        Self { size, align }
    }

    /// Layout for a zero-sized type.
    pub const fn zst() -> Self {
        Self { size: 0, align: 1 }
    }

    /// Round up `size` to the next multiple of `align`.
    fn align_to(size: u32, align: u32) -> u32 {
        (size + align - 1) & !(align - 1)
    }
}

/// Computes type layouts for code generation.
pub struct LayoutComputer<'a> {
    /// The type interner for type lookups.
    types: &'a TypeInterner,
    /// The pointer size in bytes (4 for 32-bit, 8 for 64-bit).
    pointer_size: u32,
}

impl<'a> LayoutComputer<'a> {
    /// Create a new layout computer.
    pub fn new(types: &'a TypeInterner, pointer_type: ClifType) -> Self {
        let pointer_size = pointer_type.bytes();
        Self {
            types,
            pointer_size,
        }
    }

    /// Compute the layout of a type.
    pub fn layout_of(&self, ty: TypeId) -> TypeLayout {
        let ty_data = self.types.get(ty);
        self.layout_of_inner(ty_data)
    }

    fn layout_of_inner(&self, ty: &Type) -> TypeLayout {
        match ty {
            Type::Primitive(prim) => self.primitive_layout(*prim),

            // References and pointers are pointer-sized
            Type::Ref(_, _) => TypeLayout::new(self.pointer_size, self.pointer_size),
            Type::RawPtr(_, _) => TypeLayout::new(self.pointer_size, self.pointer_size),
            Type::FnPtr { .. } => TypeLayout::new(self.pointer_size, self.pointer_size),

            // Arrays: element_size * count, element alignment
            Type::Array(elem_ty, count) => {
                if *count == 0 {
                    return TypeLayout::zst();
                }
                let elem_layout = self.layout_of(*elem_ty);
                if elem_layout.size == 0 {
                    return TypeLayout::zst();
                }
                let stride = TypeLayout::align_to(elem_layout.size, elem_layout.align);
                let size = stride * (*count as u32);
                TypeLayout::new(size, elem_layout.align)
            }

            // Tuples: sequential layout with alignment
            Type::Tuple(elems) => {
                if elems.is_empty() {
                    return TypeLayout::zst();
                }
                self.compute_struct_layout(elems)
            }

            // Structs: sequential layout with alignment
            // Note: We don't have field type info here yet, so struct layout
            // would need additional infrastructure. For now, treat as opaque.
            Type::Struct(_, fields) => {
                if fields.is_empty() {
                    return TypeLayout::zst();
                }
                self.compute_struct_layout(fields)
            }

            // Slices are unsized, but &[T] is a fat pointer (2 pointers)
            Type::Slice(_) => TypeLayout::new(self.pointer_size * 2, self.pointer_size),

            // StrRef is pointer + length (like Rust's &str)
            Type::StrRef => TypeLayout::new(self.pointer_size * 2, self.pointer_size),

            // These should not reach layout computation
            Type::Infer(_, _) | Type::Param(_) | Type::SelfType | Type::Alias(_, _) => {
                TypeLayout::zst()
            }

            // Error type
            Type::Error => TypeLayout::zst(),
        }
    }

    /// Compute the layout of a struct/tuple given its field types.
    fn compute_struct_layout(&self, field_types: &[TypeId]) -> TypeLayout {
        let mut size: u32 = 0;
        let mut align: u32 = 1;

        for &field_ty in field_types {
            let field_layout = self.layout_of(field_ty);

            // Align the current offset for this field
            size = TypeLayout::align_to(size, field_layout.align);

            // Add the field's size
            size += field_layout.size;

            // Update overall alignment to be the max of all field alignments
            align = align.max(field_layout.align);
        }

        // Final size must be a multiple of alignment (for arrays of this type)
        size = TypeLayout::align_to(size, align);

        TypeLayout::new(size, align)
    }

    /// Compute the offset of a field in a struct/tuple.
    pub fn field_offset(&self, ty: TypeId, field_idx: usize) -> u32 {
        let ty_data = self.types.get(ty);
        match ty_data {
            Type::Tuple(elems) => self.compute_field_offset(elems, field_idx),
            Type::Struct(_, fields) => self.compute_field_offset(fields, field_idx),
            // StrRef is a fat pointer: [ptr, len], both pointer-sized
            Type::StrRef => {
                if field_idx == 0 {
                    0 // ptr at offset 0
                } else if field_idx == 1 {
                    self.pointer_size // len at offset pointer_size
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn compute_field_offset(&self, field_types: &[TypeId], field_idx: usize) -> u32 {
        let mut offset: u32 = 0;

        for (i, &field_ty) in field_types.iter().enumerate() {
            let field_layout = self.layout_of(field_ty);

            // Align the current offset for this field
            offset = TypeLayout::align_to(offset, field_layout.align);

            if i == field_idx {
                return offset;
            }

            // Add the field's size
            offset += field_layout.size;
        }

        // Should not reach here if field_idx is valid
        offset
    }

    /// Get the type of a field in a struct/tuple.
    pub fn field_type(&self, ty: TypeId, field_idx: usize) -> Option<TypeId> {
        let ty_data = self.types.get(ty);
        match ty_data {
            Type::Tuple(elems) => elems.get(field_idx).copied(),
            Type::Struct(_, fields) => fields.get(field_idx).copied(),
            // StrRef fields are both i64 (ptr and len)
            Type::StrRef => {
                if field_idx < 2 {
                    Some(self.types.i64())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Compute the stride (element size with alignment) for an array.
    pub fn element_stride(&self, ty: TypeId) -> u32 {
        let ty_data = self.types.get(ty);
        match ty_data {
            Type::Array(elem_ty, _) => {
                let elem_layout = self.layout_of(*elem_ty);
                TypeLayout::align_to(elem_layout.size, elem_layout.align)
            }
            _ => 0,
        }
    }

    /// Get the element type of an array.
    pub fn element_type(&self, ty: TypeId) -> Option<TypeId> {
        let ty_data = self.types.get(ty);
        match ty_data {
            Type::Array(elem_ty, _) => Some(*elem_ty),
            Type::Slice(elem_ty) => Some(*elem_ty),
            _ => None,
        }
    }

    /// Get the pointee type of a reference or pointer.
    pub fn pointee_type(&self, ty: TypeId) -> Option<TypeId> {
        let ty_data = self.types.get(ty);
        match ty_data {
            Type::Ref(_, inner) => Some(*inner),
            _ => None,
        }
    }

    /// Layout for primitive types.
    fn primitive_layout(&self, prim: PrimitiveKind) -> TypeLayout {
        match prim {
            // Signed integers
            PrimitiveKind::I8 => TypeLayout::new(1, 1),
            PrimitiveKind::I16 => TypeLayout::new(2, 2),
            PrimitiveKind::I32 => TypeLayout::new(4, 4),
            PrimitiveKind::I64 => TypeLayout::new(8, 8),
            PrimitiveKind::I128 => TypeLayout::new(16, 16),
            PrimitiveKind::Isize => TypeLayout::new(self.pointer_size, self.pointer_size),

            // Unsigned integers (same sizes as signed)
            PrimitiveKind::U8 => TypeLayout::new(1, 1),
            PrimitiveKind::U16 => TypeLayout::new(2, 2),
            PrimitiveKind::U32 => TypeLayout::new(4, 4),
            PrimitiveKind::U64 => TypeLayout::new(8, 8),
            PrimitiveKind::U128 => TypeLayout::new(16, 16),
            PrimitiveKind::Usize => TypeLayout::new(self.pointer_size, self.pointer_size),

            // Floating point
            PrimitiveKind::F32 => TypeLayout::new(4, 4),
            PrimitiveKind::F64 => TypeLayout::new(8, 8),

            // Bool is 1 byte
            PrimitiveKind::Bool => TypeLayout::new(1, 1),

            // Char is 4 bytes (Unicode scalar)
            PrimitiveKind::Char => TypeLayout::new(4, 4),

            // ZSTs
            PrimitiveKind::Unit => TypeLayout::zst(),
            PrimitiveKind::Never => TypeLayout::zst(),

            // Str is unsized
            PrimitiveKind::Str => TypeLayout::zst(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    #[test]
    fn primitive_layouts() {
        let mut interner = TypeInterner::new();
        // Pre-intern all types we need before creating the computer
        let i8_ty = interner.primitive(PrimitiveKind::I8);
        let i16_ty = interner.primitive(PrimitiveKind::I16);
        let i32_ty = interner.primitive(PrimitiveKind::I32);
        let i64_ty = interner.primitive(PrimitiveKind::I64);
        let bool_ty = interner.bool();
        let char_ty = interner.primitive(PrimitiveKind::Char);
        let unit_ty = interner.unit();
        let never_ty = interner.never();

        let computer = LayoutComputer::new(&interner, types::I64);

        // Check integer layouts
        assert_eq!(computer.layout_of(i8_ty), TypeLayout::new(1, 1));
        assert_eq!(computer.layout_of(i16_ty), TypeLayout::new(2, 2));
        assert_eq!(computer.layout_of(i32_ty), TypeLayout::new(4, 4));
        assert_eq!(computer.layout_of(i64_ty), TypeLayout::new(8, 8));

        // Check bool and char
        assert_eq!(computer.layout_of(bool_ty), TypeLayout::new(1, 1));
        assert_eq!(computer.layout_of(char_ty), TypeLayout::new(4, 4));

        // Check ZSTs
        assert_eq!(computer.layout_of(unit_ty), TypeLayout::zst());
        assert_eq!(computer.layout_of(never_ty), TypeLayout::zst());
    }

    #[test]
    fn pointer_layouts_64bit() {
        let mut interner = TypeInterner::new();
        use crate::sema::types::Mutability;
        let ref_ty = interner.mk_ref(Mutability::Shared, interner.i32());

        let computer = LayoutComputer::new(&interner, types::I64);
        assert_eq!(computer.layout_of(ref_ty), TypeLayout::new(8, 8));
    }

    #[test]
    fn pointer_layouts_32bit() {
        let mut interner = TypeInterner::new();
        use crate::sema::types::Mutability;
        let ref_ty = interner.mk_ref(Mutability::Shared, interner.i32());

        let computer = LayoutComputer::new(&interner, types::I32);
        assert_eq!(computer.layout_of(ref_ty), TypeLayout::new(4, 4));
    }

    #[test]
    fn array_layout() {
        let mut interner = TypeInterner::new();
        // Create types before computer
        let arr = interner.mk_array(interner.i32(), 4);
        let arr64 = interner.mk_array(interner.i64(), 3);
        let empty_arr = interner.mk_array(interner.i32(), 0);

        let computer = LayoutComputer::new(&interner, types::I64);

        // [i32; 4] = 16 bytes, align 4
        assert_eq!(computer.layout_of(arr), TypeLayout::new(16, 4));

        // [i64; 3] = 24 bytes, align 8
        assert_eq!(computer.layout_of(arr64), TypeLayout::new(24, 8));

        // Zero-length array is ZST
        assert_eq!(computer.layout_of(empty_arr), TypeLayout::zst());
    }

    #[test]
    fn tuple_layout() {
        let mut interner = TypeInterner::new();
        // Create types before computer
        let i8_ty = interner.primitive(PrimitiveKind::I8);
        let i32_ty = interner.i32();
        let i64_ty = interner.i64();
        let tuple = interner.mk_tuple(vec![i32_ty, i32_ty]);
        let tuple2 = interner.mk_tuple(vec![i32_ty, i64_ty]);
        let tuple3 = interner.mk_tuple(vec![i8_ty, i32_ty, i8_ty]);
        let empty = interner.mk_tuple(vec![]);

        let computer = LayoutComputer::new(&interner, types::I64);

        // (i32, i32) = 8 bytes, align 4
        assert_eq!(computer.layout_of(tuple), TypeLayout::new(8, 4));

        // (i32, i64) = 16 bytes (4 + 4 padding + 8), align 8
        assert_eq!(computer.layout_of(tuple2), TypeLayout::new(16, 8));

        // (i8, i32, i8) = 12 bytes (1 + 3 padding + 4 + 1 + 3 padding), align 4
        assert_eq!(computer.layout_of(tuple3), TypeLayout::new(12, 4));

        // Empty tuple is ZST
        assert_eq!(computer.layout_of(empty), TypeLayout::zst());
    }

    #[test]
    fn field_offset_simple() {
        let mut interner = TypeInterner::new();
        let tuple = interner.mk_tuple(vec![interner.i32(), interner.i32()]);

        let computer = LayoutComputer::new(&interner, types::I64);

        // (i32, i32): offsets 0, 4
        assert_eq!(computer.field_offset(tuple, 0), 0);
        assert_eq!(computer.field_offset(tuple, 1), 4);
    }

    #[test]
    fn field_offset_with_padding() {
        let mut interner = TypeInterner::new();
        let i8_ty = interner.primitive(PrimitiveKind::I8);
        let i64_ty = interner.i64();
        let tuple = interner.mk_tuple(vec![i8_ty, i64_ty]);

        let computer = LayoutComputer::new(&interner, types::I64);

        // (i8, i64): offsets 0, 8 (7 bytes padding after i8)
        assert_eq!(computer.field_offset(tuple, 0), 0);
        assert_eq!(computer.field_offset(tuple, 1), 8);
    }

    #[test]
    fn element_stride() {
        let mut interner = TypeInterner::new();
        let arr = interner.mk_array(interner.i32(), 10);
        let arr64 = interner.mk_array(interner.i64(), 5);

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.element_stride(arr), 4);
        assert_eq!(computer.element_stride(arr64), 8);
    }

    #[test]
    fn field_type_lookup() {
        let mut interner = TypeInterner::new();
        let i32_ty = interner.i32();
        let bool_ty = interner.bool();
        let tuple = interner.mk_tuple(vec![i32_ty, bool_ty]);

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.field_type(tuple, 0), Some(i32_ty));
        assert_eq!(computer.field_type(tuple, 1), Some(bool_ty));
        assert_eq!(computer.field_type(tuple, 2), None);
    }

    #[test]
    fn element_type_lookup() {
        let mut interner = TypeInterner::new();
        let i32_ty = interner.i32();
        let bool_ty = interner.bool();
        let arr = interner.mk_array(i32_ty, 5);
        let slice = interner.mk_slice(bool_ty);

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.element_type(arr), Some(i32_ty));
        assert_eq!(computer.element_type(slice), Some(bool_ty));
    }

    #[test]
    fn pointee_type_lookup() {
        let mut interner = TypeInterner::new();
        use crate::sema::types::Mutability;
        let i32_ty = interner.i32();
        let ref_ty = interner.mk_ref(Mutability::Shared, i32_ty);

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.pointee_type(ref_ty), Some(i32_ty));
        assert_eq!(computer.pointee_type(i32_ty), None);
    }

    #[test]
    fn align_to_helper() {
        assert_eq!(TypeLayout::align_to(0, 4), 0);
        assert_eq!(TypeLayout::align_to(1, 4), 4);
        assert_eq!(TypeLayout::align_to(4, 4), 4);
        assert_eq!(TypeLayout::align_to(5, 4), 8);
        assert_eq!(TypeLayout::align_to(7, 8), 8);
        assert_eq!(TypeLayout::align_to(8, 8), 8);
        assert_eq!(TypeLayout::align_to(9, 8), 16);
    }

    #[test]
    fn isize_usize_layout_64bit() {
        let mut interner = TypeInterner::new();
        let isize_ty = interner.primitive(PrimitiveKind::Isize);
        let usize_ty = interner.primitive(PrimitiveKind::Usize);

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.layout_of(isize_ty), TypeLayout::new(8, 8));
        assert_eq!(computer.layout_of(usize_ty), TypeLayout::new(8, 8));
    }

    #[test]
    fn isize_usize_layout_32bit() {
        let mut interner = TypeInterner::new();
        let isize_ty = interner.primitive(PrimitiveKind::Isize);
        let usize_ty = interner.primitive(PrimitiveKind::Usize);

        let computer = LayoutComputer::new(&interner, types::I32);

        assert_eq!(computer.layout_of(isize_ty), TypeLayout::new(4, 4));
        assert_eq!(computer.layout_of(usize_ty), TypeLayout::new(4, 4));
    }

    #[test]
    fn fn_ptr_layout() {
        let mut interner = TypeInterner::new();
        let fn_ptr = interner.mk_fn_ptr(vec![interner.i32()], interner.bool());

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.layout_of(fn_ptr), TypeLayout::new(8, 8));
    }

    #[test]
    fn array_of_zst() {
        let mut interner = TypeInterner::new();
        let arr = interner.mk_array(interner.unit(), 100);

        let computer = LayoutComputer::new(&interner, types::I64);

        // Array of unit is ZST regardless of length
        assert_eq!(computer.layout_of(arr), TypeLayout::zst());
    }

    #[test]
    fn nested_tuple_layout() {
        let mut interner = TypeInterner::new();
        // ((i32, i32), i64): inner is 8 bytes align 4, so layout is:
        // 8 bytes for inner tuple + 8 bytes for i64 = 16 bytes, align 8
        let inner = interner.mk_tuple(vec![interner.i32(), interner.i32()]);
        let outer = interner.mk_tuple(vec![inner, interner.i64()]);

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.layout_of(outer), TypeLayout::new(16, 8));
    }

    #[test]
    fn float_layouts() {
        let interner = TypeInterner::new();
        let f32_ty = interner.f32();
        let f64_ty = interner.f64();

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.layout_of(f32_ty), TypeLayout::new(4, 4));
        assert_eq!(computer.layout_of(f64_ty), TypeLayout::new(8, 8));
    }

    #[test]
    fn i128_layout() {
        let mut interner = TypeInterner::new();
        let i128_ty = interner.primitive(PrimitiveKind::I128);
        let u128_ty = interner.primitive(PrimitiveKind::U128);

        let computer = LayoutComputer::new(&interner, types::I64);

        assert_eq!(computer.layout_of(i128_ty), TypeLayout::new(16, 16));
        assert_eq!(computer.layout_of(u128_ty), TypeLayout::new(16, 16));
    }
}
