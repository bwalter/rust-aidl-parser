mod ast;
mod error;
pub mod types;

pub fn parse(inputs: &[&str]) -> Result<Vec<types::File>, error::ParseError> {
    let files = inputs.iter().map(|i| {
        let lookup = line_col::LineColLookup::new(i);
        ast::rules::file(i, &lookup).map_err(error::ParseError::Peg)
    });

    files.collect()
}
