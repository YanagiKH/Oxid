//! Experimental scalar compiler and bounded reference runner. This module does not import the legacy runtime.
mod ast;
mod build_config;
mod builtin_catalog;
mod byte_strings;
mod declaration_index;
mod diagnostic;
mod documentation;
mod driver;
pub(crate) mod editor;
mod format;
mod format_cli;
mod hir;
mod hir_producer;
mod hir_protocol;
mod lexer;
mod lexical_provider;
mod native;
mod native_cache;
mod native_debug;
mod oir;
mod options;
mod owned_diagnostic;
mod parser;
mod project;
mod source;
mod typeck;
mod windows_native;
pub(crate) use build_config::{ExecutionPolicy, NativeBuildSettings, NativeOptimization};
pub use driver::dispatch;
pub(crate) use driver::{
    prepare_package_project, prepare_package_project_bounded, PreparedPackageProject,
};
pub(crate) use driver::{process_package_project, PackageCommand};
pub(crate) use native::doctor::{run as native_doctor, Report as NativeDoctorReport};
pub(crate) use project::packages::{PackageDependency, PackageSpec};
#[cfg(test)]
mod owned_syntax_tests;

#[cfg(test)]
mod enum_public_tests;
#[cfg(test)]
mod stdin_public_tests;
