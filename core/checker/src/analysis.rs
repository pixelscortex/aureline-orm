//! Eager semantic analysis and the generation-safe Checked Program.

use aureline_ast::{
    arena::ArenaId,
    ast::Ast,
    ids::{FieldId, TableId},
};

use crate::{
    Finding, Findings, TypeResolution, id_contract,
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
    id_fields: Vec<Option<FieldId>>,
    findings: Findings<Finding>,
}

/// A generation-safe semantic program. Its storage is private so consumers
/// depend on narrow queries instead of a copied schema graph.
#[derive(Debug)]
pub struct CheckedProgram<'ast> {
    index: ResolutionIndex<'ast>,
    field_types: Vec<SemanticType>,
    id_fields: Vec<Option<FieldId>>,
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
    let id_fields = validate_id_fields(&index, &field_types, &mut findings);

    Analysis {
        index,
        field_types,
        id_fields,
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
            id_fields,
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
        Ok(CheckedProgram {
            index,
            field_types,
            id_fields,
        })
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
    ///
    /// A validated explicit `id` is always required because every record has
    /// an identity, including when its semantic type is `any`.
    #[must_use]
    pub fn field_presence(&self, id: FieldId) -> Option<crate::FieldPresence> {
        self.type_of_field(id).map(|ty| {
            let is_id = self.field(id).is_some_and(|field| {
                self.id_fields
                    .get(field.owner().into_index())
                    .copied()
                    .flatten()
                    == Some(id)
            });
            if is_id || !ty.admits_none() {
                crate::FieldPresence::Required
            } else {
                crate::FieldPresence::Optional
            }
        })
    }

    /// Returns the validated contract for a table's explicit `id` field.
    ///
    /// # Panics
    ///
    /// Panics if private validation data refers to a field without a resolved
    /// type. Construction through the checker maintains this invariant.
    #[must_use]
    pub fn id_contract(&self, table: TableId) -> Option<crate::IdContract<'_>> {
        let field_id = self.id_fields.get(table.into_index()).copied().flatten()?;
        let semantic = self
            .type_of_field(field_id)
            .expect("a validated id has a resolved semantic type");
        Some(id_contract::from_validated(field_id, semantic))
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

fn validate_id_fields(
    index: &ResolutionIndex<'_>,
    field_types: &[TypeResolution<SemanticType>],
    findings: &mut Findings<Finding>,
) -> Vec<Option<FieldId>> {
    let mut id_fields = vec![None; index.tables().len()];

    for &table_id in index.tables() {
        let mut valid_id = None;
        let candidates = index.field_candidates(table_id, "id");
        for &field_id in candidates {
            let field = index
                .field(field_id)
                .expect("an indexed id field belongs to the AST");

            let outcome = field_types
                .get(field_id.into_index())
                .expect("an indexed field has a type resolution outcome");
            let TypeResolution::Resolved(semantic) = outcome else {
                continue;
            };
            if id_contract::validate_semantic_type(semantic) {
                valid_id.get_or_insert(field_id);
            } else {
                findings.report(Finding::InvalidRecordKey {
                    field: field_id,
                    span: field.source_type().span(),
                });
            }
        }

        if candidates.len() == 1 {
            id_fields[table_id.into_index()] = valid_id;
        }
    }

    id_fields
}
