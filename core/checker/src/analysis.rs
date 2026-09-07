//! Eager semantic analysis and the generation-safe Checked Program.

use aureline_ast::{
    arena::ArenaId,
    ast::Ast,
    ids::{FieldId, TableId},
};

use crate::{
    Finding, Findings, TypeResolution,
    index::{ResolutionIndex, TableResolution, judge_duplicates},
    resolver,
    semantic_type::SemanticType,
};

/// The result of checking one immutable parsed AST.
///
/// Analysis retains recovery outcomes and Findings for tools that need to
/// explain a rejected program. It performs all field resolution eagerly, in
/// table then field source order; callers never need to re-walk `SourceType`.
#[derive(Debug)]
pub struct Analysis<'ast> {
    index: ResolutionIndex<'ast>,
    field_types: Vec<TypeResolution<SemanticType>>,
    findings: Findings<Finding>,
}

/// A generation-safe semantic program. Its storage is private so consumers
/// depend on narrow queries instead of a copied schema graph.
#[derive(Debug)]
pub struct CheckedProgram<'ast> {
    index: ResolutionIndex<'ast>,
    field_types: Vec<SemanticType>,
}

pub(crate) fn run(ast: &Ast) -> Analysis<'_> {
    let index = ResolutionIndex::collect(ast);
    let mut findings = Findings::new();
    judge_duplicates(&index, &mut findings);

    let mut field_types = Vec::new();
    for &table_id in index.tables() {
        for &field_id in index.fields_of(table_id) {
            let field = index
                .field(field_id)
                .expect("a table references a field in its arena");
            let outcome = resolver::resolve(field.source_type(), &index, &mut findings);
            debug_assert_eq!(field_id.into_index(), field_types.len());
            field_types.push(outcome);
        }
    }

    Analysis {
        index,
        field_types,
        findings,
    }
}

impl<'ast> Analysis<'ast> {
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        self.findings.as_slice()
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.findings.generation_allowed()
    }

    #[must_use]
    pub fn tables(&self) -> &[TableId] {
        self.index.tables()
    }

    #[must_use]
    pub fn table(&self, id: TableId) -> Option<&'ast aureline_ast::ast::TableDecl> {
        self.index.table(id)
    }

    #[must_use]
    pub fn fields_of(&self, table: TableId) -> &[FieldId] {
        self.index.fields_of(table)
    }

    #[must_use]
    pub fn field(&self, id: FieldId) -> Option<&'ast aureline_ast::ast::FieldDecl> {
        self.index.field(id)
    }

    #[must_use]
    pub fn resolve_table(&self, name: &str) -> TableResolution {
        self.index.resolve_table(name)
    }

    #[must_use]
    pub fn type_of_field(&self, id: FieldId) -> Option<&TypeResolution<SemanticType>> {
        self.field_types.get(id.into_index())
    }

    /// Converts analysis into a checked program only when every obligation is
    /// proven. Rejection returns the original Analysis so Findings and invalid
    /// recovery proofs remain available to the caller.
    ///
    /// # Errors
    ///
    /// Returns the analysis when a generation-blocking Finding or an unresolved
    /// field outcome remains.
    pub fn into_checked(self) -> Result<CheckedProgram<'ast>, Box<Self>> {
        if self.has_errors()
            || self
                .field_types
                .iter()
                .any(|outcome| !matches!(outcome, TypeResolution::Resolved(_)))
        {
            return Err(Box::new(self));
        }
        let Self {
            index,
            field_types,
            findings: _,
        } = self;
        let field_types = field_types
            .into_iter()
            .map(|outcome| match outcome {
                TypeResolution::Resolved(semantic_type) => semantic_type,
                TypeResolution::Unknown | TypeResolution::Invalid(_) => {
                    unreachable!("the gate checked every field outcome")
                }
            })
            .collect();
        Ok(CheckedProgram { index, field_types })
    }
}

impl<'ast> CheckedProgram<'ast> {
    /// Returns a canonical string for a field's checked semantic type.
    #[must_use]
    pub fn format_type(&self, id: FieldId) -> Option<String> {
        self.type_of_field(id)
            .map(|ty| crate::type_display::format_type(self, ty))
    }

    /// Derives field presence from the resolved outer type, without revisiting syntax.
    #[must_use]
    pub fn field_presence(&self, id: FieldId) -> Option<crate::FieldPresence> {
        self.type_of_field(id).map(|ty| {
            if ty.admits_none() {
                crate::FieldPresence::Optional
            } else {
                crate::FieldPresence::Required
            }
        })
    }

    #[must_use]
    pub fn tables(&self) -> &[TableId] {
        self.index.tables()
    }

    #[must_use]
    pub fn table(&self, id: TableId) -> Option<&'ast aureline_ast::ast::TableDecl> {
        self.index.table(id)
    }

    #[must_use]
    pub fn table_named(&self, name: &str) -> Option<TableId> {
        match self.index.resolve_table(name) {
            TableResolution::Unique(id) => Some(id),
            TableResolution::Missing | TableResolution::Ambiguous(_) => None,
        }
    }

    #[must_use]
    pub fn fields_of(&self, table: TableId) -> &[FieldId] {
        self.index.fields_of(table)
    }

    #[must_use]
    pub fn field(&self, id: FieldId) -> Option<&'ast aureline_ast::ast::FieldDecl> {
        self.index.field(id)
    }

    #[must_use]
    pub fn type_of_field(&self, id: FieldId) -> Option<&SemanticType> {
        self.field_types.get(id.into_index())
    }
}
