//! Lossless name resolution facts for one parsed [`Ast`].
//!
//! [`ResolutionIndex::collect`] performs the only declaration walk needed by
//! table-level checks. It borrows the AST as the canonical declaration store
//! and derives only the two relationships the arenas cannot answer: every
//! table candidate for an exact name, and every field candidate for an exact
//! `(table, name)` pair. Collection has no reporting side effects, preserves
//! duplicate candidates, and keeps source order in the candidate vectors.
//!
//! Checks should use [`ResolutionIndex::resolve_table`] (or
//! [`ResolutionIndex::resolve_field`]) rather than treating a missing value as
//! proof of uniqueness. In particular, an ambiguous result retains every
//! candidate even though a later recovery step may choose the first one for
//! deterministic continuation.

use std::collections::HashMap;

use aureline_ast::{
    ast::Ast,
    ids::{FieldId, TableId},
};

use crate::{Findings, finding::Finding};

/// The result of resolving an exact, case-sensitive table or field name.
///
/// A `Unique` result is the only successful identity resolution. `Missing`
/// proves that no declaration has the requested name, while `Ambiguous`
/// retains all matching declarations in source-registration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableResolution {
    Missing,
    Unique(TableId),
    Ambiguous(Vec<TableId>),
}

/// The result of resolving a field name within one declared table.
///
/// This is separate from [`TableResolution`] so a field result cannot
/// accidentally be used as a table identity. Candidate order follows the
/// owning table's field order, which is also the arena allocation order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldResolution {
    Missing,
    Unique(FieldId),
    Ambiguous(Vec<FieldId>),
}

/// Immutable, lossless name facts derived from one parsed AST.
///
/// The AST remains the canonical store of declarations and ownership edges.
/// This module stores no copied tables or fields: the maps contain only names
/// borrowed from that AST and source-ordered candidate IDs. Map iteration is
/// never used for diagnostic ordering; callers walk [`Ast::root`] and
/// [`ResolutionIndex::fields_of`] when order matters.
#[derive(Debug)]
pub struct ResolutionIndex<'ast> {
    ast: &'ast Ast,
    tables_by_name: HashMap<&'ast str, Vec<TableId>>,
    fields_by_name: HashMap<TableId, HashMap<&'ast str, Vec<FieldId>>>,
}

impl<'ast> ResolutionIndex<'ast> {
    /// Collects lossless table and field candidates without reporting.
    ///
    /// Every table is registered before this function returns, so subsequent
    /// phases can resolve forward references. Each table's fields are
    /// registered against that table's own ID; fields in separate duplicate
    /// table declarations therefore never share a candidate bucket.
    ///
    /// # Panics
    ///
    /// Panics if the AST violates its builder-established arena ownership
    /// edges. A parser-produced AST cannot trigger this condition.
    #[must_use]
    pub fn collect(ast: &'ast Ast) -> Self {
        let mut tables_by_name: HashMap<&'ast str, Vec<TableId>> = HashMap::new();
        let mut fields_by_name: HashMap<TableId, HashMap<&'ast str, Vec<FieldId>>> = HashMap::new();

        for &table_id in ast.root().tables() {
            let table = ast
                .table(table_id)
                .expect("the AST root references a table in its arena");
            tables_by_name
                .entry(table.name())
                .or_default()
                .push(table_id);
            for &field_id in table.fields() {
                let field = ast
                    .field(field_id)
                    .expect("a table references a field in its arena");
                debug_assert_eq!(field.owner(), table_id);
                fields_by_name
                    .entry(table_id)
                    .or_default()
                    .entry(field.name())
                    .or_default()
                    .push(field_id);
            }
        }

        Self {
            ast,
            tables_by_name,
            fields_by_name,
        }
    }

    /// Resolves an exact, case-sensitive table name.
    #[must_use]
    pub fn resolve_table(&self, name: &str) -> TableResolution {
        match self.tables_by_name.get(name) {
            None => TableResolution::Missing,
            Some(candidates) => match candidates.as_slice() {
                [] => TableResolution::Missing,
                [candidate] => TableResolution::Unique(*candidate),
                candidates => TableResolution::Ambiguous(candidates.to_vec()),
            },
        }
    }

    /// Resolves an exact, case-sensitive field name within one table.
    #[must_use]
    pub fn resolve_field(&self, table: TableId, name: &str) -> FieldResolution {
        match self
            .fields_by_name
            .get(&table)
            .and_then(|fields| fields.get(name))
        {
            None => FieldResolution::Missing,
            Some(candidates) => match candidates.as_slice() {
                [] => FieldResolution::Missing,
                [candidate] => FieldResolution::Unique(*candidate),
                candidates => FieldResolution::Ambiguous(candidates.to_vec()),
            },
        }
    }

    /// Returns table IDs in source declaration order.
    #[must_use]
    pub fn tables(&self) -> &[TableId] {
        self.ast.root().tables()
    }

    /// Returns field IDs in source order for `table`.
    ///
    /// The ID must come from the AST borrowed by this index. IDs are
    /// compilation-local arena positions and do not carry an AST identity, so
    /// an ID from another AST may coincidentally select a declaration here.
    #[must_use]
    pub fn fields_of(&self, table: TableId) -> &[FieldId] {
        self.ast
            .table(table)
            .map_or(&[], |table_decl| table_decl.fields())
    }

    /// Reads a table declaration by its compilation-local identity.
    #[must_use]
    pub fn table(&self, table: TableId) -> Option<&'ast aureline_ast::ast::TableDecl> {
        self.ast.table(table)
    }

    /// Reads a field declaration by its compilation-local identity.
    #[must_use]
    pub fn field(&self, field: FieldId) -> Option<&'ast aureline_ast::ast::FieldDecl> {
        self.ast.field(field)
    }

    /// Returns all table candidates for an exact name in source order.
    ///
    /// This crate-visible view lets the duplicate judge inspect collected facts
    /// while keeping the map representation private to this module.
    pub(crate) fn table_candidates(&self, name: &str) -> &[TableId] {
        self.tables_by_name.get(name).map_or(&[], Vec::as_slice)
    }

    /// Returns all field candidates for one table and exact field name.
    ///
    /// Candidate order is the declaration order recorded by the AST builder.
    pub(crate) fn field_candidates(&self, table: TableId, name: &str) -> &[FieldId] {
        match self
            .fields_by_name
            .get(&table)
            .and_then(|fields| fields.get(name))
        {
            Some(candidates) => candidates,
            None => &[],
        }
    }
}

/// Reports duplicate table and field declarations found in `index`.
///
/// Collection and judgement remain separate: this function reads the already
/// collected candidate buckets, while the AST-backed order slices determine
/// emission order. It reports each later exact-name collision once, using the
/// later declaration's name span as primary and the first declaration's name
/// span as context. Table collisions are emitted before field collisions.
pub fn judge_duplicates(index: &ResolutionIndex<'_>, findings: &mut Findings<Finding>) {
    for &table_id in index.tables() {
        let table = index
            .table(table_id)
            .expect("the AST root references a table in its arena");
        let candidates = index.table_candidates(table.name());
        let Some(&first) = candidates.first() else {
            continue;
        };
        if first == table_id {
            continue;
        }
        let first_table = index
            .table(first)
            .expect("a table candidate belongs to the indexed AST");
        findings.report(Finding::DuplicateTable {
            name: table.name().to_owned(),
            primary: table.name_span(),
            first: first_table.name_span(),
        });
    }

    for &table_id in index.tables() {
        for &field_id in index.fields_of(table_id) {
            let field = index
                .field(field_id)
                .expect("a table references a field in its arena");
            let candidates = index.field_candidates(table_id, field.name());
            let Some(&first) = candidates.first() else {
                continue;
            };
            if first == field_id {
                continue;
            }
            let first_field = index
                .field(first)
                .expect("a field candidate belongs to the indexed AST");
            findings.report(Finding::DuplicateField {
                owner: table_id,
                name: field.name().to_owned(),
                primary: field.name_span(),
                first: first_field.name_span(),
            });
        }
    }
}
