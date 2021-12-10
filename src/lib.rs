mod ast;
mod diagnostic;
pub mod types;

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
        println!("Parsing one file");
        let lookup = line_col::LineColLookup::new(i.as_ref());
        let mut diagnostics = Vec::new();

        let rule_result = ast::rules::file(i.as_ref(), &lookup, &mut diagnostics);

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
