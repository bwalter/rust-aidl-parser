mod diagnostic;
mod javadoc;
mod parser;
pub mod types;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(#[allow(clippy::all)] pub aidl);

pub type ParseResult = Vec<ParseFile>;

pub struct ParseFile {
    pub file: Option<types::File>,
    pub diagnostics: Vec<diagnostic::Diagnostic>,
}

pub fn parse<T>(inputs: &[T]) -> ParseResult
where
    T: AsRef<str>,
{
    let file_results = inputs.iter().map(|i| {
        let lookup = line_col::LineColLookup::new(i.as_ref());
        let mut diagnostics = Vec::new();

        let rule_result = aidl::FileParser::new().parse(&lookup, &mut diagnostics, i.as_ref());

        match rule_result {
            Ok(file) => ParseFile { file, diagnostics },
            Err(_) => ParseFile {
                file: None,
                diagnostics,
            },
        }
    });

    file_results.collect()
}
