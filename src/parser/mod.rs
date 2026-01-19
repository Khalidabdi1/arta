//! Parser module for Arta DSL

pub mod ast;
pub mod grammar;

pub use ast::*;
pub use grammar::{parse_command, parse_script};
