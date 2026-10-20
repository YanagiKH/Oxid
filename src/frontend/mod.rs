//! Experimental scalar compiler and bounded reference runner. This module does not import the legacy runtime.
mod ast;
mod builtin_catalog;
mod byte_literal;
mod declaration_index;
mod diagnostic;
mod diagnostic_context;
mod driver;
mod format;
mod format_cli;
mod hir;
mod hir_producer;
mod hir_protocol;
mod lexer;
mod lexical_provider;
mod manifest;
mod native;
mod native_bundle;
mod oir;
mod options;
mod owned_diagnostic;
mod parser;
mod project;
mod project_build;
mod project_format;
mod project_format_cli;
mod project_package;
mod project_scaffold;
mod project_test;
mod session;
mod source;
mod toolchain_doctor;
mod typeck;
pub use driver::dispatch;
#[cfg(test)]
mod owned_syntax_tests;

#[cfg(test)]
mod enum_public_tests;
#[cfg(test)]
mod stdin_public_tests;
