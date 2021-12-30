#![doc = include_str!("../README.md")]

pub mod ast;
pub mod diagnostic;
mod javadoc;
pub mod parser;
mod rules;
pub mod symbol;
pub mod traverse;
mod validation;

pub use parser::{ParseFileResult, Parser};
