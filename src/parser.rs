use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    io::Read,
    path::{Path, PathBuf},
};

use crate::ast;
use crate::diagnostic::Diagnostic;
use crate::rules;
use crate::validation;

/// A parser instance which receives the individual AIDL files via
/// Parser::add_content() or Parser::add_file(). Once all the files
/// have been added, call Parser::parser() to trigger the validation
/// and access the results.
///
/// The ID of the files added to the parser are used to uniquely
/// identify the results returned by the parser. It can be any
/// value used as a key (e.g. number of string) or the location of
/// the content (e.g. PathBuf or Uri).
///
/// The content added to the parser can be removed or replaced
/// before or after the parsing.
///
/// Example:
/// ```
/// use aidl_parser::{Parser, ParseFileResult};
///
/// let mut parser = Parser::new();
///
/// // Add files via ID + content
/// parser.add_content("id1", "<content of AIDL file #1>");
/// parser.add_content("id2", "<content of AIDL file #2>");
/// parser.add_content("id3", "<content of AIDL file #3>");
///
/// // Parse and get results
/// let results = parser.validate();
///
/// assert_eq!(results.len(), 3);
/// assert!(results.contains_key("id1"));
/// assert!(results.contains_key("id2"));
/// assert!(results.contains_key("id3"));
///
/// // Add/replace/remove files
/// parser.add_content("id2", "<updated content of AIDL file #2>");
/// parser.add_content("id4", "<content of AIDL file #4>");
/// parser.add_content("id5", "<content of AIDL file #5>");
/// parser.remove_content("id3");
///
/// // Parse again and get updated results
/// let results = parser.validate();
///
/// assert_eq!(results.len(), 4);
/// assert!(results.contains_key("id1"));
/// assert!(results.contains_key("id2"));
/// assert!(!results.contains_key("id3"));  // removed
/// assert!(results.contains_key("id4"));
/// assert!(results.contains_key("id5"));
/// ```
pub struct Parser<ID>
where
    ID: Eq + Hash + Clone + Debug,
{
    lalrpop_results: HashMap<ID, ParseFileResult<ID>>,
}

/// The parse result of 1 file with its corresponding ID as given via
/// Parser::add_content() or Parser::add_file().
#[derive(Debug, Clone)]
pub struct ParseFileResult<ID>
where
    ID: Eq + Hash + Clone + Debug,
{
    pub id: ID,
    pub ast: Option<ast::Aidl>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<ID> Parser<ID>
where
    ID: Eq + Hash + Clone + Debug,
{
    /// Create a new, empty parser
    pub fn new() -> Self {
        Parser {
            lalrpop_results: HashMap::new(),
        }
    }

    /// Add a file content and its key to the parser.
    ///
    /// This will parse the individual content and store the result internally.
    ///
    /// Note: if a content with the same id already exists, the old content will be replaced.
    pub fn add_content(&mut self, id: ID, content: &str) {
        let lookup = line_col::LineColLookup::new(content);
        let mut diagnostics = Vec::new();

        let rule_result =
            rules::aidl::OptAidlParser::new().parse(&lookup, &mut diagnostics, content);

        let lalrpop_result = match rule_result {
            Ok(file) => ParseFileResult {
                id: id.clone(),
                ast: file,
                diagnostics,
            },
            Err(e) => {
                // Append the parse error to the diagnostics
                if let Some(diagnostic) = Diagnostic::from_parse_error(&lookup, e) {
                    diagnostics.push(diagnostic)
                }

                ParseFileResult {
                    id: id.clone(),
                    ast: None,
                    diagnostics,
                }
            }
        };

        self.lalrpop_results.insert(id, lalrpop_result);
    }

    /// Remove the file with the given key
    pub fn remove_content(&mut self, id: ID) {
        self.lalrpop_results.remove(&id);
    }

    /// Validate the results of all files previously added to the parser and return the
    /// collected results (AST + diagnostics)
    pub fn validate(&self) -> HashMap<ID, ParseFileResult<ID>> {
        let keys = self.collect_item_keys();
        validation::validate(keys, self.lalrpop_results.clone())
    }

    fn collect_item_keys(&self) -> HashMap<ast::ItemKey, ast::ItemKind> {
        self.lalrpop_results
            .iter()
            .map(|(_, fr)| &fr.ast)
            .flatten()
            .map(|f| (f.get_key(), f.item.get_kind()))
            .collect()
    }
}

impl Parser<PathBuf> {
    /// Add a file to the parser and use its path as key.
    ///
    /// If a file with the same path already exists, the old file will be replaced.
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
    fn test_validate() -> Result<()> {
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
        let res = parser.validate();

        // For each file, 1 result
        assert_eq!(res.len(), 3);

        // No error/warning
        println!("...\nDiagnostics 1:\n{:#?}", res[&0].diagnostics);
        println!("...\nDiagnostics 2:\n{:#?}", res[&1].diagnostics);
        println!("...\nDiagnostics 3:\n{:#?}", res[&2].diagnostics);

        Ok(())
    }
}
