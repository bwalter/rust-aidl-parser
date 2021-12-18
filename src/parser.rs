use std::{
    collections::{hash_map, HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
    io::Read,
    path::{Path, PathBuf},
};

use crate::diagnostic::{Diagnostic, RelatedInfo};
use crate::rules;
use crate::{ast, diagnostic::DiagnosticKind};

/// A parser instance which receives the individual AIDL files via
/// Parser::add_content() or Parser::add_file(). Once all the files
/// have been added, call Parser::parser() to trigger the validation
/// and access the results.
///
/// Example:
/// ```
/// use aidl_parser::{Parser, ParseFileResult};
///
/// let mut parser = Parser::new();
///
/// // Add files via ID + content
/// parser.add_content(1, "<content of AIDL file #1>");
/// parser.add_content(2, "<content of AIDL file #2>");
/// parser.add_content(3, "<content of AIDL file #3>");
///
/// // Parse and get results
/// let results: Vec<ParseFileResult<_>> = parser.parse();
///
/// assert_eq!(results.len(), 3);
/// assert_eq!(results[0].id, 1);
/// assert_eq!(results[1].id, 2);
/// assert_eq!(results[2].id, 3);
/// ```
pub struct Parser<ID>
where
    ID: Eq + Hash + Clone + Debug,
{
    file_results: Vec<ParseFileResult<ID>>,
}

/// The parse result of 1 file with its corresponding ID as given via
/// Parser::add_content() or Parser::add_file().
#[derive(Debug)]
pub struct ParseFileResult<ID>
where
    ID: Eq + Hash + Clone + Debug,
{
    pub id: ID,
    pub file: Option<ast::File>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<ID> Parser<ID>
where
    ID: Eq + Hash + Clone + Debug,
{
    /// Create a new, empty parser
    pub fn new() -> Self {
        Parser {
            file_results: Vec::new(),
        }
    }

    /// Add a file content and its key to the parser
    pub fn add_content(&mut self, id: ID, content: &str) {
        let lookup = line_col::LineColLookup::new(content);
        let mut diagnostics = Vec::new();

        let rule_result =
            rules::aidl::OptFileParser::new().parse(&lookup, &mut diagnostics, content);

        let file_result = match rule_result {
            Ok(file) => ParseFileResult {
                id,
                file,
                diagnostics,
            },
            Err(e) => {
                // Append the parse error to the diagnostics
                if let Some(diagnostic) = Diagnostic::from_parse_error(&lookup, e) {
                    diagnostics.push(diagnostic)
                }

                ParseFileResult {
                    id,
                    file: None,
                    diagnostics,
                }
            }
        };

        self.file_results.push(file_result);
    }

    pub fn parse(self) -> Vec<ParseFileResult<ID>> {
        let keys = self.collect_item_keys();

        self.file_results
            .into_iter()
            .map(|mut fr| {
                let mut file = match fr.file {
                    Some(f) => f,
                    None => return ParseFileResult { file: None, ..fr },
                };

                let resolved = resolve_types(&mut file, &mut fr.diagnostics);
                check_types(&mut file, &mut fr.diagnostics);
                check_imports(&file.imports, &resolved, &keys, &mut fr.diagnostics);

                ParseFileResult {
                    file: Some(file),
                    ..fr
                }
            })
            .collect()
    }

    fn collect_item_keys(&self) -> HashSet<String> {
        self.file_results
            .iter()
            .map(|f| &f.file)
            .flatten()
            .map(|f| {
                let item_name = match &f.item {
                    ast::Item::Interface(i) => i.name.clone(),
                    ast::Item::Parcelable(p) => p.name.clone(),
                    ast::Item::Enum(e) => e.name.clone(),
                };
                format!("{}.{}", f.package.name, item_name)
            })
            .collect()
    }
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

fn resolve_types(file: &mut ast::File, diagnostics: &mut Vec<Diagnostic>) -> HashSet<String> {
    let imports: Vec<String> = file
        .imports
        .iter()
        .map(|i| format!("{}.{}", i.path, i.name))
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
                    message: format!("Unknown type: {}", type_.name),
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
    defined: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // array of Import -> map of "qualified name" -> Import
    let imports: HashMap<String, &ast::Import> =
        imports.iter().fold(HashMap::new(), |mut map, import| {
            let qualified_import = format!("{}.{}", import.path, import.name);
            match map.entry(qualified_import.clone()) {
                hash_map::Entry::Occupied(previous) => {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Warning,
                        range: import.symbol_range.clone(),
                        message: format!("Duplicated import: {}", qualified_import),
                        context_message: Some("duplicated import".to_owned()),
                        hint: None,
                        related_infos: Vec::from([RelatedInfo {
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
        if !defined.contains(&qualified_import) {
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                range: import.symbol_range.clone(),
                message: "Unresolved import".to_owned(),
                context_message: Some("unresolved import".to_owned()),
                hint: None,
                related_infos: Vec::new(),
            });
        }
        if !resolved.contains(&qualified_import) {
            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Warning,
                range: import.symbol_range.clone(),
                message: format!("Unused import: `{}`", import.name),
                context_message: Some("unused import".to_owned()),
                hint: None,
                related_infos: Vec::new(),
            });
        }
    }
}

// TODO: additional type checks
fn check_types(file: &mut ast::File, _diagnostics: &mut Vec<Diagnostic>) {
    walk_types(file, |type_: &mut ast::Type| match &type_.definition {
        Some(_type_def) if type_.kind == ast::TypeKind::Custom => {
            //println!("Resolved type: {}", type_def);
        }
        _ => (),
    });
}

impl Parser<PathBuf> {
    /// Add a file to the parser and use its path as key
    pub fn add_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let mut file = std::fs::File::open(path.as_ref())?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;

        self.add_content(PathBuf::from(path.as_ref()), &buffer);
        Ok(())
    }
}

impl<ID> Default for Parser<ID>
where
    ID: Eq + Hash + Clone + Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_parse() -> Result<()> {
        let interface_aidl = r#"
            package com.bwa.aidl_test;
        
            import com.bwa.aidl_test.MyEnum;
            import com.bwa.aidl_test.MyEnum;
            import com.bwa.aidl_test.MyEnum;
            import com.bwa.aidl_test.MyParcelable;
            import com.bwa.aidl_test.MyUnexisting;

            interface MyInterface {
                const int MY_CONST = 12;
                /**
                 * Be polite and say hello
                 */
                //String hello(MyEnum e, MyParcelable);
                String servus(MyEnum e, MyWrong);
                String bonjour(MyEnum e, MyUnexisting);
            }
        "#;

        let enum_aidl = r#"
            package com.bwa.aidl_test;
        
            enum MyEnum {
                VALUE1 = 1,
                VALUE2 = 2,
            }
        "#;

        let parcelable_aidl = r#"
            package com.bwa.aidl_test;
        
            parcelable MyParcelable {
                String name;
                byte[] data;
            }
        "#;

        // Parse AIDL files
        let mut parser = Parser::new();
        parser.add_content(0, interface_aidl);
        parser.add_content(1, parcelable_aidl);
        parser.add_content(2, enum_aidl);
        let res = parser.parse();

        // For each file, 1 result
        assert_eq!(res.len(), 3);

        // No error/warning
        println!("...\nDiagnostics 1:\n{:#?}", res[0].diagnostics);
        println!("...\nDiagnostics 2:\n{:#?}", res[1].diagnostics);
        println!("...\nDiagnostics 3:\n{:#?}", res[2].diagnostics);

        Ok(())
    }
}
