use std::{collections::HashMap, path::PathBuf};

use serde_derive::Serialize;

pub struct Project {
    pub root_path: PathBuf,
    pub files: HashMap<ItemKey, File>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct File {
    pub package: Package,
    pub imports: Vec<Import>,
    pub item: Item,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Position {
    pub offset: usize,
    pub line_col: (usize, usize),
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(lookup: &line_col::LineColLookup, start: usize, end: usize) -> Self {
        let start = Position {
            offset: start,
            line_col: lookup.get_by_cluster(start),
        };

        let end = Position {
            offset: end,
            line_col: lookup.get_by_cluster(end),
        };

        Range { start, end }
    }
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Package {
    pub name: String,
    pub symbol_range: Range,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Import {
    pub name: String,
    pub symbol_range: Range,
}

#[derive(Serialize, Debug, PartialEq)]
pub enum InterfaceElement {
    Const(Const),
    Method(Method),
}

#[derive(Serialize, Debug, PartialEq)]
pub enum Item {
    Interface(Interface),
    Parcelable(Parcelable),
    Enum(Enum),
}

pub type ItemKey = PathBuf;

#[derive(Serialize, Debug, PartialEq)]
pub struct Interface {
    pub name: String,
    pub elements: Vec<InterfaceElement>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Parcelable {
    pub name: String,
    pub members: Vec<Member>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Enum {
    pub name: String,
    pub elements: Vec<EnumElement>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}
#[derive(Serialize, Debug, PartialEq)]
pub struct Const {
    pub name: String,
    pub const_type: Type,
    pub value: String,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Method {
    pub oneway: bool,
    pub name: String,
    pub return_type: Type,
    pub args: Vec<Arg>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Arg {
    pub direction: Direction,
    pub name: Option<String>,
    pub arg_type: Type,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
}

#[derive(Serialize, Debug, PartialEq)]
pub enum Direction {
    In,
    Out,
    InOut,
    Unspecified,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Unspecified
    }
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub member_type: Type,
    pub symbol_range: Range,
    pub doc: Option<String>,
    pub full_range: Range,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct EnumElement {
    pub name: String,
    pub value: Option<String>,
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Annotation {
    pub key_values: HashMap<String, Option<String>>,
}

#[derive(Serialize, Debug, PartialEq)]
pub enum TypeKind {
    Primitive,
    Void,
    Array,
    Map,
    List,
    String,
    Custom,
}

#[derive(Serialize, Debug, PartialEq)]
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
}
