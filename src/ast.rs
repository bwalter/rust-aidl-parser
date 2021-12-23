use core::fmt;
use std::{collections::HashMap, path::PathBuf};

use serde_derive::Serialize;

pub struct Project {
    pub root_path: PathBuf,
    pub files: HashMap<ItemKey, File>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct File {
    pub package: Package,
    pub imports: Vec<Import>,
    pub item: Item,
}

pub type ItemKey = String;
pub type ItemKeyRef<'a> = &'a str;

impl File {
    // TODO: cache it
    pub fn get_key(&self) -> ItemKey {
        format!("{}.{}", self.package.name, self.item.get_name())
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Position {
    pub offset: usize,

    /// 1-based line and column
    pub line_col: (usize, usize),
}

impl Position {
    pub(crate) fn new(lookup: &line_col::LineColLookup, offset: usize) -> Self {
        Position {
            offset,
            line_col: lookup.get_by_cluster(offset),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub(crate) fn new(lookup: &line_col::LineColLookup, start: usize, end: usize) -> Self {
        let start = Position::new(lookup, start);
        let end = Position::new(lookup, end);

        Range { start, end }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Package {
    pub name: String,
    pub symbol_range: Range,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Import {
    pub path: String,
    pub name: String,
    pub symbol_range: Range,
}

impl Import {
    // TODO: cache it?
    pub fn get_qualified_name(&self) -> String {
        format!("{}.{}", self.path, self.name)
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub enum InterfaceElement {
    Const(Const),
    Method(Method),
}

impl InterfaceElement {
    pub fn get_name(&self) -> &str {
        match self {
            InterfaceElement::Const(c) => &c.name,
            InterfaceElement::Method(m) => &m.name,
        }
    }

    pub fn get_symbol_range(&self) -> &Range {
        match self {
            InterfaceElement::Const(c) => &c.symbol_range,
            InterfaceElement::Method(m) => &m.symbol_range,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub enum ItemKind {
    Interface,
    Parcelable,
    Enum,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub enum Item {
    Interface(Interface),
    Parcelable(Parcelable),
    Enum(Enum),
}

impl Item {
    pub fn get_kind(&self) -> ItemKind {
        match self {
            Item::Interface(_) => ItemKind::Interface,
            Item::Parcelable(_) => ItemKind::Parcelable,
            Item::Enum(_) => ItemKind::Enum,
        }
    }

    pub fn get_name(&self) -> &str {
        match self {
            Item::Interface(i) => &i.name,
            Item::Parcelable(p) => &p.name,
            Item::Enum(e) => &e.name,
        }
    }

    pub fn get_symbol_range(&self) -> &Range {
        match self {
            Item::Interface(i) => &i.symbol_range,
            Item::Parcelable(p) => &p.symbol_range,
            Item::Enum(e) => &e.symbol_range,
        }
    }

    pub fn get_full_range(&self) -> &Range {
        match self {
            Item::Interface(i) => &i.full_range,
            Item::Parcelable(p) => &p.full_range,
            Item::Enum(e) => &e.full_range,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Interface {
    pub name: String,
    pub elements: Vec<InterfaceElement>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Parcelable {
    pub name: String,
    pub members: Vec<Member>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Enum {
    pub name: String,
    pub elements: Vec<EnumElement>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Const {
    pub name: String,
    pub const_type: Type,
    pub value: String,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Method {
    pub oneway: bool,
    pub name: String,
    pub return_type: Type,
    pub args: Vec<Arg>,
    pub annotations: Vec<Annotation>,
    pub value: Option<u32>,
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

impl Method {
    pub fn get_signature(&self) -> String {
        format!(
            "{} {}({})",
            self.return_type.name,
            self.name,
            self.args
                .iter()
                .map(|a| a.arg_type.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Arg {
    pub direction: Direction,
    pub name: Option<String>,
    pub arg_type: Type,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub enum Direction {
    In(Range),
    Out(Range),
    InOut(Range),
    Unspecified,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Unspecified
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::In(_) => write!(f, "in"),
            Direction::Out(_) => write!(f, "out"),
            Direction::InOut(_) => write!(f, "inout"),
            Direction::Unspecified => Ok(()),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub member_type: Type,
    pub value: Option<String>,
    pub symbol_range: Range,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
    pub full_range: Range,
}

impl Member {
    pub fn get_signature(&self) -> String {
        format!("{} {}", self.member_type.name, self.name,)
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct EnumElement {
    pub name: String,
    pub value: Option<String>,
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Annotation {
    pub name: String,
    pub key_values: HashMap<String, Option<String>>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub enum TypeKind {
    Primitive,
    Void,
    Array,
    Map,
    List,
    String,
    Custom,
    Invalid,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Type {
    pub name: String,
    pub kind: TypeKind,
    pub generic_types: Vec<Type>,
    pub definition: Option<ItemKey>,
    pub symbol_range: Range,
}

impl Type {
    pub fn simple_type<S: Into<String>>(
        name: S,
        kind: TypeKind,
        lookup: &line_col::LineColLookup,
        start: usize,
        end: usize,
    ) -> Self {
        Type {
            name: name.into(),
            kind,
            generic_types: Vec::new(),
            definition: None,
            symbol_range: Range::new(lookup, start, end),
        }
    }

    pub fn array(param: Type, lookup: &line_col::LineColLookup, start: usize, end: usize) -> Self {
        Type {
            name: "Array".to_owned(),
            kind: TypeKind::Array,
            generic_types: Vec::from([param]),
            definition: None,
            symbol_range: Range::new(lookup, start, end),
        }
    }

    pub fn list(param: Type, lookup: &line_col::LineColLookup, start: usize, end: usize) -> Self {
        Type {
            name: "List".to_owned(),
            kind: TypeKind::List,
            generic_types: Vec::from([param]),
            definition: None,
            symbol_range: Range::new(lookup, start, end),
        }
    }

    pub fn map(
        key_param: Type,
        value_param: Type,
        lookup: &line_col::LineColLookup,
        start: usize,
        end: usize,
    ) -> Self {
        Type {
            name: "Map".to_owned(),
            kind: TypeKind::Map,
            generic_types: Vec::from([key_param, value_param]),
            definition: None,
            symbol_range: Range::new(lookup, start, end),
        }
    }

    pub fn invalid_with_generics<S: Into<String>>(
        name: S,
        params: &[Type],
        lookup: &line_col::LineColLookup,
        start: usize,
        end: usize,
    ) -> Self {
        Type {
            name: name.into(),
            kind: TypeKind::Invalid,
            generic_types: params.to_vec(),
            definition: None,
            symbol_range: Range::new(lookup, start, end),
        }
    }
}
