use std::{collections::HashMap, path::PathBuf};

use serde_derive::Serialize;

pub struct Project {
    pub root_path: PathBuf,
    pub files: HashMap<ItemKey, File>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct File {
    pub package: Package,
    pub imports: Vec<Package>,
    pub item: Item,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Position {
    pub offset: usize,
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

#[derive(Serialize, Debug, PartialEq)]
pub struct Package {
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
    pub value: Option<String>,
    pub symbol_range: Range,
    pub doc: Option<String>,
    pub annotations: Vec<Annotation>,
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
    pub name: String,
    pub key_values: HashMap<String, Option<String>>,
}

#[derive(Serialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Debug, PartialEq, Clone)]
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
            kind: TypeKind::List,
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
