#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Parsing error")]
    Peg(#[from] peg::error::ParseError<peg::str::LineCol>),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
