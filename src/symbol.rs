use serde_derive::Serialize;

use crate::ast;

#[derive(Serialize, Clone, Debug)]
pub enum Symbol<'a> {
    Interface(&'a ast::Interface),
    Parcelable(&'a ast::Parcelable),
    Enum(&'a ast::Enum),
    Method(&'a ast::Method),
    Arg(&'a ast::Arg),
    Const(&'a ast::Const),
    Member(&'a ast::Member),
    EnumElement(&'a ast::EnumElement),
    Type(&'a ast::Type),
}

impl<'a> Symbol<'a> {
    pub fn get_name(&self) -> Option<&str> {
        match self {
            Symbol::Interface(i) => Some(&i.name),
            Symbol::Parcelable(p) => Some(&p.name),
            Symbol::Enum(e) => Some(&e.name),
            Symbol::Method(m) => Some(&m.name),
            Symbol::Arg(a) => a.name.as_deref(),
            Symbol::Const(c) => Some(&c.name),
            Symbol::Member(m) => Some(&m.name),
            Symbol::EnumElement(e) => Some(&e.name),
            Symbol::Type(t) => Some(&t.name),
        }
    }

    pub fn get_range(&self) -> &ast::Range {
        match self {
            Symbol::Interface(i) => &i.symbol_range,
            Symbol::Parcelable(p) => &p.symbol_range,
            Symbol::Enum(e) => &e.symbol_range,
            Symbol::Method(m) => &m.symbol_range,
            Symbol::Arg(a) => &a.symbol_range,
            Symbol::Const(c) => &c.symbol_range,
            Symbol::Member(m) => &m.symbol_range,
            Symbol::EnumElement(e) => &e.symbol_range,
            Symbol::Type(t) => &t.symbol_range,
        }
    }

    pub fn get_full_range(&self) -> &ast::Range {
        match self {
            Symbol::Interface(i) => &i.full_range,
            Symbol::Parcelable(p) => &p.full_range,
            Symbol::Enum(e) => &e.full_range,
            Symbol::Method(m) => &m.full_range,
            Symbol::Arg(a) => &a.full_range,
            Symbol::Const(c) => &c.full_range,
            Symbol::Member(m) => &m.full_range,
            Symbol::EnumElement(e) => &e.full_range,
            Symbol::Type(t) => &t.symbol_range, // TODO: consider full range (might be different for generics)?
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
            Symbol::Interface(_) => String::from("interface"),
            Symbol::Parcelable(_) => String::from("parcelable"),
            Symbol::Enum(_) => String::from("enum"),
            Symbol::Method(m) => {
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
            Symbol::Arg(a) => get_arg_str(a),
            Symbol::Const(c) => format!("const {}", get_type_str(&c.const_type)),
            Symbol::Member(m) => get_type_str(&m.member_type),
            Symbol::EnumElement(_) => return None,
            Symbol::Type(t) => {
                if t.generic_types.is_empty() {
                    return None;
                }
                t.generic_types
                    .iter()
                    .map(get_type_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        })
    }

    pub fn get_signature(&self) -> Option<String> {
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

        Some(match self {
            Symbol::Interface(i) => format!("interface {}", i.name),
            Symbol::Parcelable(p) => format!("parcelable {}", p.name),
            Symbol::Enum(e) => format!("enum {}", e.name),
            Symbol::Method(m) => {
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
            Symbol::Arg(a) => get_arg_str(a),
            Symbol::Const(c) => format!("const {} {}", get_type_str(&c.const_type), c.name),
            Symbol::Member(m) => format!("{} {}", get_type_str(&m.member_type), m.name),
            Symbol::EnumElement(el) => el.name.clone(),
            Symbol::Type(t) => {
                if t.generic_types.is_empty() {
                    return None;
                }
                t.generic_types
                    .iter()
                    .map(get_type_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        })
    }
}
