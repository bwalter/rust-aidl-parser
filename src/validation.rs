use std::collections::{hash_map, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::ast;
use crate::diagnostic::{self, Diagnostic, DiagnosticKind};
use crate::parser::ParseFileResult;
use crate::traverse;

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
            let mut ast = match fr.ast {
                Some(f) => f,
                None => return (id, ParseFileResult { ast: None, ..fr }),
            };

            // Map qualified name -> import
            let imports: HashSet<String> =
                ast.imports.iter().map(|i| i.get_qualified_name()).collect();

            // Resolve types (check custom types and set definition if found in imports)
            let resolved = resolve_types(&mut ast, &imports, &mut fr.diagnostics);

            // Check imports (e.g. unresolved, unused, duplicated)
            check_imports(&ast.imports, &resolved, &keys, &mut fr.diagnostics);

            // Check types (e.g.: map parameters)
            check_types(&ast, &keys, &mut fr.diagnostics);

            if let ast::Item::Interface(ref mut interface) = ast.item {
                // Set up oneway interface (adjust methods to be oneway)
                set_up_oneway_interface(interface, &mut fr.diagnostics);
            }

            // Check methods (e.g.: return type of async methods)
            check_methods(&ast, &keys, &mut fr.diagnostics);

            // Sort diagnostics by line
            fr.diagnostics.sort_by_key(|d| d.range.start.line_col.0);

            (
                id,
                ParseFileResult {
                    ast: Some(ast),
                    ..fr
                },
            )
        })
        .collect()
}

fn set_up_oneway_interface(interface: &mut ast::Interface, diagnostics: &mut Vec<Diagnostic>) {
    if !interface.oneway {
        return;
    }

    interface
        .elements
        .iter_mut()
        .filter_map(|el| match el {
            ast::InterfaceElement::Const(_) => None,
            ast::InterfaceElement::Method(m) => Some(m),
        })
        .for_each(|method| {
            if method.oneway {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Warning,
                    range: method.oneway_range.clone(),
                    message: format!(
                        "Method `{}` of oneway interface does not need to be marked as oneway",
                        method.name
                    ),
                    context_message: Some("redundant oneway".to_owned()),
                    hint: None,
                    related_infos: Vec::from([diagnostic::RelatedInfo {
                        message: "oneway interface".to_owned(),
                        range: interface.symbol_range.clone(),
                    }]),
                });
            } else {
                // Force me
                method.oneway = true;
            }
        });
}

fn resolve_types(
    ast: &mut ast::Aidl,
    imports: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<String> {
    let mut resolved = HashSet::new();
    traverse::walk_types_mut(ast, |type_: &mut ast::Type| {
        resolve_type(type_, imports, diagnostics);
        if let Some(definition) = &type_.definition {
            resolved.insert(definition.clone());
        }
    });

    resolved
}

fn resolve_type(
    type_: &mut ast::Type,
    imports: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if type_.kind == ast::TypeKind::Custom && type_.definition.is_none() {
        if let Some(import_path) = imports.iter().find(|import_path| {
            &type_.name == *import_path || import_path.ends_with(&format!(".{}", type_.name))
        }) {
            type_.definition = Some(import_path.to_owned());
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
}

fn check_imports(
    imports: &[ast::Import],
    resolved: &HashSet<String>,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // - generate diagnostics for duplicated, unsued and unresolved imports
    // - create array of Import -> map of "qualified name" -> Import
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
    ast: &ast::Aidl,
    keys: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    traverse::walk_types(ast, |type_: &ast::Type| {
        check_type(type_, keys, diagnostics)
    });
}

fn check_type(
    type_: &ast::Type,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &type_.kind {
        ast::TypeKind::Array => {
            let value_type = &type_.generic_types[0];
            check_array_element(value_type, defined, diagnostics);
        }
        ast::TypeKind::List => {
            // Handle wrong number of generics
            match type_.generic_types.len() {
                0 => {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Warning,
                        message: String::from("Declaring a non-generic list is not recommended"),
                        context_message: Some("non-generic list".to_owned()),
                        range: type_.symbol_range.clone(),
                        hint: Some("consider adding a parameter (e.g.: List<String>)".to_owned()),
                        related_infos: Vec::new(),
                    });
                    return;
                }
                1 => (),
                _ => unreachable!(), // handled via lalrpop rule
            }

            let value_type = &type_.generic_types[0];
            check_list_element(value_type, defined, diagnostics);
        }
        ast::TypeKind::Map => {
            // Handle wrong number of generics
            match type_.generic_types.len() {
                0 => {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Warning,
                        message: String::from("Declaring a non-generic map is not recommended"),
                        context_message: Some("non-generic map".to_owned()),
                        range: type_.symbol_range.clone(),
                        hint: Some(
                            "consider adding key and value parameters (e.g.: Map<String, String>)"
                                .to_owned(),
                        ),
                        related_infos: Vec::new(),
                    });
                    return;
                }
                2 => (),
                _ => unreachable!(), // handled via lalrpop rule
            }

            // Handle invalid generic types
            check_map_key(&type_.generic_types[0], defined, diagnostics);
            check_map_value(&type_.generic_types[1], defined, diagnostics);
        }
        _ => {}
    };
}

// TODO: check no duplicates, check valid method IDs (either none or all, no duplicates)
fn check_methods(
    file: &ast::Aidl,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut method_names: HashMap<String, &ast::Method> = HashMap::new();
    let mut first_method_without_id: Option<&ast::Method> = None;
    let mut first_method_with_id: Option<&ast::Method> = None;
    let mut method_ids: HashMap<u32, &ast::Method> = HashMap::new();

    traverse::walk_methods(file, |method: &ast::Method| {
        // Check individual method (e.g. return value, args, ...)
        check_method(method, defined, diagnostics);

        if let Some(previous) = method_names.get(&method.name) {
            // Found already exists => ERROR
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                range: method.symbol_range.clone(),
                message: format!("Duplicated method name `{}`", method.name),
                context_message: Some("duplicated method name".to_owned()),
                hint: None,
                related_infos: Vec::from([diagnostic::RelatedInfo {
                    message: "previous location".to_owned(),
                    range: previous.symbol_range.clone(),
                }]),
            });
            return;
        }

        method_names.insert(method.name.clone(), method);

        let is_mixed_now_with_id = first_method_with_id.is_none()
            && first_method_without_id.is_some()
            && method.value.is_some();
        let is_mixed_now_without_id =
            first_method_without_id.is_none() && !method_ids.is_empty() && method.value.is_none();

        if is_mixed_now_with_id || is_mixed_now_without_id {
            let info_previous = if is_mixed_now_with_id {
                diagnostic::RelatedInfo {
                    message: "method without id".to_owned(),
                    range: first_method_without_id
                        .as_ref()
                        .unwrap()
                        .value_range
                        .clone(),
                }
            } else {
                diagnostic::RelatedInfo {
                    message: "method with id".to_owned(),
                    range: first_method_with_id.as_ref().unwrap().value_range.clone(),
                }
            };

            // Methods are mixed (with/without id)
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                range: method.value_range.clone(),
                message: String::from("Mixed usage of method ids"),
                context_message: None,
                hint: Some(String::from(
                    "Either all methods should have an id or none of them",
                )),
                related_infos: Vec::from([info_previous]),
            });
        }

        if method.value.is_some() {
            // First method with id
            if first_method_with_id.is_none() {
                first_method_with_id = Some(method);
            }
        } else {
            // First method without id
            if first_method_without_id.is_none() {
                first_method_without_id = Some(method);
            }
        }

        if let Some(id) = method.value {
            match method_ids.entry(id) {
                hash_map::Entry::Occupied(oe) => {
                    // Method id already defined
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        range: method.value_range.clone(),
                        message: String::from("Duplicated method id"),
                        context_message: Some("duplicated import".to_owned()),
                        hint: None,
                        related_infos: Vec::from([diagnostic::RelatedInfo {
                            range: oe.get().value_range.clone(),
                            message: String::from("previous method"),
                        }]),
                    });
                }
                hash_map::Entry::Vacant(ve) => {
                    // First method with this id
                    ve.insert(method);
                }
            }
        }
    });
}

fn check_method(
    method: &ast::Method,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

    check_method_args(method, defined, diagnostics);
}

// Check arg direction (e.g. depending on type or method being oneway)
fn check_method_args(
    method: &ast::Method,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for arg in &method.args {
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

        match get_requirement_for_arg_direction(&arg.arg_type, defined) {
            RequirementForArgDirection::DirectionRequired => {
                if arg.direction == ast::Direction::Unspecified {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        message: format!("Missing direction for `{}`", arg.arg_type.name,),
                        context_message: Some("missing direction".to_owned()),
                        range: range.clone(),
                        hint: Some("direction is required for objects".to_owned()),
                        related_infos: Vec::new(),
                    });
                }
            }
            RequirementForArgDirection::CanOnlyBeInOrUnspecified => {
                if !matches!(
                    arg.direction,
                    ast::Direction::Unspecified | ast::Direction::In(_)
                ) {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        message: format!("Invalid direction for `{}`", arg.arg_type.name),
                        context_message: Some("invalid direction".to_owned()),
                        range: range.clone(),
                        hint: Some("can only be `in` or omitted".to_owned()),
                        related_infos: Vec::new(),
                    });
                }
            }
            RequirementForArgDirection::NoRequirement => (),
        }

        if method.oneway
            && matches!(
                arg.direction,
                ast::Direction::Out(_) | ast::Direction::InOut(_)
            )
        {
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                message: format!("Invalid direction for `{}`", arg.arg_type.name),
                context_message: Some("invalid direction".to_owned()),
                range,
                hint: Some(
                    "arguments of oneway methods can be neither `out` nor `inout`".to_owned(),
                ),
                related_infos: Vec::new(),
            });
        }
    }
}

enum RequirementForArgDirection {
    DirectionRequired,
    CanOnlyBeInOrUnspecified,
    NoRequirement,
}

fn get_requirement_for_arg_direction(
    type_: &ast::Type,
    defined: &HashMap<String, ast::ItemKind>,
) -> RequirementForArgDirection {
    match type_.kind {
        ast::TypeKind::Primitive => RequirementForArgDirection::CanOnlyBeInOrUnspecified,
        ast::TypeKind::Void => RequirementForArgDirection::CanOnlyBeInOrUnspecified,
        ast::TypeKind::Array => RequirementForArgDirection::DirectionRequired,
        ast::TypeKind::Map | ast::TypeKind::List => RequirementForArgDirection::DirectionRequired,
        ast::TypeKind::String => RequirementForArgDirection::CanOnlyBeInOrUnspecified,
        ast::TypeKind::Custom => {
            if let Some(ref def) = type_.definition {
                match defined.get(def) {
                    Some(ast::ItemKind::Parcelable) => {
                        RequirementForArgDirection::DirectionRequired
                    }
                    Some(ast::ItemKind::Interface) => {
                        RequirementForArgDirection::CanOnlyBeInOrUnspecified
                    }
                    Some(ast::ItemKind::Enum) => {
                        RequirementForArgDirection::CanOnlyBeInOrUnspecified
                    }
                    None => RequirementForArgDirection::NoRequirement,
                }
            } else {
                RequirementForArgDirection::NoRequirement
            }
        }
        ast::TypeKind::Invalid => RequirementForArgDirection::NoRequirement,
    }
}

// Can only have one dimensional arrays
// "Binder" type cannot be an array (with interface element...)
// TODO: not allowed for ParcelableHolder, allowed for IBinder, ...
fn check_array_element(
    type_: &ast::Type,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match type_.kind {
        ast::TypeKind::Array => {
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                message: String::from("Unsupported multi-dimensional array"),
                context_message: Some("unsupported array".to_owned()),
                range: type_.symbol_range.clone(),
                hint: Some("must be one-dimensional".to_owned()),
                related_infos: Vec::new(),
            });
            return;
        }
        ast::TypeKind::Invalid => return,   // not applicable
        ast::TypeKind::Primitive => return, // OK
        ast::TypeKind::String => {
            // String: OK, CharSequence: error
            if type_.name == "String" {
                return;
            }
        }
        ast::TypeKind::List => (),
        ast::TypeKind::Map => (),
        ast::TypeKind::Void => (),
        ast::TypeKind::Custom => {
            if let Some(ref def) = type_.definition {
                match defined.get(def) {
                    Some(ast::ItemKind::Parcelable) => return, // OK: it is allowed for Parcelable...
                    Some(ast::ItemKind::Interface) => (),      // "Binder" type cannot be an array
                    Some(ast::ItemKind::Enum) => return,       // OK: enum is backed by a primitive
                    None => return,                            // we don't know
                }
            } else {
                return; // we don't know
            }
        }
    }

    diagnostics.push(Diagnostic {
        kind: DiagnosticKind::Error,
        message: format!("Invalid array element `{}`", type_.name),
        context_message: Some("invalid parameter".to_owned()),
        range: type_.symbol_range.clone(),
        hint: Some("must be a primitive, an enum, a String, a parcelable or a IBinder".to_owned()),
        related_infos: Vec::new(),
    });
}

// List<T> supports parcelable/union, String, IBinder, and ParcelFileDescriptor
// TODO: IBinder + ParcelFileDescriptor
fn check_list_element(
    type_: &ast::Type,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match type_.kind {
        ast::TypeKind::Array => (),
        ast::TypeKind::Invalid => return, // we don't know
        ast::TypeKind::List => (),
        ast::TypeKind::Map => (),
        ast::TypeKind::Primitive => (),
        ast::TypeKind::String => {
            // String: OK, CharSequence: error
            if type_.name == "String" {
                return;
            }
        }
        ast::TypeKind::Void => (),
        ast::TypeKind::Custom => {
            if let Some(ref def) = type_.definition {
                match defined.get(def) {
                    Some(ast::ItemKind::Parcelable) => return, // OK
                    Some(ast::ItemKind::Interface) => (),
                    Some(ast::ItemKind::Enum) => (), // enum is backed by a primitive
                    None => return,                  // we don't know
                }
            } else {
                return; // we don't know
            }
        }
    }

    diagnostics.push(Diagnostic {
        kind: DiagnosticKind::Error,
        message: format!("Invalid list element `{}`", type_.name),
        context_message: Some("invalid element".to_owned()),
        range: type_.symbol_range.clone(),
        hint: Some(
            "must be a parcelable/enum, a String, a IBinder or a ParcelFileDescriptor".to_owned(),
        ),
        related_infos: Vec::new(),
    });
}

// The type of key in map must be String
fn check_map_key(
    type_: &ast::Type,
    _defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !matches!(type_.kind, ast::TypeKind::String if type_.name == "String") {
        diagnostics.push(Diagnostic {
            kind: DiagnosticKind::Error,
            message: format!("Invalid map key `{}`", type_.name),
            context_message: Some("invalid map key".to_owned()),
            range: type_.symbol_range.clone(),
            hint: Some(
                "must be a parcelable/enum, a String, a IBinder or a ParcelFileDescriptor"
                    .to_owned(),
            ),
            related_infos: Vec::new(),
        });
    }
}

// A generic type cannot have any primitive type parameters
fn check_map_value(
    type_: &ast::Type,
    defined: &HashMap<String, ast::ItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match type_.kind {
        ast::TypeKind::Array => return,   // OK
        ast::TypeKind::Invalid => return, // we don't know
        ast::TypeKind::List => return,    // OK
        ast::TypeKind::Map => return,     // OK
        ast::TypeKind::String => return,  // OK
        ast::TypeKind::Primitive => (),
        ast::TypeKind::Void => (),
        ast::TypeKind::Custom => {
            if let Some(ref def) = type_.definition {
                match defined.get(def) {
                    Some(ast::ItemKind::Parcelable) => return,
                    Some(ast::ItemKind::Interface) => return,
                    Some(ast::ItemKind::Enum) => (), // enum is backed by a primitive
                    None => return,                  // we don't know
                }
            } else {
                return; // we don't know
            }
        }
    }

    diagnostics.push(Diagnostic {
        kind: DiagnosticKind::Error,
        message: format!("Invalid map value `{}`", type_.name),
        context_message: Some("invalid map value".to_owned()),
        range: type_.symbol_range.clone(),
        hint: Some("cannot not be a primitive".to_owned()),
        related_infos: Vec::new(),
    });
}

#[cfg(test)]
mod tests {
    use self::utils::create_method_with_name_and_id;

    use super::*;
    use crate::ast;

    #[test]
    fn test_check_imports() {
        let imports = Vec::from([
            utils::create_import("TestParcelable", 1),
            utils::create_import("TestParcelable", 2),
            utils::create_import("TestInterface", 3),
            utils::create_import("UnusedEnum", 4),
            utils::create_import("NonExisting", 5),
        ]);

        let resolved = HashSet::from([
            "test.path.TestParcelable".into(),
            "test.path.TestInterface".into(),
        ]);
        let defined = HashMap::from([
            ("test.path.TestParcelable".into(), ast::ItemKind::Parcelable),
            ("test.path.TestInterface".into(), ast::ItemKind::Interface),
            ("test.path.UnusedEnum".into(), ast::ItemKind::Enum),
        ]);
        let mut diagnostics = Vec::new();

        check_imports(&imports, &resolved, &defined, &mut diagnostics);

        diagnostics.sort_by_key(|d| d.range.start.line_col.0);

        assert_eq!(diagnostics.len(), 3);

        let d = &diagnostics[0];
        assert_eq!(d.kind, DiagnosticKind::Error);
        assert!(d.message.contains("Duplicated import"));
        assert!(d.range.start.line_col.0 == 2);

        let d = &diagnostics[1];
        assert_eq!(d.kind, DiagnosticKind::Warning);
        assert!(d.message.contains("Unused import `UnusedEnum`"));
        assert!(d.range.start.line_col.0 == 4);

        let d = &diagnostics[2];
        assert_eq!(d.kind, DiagnosticKind::Error);
        assert!(d.message.contains("Unresolved import `NonExisting`"));
        assert!(d.range.start.line_col.0 == 5);
    }

    #[test]
    fn test_check_type() {
        let keys = HashMap::from([
            ("test.TestParcelable".into(), ast::ItemKind::Parcelable),
            ("test.TestInterface".into(), ast::ItemKind::Interface),
            ("test.TestEnum".into(), ast::ItemKind::Enum),
        ]);

        // Valid arrays
        for t in [
            utils::create_int(0),
            utils::create_simple_type("String", ast::TypeKind::String, 0),
            utils::create_custom_type("test.TestParcelable", 0),
            utils::create_custom_type("test.TestEnum", 0),
        ]
        .into_iter()
        {
            let array = utils::create_array(t, 0);
            let mut diagnostics = Vec::new();
            check_type(&array, &keys, &mut diagnostics);
            assert_eq!(diagnostics.len(), 0);
        }

        // Multi-dimensional array
        let mut diagnostics = Vec::new();
        let array = utils::create_array(utils::create_array(utils::create_int(0), 0), 0);
        check_type(&array, &HashMap::new(), &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("Unsupported multi-dimensional array"));

        // Invalid arrays
        for t in [
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_custom_type("test.TestInterface", 0),
            utils::create_simple_type("CharSequence", ast::TypeKind::String, 0),
            utils::create_simple_type("void", ast::TypeKind::Void, 0),
        ]
        .into_iter()
        {
            let array = utils::create_array(t, 0);
            let mut diagnostics = Vec::new();
            check_type(&array, &keys, &mut diagnostics);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("Invalid array"));
        }

        // Valid list
        for t in [
            utils::create_simple_type("String", ast::TypeKind::String, 0),
            utils::create_custom_type("test.TestParcelable", 0),
        ]
        .into_iter()
        {
            let list = utils::create_list(Some(t), 0);
            let mut diagnostics = Vec::new();
            check_type(&list, &keys, &mut diagnostics);
            assert_eq!(diagnostics.len(), 0);
        }

        // Non-generic list -> warning
        let mut diagnostics = Vec::new();
        let list = utils::create_list(None, 105);
        check_type(&list, &HashMap::new(), &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::Warning);
        assert_eq!(diagnostics[0].range.start.line_col.0, 105);
        assert_eq!(diagnostics[0].range.end.line_col.0, 105);
        assert!(diagnostics[0].message.contains("not recommended"));

        // Invalid lists
        for t in [
            utils::create_simple_type("void", ast::TypeKind::Void, 0),
            utils::create_simple_type("CharSequence", ast::TypeKind::String, 0),
            utils::create_array(utils::create_int(0), 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_custom_type("test.TestInterface", 0),
            utils::create_custom_type("test.TestEnum", 0),
        ]
        .into_iter()
        {
            let list = utils::create_list(Some(t), 0);
            let mut diagnostics = Vec::new();
            check_type(&list, &keys, &mut diagnostics);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("Invalid list"));
        }

        // Valid map
        for vt in [
            utils::create_simple_type("String", ast::TypeKind::String, 0),
            utils::create_array(utils::create_int(0), 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_custom_type("test.TestParcelable", 0),
            utils::create_custom_type("test.TestInterface", 0),
        ]
        .into_iter()
        {
            let map = utils::create_map(
                Some((
                    utils::create_simple_type("String", ast::TypeKind::String, 0),
                    vt,
                )),
                0,
            );
            let mut diagnostics = Vec::new();
            check_type(&map, &keys, &mut diagnostics);
            assert_eq!(diagnostics.len(), 0);
        }

        // Non-generic map -> warning
        let mut diagnostics = Vec::new();
        let map = utils::create_map(None, 205);
        check_type(&map, &HashMap::new(), &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::Warning);
        assert_eq!(diagnostics[0].range.start.line_col.0, 205);
        assert_eq!(diagnostics[0].range.end.line_col.0, 205);
        assert!(diagnostics[0].message.contains("not recommended"));

        // Invalid map keys
        for kt in [
            utils::create_simple_type("void", ast::TypeKind::Void, 0),
            utils::create_simple_type("CharSequence", ast::TypeKind::String, 0),
            utils::create_array(utils::create_int(0), 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_custom_type("test.TestParcelable", 0),
            utils::create_custom_type("test.TestInterface", 0),
            utils::create_custom_type("test.TestEnum", 0),
            utils::create_simple_type("CharSequence", ast::TypeKind::String, 0),
        ]
        .into_iter()
        {
            let map = utils::create_map(
                Some((
                    kt,
                    utils::create_simple_type("String", ast::TypeKind::String, 0),
                )),
                0,
            );
            let mut diagnostics = Vec::new();
            check_type(&map, &keys, &mut diagnostics);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("Invalid map"));
        }

        // Invalid map values
        for vt in [
            utils::create_simple_type("void", ast::TypeKind::Void, 0),
            utils::create_custom_type("test.TestEnum", 0),
        ]
        .into_iter()
        {
            let map = utils::create_map(
                Some((
                    utils::create_simple_type("String", ast::TypeKind::String, 0),
                    vt,
                )),
                0,
            );
            let mut diagnostics = Vec::new();
            check_type(&map, &keys, &mut diagnostics);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("Invalid map"));
        }
    }

    #[test]
    fn test_set_up_oneway() {
        let blocking_method = utils::create_method_with_name_and_id("blocking_method", None, 20);

        let mut oneway_method = utils::create_method_with_name_and_id("oneway_method", None, 10);
        oneway_method.oneway = true;

        let mut interface = ast::Interface {
            oneway: false,
            name: "testMethod".into(),
            elements: [blocking_method, oneway_method]
                .into_iter()
                .map(ast::InterfaceElement::Method)
                .collect(),
            annotations: Vec::new(),
            doc: None,
            full_range: utils::create_range(5),
            symbol_range: utils::create_range(5),
        };

        // "normal" interface -> no change, no diagnostic
        assert!(!interface.oneway);
        let mut diagnostics = Vec::new();
        set_up_oneway_interface(&mut interface, &mut diagnostics);
        assert!(!interface.elements[0].as_method().unwrap().oneway,);
        assert!(interface.elements[1].as_method().unwrap().oneway,);
        assert_eq!(diagnostics.len(), 0);

        interface.oneway = true;

        // oneway interface -> blocking method will be oneway, oneway method will cause a warning
        let mut diagnostics = Vec::new();
        set_up_oneway_interface(&mut interface, &mut diagnostics);
        assert!(interface.elements[0].as_method().unwrap().oneway);
        assert!(interface.elements[1].as_method().unwrap().oneway);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::Warning);
        assert!(diagnostics[0]
            .message
            .contains("does not need to be marked as oneway"));
        assert_eq!(diagnostics[0].related_infos.len(), 1);
        assert_eq!(diagnostics[0].related_infos[0].range.start.line_col.0, 5);
    }

    #[test]
    fn test_check_method() {
        // Non-async method with return value -> ok
        let void_method = ast::Method {
            oneway: false,
            name: "test".into(),
            return_type: utils::create_simple_type("void", ast::TypeKind::Void, 0),
            args: Vec::new(),
            annotations: Vec::new(),
            value: None,
            doc: None,
            symbol_range: utils::create_range(0),
            full_range: utils::create_range(0),
            value_range: utils::create_range(0),
            oneway_range: utils::create_range(0),
        };
        let mut diagnostics = Vec::new();
        check_method(&void_method, &HashMap::new(), &mut diagnostics);
        assert_eq!(diagnostics.len(), 0);

        // Oneway method returning void -> ok
        let mut oneway_void_method = void_method.clone();
        oneway_void_method.oneway = true;
        let mut diagnostics = Vec::new();
        check_method(&oneway_void_method, &HashMap::new(), &mut diagnostics);
        assert_eq!(diagnostics.len(), 0);

        // Async method with return value -> error
        let mut oneway_int_method = oneway_void_method.clone();
        oneway_int_method.return_type = utils::create_int(0);
        let mut diagnostics = Vec::new();
        check_method(&oneway_int_method, &HashMap::new(), &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("Invalid return type of async"));
    }

    #[test]
    fn test_check_method_ids() {
        let methods = Vec::from([
            create_method_with_name_and_id("method0", None, 10),
            create_method_with_name_and_id("method1", Some(1), 20),
            create_method_with_name_and_id("method2", Some(2), 30),
            create_method_with_name_and_id("method2", Some(3), 40),
            create_method_with_name_and_id("method3", Some(1), 50),
        ]);

        let ast = ast::Aidl {
            package: ast::Package {
                name: "test.package".into(),
                symbol_range: utils::create_range(0),
                full_range: utils::create_range(0),
            },
            imports: Vec::new(),
            item: ast::Item::Interface(ast::Interface {
                oneway: false,
                name: "testMethod".into(),
                elements: methods
                    .into_iter()
                    .map(ast::InterfaceElement::Method)
                    .collect(),
                annotations: Vec::new(),
                doc: None,
                full_range: utils::create_range(0),
                symbol_range: utils::create_range(0),
            }),
        };

        let mut diagnostics = Vec::new();
        check_methods(&ast, &HashMap::new(), &mut diagnostics);

        assert_eq!(diagnostics.len(), 3);

        // Mixed methods with/without id
        assert_eq!(diagnostics[0].kind, DiagnosticKind::Error);
        assert!(diagnostics[0].message.contains("Mixed usage of method id"));
        assert_eq!(diagnostics[0].range.start.line_col.0, 21);

        // Duplicated method name
        assert_eq!(diagnostics[1].kind, DiagnosticKind::Error);
        assert!(diagnostics[1].message.contains("Duplicated method name"));
        assert_eq!(diagnostics[1].range.start.line_col.0, 40);

        // Duplicated method id
        assert_eq!(diagnostics[2].kind, DiagnosticKind::Error);
        assert!(diagnostics[2].message.contains("Duplicated method id"));
        assert_eq!(diagnostics[2].range.start.line_col.0, 51);
    }

    #[test]
    fn test_check_method_args() {
        let base_method = ast::Method {
            oneway: false,
            name: "testMethod".into(),
            return_type: utils::create_simple_type("void", ast::TypeKind::Void, 0),
            args: Vec::new(),
            value: None,
            annotations: Vec::new(),
            doc: None,
            symbol_range: utils::create_range(0),
            full_range: utils::create_range(1),
            value_range: utils::create_range(0),
            oneway_range: utils::create_range(0),
        };

        let keys = HashMap::from([
            ("test.TestParcelable".into(), ast::ItemKind::Parcelable),
            ("test.TestInterface".into(), ast::ItemKind::Interface),
            ("test.TestEnum".into(), ast::ItemKind::Enum),
        ]);

        // Primitives, String and Interfaces can only be in or unspecified
        for t in [
            utils::create_int(0),
            utils::create_simple_type("String", ast::TypeKind::String, 0),
            utils::create_simple_type("CharSequence", ast::TypeKind::String, 0),
            utils::create_custom_type("test.TestInterface", 0),
            utils::create_custom_type("test.TestEnum", 0),
        ]
        .into_iter()
        {
            // Unspecified or In => OK
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.args = Vec::from([
                    utils::create_arg(t.clone(), ast::Direction::Unspecified),
                    utils::create_arg(t.clone(), ast::Direction::In(utils::create_range(0))),
                ]);
                check_method_args(&method, &keys, &mut diagnostics);
                assert_eq!(diagnostics.len(), 0);
            }

            // Out or InOut => ERROR
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.args = Vec::from([
                    utils::create_arg(t.clone(), ast::Direction::Out(utils::create_range(0))),
                    utils::create_arg(t, ast::Direction::InOut(utils::create_range(0))),
                ]);
                check_method_args(&method, &keys, &mut diagnostics);
                assert_eq!(diagnostics.len(), method.args.len());
                for d in diagnostics {
                    assert_eq!(d.kind, DiagnosticKind::Error);
                }
            }
        }

        // Arrays, maps and parcelables require direction
        for t in [
            utils::create_array(utils::create_int(0), 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_custom_type("test.TestParcelable", 0),
        ]
        .into_iter()
        {
            // In, Out or InOut => OK
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.args = Vec::from([
                    utils::create_arg(t.clone(), ast::Direction::In(utils::create_range(0))),
                    utils::create_arg(t.clone(), ast::Direction::Out(utils::create_range(0))),
                    utils::create_arg(t.clone(), ast::Direction::InOut(utils::create_range(0))),
                ]);
                check_method_args(&method, &keys, &mut diagnostics);
                assert_eq!(diagnostics.len(), 0);
            }

            // Unspecified => ERROR
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.args = Vec::from([utils::create_arg(t, ast::Direction::Unspecified)]);
                check_method_args(&method, &keys, &mut diagnostics);
                assert_eq!(diagnostics.len(), method.args.len());
                for d in diagnostics {
                    assert_eq!(d.kind, DiagnosticKind::Error);
                }
            }
        }

        // Arguments of oneway methods cannot be out or inout
        for t in [
            utils::create_array(utils::create_int(0), 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_custom_type("test.TestParcelable", 0),
        ]
        .into_iter()
        {
            // async + In => OK
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.oneway = true;
                method.args = Vec::from([utils::create_arg(
                    t.clone(),
                    ast::Direction::In(utils::create_range(0)),
                )]);
                check_method_args(&method, &keys, &mut diagnostics);
                assert_eq!(diagnostics.len(), 0);
            }

            // async + Out, InOut => ERROR
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.oneway = true;
                method.args = Vec::from([
                    utils::create_arg(t.clone(), ast::Direction::Out(utils::create_range(0))),
                    utils::create_arg(t, ast::Direction::InOut(utils::create_range(0))),
                ]);
                check_method_args(&method, &keys, &mut diagnostics);
                assert_eq!(diagnostics.len(), method.args.len());
                for d in diagnostics {
                    assert_eq!(d.kind, DiagnosticKind::Error);
                }
            }
        }
    }

    // Test utils
    // ---

    mod utils {
        use crate::ast;

        pub fn create_range(line: usize) -> ast::Range {
            ast::Range {
                start: ast::Position {
                    offset: 0,
                    line_col: (line, 10),
                },
                end: ast::Position {
                    offset: 0,
                    line_col: (line, 20),
                },
            }
        }

        pub fn create_import(name: &str, line: usize) -> ast::Import {
            ast::Import {
                path: "test.path".into(),
                name: name.to_owned(),
                symbol_range: create_range(line),
                full_range: create_range(line),
            }
        }

        pub fn create_int(line: usize) -> ast::Type {
            create_simple_type("int", ast::TypeKind::Primitive, line)
        }

        pub fn create_simple_type(
            name: &'static str,
            kind: ast::TypeKind,
            line: usize,
        ) -> ast::Type {
            ast::Type {
                name: name.into(),
                kind,
                generic_types: Vec::new(),
                definition: None,
                symbol_range: create_range(line),
            }
        }

        pub fn create_array(generic_type: ast::Type, line: usize) -> ast::Type {
            ast::Type {
                name: "Array".into(),
                kind: ast::TypeKind::Array,
                generic_types: Vec::from([generic_type]),
                definition: None,
                symbol_range: create_range(line),
            }
        }

        pub fn create_list(generic_type: Option<ast::Type>, line: usize) -> ast::Type {
            ast::Type {
                name: "List".into(),
                kind: ast::TypeKind::List,
                generic_types: generic_type.map(|t| [t].into()).unwrap_or_default(),
                definition: None,
                symbol_range: create_range(line),
            }
        }

        pub fn create_map(
            key_value_types: Option<(ast::Type, ast::Type)>,
            line: usize,
        ) -> ast::Type {
            ast::Type {
                name: "Map".into(),
                kind: ast::TypeKind::Map,
                generic_types: key_value_types
                    .map(|(k, v)| Vec::from([k, v]))
                    .unwrap_or_default(),
                definition: None,
                symbol_range: create_range(line),
            }
        }

        pub fn create_custom_type(def: &str, line: usize) -> ast::Type {
            ast::Type {
                name: "TestCustomType".into(),
                kind: ast::TypeKind::Custom,
                generic_types: Vec::new(),
                definition: Some(def.into()),
                symbol_range: create_range(line),
            }
        }

        pub fn create_method_with_name_and_id(
            name: &str,
            id: Option<u32>,
            line: usize,
        ) -> ast::Method {
            ast::Method {
                oneway: false,
                name: name.into(),
                return_type: create_int(0),
                args: Vec::new(),
                annotations: Vec::new(),
                value: id,
                doc: None,
                symbol_range: create_range(line),
                full_range: create_range(line),
                value_range: create_range(line + 1),
                oneway_range: create_range(line + 2),
            }
        }
        pub fn create_arg(arg_type: ast::Type, direction: ast::Direction) -> ast::Arg {
            ast::Arg {
                direction,
                name: None,
                arg_type,
                annotations: Vec::new(),
                doc: None,
                symbol_range: create_range(0),
                full_range: create_range(0),
            }
        }
    }
}
