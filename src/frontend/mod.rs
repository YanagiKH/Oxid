//! Experimental scalar compiler and bounded reference runner. This module does not import the legacy runtime.
mod ast;
mod declaration_index;
mod diagnostic;
mod driver;
mod hir;
mod lexer;
mod native;
mod oir;
mod options;
mod owned_diagnostic;
mod parser;
mod project;
mod source;
mod typeck;
pub use driver::dispatch;
#[cfg(test)]
mod owned_syntax_tests;
