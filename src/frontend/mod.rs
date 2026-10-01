//! Experimental scalar compiler and bounded reference runner. This module does not import the legacy runtime.
mod ast;
mod diagnostic;
mod driver;
mod hir;
mod lexer;
mod native;
mod oir;
mod options;
mod parser;
mod source;
mod typeck;
pub use driver::dispatch;
