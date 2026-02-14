/// Pretty printer: RustCrate → map of filepath → content.
use std::collections::HashMap;

use super::rust_ast::*;

/// Generated crate files: relative path → file content.
pub type CrateFiles = HashMap<String, String>;

/// Emit a complete Rust crate as a set of files.
pub fn emit_crate(krate: &RustCrate) -> CrateFiles {
    let mut files = CrateFiles::new();

    // Cargo.toml
    files.insert(
        "Cargo.toml".to_string(),
        format!(
            "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            krate.name
        ),
    );

    // src/lib.rs — feature flags + module declaration + re-export
    let mut lib_rs = String::new();
    if krate.has_box_patterns {
        lib_rs.push_str("#![feature(box_patterns)]\n\n");
    }
    for module in &krate.modules {
        lib_rs.push_str(&format!("pub mod {};\n", module.name));
        lib_rs.push_str(&format!("pub use {}::*;\n", module.name));
    }
    files.insert("src/lib.rs".to_string(), lib_rs);

    // Each module → src/{name}.rs
    for module in &krate.modules {
        let content = emit_module(module);
        files.insert(format!("src/{}.rs", module.name), content);
    }

    files
}

fn emit_module(module: &RustModule) -> String {
    let mut buf = String::new();

    for u in &module.uses {
        buf.push_str(&format!("use {};\n", u));
    }
    if !module.uses.is_empty() {
        buf.push('\n');
    }

    for (i, item) in module.items.iter().enumerate() {
        if i > 0 {
            buf.push('\n');
        }
        match item {
            RustItem::Enum(e) => emit_enum(e, &mut buf),
            RustItem::Function(f) => emit_function(f, &mut buf),
            RustItem::Trait(t) => emit_trait(t, &mut buf),
        }
    }

    buf
}

fn emit_enum(e: &RustEnum, buf: &mut String) {
    buf.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    buf.push_str(&format!("pub enum {} {{\n", e.name));
    for v in &e.variants {
        if v.fields.is_empty() {
            buf.push_str(&format!("    {},\n", v.name));
        } else {
            buf.push_str(&format!("    {}(", v.name));
            for (i, f) in v.fields.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_type(&f.ty, buf);
            }
            buf.push_str("),\n");
        }
    }
    buf.push_str("}\n");
}

fn emit_function(f: &RustFunction, buf: &mut String) {
    buf.push_str(&format!("pub fn {}(", f.name));
    let mut first = true;
    // Emit effects parameter if this is an effectful function
    if let Some(trait_name) = &f.effects_trait {
        buf.push_str(&format!("effects: &mut impl {}", trait_name));
        first = false;
    }
    for p in &f.params {
        if !first {
            buf.push_str(", ");
        }
        first = false;
        buf.push_str(&format!("{}: ", p.name));
        emit_type(&p.ty, buf);
    }
    buf.push(')');
    // Skip -> () for effectful functions (unit return)
    match &f.ret {
        RustType::Named(n) if n == "()" => {}
        other => {
            buf.push_str(" -> ");
            emit_type(other, buf);
        }
    }
    buf.push_str(" {\n");
    emit_expr(&f.body, buf, 1);
    buf.push('\n');
    buf.push_str("}\n");
}

fn emit_trait(t: &RustTrait, buf: &mut String) {
    buf.push_str(&format!("pub trait {} {{\n", t.name));
    for m in &t.methods {
        buf.push_str(&format!("    fn {}(&mut self", m.name));
        for p in &m.params {
            buf.push_str(&format!(", {}: ", p.name));
            emit_type(&p.ty, buf);
        }
        buf.push(')');
        if let Some(ret) = &m.ret {
            buf.push_str(" -> ");
            emit_type(ret, buf);
        }
        buf.push_str(";\n");
    }
    buf.push_str("}\n");
}

fn emit_type(ty: &RustType, buf: &mut String) {
    match ty {
        RustType::Named(n) => buf.push_str(n),
        RustType::Boxed(inner) => {
            buf.push_str("Box<");
            emit_type(inner, buf);
            buf.push('>');
        }
    }
}

fn emit_expr(expr: &RustExpr, buf: &mut String, depth: usize) {
    buf.push_str(&"    ".repeat(depth));
    emit_expr_impl(expr, buf, Some(depth));
}

/// Emit an expression inline (no leading indentation).
fn emit_expr_inline(expr: &RustExpr, buf: &mut String) {
    emit_expr_impl(expr, buf, None);
}

/// Shared expression emitter. `indent` controls leading whitespace and
/// multi-line vs inline formatting for Match.
fn emit_expr_impl(expr: &RustExpr, buf: &mut String, indent: Option<usize>) {
    match expr {
        RustExpr::Var(v) => buf.push_str(v),
        RustExpr::Constructor {
            enum_name,
            variant,
            args,
        } => {
            buf.push_str(&format!("{}::{}", enum_name, variant));
            if !args.is_empty() {
                buf.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    emit_expr_inline(a, buf);
                }
                buf.push(')');
            }
        }
        RustExpr::Call { func, args } => {
            buf.push_str(&format!("{}(", func));
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr_inline(a, buf);
            }
            buf.push(')');
        }
        RustExpr::Tuple(elems) => {
            buf.push('(');
            for (i, e) in elems.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr_inline(e, buf);
            }
            buf.push(')');
        }
        RustExpr::BoxNew(inner) => {
            buf.push_str("Box::new(");
            emit_expr_inline(inner, buf);
            buf.push(')');
        }
        RustExpr::Deref(inner) => {
            buf.push('*');
            emit_expr_inline(inner, buf);
        }
        RustExpr::Clone(inner) => {
            emit_expr_inline(inner, buf);
            buf.push_str(".clone()");
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            buf.push_str(&format!("{}.{}(", receiver, method));
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr_inline(a, buf);
            }
            buf.push(')');
        }
        RustExpr::Unreachable => buf.push_str("unreachable!()"),
        RustExpr::Match { scrutinee, arms } => {
            buf.push_str("match ");
            emit_expr_inline(scrutinee, buf);
            if let Some(depth) = indent {
                // Multi-line format
                buf.push_str(" {\n");
                let arm_indent = "    ".repeat(depth + 1);
                let close_indent = "    ".repeat(depth);
                for arm in arms {
                    buf.push_str(&arm_indent);
                    emit_pattern(&arm.pattern, buf);
                    if let Some(guard) = &arm.guard {
                        buf.push_str(&format!(" if {}", guard));
                    }
                    buf.push_str(" => ");
                    emit_expr_inline(&arm.body, buf);
                    buf.push_str(",\n");
                }
                buf.push_str(&format!("{}}}", close_indent));
            } else {
                // Inline format
                buf.push_str(" { ");
                for (i, arm) in arms.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    emit_pattern(&arm.pattern, buf);
                    if let Some(guard) = &arm.guard {
                        buf.push_str(&format!(" if {}", guard));
                    }
                    buf.push_str(" => ");
                    emit_expr_inline(&arm.body, buf);
                }
                buf.push_str(" }");
            }
        }
    }
}

fn emit_pattern(pat: &RustPattern, buf: &mut String) {
    match pat {
        RustPattern::Var(v) => buf.push_str(v),
        RustPattern::Wildcard => buf.push('_'),
        RustPattern::Constructor {
            enum_name,
            variant,
            fields,
        } => {
            buf.push_str(&format!("{}::{}", enum_name, variant));
            if !fields.is_empty() {
                buf.push('(');
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    emit_pattern(f, buf);
                }
                buf.push(')');
            }
        }
        RustPattern::Tuple(pats) => {
            buf.push('(');
            for (i, p) in pats.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_pattern(p, buf);
            }
            buf.push(')');
        }
        RustPattern::Box(inner) => {
            buf.push_str("box ");
            emit_pattern(inner, buf);
        }
    }
}
