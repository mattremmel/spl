//! MIR pretty-printer for debugging.
//!
//! Provides a human-readable representation of MIR for debugging purposes.
//! Output format is similar to rustc's MIR dump.

use crate::mir::body::{BasicBlockData, Body};
use crate::mir::operand::{
    AggregateKind, BinOp, BorrowKind, CastKind, Constant, Operand, Rvalue, UnOp,
};
use crate::mir::statement::{Statement, StatementKind};
use crate::mir::terminator::{BasicBlock, Terminator, TerminatorKind};
use crate::mir::types::{FieldIdx, Local, Place, PlaceElem};
use crate::sema::types::Mutability;
use std::fmt::Write;

/// Pretty-printer for MIR.
pub struct MirPrinter {
    output: String,
    indent: usize,
}

impl Default for MirPrinter {
    fn default() -> Self {
        Self::new()
    }
}

impl MirPrinter {
    /// Create a new printer.
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    /// Consume the printer and return the output.
    pub fn finish(self) -> String {
        self.output
    }

    // === Primitives ===

    /// Format a local variable.
    pub fn print_local(&self, local: Local) -> String {
        format!("_{}", local.0)
    }

    /// Format a basic block identifier.
    pub fn print_basic_block(&self, bb: BasicBlock) -> String {
        format!("bb{}", bb.0)
    }

    /// Format a field index.
    pub fn print_field_idx(&self, field: FieldIdx) -> String {
        format!("{}", field.0)
    }

    // === Constants ===

    /// Format a constant value.
    pub fn print_constant(&self, constant: &Constant) -> String {
        match constant {
            Constant::Int(v, ty) => format!("const {v}_ty{}", ty.0),
            Constant::Float(v, ty) => format!("const {v}_ty{}", ty.0),
            Constant::Bool(v) => format!("const {v}"),
            Constant::Char(c) => format!("const '{c}'"),
            Constant::String(s) => format!("const \"{s}\""),
            Constant::Unit => "const ()".to_string(),
            Constant::FnDef(def_id) => format!("const fn_{}", def_id.0),
            Constant::Zeroed(ty_id) => format!("const zeroed(ty{})", ty_id.0),
        }
    }

    // === Operands ===

    /// Format an operand.
    pub fn print_operand(&self, operand: &Operand) -> String {
        match operand {
            Operand::Copy(place) => format!("copy {}", self.print_place(place)),
            Operand::Move(place) => format!("move {}", self.print_place(place)),
            Operand::Constant(c) => self.print_constant(c),
        }
    }

    // === Places & Projections ===

    /// Format a place with all its projections.
    pub fn print_place(&self, place: &Place) -> String {
        let mut result = self.print_local(place.local);

        for proj in &place.projection {
            result = self.apply_projection(&result, proj);
        }

        result
    }

    fn apply_projection(&self, base: &str, proj: &PlaceElem) -> String {
        match proj {
            PlaceElem::Deref => format!("(*{base})"),
            PlaceElem::Field(idx) => format!("{base}.{}", idx.0),
            PlaceElem::Index(local) => format!("{base}[{}]", self.print_local(*local)),
            PlaceElem::ConstantIndex { offset, from_end } => {
                if *from_end {
                    format!("{base}[-{offset}]")
                } else {
                    format!("{base}[{offset}]")
                }
            }
            PlaceElem::Subslice { from, to } => format!("{base}[{from}..{to}]"),
            PlaceElem::Downcast(variant) => format!("{base} as variant#{variant}"),
        }
    }

    // === Operators ===

    /// Format a binary operator.
    pub fn print_binop(&self, op: BinOp) -> &'static str {
        match op {
            BinOp::Add => "Add",
            BinOp::Sub => "Sub",
            BinOp::Mul => "Mul",
            BinOp::Div => "Div",
            BinOp::Rem => "Rem",
            BinOp::BitAnd => "BitAnd",
            BinOp::BitOr => "BitOr",
            BinOp::BitXor => "BitXor",
            BinOp::Shl => "Shl",
            BinOp::Shr => "Shr",
            BinOp::Eq => "Eq",
            BinOp::Ne => "Ne",
            BinOp::Lt => "Lt",
            BinOp::Le => "Le",
            BinOp::Gt => "Gt",
            BinOp::Ge => "Ge",
        }
    }

    /// Format a unary operator.
    pub fn print_unop(&self, op: UnOp) -> &'static str {
        match op {
            UnOp::Neg => "Neg",
            UnOp::Not => "Not",
        }
    }

    /// Format a cast kind.
    pub fn print_cast_kind(&self, kind: CastKind) -> &'static str {
        match kind {
            CastKind::IntToInt => "IntToInt",
            CastKind::IntToFloat => "IntToFloat",
            CastKind::FloatToInt => "FloatToInt",
            CastKind::FloatToFloat => "FloatToFloat",
            CastKind::PtrToPtr => "PtrToPtr",
            CastKind::Unsize => "Unsize",
        }
    }

    // === Rvalues ===

    /// Format an rvalue.
    pub fn print_rvalue(&self, rvalue: &Rvalue) -> String {
        match rvalue {
            Rvalue::Use(operand) => self.print_operand(operand),
            Rvalue::Ref(BorrowKind::Shared, place, _) => format!("&{}", self.print_place(place)),
            Rvalue::Ref(BorrowKind::Mut, place, _) => format!("&mut {}", self.print_place(place)),
            Rvalue::AddressOf(Mutability::Shared, place, _) => {
                format!("&raw const {}", self.print_place(place))
            }
            Rvalue::AddressOf(Mutability::Mutable, place, _) => {
                format!("&raw mut {}", self.print_place(place))
            }
            Rvalue::BinaryOp(op, lhs, rhs) => {
                format!(
                    "{}({}, {})",
                    self.print_binop(*op),
                    self.print_operand(lhs),
                    self.print_operand(rhs)
                )
            }
            Rvalue::UnaryOp(op, operand) => {
                format!("{}({})", self.print_unop(*op), self.print_operand(operand))
            }
            Rvalue::Cast(kind, operand, ty) => {
                format!(
                    "{} as ty{} ({})",
                    self.print_operand(operand),
                    ty.0,
                    self.print_cast_kind(*kind)
                )
            }
            Rvalue::Len(place) => format!("Len({})", self.print_place(place)),
            Rvalue::Discriminant(place) => format!("discriminant({})", self.print_place(place)),
            Rvalue::Repeat(operand, count) => {
                format!("[{}; {count}]", self.print_operand(operand))
            }
            Rvalue::Aggregate(kind, operands) => self.print_aggregate(kind, operands),
        }
    }

    fn print_aggregate(&self, kind: &AggregateKind, operands: &[Operand]) -> String {
        let ops: Vec<_> = operands.iter().map(|op| self.print_operand(op)).collect();
        let ops_str = ops.join(", ");

        match kind {
            AggregateKind::Tuple => format!("({ops_str})"),
            AggregateKind::Array => format!("[{ops_str}]"),
            AggregateKind::Adt(def_id) => format!("adt_{} {{ {ops_str} }}", def_id.0),
        }
    }

    // === Statements ===

    /// Format a statement.
    pub fn print_statement(&self, stmt: &Statement) -> String {
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                format!(
                    "{} = {}",
                    self.print_place(place),
                    self.print_rvalue(rvalue)
                )
            }
            StatementKind::StorageLive(local) => {
                format!("StorageLive({})", self.print_local(*local))
            }
            StatementKind::StorageDead(local) => {
                format!("StorageDead({})", self.print_local(*local))
            }
            StatementKind::Nop => "nop".to_string(),
        }
    }

    // === Terminators ===

    /// Format a terminator.
    pub fn print_terminator(&self, term: &Terminator) -> String {
        match &term.kind {
            TerminatorKind::Return => "return".to_string(),
            TerminatorKind::Goto(target) => format!("goto -> {}", self.print_basic_block(*target)),
            TerminatorKind::Unreachable => "unreachable".to_string(),
            TerminatorKind::Resume => "resume".to_string(),
            TerminatorKind::Drop { place, target } => {
                format!(
                    "drop({}) -> {}",
                    self.print_place(place),
                    self.print_basic_block(*target)
                )
            }
            TerminatorKind::Assert {
                cond,
                expected,
                target,
            } => {
                let cond_str = if *expected {
                    self.print_operand(cond)
                } else {
                    format!("!{}", self.print_operand(cond))
                };
                format!("assert({cond_str}) -> {}", self.print_basic_block(*target))
            }
            TerminatorKind::SwitchInt { discr, targets } => {
                let mut arms: Vec<String> = targets
                    .iter()
                    .map(|(val, bb)| format!("{val}: {}", self.print_basic_block(bb)))
                    .collect();
                arms.push(format!(
                    "otherwise: {}",
                    self.print_basic_block(targets.otherwise())
                ));
                format!(
                    "switchInt({}) -> [{}]",
                    self.print_operand(discr),
                    arms.join(", ")
                )
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
            } => {
                let args_str: Vec<_> = args.iter().map(|a| self.print_operand(a)).collect();
                let target_str = match target {
                    Some(bb) => self.print_basic_block(*bb),
                    None => "!".to_string(),
                };
                format!(
                    "{} = call {}({}) -> {target_str}",
                    self.print_place(destination),
                    self.print_operand(func),
                    args_str.join(", ")
                )
            }
        }
    }

    // === Basic Blocks ===

    /// Print a basic block.
    pub fn print_block(&mut self, idx: usize, block: &BasicBlockData) {
        self.line(&format!("bb{idx}:"));
        self.indented(|p| {
            for stmt in &block.statements {
                p.line(&p.print_statement(stmt));
            }
            if let Some(term) = &block.terminator {
                p.line(&p.print_terminator(term));
            }
        });
    }

    // === Function Bodies ===

    /// Print a complete MIR body.
    pub fn print_body(&mut self, body: &Body, fn_name: Option<&str>) {
        // Function signature
        let name = fn_name.unwrap_or("fn");

        // Build args string
        let args: Vec<_> = body
            .args()
            .map(|local| {
                let decl = body.local_decl(local);
                format!("{}: ty{}", self.print_local(local), decl.ty.0)
            })
            .collect();
        let args_str = args.join(", ");

        let ret_ty = body.return_ty();
        self.line(&format!("fn {name}({args_str}) -> ty{} {{", ret_ty.0));

        self.indented(|p| {
            // Print locals (skip return place and args)
            for local in body.user_locals() {
                let decl = body.local_decl(local);
                let mut_str = if decl.mutable { "mut " } else { "" };
                let name_str = decl
                    .name
                    .as_ref()
                    .map(|n| format!(" // {n}"))
                    .unwrap_or_default();
                p.line(&format!(
                    "let {mut_str}{}: ty{};{name_str}",
                    p.print_local(local),
                    decl.ty.0
                ));
            }

            if !body.basic_blocks.is_empty() && body.user_locals().count() > 0 {
                p.empty_line();
            }

            // Print basic blocks
            for (idx, block) in body.basic_blocks.iter().enumerate() {
                p.print_block(idx, block);
            }
        });

        self.line("}");
    }

    // === Helpers ===

    fn line(&mut self, text: &str) {
        let _ = writeln!(self.output, "{:indent$}{text}", "", indent = self.indent);
    }

    fn empty_line(&mut self) {
        let _ = writeln!(self.output);
    }

    fn indented<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Self),
    {
        self.indent += 4;
        f(self);
        self.indent -= 4;
    }
}

/// Convenience function to pretty-print a MIR body.
pub fn pretty_print(body: &Body, fn_name: Option<&str>) -> String {
    let mut printer = MirPrinter::new();
    printer.print_body(body, fn_name);
    printer.finish()
}

#[cfg(test)]
mod tests;
