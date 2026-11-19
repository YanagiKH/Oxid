//! File syntax is a private loader parameter, never a conversion between source owners.
use super::*;

pub(in crate::frontend) trait ModuleSyntax: std::fmt::Debug + Sized {
    fn modules(&self) -> &[ast::ModuleDecl];
    fn binding_names(&self) -> impl Iterator<Item = Span>;
    fn uses_project_syntax(&self) -> bool;
    fn ordinary(&self) -> Option<&ast::Program>;
    #[allow(clippy::too_many_arguments)]
    fn parse(
        source: &super::super::source::SourceFile,
        tokens: Vec<lexer::Token>,
        mode: parser::SourceMode,
        arrays: parser::ArraySyntaxPolicy,
        enums: ProjectEnumSyntax,
        remaining_nodes: usize,
        allocator: &mut Allocator,
    ) -> Result<(Self, usize), Errors>;
}
impl ModuleSyntax for ast::Program {
    fn modules(&self) -> &[ast::ModuleDecl] {
        &self.modules
    }
    fn binding_names(&self) -> impl Iterator<Item = Span> {
        self.functions
            .iter()
            .map(|v| v.name)
            .chain(self.records.iter().map(|v| v.name))
            .chain(self.enums.iter().map(|v| v.name))
            .chain(self.modules.iter().map(|v| v.name))
            .chain(self.imports.iter().map(|v| v.alias))
    }
    fn uses_project_syntax(&self) -> bool {
        self.uses_project_syntax()
    }
    fn ordinary(&self) -> Option<&ast::Program> {
        Some(self)
    }
    fn parse(
        source: &super::super::source::SourceFile,
        tokens: Vec<lexer::Token>,
        mode: parser::SourceMode,
        arrays: parser::ArraySyntaxPolicy,
        enums: ProjectEnumSyntax,
        remaining_nodes: usize,
        allocator: &mut Allocator,
    ) -> Result<(Self, usize), Errors> {
        let program = match enums {
            #[cfg(test)]
            ProjectEnumSyntax::StdClosed => parser::parse_typed_closed_std_counted(
                source,
                tokens,
                mode,
                remaining_nodes,
                allocator,
                &mut Default::default(),
            )?,
            #[cfg(test)]
            ProjectEnumSyntax::OutputCandidate => parser::parse_output_candidate_counted(
                source,
                tokens,
                mode,
                remaining_nodes,
                allocator,
                &mut Default::default(),
            )?,
            #[cfg(test)]
            ProjectEnumSyntax::BuiltinCandidate => parser::parse_builtin_candidate_counted(
                source,
                tokens,
                mode,
                remaining_nodes,
                allocator,
                &mut Default::default(),
            )?,
            ProjectEnumSyntax::Closed => {
                if arrays == parser::ArraySyntaxPolicy::Closed {
                    parser::parse_counted(source, tokens, mode, remaining_nodes, allocator)?
                } else {
                    parser::parse_counted_with_arrays(
                        source,
                        tokens,
                        mode,
                        remaining_nodes,
                        allocator,
                        arrays,
                    )?
                }
            }
            ProjectEnumSyntax::Enabled => parser::parse_typed_counted(
                source,
                tokens,
                mode,
                remaining_nodes,
                allocator,
                &mut Default::default(),
            )?,
            #[cfg(test)]
            ProjectEnumSyntax::Candidate => parser::parse_enum_candidate_counted(
                source,
                tokens,
                mode,
                remaining_nodes,
                allocator,
                &mut Default::default(),
            )?,
        };
        Ok(program)
    }
}

impl ModuleSyntax for parser::editor_query::EditorFile {
    fn modules(&self) -> &[ast::ModuleDecl] {
        self.modules()
    }
    fn binding_names(&self) -> impl Iterator<Item = Span> {
        self.binding_names()
    }
    fn uses_project_syntax(&self) -> bool {
        true
    }
    fn ordinary(&self) -> Option<&ast::Program> {
        None
    }
    fn parse(
        source: &super::super::source::SourceFile,
        tokens: Vec<lexer::Token>,
        _mode: parser::SourceMode,
        _arrays: parser::ArraySyntaxPolicy,
        _enums: ProjectEnumSyntax,
        remaining_nodes: usize,
        allocator: &mut Allocator,
    ) -> Result<(Self, usize), Errors> {
        Self::parse(source, tokens, remaining_nodes, allocator)
    }
}
