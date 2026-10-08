//! Experimental scalar compiler and bounded reference runner. This module does not import the legacy runtime.
mod ast;
mod builtin_catalog;
mod declaration_index;
mod diagnostic;
mod driver;
mod format;
mod format_cli;
mod hir;
mod hir_producer;
mod hir_protocol;
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

#[cfg(test)]
mod enum_public_tests;
#[cfg(test)]
mod stdin_public_tests;
