use core::fmt;
use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Aidl {
    pub package: Package,
    pub imports: Vec<Import>,
    pub declared_parcelables: Vec<Import>,
    pub item: Item,
}

pub type ItemKey = String;
pub type ItemKeyRef<'a> = &'a str;

impl Aidl {
    // TODO: cache it
    pub fn get_key(&self) -> ItemKey {
        format!("{}.{}", self.package.name, self.item.get_name())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Package {
    pub name: String,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Import {
    pub path: String,
    pub name: String,
    pub symbol_range: Range,
    pub full_range: Range,
}

impl Import {
    // TODO: cache it?
    pub fn get_qualified_name(&self) -> String {
        format!("{}.{}", self.path, self.name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceElement {
    Const(Const),
    Method(Method),
}

impl InterfaceElement {
    pub fn as_method(&self) -> Option<&Method> {
        match &self {
            InterfaceElement::Method(m) => Some(m),
            _ => None,
        }
    }

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedItemKind {
    Interface,
    Parcelable,
    Enum,
    ForwardDeclaredParcelable,
    UnknwonImport,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Item {
    Interface(Interface),
    Parcelable(Parcelable),
    Enum(Enum),
}

impl Item {
    pub fn as_interface(&self) -> Option<&Interface> {
        match &self {
            Item::Interface(i) => Some(i),
            _ => None,
        }
    }

    pub fn as_parcelable(&self) -> Option<&Parcelable> {
        match &self {
            Item::Parcelable(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_enum(&self) -> Option<&Enum> {
        match &self {
            Item::Enum(e) => Some(e),
            _ => None,
        }
    }

    pub fn get_kind(&self) -> ResolvedItemKind {
        match self {
            Item::Interface(_) => ResolvedItemKind::Interface,
            Item::Parcelable(_) => ResolvedItemKind::Parcelable,
            Item::Enum(_) => ResolvedItemKind::Enum,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Interface {
    pub oneway: bool,
    pub name: String,
    pub elements: Vec<InterfaceElement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Parcelable {
    pub name: String,
    pub elements: Vec<ParcelableElement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Enum {
    pub name: String,
    pub elements: Vec<EnumElement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub full_range: Range,
    pub symbol_range: Range,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Const {
    pub name: String,
    #[serde(rename = "type")]
    pub const_type: Type,
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Method {
    #[serde(default, skip_serializing_if = "BoolExt::is_true")]
    pub oneway: bool,
    pub name: String,
    pub return_type: Type,
    pub args: Vec<Arg>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transact_code: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
    pub transact_code_range: Range,
    pub oneway_range: Range,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Arg {
    #[serde(default, skip_serializing_if = "Direction::is_unspecified")]
    pub direction: Direction,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub arg_type: Type,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    In(Range),
    Out(Range),
    InOut(Range),
    Unspecified,
}

impl Direction {
    fn is_unspecified(&self) -> bool {
        matches!(self, Self::Unspecified)
    }
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParcelableElement {
    Const(Const),
    Field(Field),
}

impl ParcelableElement {
    pub fn as_field(&self) -> Option<&Field> {
        match &self {
            ParcelableElement::Field(f) => Some(f),
            _ => None,
        }
    }

    pub fn get_name(&self) -> &str {
        match self {
            ParcelableElement::Const(c) => &c.name,
            ParcelableElement::Field(f) => &f.name,
        }
    }

    pub fn get_symbol_range(&self) -> &Range {
        match self {
            ParcelableElement::Const(c) => &c.symbol_range,
            ParcelableElement::Field(f) => &f.symbol_range,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: Type,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

impl Field {
    pub fn get_signature(&self) -> String {
        format!("{} {}", self.field_type.name, self.name,)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EnumElement {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub symbol_range: Range,
    pub full_range: Range,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub key_values: HashMap<String, Option<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TypeKind {
    Primitive,
    Void,
    Array,
    Map,
    List,
    String,
    CharSequence,
    AndroidType(AndroidTypeKind),
    Resolved(String, ResolvedItemKind),
    Unresolved,
}

/// Android (or Java) built-in types which do not require explicit import
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AndroidTypeKind {
    IBinder,
    FileDescriptor,
    ParcelFileDescriptor,
    ParcelableHolder,
}

impl AndroidTypeKind {
    fn get_all() -> &'static [Self] {
        &[
            Self::IBinder,
            Self::FileDescriptor,
            Self::ParcelFileDescriptor,
            Self::ParcelableHolder,
        ]
    }

    pub fn is_android_type_kind(qualified_name: &str) -> bool {
        Self::get_all()
            .iter()
            .any(|at| at.get_qualified_name() == qualified_name)
    }

    pub fn get_qualified_name(&self) -> &str {
        match self {
            AndroidTypeKind::IBinder => "android.os.IBinder",
            AndroidTypeKind::FileDescriptor => "java.os.FileDescriptor",
            AndroidTypeKind::ParcelFileDescriptor => "android.os.ParcelFileDescriptor",
            AndroidTypeKind::ParcelableHolder => "android.os.ParcelableHolder",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Type {
    pub name: String,
    pub kind: TypeKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generic_types: Vec<Type>,
    pub symbol_range: Range,
    pub full_range: Range,
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
            symbol_range: Range::new(lookup, start, end),
            full_range: Range::new(lookup, start, end),
        }
    }

    pub fn array(
        param: Type,
        lookup: &line_col::LineColLookup,
        start: usize,
        end: usize,
        fr_start: usize,
        fr_end: usize,
    ) -> Self {
        Type {
            name: "Array".to_owned(),
            kind: TypeKind::Array,
            generic_types: Vec::from([param]),
            symbol_range: Range::new(lookup, start, end),
            full_range: Range::new(lookup, fr_start, fr_end),
        }
    }

    pub fn list(
        param: Type,
        lookup: &line_col::LineColLookup,
        start: usize,
        end: usize,
        fr_start: usize,
        fr_end: usize,
    ) -> Self {
        Type {
            name: "List".to_owned(),
            kind: TypeKind::List,
            generic_types: Vec::from([param]),
            symbol_range: Range::new(lookup, start, end),
            full_range: Range::new(lookup, fr_start, fr_end),
        }
    }

    pub fn non_generic_list(lookup: &line_col::LineColLookup, start: usize, end: usize) -> Self {
        Type {
            name: "List".to_owned(),
            kind: TypeKind::List,
            generic_types: Vec::new(),
            symbol_range: Range::new(lookup, start, end),
            full_range: Range::new(lookup, start, end),
        }
    }

    pub fn map(
        key_param: Type,
        value_param: Type,
        lookup: &line_col::LineColLookup,
        start: usize,
        end: usize,
        fr_start: usize,
        fr_end: usize,
    ) -> Self {
        Type {
            name: "Map".to_owned(),
            kind: TypeKind::Map,
            generic_types: Vec::from([key_param, value_param]),
            symbol_range: Range::new(lookup, start, end),
            full_range: Range::new(lookup, fr_start, fr_end),
        }
    }

    pub fn non_generic_map(lookup: &line_col::LineColLookup, start: usize, end: usize) -> Self {
        Type {
            name: "Map".to_owned(),
            kind: TypeKind::Map,
            generic_types: Vec::new(),
            symbol_range: Range::new(lookup, start, end),
            full_range: Range::new(lookup, start, end),
        }
    }

    pub fn android_type<S: Into<String>>(
        name: S,
        android_kind: AndroidTypeKind,
        lookup: &line_col::LineColLookup,
        start: usize,
        end: usize,
    ) -> Self {
        Type {
            name: name.into(),
            kind: TypeKind::AndroidType(android_kind),
            generic_types: Vec::new(),
            symbol_range: Range::new(lookup, start, end),
            full_range: Range::new(lookup, start, end),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TypeDefinition {
    Unresolved,
    Resolved {
        key: ItemKey,
        item_kind: ResolvedItemKind,
    },
    ForwardDeclared {
        qualified_name: String,
        range: Range,
    },
}

trait BoolExt {
    fn is_true(&self) -> bool;
}

impl BoolExt for bool {
    fn is_true(&self) -> bool {
        *self
    }
}
