//! Experimental check-only compiler. This module does not import the legacy runtime.
mod ast;
mod diagnostic;
mod driver;
mod hir;
mod lexer;
mod oir;
mod options;
mod parser;
mod source;
mod typeck;
pub use driver::dispatch;
