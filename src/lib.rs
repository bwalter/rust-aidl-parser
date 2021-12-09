mod ast;
mod diagnostic;
pub mod types;

pub type ParseResult = Vec<ParseFile>;

pub struct ParseFile {
    pub file: Option<types::File>,
    pub diagnostics: Vec<diagnostic::Diagnostic>,
}

pub fn parse(inputs: &[&str]) -> ParseResult {
    let file_results = inputs.iter().map(|i| {
        let lookup = line_col::LineColLookup::new(i);
        let mut diagnostics = Vec::new();

        let rule_result = ast::rules::file(i, &lookup, &mut diagnostics);

        match rule_result {
            Ok(file) => ParseFile { file, diagnostics },
            Err(e) => ParseFile {
                file: None,
                diagnostics: Vec::from([diagnostic::from_peg_error(&lookup, e)]),
            },
        }
    });

    file_results.collect()
}
