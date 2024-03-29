use std::collections::{hash_map, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::ast;
use crate::diagnostic::{self, Diagnostic, DiagnosticKind};
use crate::parser::ParseFileResult;
use crate::traverse;

pub(crate) fn validate<ID>(
    keys: HashMap<String, ast::ResolvedItemKind>,
    lalrpop_results: HashMap<ID, ParseFileResult<ID>>,
) -> HashMap<ID, ParseFileResult<ID>>
where
    ID: Eq + Hash + Clone + Debug,
{
    // Defined imports: all the imported item keys + add the Android built-in (as unknown)
    let defined = keys;

    lalrpop_results
        .into_iter()
        .map(|(id, mut fr)| {
            let mut ast = match fr.ast {
                Some(f) => f,
                None => return (id, ParseFileResult { ast: None, ..fr }),
            };

            // Imports as qualified names
            let imports: HashSet<String> =
                ast.imports.iter().map(|i| i.get_qualified_name()).collect();

            // Declared parcelables as qualified names
            let declared_parcelables: HashSet<String> = ast
                .declared_parcelables
                .iter()
                .map(|i| i.get_qualified_name())
                .collect();

            // Resolve types (check custom types and set definition if found in imports)
            let resolved = resolve_types(
                &mut ast,
                &imports,
                &declared_parcelables,
                &defined,
                &mut fr.diagnostics,
            );

            // Check imports (e.g. unresolved, unused, duplicated)
            let import_map = check_imports(&ast.imports, &resolved, &defined, &mut fr.diagnostics);

            // Check declared parcelables
            check_declared_parcelables(
                &ast.declared_parcelables,
                &import_map,
                &resolved,
                &mut fr.diagnostics,
            );

            // Check containers (e.g.: map parameters)
            check_containers(&ast, &mut fr.diagnostics);

            if let ast::Item::Interface(ref mut interface) = ast.item {
                // Set up oneway interface (adjust methods to be oneway)
                set_up_oneway_interface(interface, &mut fr.diagnostics);
            }

            // Check methods (e.g.: return type of async methods)
            check_methods(&ast, &mut fr.diagnostics);

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
    declared_parcelables: &HashSet<String>,
    defined: &HashMap<String, ast::ResolvedItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<String> {
    let mut resolved = HashSet::new();

    traverse::walk_types_mut(ast, |type_: &mut ast::Type| {
        resolve_type(type_, imports, declared_parcelables, defined, diagnostics);
        match &type_.kind {
            ast::TypeKind::ResolvedItem(key, _) => {
                resolved.insert(key.clone());
            }
            ast::TypeKind::CharSequence => {
                resolved.insert("java.lang.CharSequence".to_owned());
            }
            ast::TypeKind::String => {
                resolved.insert("java.lang.String".to_owned());
            }
            ast::TypeKind::AndroidType(a) => {
                resolved.insert(a.get_qualified_name().to_owned());
            }
            _ => (),
        }
    });

    resolved
}

fn resolve_type(
    type_: &mut ast::Type,
    imports: &HashSet<String>,
    declared_parcelables: &HashSet<String>,
    defined: &HashMap<String, ast::ResolvedItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if type_.kind != ast::TypeKind::Unresolved {
        return;
    }

    // Unresolved type has the full qualification of a built-in Android type (e.g. android.os.IBinder)?
    let opt_android = ast::AndroidTypeKind::from_qualified_name(&type_.name);
    if let Some(android) = opt_android {
        if android.can_be_qualified() || imports.contains(&type_.name) {
            type_.kind = ast::TypeKind::AndroidType(android);
            return;
        }
    }

    // Unresolved type is in import path?
    if let Some(import_path) = imports.iter().find(|import_path| {
        &type_.name == *import_path || import_path.ends_with(&format!(".{}", type_.name))
    }) {
        if let Some(item_kind) = defined.get(import_path) {
            // Imported type is defined => set resolved item
            type_.kind = ast::TypeKind::ResolvedItem(import_path.to_owned(), item_kind.clone());
            return;
        }

        // Imported but not defined => set resolved item as unknown import
        type_.kind = ast::TypeKind::ResolvedItem(
            import_path.to_owned(),
            ast::ResolvedItemKind::UnknownImport,
        );
        return;
    }

    // Unresolved type is forward-declared?
    // Note: it is supposed to only work with path
    if let Some(import_path) = declared_parcelables
        .iter()
        .find(|import_path| &type_.name == *import_path && !import_path.contains('.'))
    {
        // Set resolved item as forward-declared parcelable
        type_.kind = ast::TypeKind::ResolvedItem(
            import_path.to_owned(),
            ast::ResolvedItemKind::ForwardDeclaredParcelable,
        );
        return;
    }

    // Unresolved type has the full qualification of a built-in Android type (e.g. android.os.IBinder)?
    let opt_android = ast::AndroidTypeKind::from_qualified_name(&type_.name);
    if let Some(android) = opt_android {
        if android.can_be_qualified() {
            type_.kind = ast::TypeKind::AndroidType(android);
            return;
        }
    } else {
        // Unresolved type has the name of a built-in Android type (e.g. aIBinder)?
        let opt_android = ast::AndroidTypeKind::from_name(&type_.name);
        if let Some(android) = opt_android {
            type_.kind = ast::TypeKind::AndroidType(android);
            return;
        }
    }

    // Unresolved type
    diagnostics.push(Diagnostic {
        kind: DiagnosticKind::Error,
        range: type_.symbol_range.clone(),
        message: format!("Unknown type `{}`", type_.name),
        context_message: Some("unknown type".to_owned()),
        hint: None,
        related_infos: Vec::new(),
    });
}

fn check_imports<'a>(
    imports: &'a [ast::Import],
    resolved: &'a HashSet<String>,
    defined: &'a HashMap<String, ast::ResolvedItemKind>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<String, &'a ast::Import> {
    // - detect duplicated imports
    // - create map of "qualified name" -> Import
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

    // - generate diagnostics for unused and unresolved imports
    for (qualified_import, import) in imports.iter() {
        if !defined.contains_key(qualified_import)
            && ast::AndroidTypeKind::from_qualified_name(qualified_import).is_none()
        {
            // No item can be found with the given import path
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Warning,
                range: import.symbol_range.clone(),
                message: format!("Unresolved import `{qualified_import}`"),
                context_message: Some("unresolved import".to_owned()),
                hint: Some(
                    "Note: this is fine if your client is able to import the same item".to_owned(),
                ),
                related_infos: Vec::new(),
            });
        } else if !resolved.contains(qualified_import) {
            // No type resolved for this import
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Warning,
                range: import.symbol_range.clone(),
                message: format!("Unused import `{qualified_import}`"),
                context_message: Some("unused import".to_owned()),
                hint: None,
                related_infos: Vec::new(),
            });
        }
    }

    imports
}

fn check_declared_parcelables(
    declared_parcelables: &[ast::Import],
    imports: &HashMap<String, &ast::Import>,
    resolved: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // - detect duplicated parcelables (or name which was already imported)
    // - create map "qualified name" -> Import
    let declared_parcelables: HashMap<String, &ast::Import> =
        declared_parcelables
            .iter()
            .fold(HashMap::new(), |mut map, declared_parcelable| {
                let qualified_name = declared_parcelable.get_qualified_name();

                if let Some((_, conflicting_import)) = imports
                    .iter()
                    .find(|(_, import)| import.name == declared_parcelable.name)
                {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        range: declared_parcelable.symbol_range.clone(),
                        message: format!(
                            "Declared parcelable conflicts with import `{}`",
                            conflicting_import.get_qualified_name()
                        ),
                        context_message: Some("conflicting declaration".to_owned()),
                        hint: None,
                        related_infos: Vec::from([diagnostic::RelatedInfo {
                            message: "location of conflicting import".to_owned(),
                            range: conflicting_import.symbol_range.clone(),
                        }]),
                    });

                    return map;
                }

                match map.entry(qualified_name.clone()) {
                    hash_map::Entry::Occupied(previous) => {
                        diagnostics.push(Diagnostic {
                            kind: DiagnosticKind::Error,
                            range: declared_parcelable.symbol_range.clone(),
                            message: format!("Multiple parcelable declarations `{qualified_name}`"),
                            context_message: Some("duplicated declaration".to_owned()),
                            hint: None,
                            related_infos: Vec::from([diagnostic::RelatedInfo {
                                message: "previous location".to_owned(),
                                range: previous.get().symbol_range.clone(),
                            }]),
                        });
                    }
                    hash_map::Entry::Vacant(v) => {
                        v.insert(declared_parcelable);
                    }
                }
                map
            });

    // - generate diagnostics for unrecommended usage and for unused declared parcelables
    for (qualified_import, declared_parcelable) in declared_parcelables.into_iter() {
        if !resolved.contains(&qualified_import) {
            // No type resolved for this import
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Warning,
                range: declared_parcelable.symbol_range.clone(),
                message: format!("Unused declared parcelable `{}`", declared_parcelable.name),
                context_message: Some("unused declared parcelable".to_owned()),
                hint: None,
                related_infos: Vec::new(),
            });
        } else {
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Warning,
                range: declared_parcelable.full_range.clone(),
                message: format!("Usage of declared parcelable `{}`", declared_parcelable.name),
                context_message: Some(String::from("declared parcelable")),
                hint: Some(String::from("It is recommended to define parcelables in AIDL to garantee compatilibity between languages")),
                related_infos: Vec::new(),
            });
        }
    }
}

fn check_containers(ast: &ast::Aidl, diagnostics: &mut Vec<Diagnostic>) {
    traverse::walk_types(ast, |type_: &ast::Type| check_container(type_, diagnostics));
}

fn check_container(type_: &ast::Type, diagnostics: &mut Vec<Diagnostic>) {
    match &type_.kind {
        ast::TypeKind::Array => {
            let value_type = &type_.generic_types[0];
            check_array_element(value_type, diagnostics);
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
            check_list_element(value_type, diagnostics);
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
            check_map_key(&type_.generic_types[0], diagnostics);
            check_map_value(&type_.generic_types[1], diagnostics);
        }
        _ => {}
    };
}

fn check_methods(file: &ast::Aidl, diagnostics: &mut Vec<Diagnostic>) {
    let mut method_names: HashMap<String, &ast::Method> = HashMap::new();
    let mut first_method_without_id: Option<&ast::Method> = None;
    let mut first_method_with_id: Option<&ast::Method> = None;
    let mut method_ids: HashMap<u32, &ast::Method> = HashMap::new();

    traverse::walk_methods(file, |method: &ast::Method| {
        // Check individual method (e.g. return value, args, ...)
        check_method(method, diagnostics);

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
            && method.transact_code.is_some();
        let is_mixed_now_without_id = first_method_without_id.is_none()
            && !method_ids.is_empty()
            && method.transact_code.is_none();

        if is_mixed_now_with_id || is_mixed_now_without_id {
            let info_previous = if is_mixed_now_with_id {
                diagnostic::RelatedInfo {
                    message: "method without id".to_owned(),
                    range: first_method_without_id
                        .as_ref()
                        .unwrap()
                        .transact_code_range
                        .clone(),
                }
            } else {
                diagnostic::RelatedInfo {
                    message: "method with id".to_owned(),
                    range: first_method_with_id
                        .as_ref()
                        .unwrap()
                        .transact_code_range
                        .clone(),
                }
            };

            // Methods are mixed (with/without id)
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                range: method.transact_code_range.clone(),
                message: String::from("Mixed usage of method ids"),
                context_message: None,
                hint: Some(String::from(
                    "Either all methods should have an id or none of them",
                )),
                related_infos: Vec::from([info_previous]),
            });
        }

        if method.transact_code.is_some() {
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

        if let Some(id) = method.transact_code {
            match method_ids.entry(id) {
                hash_map::Entry::Occupied(oe) => {
                    // Method id already defined
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        range: method.transact_code_range.clone(),
                        message: String::from("Duplicated method id"),
                        context_message: Some("duplicated import".to_owned()),
                        hint: None,
                        related_infos: Vec::from([diagnostic::RelatedInfo {
                            range: oe.get().transact_code_range.clone(),
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

fn check_method(method: &ast::Method, diagnostics: &mut Vec<Diagnostic>) {
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

    check_method_args(method, diagnostics);
}

// Check arg direction (e.g. depending on type or method being oneway)
fn check_method_args(method: &ast::Method, diagnostics: &mut Vec<Diagnostic>) {
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

        match get_requirement_for_arg_direction(&arg.arg_type) {
            RequirementForArgDirection::DirectionRequired(for_elements) => {
                if arg.direction == ast::Direction::Unspecified {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        message: format!("Missing direction for `{}`", arg.arg_type.name,),
                        context_message: Some("missing direction".to_owned()),
                        range: range.clone(),
                        hint: Some(format!("direction is required for {for_elements}")),
                        related_infos: Vec::new(),
                    });
                }
            }
            RequirementForArgDirection::CanOnlyBeInOrUnspecified(for_elements) => {
                if !matches!(
                    arg.direction,
                    ast::Direction::Unspecified | ast::Direction::In(_)
                ) {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        message: format!("Invalid direction for `{}`", arg.arg_type.name),
                        context_message: Some("invalid direction".to_owned()),
                        range: range.clone(),
                        hint: Some(format!("{for_elements} can only be `in` or omitted")),
                        related_infos: Vec::new(),
                    });
                }
            }
            RequirementForArgDirection::CanOnlyBeInOrInOut(for_elements) => {
                if !matches!(
                    arg.direction,
                    ast::Direction::In(_) | ast::Direction::InOut(_)
                ) {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        message: format!("Invalid direction for `{}`", arg.arg_type.name),
                        context_message: Some("invalid direction".to_owned()),
                        range: range.clone(),
                        hint: Some(if matches!(arg.direction, ast::Direction::Out(_)) {
                            format!("{for_elements} cannot be out")
                        } else {
                            format!("{for_elements} must be specified")
                        }),
                        related_infos: Vec::new(),
                    });
                }
            }
            RequirementForArgDirection::CannotBeAnArg(for_elements) => {
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Error,
                    message: format!("Invalid argument `{}`", arg.arg_type.name,),
                    context_message: Some("invalid argument".to_owned()),
                    range: range.clone(),
                    hint: Some(format!("{for_elements} cannot be an argument")),
                    related_infos: Vec::new(),
                });
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

// Parameters describe for which elements the requirement applies
enum RequirementForArgDirection {
    DirectionRequired(&'static str),
    CanOnlyBeInOrUnspecified(&'static str),
    CanOnlyBeInOrInOut(&'static str),
    CannotBeAnArg(&'static str),
    NoRequirement,
}

fn get_requirement_for_arg_direction(type_: &ast::Type) -> RequirementForArgDirection {
    match type_.kind {
        ast::TypeKind::Primitive => {
            RequirementForArgDirection::CanOnlyBeInOrUnspecified("primitives")
        }
        ast::TypeKind::Void => RequirementForArgDirection::CanOnlyBeInOrUnspecified("void"),
        ast::TypeKind::Array => RequirementForArgDirection::DirectionRequired("arrays"),
        ast::TypeKind::Map | ast::TypeKind::List => {
            RequirementForArgDirection::DirectionRequired("maps")
        }
        ast::TypeKind::String => RequirementForArgDirection::CanOnlyBeInOrUnspecified("strings"),
        ast::TypeKind::CharSequence => {
            RequirementForArgDirection::CanOnlyBeInOrUnspecified("CharSequence")
        }
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::IBinder) => {
            RequirementForArgDirection::CanOnlyBeInOrUnspecified("IBinder")
        }
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::FileDescriptor) => {
            RequirementForArgDirection::CanOnlyBeInOrUnspecified("FileDescriptor")
        }
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::ParcelFileDescriptor) => {
            RequirementForArgDirection::CanOnlyBeInOrInOut("ParcelFileDescriptor")
        } // because it is not default-constructible
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::ParcelableHolder) => {
            RequirementForArgDirection::CannotBeAnArg("ParcelableHolder")
        }
        ast::TypeKind::ResolvedItem(
            _,
            ast::ResolvedItemKind::Parcelable | ast::ResolvedItemKind::ForwardDeclaredParcelable,
        ) => RequirementForArgDirection::DirectionRequired("parcelables"),
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Interface) => {
            RequirementForArgDirection::CanOnlyBeInOrUnspecified("interfaces")
        }
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Enum) => {
            RequirementForArgDirection::CanOnlyBeInOrUnspecified("enums")
        }
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::UnknownImport) => {
            RequirementForArgDirection::CanOnlyBeInOrUnspecified("objects")
        }
        ast::TypeKind::Unresolved => RequirementForArgDirection::NoRequirement,
    }
}

// Can only have one dimensional arrays
// "Binder" type cannot be an array (with interface element...)
// TODO: not allowed for ParcelableHolder, allowed for IBinder, ...
fn check_array_element(type_: &ast::Type, diagnostics: &mut Vec<Diagnostic>) {
    let ok = match type_.kind {
        // Not OK (custom diagnostic and return)
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
        ast::TypeKind::Primitive => true,
        ast::TypeKind::String => true,
        ast::TypeKind::CharSequence => false,
        ast::TypeKind::List => false,
        ast::TypeKind::Map => false,
        ast::TypeKind::Void => false,
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::IBinder) => true,
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::FileDescriptor) => true,
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::ParcelFileDescriptor) => true,
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::ParcelableHolder) => false,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Parcelable) => true,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Interface) => false,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Enum) => true, // OK: enum is backed by a primitive
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::ForwardDeclaredParcelable) => true,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::UnknownImport) => true, // OK: it is an unknown object
        ast::TypeKind::Unresolved => true, // we don't know
    };

    if !ok {
        diagnostics.push(Diagnostic {
            kind: DiagnosticKind::Error,
            message: format!("Invalid array element `{}`", type_.name),
            context_message: Some("invalid parameter".to_owned()),
            range: type_.symbol_range.clone(),
            hint: Some(
                "must be a primitive, an enum, a String, a parcelable or a IBinder".to_owned(),
            ),
            related_infos: Vec::new(),
        });
    }
}

// List<T> supports parcelable/union, String, IBinder, and ParcelFileDescriptor
fn check_list_element(type_: &ast::Type, diagnostics: &mut Vec<Diagnostic>) {
    let ok = match type_.kind {
        ast::TypeKind::Array => false,
        ast::TypeKind::List => false,
        ast::TypeKind::Map => false,
        ast::TypeKind::Primitive => false,
        ast::TypeKind::String => true,
        ast::TypeKind::CharSequence => false,
        ast::TypeKind::Void => false,
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::IBinder) => true,
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::FileDescriptor) => false,
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::ParcelFileDescriptor) => true,
        ast::TypeKind::AndroidType(ast::AndroidTypeKind::ParcelableHolder) => false,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Parcelable) => true,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Interface) => false,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Enum) => false, // NO: enum is backed by a primitive
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::ForwardDeclaredParcelable) => true,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::UnknownImport) => true, // OK: it is an (unknown) object
        ast::TypeKind::Unresolved => true, // we don't know
    };

    if !ok {
        diagnostics.push(Diagnostic {
            kind: DiagnosticKind::Error,
            message: format!("Invalid list element `{}`", type_.name),
            context_message: Some("invalid element".to_owned()),
            range: type_.symbol_range.clone(),
            hint: Some(
                "must be a parcelable/enum, a String, a IBinder or a ParcelFileDescriptor"
                    .to_owned(),
            ),
            related_infos: Vec::new(),
        });
    }
}

// The type of key in map must be String
fn check_map_key(type_: &ast::Type, diagnostics: &mut Vec<Diagnostic>) {
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
fn check_map_value(type_: &ast::Type, diagnostics: &mut Vec<Diagnostic>) {
    let ok = match type_.kind {
        ast::TypeKind::Array => true,
        ast::TypeKind::List => true,
        ast::TypeKind::Map => true,
        ast::TypeKind::String => true,
        ast::TypeKind::CharSequence => true,
        ast::TypeKind::Primitive => false,
        ast::TypeKind::Void => false,
        ast::TypeKind::AndroidType(_) => true,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Parcelable) => true,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Interface) => true,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::Enum) => false,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::ForwardDeclaredParcelable) => true,
        ast::TypeKind::ResolvedItem(_, ast::ResolvedItemKind::UnknownImport) => true,
        ast::TypeKind::Unresolved => true, // we don't know
    };

    if !ok {
        diagnostics.push(Diagnostic {
            kind: DiagnosticKind::Error,
            message: format!("Invalid map value `{}`", type_.name),
            context_message: Some("invalid map value".to_owned()),
            range: type_.symbol_range.clone(),
            hint: Some("cannot not be a primitive".to_owned()),
            related_infos: Vec::new(),
        });
    }
}

#[cfg(test)]
#[allow(clippy::single_element_loop)]
mod tests {
    use self::utils::create_method_with_name_and_id;

    use super::*;
    use crate::ast;

    #[test]
    fn test_check_resolve_type() {
        let defined = HashMap::from([]);

        {
            // IBinder properly resolved
            let mut t = utils::create_unresolved_type(
                ast::AndroidTypeKind::IBinder.get_name(),
                1,
            );
            let mut diagnostics = Vec::new();
            resolve_type(
                &mut t,
                &HashSet::new(),
                &HashSet::new(),
                &defined,
                &mut diagnostics,
            );
            assert_eq!(
                t.kind,
                ast::TypeKind::AndroidType(ast::AndroidTypeKind::IBinder)
            );
            assert_eq!(diagnostics.len(), 0);
        }

        {
            // android.os.IBinder not properly resolved
            let mut t = utils::create_unresolved_type(
                ast::AndroidTypeKind::IBinder.get_qualified_name(),
                1,
            );
            let mut diagnostics = Vec::new();
            resolve_type(
                &mut t,
                &HashSet::new(),
                &HashSet::new(),
                &defined,
                &mut diagnostics,
            );
            assert_eq!(t.kind, ast::TypeKind::Unresolved);
            assert_eq!(diagnostics.len(), 1); // Error because type is not resolved
        }

        {
            // android.os.ParcelFileDescriptor
            let mut t = utils::create_unresolved_type(
                ast::AndroidTypeKind::ParcelFileDescriptor.get_qualified_name(),
                1,
            );
            let mut diagnostics = Vec::new();
            resolve_type(
                &mut t,
                &HashSet::new(),
                &HashSet::new(),
                &defined,
                &mut diagnostics,
            );
            assert_eq!(
                t.kind,
                ast::TypeKind::AndroidType(ast::AndroidTypeKind::ParcelFileDescriptor)
            );
            assert_eq!(diagnostics.len(), 0);
        }

        {
            // UnknownType
            let mut t = utils::create_unresolved_type("UnknownType", 1);
            let mut diagnostics = Vec::new();
            resolve_type(
                &mut t,
                &HashSet::new(),
                &HashSet::new(),
                &defined,
                &mut diagnostics,
            );
            assert_eq!(t.kind, ast::TypeKind::Unresolved);
            assert_eq!(diagnostics.len(), 1);
        }

        {
            // UnknownType but imported
            let mut t = utils::create_unresolved_type("UnknownType", 1);
            let imports = HashSet::from(["path.to.UnknownType".to_owned()]);
            let mut diagnostics = Vec::new();
            resolve_type(
                &mut t,
                &imports,
                &HashSet::new(),
                &defined,
                &mut diagnostics,
            );
            assert_eq!(
                t.kind,
                ast::TypeKind::ResolvedItem(
                    "path.to.UnknownType".to_owned(),
                    ast::ResolvedItemKind::UnknownImport
                )
            );
            assert_eq!(diagnostics.len(), 0);
        }

        {
            // Forward-declared parcelable
            let mut t = utils::create_unresolved_type("ForwardDeclaredParcelable", 1);
            let declared_parcelables = HashSet::from(["ForwardDeclaredParcelable".to_owned()]);
            let mut diagnostics = Vec::new();
            resolve_type(
                &mut t,
                &HashSet::new(),
                &declared_parcelables,
                &defined,
                &mut diagnostics,
            );
            assert_eq!(
                t.kind,
                ast::TypeKind::ResolvedItem(
                    "ForwardDeclaredParcelable".to_owned(),
                    ast::ResolvedItemKind::ForwardDeclaredParcelable
                )
            );
            assert_eq!(diagnostics.len(), 0);
        }

        {
            // Forward-declared parcelable (with path, which is not supposed to work)
            let mut t = utils::create_unresolved_type("ForwardDeclaredParcelable", 1);
            let declared_parcelables =
                HashSet::from(["path.to.ForwardDeclaredParcelable".to_owned()]);
            let mut diagnostics = Vec::new();
            resolve_type(
                &mut t,
                &HashSet::new(),
                &declared_parcelables,
                &defined,
                &mut diagnostics,
            );
            assert_eq!(t.kind, ast::TypeKind::Unresolved);
            assert_eq!(diagnostics.len(), 1);
        }
    }

    #[test]
    fn test_check_imports() {
        let imports = Vec::from([
            utils::create_import("test.path", "TestParcelable", 1),
            utils::create_import("test.path", "TestParcelable", 2),
            utils::create_import("test.path", "TestInterface", 3),
            utils::create_import("test.path", "UnusedEnum", 4),
            utils::create_import("test.path", "NonExisting", 5),
            utils::create_import("android.os", "IBinder", 6),
            utils::create_import("android.os", "ParcelFileDescriptor", 7),
        ]);

        let resolved = HashSet::from([
            "test.path.TestParcelable".into(),
            "test.path.TestInterface".into(),
            "android.os.ParcelFileDescriptor".into(),
        ]);
        let defined = HashMap::from([
            (
                "test.path.TestParcelable".into(),
                ast::ResolvedItemKind::Parcelable,
            ),
            (
                "test.path.TestInterface".into(),
                ast::ResolvedItemKind::Interface,
            ),
            ("test.path.UnusedEnum".into(), ast::ResolvedItemKind::Enum),
        ]);
        let mut diagnostics = Vec::new();

        check_imports(&imports, &resolved, &defined, &mut diagnostics);

        diagnostics.sort_by_key(|d| d.range.start.line_col.0);

        assert_eq!(diagnostics.len(), 4);

        let d = &diagnostics[0];
        assert_eq!(d.kind, DiagnosticKind::Error);
        assert!(d.message.contains("Duplicated import"));
        assert_eq!(d.range.start.line_col.0, 2);

        let d = &diagnostics[1];
        assert_eq!(d.kind, DiagnosticKind::Warning);
        assert!(d.message.contains("Unused import `test.path.UnusedEnum`"));
        assert_eq!(d.range.start.line_col.0, 4);

        let d = &diagnostics[2];
        assert_eq!(d.kind, DiagnosticKind::Warning);
        assert!(d
            .message
            .contains("Unresolved import `test.path.NonExisting`"));
        assert_eq!(d.range.start.line_col.0, 5);

        let d = &diagnostics[3];
        assert_eq!(d.kind, DiagnosticKind::Warning);
        assert!(d.message.contains("Unused import `android.os.IBinder`"));
        assert_eq!(d.range.start.line_col.0, 6);
    }

    #[test]
    fn test_check_declared_parcelables() {
        let declared_parcelables = Vec::from([
            utils::create_import("", "DeclaredParcelable1", 2),
            utils::create_import("", "DeclaredParcelable1", 3),
            utils::create_import("", "DeclaredParcelable2", 4),
            utils::create_import("", "UnusedParcelable", 5),
            utils::create_import("", "AlreadyImported", 6),
        ]);

        let import = ast::Import {
            path: "test.other.path".into(),
            name: "AlreadyImported".into(),
            symbol_range: utils::create_range(1),
            full_range: utils::create_range(1),
        };
        let import_map = HashMap::from([(import.get_qualified_name(), &import)]);
        let resolved = HashSet::from([
            "test.path.DeclaredParcelable1".into(),
            "test.path.DeclaredParcelable2".into(),
        ]);
        let mut diagnostics = Vec::new();

        check_declared_parcelables(
            &declared_parcelables,
            &import_map,
            &resolved,
            &mut diagnostics,
        );

        diagnostics.sort_by_key(|d| d.range.start.line_col.0);

        assert_eq!(diagnostics.len(), 5);

        let d = &diagnostics[0];
        assert_eq!(d.kind, DiagnosticKind::Warning);
        assert_eq!(d.range.start.line_col.0, 2);

        let d = &diagnostics[1];
        assert_eq!(d.kind, DiagnosticKind::Error);
        assert!(d.message.contains("Multiple parcelable declarations"));
        assert_eq!(d.range.start.line_col.0, 3);

        let d = &diagnostics[2];
        assert_eq!(d.kind, DiagnosticKind::Warning);
        assert_eq!(d.range.start.line_col.0, 4);

        let d = &diagnostics[3];
        assert_eq!(d.kind, DiagnosticKind::Warning);
        assert!(d
            .message
            .contains("Unused declared parcelable `UnusedParcelable`"));
        assert_eq!(d.range.start.line_col.0, 5);

        let d = &diagnostics[4];
        assert_eq!(d.kind, DiagnosticKind::Error);
        assert!(d.message.contains("conflicts"));
        assert_eq!(d.range.start.line_col.0, 6);
    }

    #[test]
    fn test_check_container() {
        // Valid arrays
        for t in [
            utils::create_int(0),
            utils::create_string(0),
            utils::create_android_builtin(ast::AndroidTypeKind::IBinder, 0),
            utils::create_android_builtin(ast::AndroidTypeKind::FileDescriptor, 0),
            utils::create_android_builtin(ast::AndroidTypeKind::ParcelFileDescriptor, 0),
            utils::create_resolved_item_type(
                "test.TestParcelable",
                ast::ResolvedItemKind::Parcelable,
                0,
            ),
            utils::create_resolved_item_type("test.TestEnum", ast::ResolvedItemKind::Enum, 0),
        ]
        .into_iter()
        {
            let array = utils::create_array(t, 0);
            let mut diagnostics = Vec::new();
            check_container(&array, &mut diagnostics);
            assert_eq!(diagnostics.len(), 0);
        }

        // Multi-dimensional array
        let mut diagnostics = Vec::new();
        let array = utils::create_array(utils::create_array(utils::create_int(0), 0), 0);
        check_container(&array, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("Unsupported multi-dimensional array"));

        // Invalid arrays
        for t in [
            utils::create_android_builtin(ast::AndroidTypeKind::ParcelableHolder, 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_resolved_item_type(
                "test.TestInterface",
                ast::ResolvedItemKind::Interface,
                0,
            ),
            utils::create_char_sequence(0),
            utils::create_void(0),
        ]
        .into_iter()
        {
            let array = utils::create_array(t, 0);
            let mut diagnostics = Vec::new();
            check_container(&array, &mut diagnostics);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("Invalid array"));
        }

        // Valid list
        for t in [
            utils::create_string(0),
            utils::create_android_builtin(ast::AndroidTypeKind::IBinder, 0),
            utils::create_android_builtin(ast::AndroidTypeKind::ParcelFileDescriptor, 0),
            utils::create_resolved_item_type(
                "test.TestParcelable",
                ast::ResolvedItemKind::Parcelable,
                0,
            ),
        ]
        .into_iter()
        {
            let list = utils::create_list(Some(t), 0);
            let mut diagnostics = Vec::new();
            check_container(&list, &mut diagnostics);
            assert_eq!(diagnostics.len(), 0);
        }

        // Non-generic list -> warning
        let mut diagnostics = Vec::new();
        let list = utils::create_list(None, 105);
        check_container(&list, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::Warning);
        assert_eq!(diagnostics[0].range.start.line_col.0, 105);
        assert_eq!(diagnostics[0].range.end.line_col.0, 105);
        assert!(diagnostics[0].message.contains("not recommended"));

        // Invalid lists
        for t in [
            utils::create_void(0),
            utils::create_char_sequence(0),
            utils::create_android_builtin(ast::AndroidTypeKind::ParcelableHolder, 0),
            utils::create_android_builtin(ast::AndroidTypeKind::FileDescriptor, 0),
            utils::create_array(utils::create_int(0), 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_resolved_item_type(
                "test.TestInterface",
                ast::ResolvedItemKind::Interface,
                0,
            ),
            utils::create_resolved_item_type("test.TestEnum", ast::ResolvedItemKind::Enum, 0),
        ]
        .into_iter()
        {
            let list = utils::create_list(Some(t), 0);
            let mut diagnostics = Vec::new();
            check_container(&list, &mut diagnostics);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("Invalid list"));
        }

        // Valid map
        for vt in [
            utils::create_string(0),
            utils::create_android_builtin(ast::AndroidTypeKind::ParcelableHolder, 0),
            utils::create_android_builtin(ast::AndroidTypeKind::IBinder, 0),
            utils::create_android_builtin(ast::AndroidTypeKind::FileDescriptor, 0),
            utils::create_android_builtin(ast::AndroidTypeKind::ParcelFileDescriptor, 0),
            utils::create_array(utils::create_int(0), 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_resolved_item_type(
                "test.TestParcelable",
                ast::ResolvedItemKind::Parcelable,
                0,
            ),
            utils::create_resolved_item_type(
                "test.TestInterface",
                ast::ResolvedItemKind::Interface,
                0,
            ),
        ]
        .into_iter()
        {
            let map = utils::create_map(Some((utils::create_string(0), vt)), 0);
            let mut diagnostics = Vec::new();
            check_container(&map, &mut diagnostics);
            assert_eq!(diagnostics.len(), 0);
        }

        // Non-generic map -> warning
        let mut diagnostics = Vec::new();
        let map = utils::create_map(None, 205);
        check_container(&map, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::Warning);
        assert_eq!(diagnostics[0].range.start.line_col.0, 205);
        assert_eq!(diagnostics[0].range.end.line_col.0, 205);
        assert!(diagnostics[0].message.contains("not recommended"));

        // Invalid map keys
        for kt in [
            utils::create_void(0),
            utils::create_char_sequence(0),
            utils::create_array(utils::create_int(0), 0),
            utils::create_list(None, 0),
            utils::create_map(None, 0),
            utils::create_resolved_item_type(
                "test.TestParcelable",
                ast::ResolvedItemKind::Parcelable,
                0,
            ),
            utils::create_resolved_item_type(
                "test.TestInterface",
                ast::ResolvedItemKind::Interface,
                0,
            ),
            utils::create_resolved_item_type("test.TestEnum", ast::ResolvedItemKind::Enum, 0),
        ]
        .into_iter()
        {
            let map = utils::create_map(Some((kt, utils::create_string(0))), 0);
            let mut diagnostics = Vec::new();
            check_container(&map, &mut diagnostics);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("Invalid map"));
        }

        // Invalid map values
        for vt in [
            utils::create_int(0),
            utils::create_void(0),
            utils::create_resolved_item_type("test.TestEnum", ast::ResolvedItemKind::Enum, 0),
        ]
        .into_iter()
        {
            let map = utils::create_map(Some((utils::create_string(0), vt)), 0);
            let mut diagnostics = Vec::new();
            check_container(&map, &mut diagnostics);
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
            return_type: utils::create_void(0),
            args: Vec::new(),
            annotations: Vec::new(),
            transact_code: None,
            doc: None,
            symbol_range: utils::create_range(0),
            full_range: utils::create_range(0),
            transact_code_range: utils::create_range(0),
            oneway_range: utils::create_range(0),
        };
        let mut diagnostics = Vec::new();
        check_method(&void_method, &mut diagnostics);
        assert_eq!(diagnostics.len(), 0);

        // Oneway method returning void -> ok
        let mut oneway_void_method = void_method.clone();
        oneway_void_method.oneway = true;
        let mut diagnostics = Vec::new();
        check_method(&oneway_void_method, &mut diagnostics);
        assert_eq!(diagnostics.len(), 0);

        // Async method with return value -> error
        let mut oneway_int_method = oneway_void_method.clone();
        oneway_int_method.return_type = utils::create_int(0);
        let mut diagnostics = Vec::new();
        check_method(&oneway_int_method, &mut diagnostics);
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
            declared_parcelables: Vec::new(),
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
        check_methods(&ast, &mut diagnostics);

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
            return_type: utils::create_void(0),
            args: Vec::new(),
            transact_code: None,
            annotations: Vec::new(),
            doc: None,
            symbol_range: utils::create_range(0),
            full_range: utils::create_range(1),
            transact_code_range: utils::create_range(0),
            oneway_range: utils::create_range(0),
        };

        // Types which are not allowed to be used for args
        for t in [utils::create_android_builtin(
            ast::AndroidTypeKind::ParcelableHolder,
            0,
        )]
        .into_iter()
        {
            let mut diagnostics = Vec::new();
            let mut method = base_method.clone();
            method.args = Vec::from([utils::create_arg(
                t,
                ast::Direction::In(utils::create_range(0)),
            )]);
            check_method_args(&method, &mut diagnostics);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("Invalid argument"));
        }

        // Primitives, String, Interfaces, Enums, IBinder and FileDescriptor can only be in or unspecified
        for t in [
            utils::create_int(0),
            utils::create_string(0),
            utils::create_char_sequence(0),
            utils::create_android_builtin(ast::AndroidTypeKind::IBinder, 0),
            utils::create_android_builtin(ast::AndroidTypeKind::FileDescriptor, 0),
            utils::create_resolved_item_type(
                "test.TestInterface",
                ast::ResolvedItemKind::Interface,
                0,
            ),
            utils::create_resolved_item_type("test.TestEnum", ast::ResolvedItemKind::Enum, 0),
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
                check_method_args(&method, &mut diagnostics);
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
                check_method_args(&method, &mut diagnostics);
                assert_eq!(diagnostics.len(), method.args.len());
                for d in diagnostics {
                    assert_eq!(d.kind, DiagnosticKind::Error);
                }
            }
        }

        // ParcelFileDescriptor cannot be out
        for t in [utils::create_android_builtin(
            ast::AndroidTypeKind::ParcelFileDescriptor,
            0,
        )]
        .into_iter()
        {
            // In or InOut => OK
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.args = Vec::from([
                    utils::create_arg(t.clone(), ast::Direction::In(utils::create_range(0))),
                    utils::create_arg(t.clone(), ast::Direction::InOut(utils::create_range(0))),
                ]);
                check_method_args(&method, &mut diagnostics);
                assert_eq!(diagnostics.len(), 0);
            }

            // Unspecified or Out => ERROR
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.args = Vec::from([
                    utils::create_arg(t.clone(), ast::Direction::Unspecified),
                    utils::create_arg(t, ast::Direction::Out(utils::create_range(0))),
                ]);
                check_method_args(&method, &mut diagnostics);
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
            utils::create_resolved_item_type(
                "test.TestParcelable",
                ast::ResolvedItemKind::Parcelable,
                0,
            ),
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
                check_method_args(&method, &mut diagnostics);
                assert_eq!(diagnostics.len(), 0);
            }

            // Unspecified => ERROR
            {
                let mut diagnostics = Vec::new();
                let mut method = base_method.clone();
                method.args = Vec::from([utils::create_arg(t, ast::Direction::Unspecified)]);
                check_method_args(&method, &mut diagnostics);
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
            utils::create_resolved_item_type(
                "test.TestParcelable",
                ast::ResolvedItemKind::Parcelable,
                0,
            ),
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
                check_method_args(&method, &mut diagnostics);
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
                check_method_args(&method, &mut diagnostics);
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

        pub fn create_import(path: &str, name: &str, line: usize) -> ast::Import {
            ast::Import {
                path: path.to_owned(),
                name: name.to_owned(),
                symbol_range: create_range(line),
                full_range: create_range(line),
            }
        }

        pub fn create_int(line: usize) -> ast::Type {
            create_simple_type("int", ast::TypeKind::Primitive, line)
        }

        pub fn create_void(line: usize) -> ast::Type {
            create_simple_type("void", ast::TypeKind::Void, line)
        }

        pub fn create_string(line: usize) -> ast::Type {
            create_simple_type("String", ast::TypeKind::String, line)
        }

        pub fn create_char_sequence(line: usize) -> ast::Type {
            create_simple_type("CharSequence", ast::TypeKind::CharSequence, line)
        }

        pub fn create_android_builtin(
            android_kind: ast::AndroidTypeKind,
            line: usize,
        ) -> ast::Type {
            let name = android_kind.get_name().to_owned();
            create_simple_type(&name, ast::TypeKind::AndroidType(android_kind), line)
        }

        fn create_simple_type(name: &str, kind: ast::TypeKind, line: usize) -> ast::Type {
            ast::Type {
                name: name.into(),
                kind,
                generic_types: Vec::new(),
                symbol_range: create_range(line),
                full_range: create_range(line),
            }
        }

        pub fn create_array(generic_type: ast::Type, line: usize) -> ast::Type {
            ast::Type {
                name: "Array".into(),
                kind: ast::TypeKind::Array,
                generic_types: Vec::from([generic_type]),
                symbol_range: create_range(line),
                full_range: create_range(line),
            }
        }

        pub fn create_list(generic_type: Option<ast::Type>, line: usize) -> ast::Type {
            ast::Type {
                name: "List".into(),
                kind: ast::TypeKind::List,
                generic_types: generic_type.map(|t| [t].into()).unwrap_or_default(),
                symbol_range: create_range(line),
                full_range: create_range(line),
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
                symbol_range: create_range(line),
                full_range: create_range(line),
            }
        }

        pub fn create_resolved_item_type(
            path: &str,
            item_kind: ast::ResolvedItemKind,
            line: usize,
        ) -> ast::Type {
            ast::Type {
                name: "TestCustomType".into(),
                kind: ast::TypeKind::ResolvedItem(path.into(), item_kind),
                generic_types: Vec::new(),
                symbol_range: create_range(line),
                full_range: create_range(line),
            }
        }

        pub fn create_unresolved_type(path: &str, line: usize) -> ast::Type {
            ast::Type {
                name: path.to_owned(),
                kind: ast::TypeKind::Unresolved,
                generic_types: Vec::new(),
                symbol_range: create_range(line),
                full_range: create_range(line),
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
                transact_code: id,
                doc: None,
                symbol_range: create_range(line),
                full_range: create_range(line),
                transact_code_range: create_range(line + 1),
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
