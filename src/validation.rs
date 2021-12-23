use std::collections::{hash_map, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::parser::ParseFileResult;
use crate::{ast, diagnostic};

pub(crate) fn validate<ID>(
    keys: HashMap<String, ast::ItemKind>,
    lalrpop_results: HashMap<ID, ParseFileResult<ID>>,
) -> HashMap<ID, ParseFileResult<ID>>
where
    ID: Eq + Hash + Clone + Debug,
{
    lalrpop_results
        .into_iter()
        .map(|(id, mut fr)| {
            let mut file = match fr.file {
                Some(f) => f,
                None => return (id, ParseFileResult { file: None, ..fr }),
            };

            let resolved = resolve_types(&mut file, &mut fr.diagnostics);
            check_types(&mut file, &keys, &mut fr.diagnostics);
            check_methods(&mut file, &keys, &mut fr.diagnostics);
            check_args(&mut file, &keys, &mut fr.diagnostics);
            check_imports(&file.imports, &resolved, &keys, &mut fr.diagnostics);

            // Sort diagnostics by line
            fr.diagnostics.sort_by_key(|d| d.range.start.line_col.0);

            (
                id,
                ParseFileResult {
                    file: Some(file),
                    ..fr
                },
            )
        })
        .collect()
}

fn walk_types<F: FnMut(&mut ast::Type)>(file: &mut ast::File, mut f: F) {
    let mut visit_type_helper = |type_: &mut ast::Type| {
        f(type_);
        type_.generic_types.iter_mut().for_each(|t| f(t));
    };

    match file.item {
        ast::Item::Interface(ref mut i) => {
            i.elements.iter_mut().for_each(|el| match el {
                ast::InterfaceElement::Method(m) => {
                    visit_type_helper(&mut m.return_type);
                    m.args.iter_mut().for_each(|arg| {
                        visit_type_helper(&mut arg.arg_type);
                    })
                }
                ast::InterfaceElement::Const(c) => {
                    visit_type_helper(&mut c.const_type);
                }
            });
        }
        ast::Item::Parcelable(ref mut p) => {
            p.members.iter_mut().for_each(|m| {
                visit_type_helper(&mut m.member_type);
            });
        }
        ast::Item::Enum(_) => (),
    }
}

fn walk_methods<F: FnMut(&mut ast::Method)>(file: &mut ast::File, mut f: F) {
    match file.item {
        ast::Item::Interface(ref mut i) => {
            i.elements.iter_mut().for_each(|el| match el {
                ast::InterfaceElement::Method(m) => f(m),
                ast::InterfaceElement::Const(_) => (),
            });
        }
        ast::Item::Parcelable(_) => (),
        ast::Item::Enum(_) => (),
    }
}

fn walk_args<F: FnMut(&mut ast::Arg)>(file: &mut ast::File, mut f: F) {
    match file.item {
        ast::Item::Interface(ref mut i) => {
            i.elements.iter_mut().for_each(|el| match el {
                ast::InterfaceElement::Method(m) => m.args.iter_mut().for_each(|arg| {
                    f(arg);
                }),
                ast::InterfaceElement::Const(_) => (),
            });
        }
        ast::Item::Parcelable(_) => (),
        ast::Item::Enum(_) => (),
    }
}

fn resolve_types(file: &mut ast::File, diagnostics: &mut Vec<Diagnostic>) -> HashSet<String> {
    let imports: Vec<String> = file
        .imports
        .iter()
        .map(|i| i.get_qualified_name())
        .collect();

    let mut resolved = HashSet::new();

    walk_types(file, |type_: &mut ast::Type| {
        if type_.kind == ast::TypeKind::Custom && type_.definition.is_none() {
            if let Some(import) = imports
                .iter()
                .find(|i| &type_.name == *i || i.ends_with(&format!(".{}", type_.name)))
            {
                resolved.insert(import.clone());
                type_.definition = Some(import.clone());
            } else {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Error,
                    range: type_.symbol_range.clone(),
                    message: format!("Unknown type `{}`", type_.name),
                    context_message: Some("unknown type".to_owned()),
                    hint: None,
                    related_infos: Vec::new(),
                });
            }
        }
    });

    resolved
}

fn check_imports(
    imports: &[ast::Import],
    resolved: &HashSet<String>,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // array of Import -> map of "qualified name" -> Import
    let imports: HashMap<String, &ast::Import> =
        imports.iter().fold(HashMap::new(), |mut map, import| {
            match map.entry(import.get_qualified_name()) {
                hash_map::Entry::Occupied(previous) => {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        range: import.symbol_range.clone(),
                        message: format!("Duplicated import `{}`", import.get_qualified_name()),
                        context_message: Some("duplicated import".to_owned()),
                        hint: None,
                        related_infos: Vec::from([diagnostic::RelatedInfo {
                            message: "previous location".to_owned(),
                            range: previous.get().symbol_range.clone(),
                        }]),
                    });
                }
                hash_map::Entry::Vacant(v) => {
                    v.insert(import);
                }
            }
            map
        });

    for (qualified_import, import) in imports.into_iter() {
        if !defined.contains_key(&qualified_import) {
            // No item can be found with the given import path
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                range: import.symbol_range.clone(),
                message: format!("Unresolved import `{}`", import.name),
                context_message: Some("unresolved import".to_owned()),
                hint: None,
                related_infos: Vec::new(),
            });
        } else if !resolved.contains(&qualified_import) {
            // No type resolved for this import
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Warning,
                range: import.symbol_range.clone(),
                message: format!("Unused import `{}`", import.name),
                context_message: Some("unused import".to_owned()),
                hint: None,
                related_infos: Vec::new(),
            });
        }
    }
}

fn check_types(
    file: &mut ast::File,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    walk_types(file, |type_: &mut ast::Type| match &type_.kind {
        ast::TypeKind::Map => {
            let key_type = &type_.generic_types[0];
            let value_type = &type_.generic_types[1];
            let forbidden_key = is_collection_generic_type_forbidden(key_type, defined);
            let forbidden_value = is_collection_generic_type_forbidden(value_type, defined);
            if forbidden_key && forbidden_value {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Error,
                    message: format!(
                        "Invalid map parameters `{}`, `{}`",
                        key_type.name, value_type.name
                    ),
                    context_message: Some("invalid parameters".to_owned()),
                    range: ast::Range {
                        start: key_type.symbol_range.start.clone(),
                        end: value_type.symbol_range.end.clone(),
                    },
                    hint: Some("key and value must be objects".to_owned()),
                    related_infos: Vec::new(),
                });
            } else if forbidden_key {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Error,
                    message: format!("Invalid map key `{}`", key_type.name),
                    context_message: Some("invalid key".to_owned()),
                    range: key_type.symbol_range.clone(),
                    hint: Some("key must be an object".to_owned()),
                    related_infos: Vec::new(),
                });
            } else if forbidden_value {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Error,
                    message: format!("Invalid map value `{}`", value_type.name),
                    context_message: Some("invalid value".to_owned()),
                    range: value_type.symbol_range.clone(),
                    hint: Some("value must be an object".to_owned()),
                    related_infos: Vec::new(),
                });
            }
        }
        ast::TypeKind::List => {
            let value_type = &type_.generic_types[0];
            if is_collection_generic_type_forbidden(value_type, defined) {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Error,
                    message: format!("Invalid list parameter `{}`", value_type.name),
                    context_message: Some("invalid parameter".to_owned()),
                    range: value_type.symbol_range.clone(),
                    hint: Some("must be an object".to_owned()),
                    related_infos: Vec::new(),
                });
            }
        }
        ast::TypeKind::Array => {
            let value_type = &type_.generic_types[0];
            if is_array_generic_type_forbidden(value_type, defined) {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Error,
                    message: format!("Invalid array parameter `{}`", value_type.name),
                    context_message: Some("invalid parameter".to_owned()),
                    range: value_type.symbol_range.clone(),
                    hint: Some("must be a primitive or an enum".to_owned()),
                    related_infos: Vec::new(),
                });
            }
        }
        _ => {}
    });
}

fn check_methods(
    file: &mut ast::File,
    _defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    walk_methods(file, |method: &mut ast::Method| {
        if method.oneway && method.return_type.kind != ast::TypeKind::Void {
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                message: format!(
                    "Invalid return type of async method `{}`",
                    method.return_type.name,
                ),
                context_message: Some("must be void".to_owned()),
                range: method.return_type.symbol_range.clone(),
                hint: Some("return type of async methods must be `void`".to_owned()),
                related_infos: Vec::new(),
            });
        }
    });
}

fn check_args(
    file: &mut ast::File,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    walk_args(file, |arg: &mut ast::Arg| {
        // Range of direction (or position of arg type)
        let range = match &arg.direction {
            ast::Direction::In(range)
            | ast::Direction::Out(range)
            | ast::Direction::InOut(range) => range.clone(),
            ast::Direction::Unspecified => ast::Range {
                start: arg.arg_type.symbol_range.start.clone(),
                end: arg.arg_type.symbol_range.start.clone(),
            },
        };

        match get_requirement_for_direction(&arg.arg_type, defined) {
            RequirementForDirection::DirectionRequired => {
                if arg.direction == ast::Direction::Unspecified {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        message: format!("Missing direction for {}", arg.arg_type.name,),
                        context_message: Some("missing direction".to_owned()),
                        range,
                        hint: Some("direction is required for objects".to_owned()),
                        related_infos: Vec::new(),
                    });
                }
            }
            RequirementForDirection::CanOnlyBeInOrUnspecified => {
                if !matches!(
                    arg.direction,
                    ast::Direction::Unspecified | ast::Direction::In(_)
                ) {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        message: format!("Invalid direction for {}`", arg.arg_type.name),
                        context_message: Some("invalid direction".to_owned()),
                        range,
                        hint: Some("can only be `in` or omitted".to_owned()),
                        related_infos: Vec::new(),
                    });
                }
            }
            RequirementForDirection::NoRequirement => (),
        }
    });
}

enum RequirementForDirection {
    DirectionRequired,
    CanOnlyBeInOrUnspecified,
    NoRequirement,
}

fn get_requirement_for_direction(
    type_: &ast::Type,
    defined: &HashMap<String, ast::ItemKind>,
) -> RequirementForDirection {
    match type_.kind {
        ast::TypeKind::Primitive => RequirementForDirection::CanOnlyBeInOrUnspecified,
        ast::TypeKind::Void => RequirementForDirection::CanOnlyBeInOrUnspecified,
        ast::TypeKind::Array => RequirementForDirection::DirectionRequired,
        ast::TypeKind::Map | ast::TypeKind::List => RequirementForDirection::DirectionRequired,
        ast::TypeKind::String => RequirementForDirection::CanOnlyBeInOrUnspecified,
        ast::TypeKind::Custom => {
            if let Some(ref def) = type_.definition {
                match defined.get(def) {
                    Some(ast::ItemKind::Parcelable) => RequirementForDirection::DirectionRequired,
                    Some(ast::ItemKind::Interface) => RequirementForDirection::DirectionRequired,
                    Some(ast::ItemKind::Enum) => RequirementForDirection::CanOnlyBeInOrUnspecified,
                    None => RequirementForDirection::NoRequirement,
                }
            } else {
                RequirementForDirection::NoRequirement
            }
        }
        ast::TypeKind::Invalid => RequirementForDirection::NoRequirement,
    }
}

fn is_collection_generic_type_forbidden(
    type_: &ast::Type,
    defined: &HashMap<String, ast::ItemKind>,
) -> bool {
    match type_.kind {
        ast::TypeKind::Array => false,
        ast::TypeKind::Invalid => false, // we don't know
        ast::TypeKind::List => false,
        ast::TypeKind::Map => false,
        ast::TypeKind::Primitive => true,
        ast::TypeKind::String => false,
        ast::TypeKind::Void => true,
        ast::TypeKind::Custom => {
            if let Some(ref def) = type_.definition {
                match defined.get(def) {
                    Some(ast::ItemKind::Parcelable) => false,
                    Some(ast::ItemKind::Interface) => false,
                    Some(ast::ItemKind::Enum) => true, // enum is backed by a primitive
                    None => false,                     // we don't know
                }
            } else {
                false // we don't know
            }
        }
    }
}

fn is_array_generic_type_forbidden(
    type_: &ast::Type,
    defined: &HashMap<String, ast::ItemKind>,
) -> bool {
    match type_.kind {
        ast::TypeKind::Array => false,
        ast::TypeKind::Invalid => false, // not applicable
        ast::TypeKind::List => true,
        ast::TypeKind::Map => true,
        ast::TypeKind::Primitive => false,
        ast::TypeKind::String => true,
        ast::TypeKind::Void => true,
        ast::TypeKind::Custom => {
            if let Some(ref def) = type_.definition {
                match defined.get(def) {
                    Some(ast::ItemKind::Parcelable) => true,
                    Some(ast::ItemKind::Interface) => true,
                    Some(ast::ItemKind::Enum) => false, // enum is backed by a primitive
                    None => false,                      // we don't know
                }
            } else {
                false // we don't know
            }
        }
    }
}
