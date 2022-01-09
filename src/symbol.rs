use serde_derive::Serialize;

use crate::ast;

#[derive(Serialize, Clone, Debug)]
pub enum Symbol<'a> {
    Package(&'a ast::Package),
    Import(&'a ast::Import),
    Interface(&'a ast::Interface, &'a ast::Package),
    Parcelable(&'a ast::Parcelable, &'a ast::Package),
    Enum(&'a ast::Enum, &'a ast::Package),
    Method(&'a ast::Method, &'a ast::Interface),
    Arg(&'a ast::Arg, &'a ast::Method),
    Const(&'a ast::Const, &'a ast::Interface),
    Field(&'a ast::Field, &'a ast::Parcelable),
    EnumElement(&'a ast::EnumElement, &'a ast::Enum),
    Type(&'a ast::Type),
}

impl<'a> Symbol<'a> {
    pub fn get_name(&self) -> Option<String> {
        match self {
            Symbol::Package(p) => Some(p.name.clone()),
            Symbol::Import(i) => Some(i.get_qualified_name()),
            Symbol::Interface(i, _) => Some(i.name.clone()),
            Symbol::Parcelable(p, _) => Some(p.name.clone()),
            Symbol::Enum(e, _) => Some(e.name.clone()),
            Symbol::Method(m, _) => Some(m.name.clone()),
            Symbol::Arg(a, _) => a.name.clone(),
            Symbol::Const(c, _) => Some(c.name.clone()),
            Symbol::Field(m, _) => Some(m.name.clone()),
            Symbol::EnumElement(e, _) => Some(e.name.clone()),
            Symbol::Type(t) => Some(t.name.clone()),
        }
    }

    pub fn get_qualified_name(&self) -> Option<String> {
        match self {
            Symbol::Package(p) => Some(p.name.clone()),
            Symbol::Import(i) => Some(i.get_qualified_name()),
            Symbol::Interface(i, pkg) => Some(format!("{}.{}", pkg.name, i.name)),
            Symbol::Parcelable(p, pkg) => Some(format!("{}.{}", pkg.name, p.name)),
            Symbol::Enum(e, pkg) => Some(format!("{}{}", pkg.name, e.name)),
            Symbol::Method(m, i) => Some(format!("{}::{}", i.name, m.name)),
            Symbol::Arg(a, _) => a.name.clone(),
            Symbol::Const(c, i) => Some(format!("{}::{}", i.name, c.name)),
            Symbol::Field(m, p) => Some(format!("{}::{}", p.name, m.name)),
            Symbol::EnumElement(el, e) => Some(format!("{}::{}", e.name, el.name)),
            Symbol::Type(ast::Type {
                kind: ast::TypeKind::Resolved(qualified_name, _),
                ..
            }) => Some(qualified_name.clone()),
            Symbol::Type(_) => None,
        }
    }

    pub fn get_range(&self) -> &ast::Range {
        match self {
            Symbol::Package(p) => &p.symbol_range,
            Symbol::Import(i) => &i.symbol_range,
            Symbol::Interface(i, _) => &i.symbol_range,
            Symbol::Parcelable(p, _) => &p.symbol_range,
            Symbol::Enum(e, _) => &e.symbol_range,
            Symbol::Method(m, _) => &m.symbol_range,
            Symbol::Arg(a, _) => &a.symbol_range,
            Symbol::Const(c, _) => &c.symbol_range,
            Symbol::Field(m, _) => &m.symbol_range,
            Symbol::EnumElement(e, _) => &e.symbol_range,
            Symbol::Type(t) => &t.symbol_range,
        }
    }

    pub fn get_full_range(&self) -> &ast::Range {
        match self {
            Symbol::Package(p) => &p.full_range,
            Symbol::Import(i) => &i.full_range,
            Symbol::Interface(i, _) => &i.full_range,
            Symbol::Parcelable(p, _) => &p.full_range,
            Symbol::Enum(e, _) => &e.full_range,
            Symbol::Method(m, _) => &m.full_range,
            Symbol::Arg(a, _) => &a.full_range,
            Symbol::Const(c, _) => &c.full_range,
            Symbol::Field(m, _) => &m.full_range,
            Symbol::EnumElement(e, _) => &e.full_range,
            Symbol::Type(t) => &t.full_range,
        }
    }

    pub fn get_details(&self) -> Option<String> {
        fn get_type_str(t: &ast::Type) -> String {
            if t.generic_types.is_empty() {
                t.name.clone()
            } else {
                format!(
                    "{}<{}>",
                    t.name,
                    t.generic_types
                        .iter()
                        .map(get_type_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }

        fn get_arg_str(a: &ast::Arg) -> String {
            let direction_str = match a.direction {
                ast::Direction::In(_) => "in ",
                ast::Direction::Out(_) => "out ",
                ast::Direction::InOut(_) => "inout ",
                ast::Direction::Unspecified => "",
            };

            format!("{}{}", direction_str, get_type_str(&a.arg_type))
        }

        Some(match self {
            Symbol::Package(..) => String::from("package"),
            Symbol::Import(..) => String::from("import"),
            Symbol::Interface(..) => String::from("interface"),
            Symbol::Parcelable(..) => String::from("parcelable"),
            Symbol::Enum(..) => String::from("enum"),
            Symbol::Method(m, _) => {
                format!(
                    "{}({})",
                    get_type_str(&m.return_type),
                    m.args
                        .iter()
                        .map(get_arg_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Symbol::Arg(a, _) => get_arg_str(a),
            Symbol::Const(c, _) => format!("const {}", get_type_str(&c.const_type)),
            Symbol::Field(m, _) => get_type_str(&m.field_type),
            Symbol::EnumElement(..) => return None,
            Symbol::Type(t) => get_type_str(t),
        })
    }

    pub fn get_signature(&self) -> String {
        fn get_type_str(t: &ast::Type) -> String {
            if t.generic_types.is_empty() {
                t.name.clone()
            } else {
                format!(
                    "{}<{}>",
                    t.name,
                    t.generic_types
                        .iter()
                        .map(get_type_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }

        fn get_arg_str(a: &ast::Arg) -> String {
            let direction_str = match a.direction {
                ast::Direction::In(_) => "in ",
                ast::Direction::Out(_) => "out ",
                ast::Direction::InOut(_) => "inout ",
                ast::Direction::Unspecified => "",
            };

            let suffix = a
                .name
                .as_ref()
                .map(|s| format!(" {}", s))
                .unwrap_or_default();

            format!("{}{}{}", direction_str, get_type_str(&a.arg_type), suffix)
        }

        match self {
            Symbol::Package(p) => format!("package {}", p.name),
            Symbol::Import(i) => format!("import {}", i.get_qualified_name()),
            Symbol::Parcelable(p, _) => format!("parcelable {}", p.name),
            Symbol::Interface(i, _) => format!("interface {}", i.name),
            Symbol::Enum(e, _) => format!("enum {}", e.name),
            Symbol::Method(m, _) => {
                format!(
                    "{} {}({})",
                    get_type_str(&m.return_type),
                    m.name,
                    m.args
                        .iter()
                        .map(get_arg_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Symbol::Arg(a, _) => get_arg_str(a),
            Symbol::Const(c, _) => format!("const {} {}", get_type_str(&c.const_type), c.name),
            Symbol::Field(m, _) => format!("{} {}", get_type_str(&m.field_type), m.name),
            Symbol::EnumElement(el, _) => el.name.clone(),
            Symbol::Type(t) => get_type_str(t),
        }
    }
}
