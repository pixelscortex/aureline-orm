//! Eager semantic analysis and the generation-safe Checked Program.
//!
//! Analysis runs the declaration/index pass, resolves each field type once,
//! validates explicit record keys, and retains rejected findings alongside the
//! recovery proofs that explain dependent failures. Only `into_checked` crosses
//! the generation gate; downstream consumers receive resolved facts rather than
//! re-walking source syntax.

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
    /// Returns all typed Findings emitted while checking, in deterministic
    /// source and phase order. A rejected analysis keeps both the root
    /// problems and any later independent problems visible to diagnostics.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        self.findings.as_slice()
    }

    /// Returns whether a generation-blocking Finding was reported.
    ///
    /// `false` does not by itself make [`Self::into_checked`] succeed: an
    /// unresolved field outcome can still prevent construction of a checked
    /// program.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.findings.generation_allowed()
    }

    /// Returns every table ID in source declaration order, including duplicate
    /// declarations retained for diagnostics.
    #[must_use]
    pub fn tables(&self) -> &[TableId] {
        self.index.tables()
    }

    /// Returns the source declaration for a table ID from this analysis.
    ///
    /// Returns `None` when the ID is not present in the borrowed AST. IDs are
    /// arena-local and must not be reused across analyses.
    #[must_use]
    pub fn table(&self, id: TableId) -> Option<&'ast aureline_ast::ast::TableDecl> {
        self.index.table(id)
    }

    /// Returns field IDs for a table in source order.
    ///
    /// An unknown table ID produces an empty slice; a valid table's duplicate
    /// field declarations remain present so callers can explain every Finding.
    #[must_use]
    pub fn fields_of(&self, table: TableId) -> &[FieldId] {
        self.index.fields_of(table)
    }

    /// Returns the source declaration for a field ID from this analysis.
    ///
    /// Returns `None` for an ID outside the borrowed AST. The returned field is
    /// the original source declaration, not a checked or deduplicated copy.
    #[must_use]
    pub fn field(&self, id: FieldId) -> Option<&'ast aureline_ast::ast::FieldDecl> {
        self.index.field(id)
    }

    /// Resolves an exact, case-sensitive table name while retaining ambiguity.
    ///
    /// [`TableResolution::Missing`] means no declaration matched; an
    /// ambiguous result contains every matching table ID in source order.
    #[must_use]
    pub fn resolve_table(&self, name: &str) -> TableResolution {
        self.index.resolve_table(name)
    }

    /// Returns the field's semantic resolution outcome, if its ID is indexed.
    ///
    /// `Resolved` is a usable contract; `Unknown` is recoverable uncertainty;
    /// `Invalid` proves that a root Finding was already reported. `None`
    /// means the field ID does not belong to this analysis.
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
    /// Returns `None` when the table has no single valid top-level `id` field,
    /// including missing, duplicate, or invalid declarations.
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

    /// Returns every checked table ID in source declaration order.
    #[must_use]
    pub fn tables(&self) -> &[TableId] {
        self.index.tables()
    }

    /// Returns the source declaration for a checked table ID.
    ///
    /// Returns `None` for an ID outside this checked program's AST.
    #[must_use]
    pub fn table(&self, id: TableId) -> Option<&'ast aureline_ast::ast::TableDecl> {
        self.index.table(id)
    }

    /// Resolves a table name only when it identifies one checked table.
    ///
    /// Matching is exact and case-sensitive. Missing and ambiguous names both
    /// return `None`, because neither identifies a safe table identity.
    #[must_use]
    pub fn table_named(&self, name: &str) -> Option<TableId> {
        match self.index.resolve_table(name) {
            TableResolution::Unique(id) => Some(id),
            TableResolution::Missing | TableResolution::Ambiguous(_) => None,
        }
    }

    /// Returns field IDs for a checked table in source declaration order.
    ///
    /// An unknown table ID produces an empty slice.
    #[must_use]
    pub fn fields_of(&self, table: TableId) -> &[FieldId] {
        self.index.fields_of(table)
    }

    /// Returns the original source declaration for a checked field ID.
    ///
    /// Returns `None` when the ID is outside this checked program's AST.
    #[must_use]
    pub fn field(&self, id: FieldId) -> Option<&'ast aureline_ast::ast::FieldDecl> {
        self.index.field(id)
    }

    /// Returns the resolved, target-neutral semantic contract for a field.
    ///
    /// Every returned type is generation-safe; `None` means the field ID is
    /// not present in this checked program.
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
