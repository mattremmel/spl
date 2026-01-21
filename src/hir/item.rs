//! HIR item representation.
//!
//! This module defines the HIR item types for functions, structs, etc.

use crate::lexer::Span;
use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;

use super::{ExprId, PatId};

/// HIR items (top-level definitions).
#[derive(Debug, Clone)]
pub enum HirItem {
    Function(HirFunction),
    Struct(HirStruct),
    TypeAlias(HirTypeAlias),
    Impl(HirImpl),
}

/// HIR function definition.
#[derive(Debug, Clone)]
pub struct HirFunction {
    pub def_id: DefId,
    pub name: String,
    pub type_params: Vec<DefId>,
    pub params: Vec<HirParam>,
    pub ret_type: TypeId,
    pub body: Option<ExprId>,
    pub span: Span,
}

/// HIR function parameter.
#[derive(Debug, Clone)]
pub struct HirParam {
    pub pat: PatId,
    pub ty: TypeId,
    pub span: Span,
}

/// HIR struct definition.
#[derive(Debug, Clone)]
pub struct HirStruct {
    pub def_id: DefId,
    pub name: String,
    pub type_params: Vec<DefId>,
    pub fields: Vec<HirField>,
    pub span: Span,
}

/// HIR struct field.
#[derive(Debug, Clone)]
pub struct HirField {
    pub def_id: DefId,
    pub name: String,
    pub ty: TypeId,
    pub span: Span,
}

/// HIR type alias.
#[derive(Debug, Clone)]
pub struct HirTypeAlias {
    pub def_id: DefId,
    pub name: String,
    pub type_params: Vec<DefId>,
    pub ty: TypeId,
    pub span: Span,
}

/// HIR impl block.
#[derive(Debug, Clone)]
pub struct HirImpl {
    pub type_params: Vec<DefId>,
    pub self_ty: TypeId,
    pub items: Vec<HirItem>,
    pub span: Span,
}
