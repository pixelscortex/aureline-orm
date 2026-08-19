use crate::{
    arena::Arena,
    ast::{Ast, FieldDecl, SchemaType, SourceFile, SourceType, TableDecl},
    ids::{FieldId, TableId},
    source::SourceSpan,
};

pub struct AstBuilder {
    tables: Arena<TableId, TableDecl>,
    fields: Arena<FieldId, FieldDecl>,
    table_order: Vec<TableId>,
}

impl AstBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tables: Arena::new(),
            fields: Arena::new(),
            table_order: Vec::new(),
        }
    }

    /// Allocates a table and its fields as one construction step.
    ///
    /// Calls to `build_fields` allocate fields in source order. The builder assigns
    /// each field the new table's identity and records the resulting field identities
    /// on the table, so callers cannot construct only one ownership edge. The name
    /// and schema-mode spans must use the same source as `span` and be contained by it.
    pub fn alloc_table<Name, BuildFields>(
        &mut self,
        span: SourceSpan,
        name: Name,
        name_span: SourceSpan,
        schema_type: SchemaType,
        schema_type_span: SourceSpan,
        build_fields: BuildFields,
    ) -> TableId
    where
        Name: Into<String>,
        BuildFields: FnOnce(&mut TableFieldBuilder<'_>),
    {
        let fields = &mut self.fields;
        let table_id = self.tables.alloc_with(|table_id| {
            let mut table_fields = TableFieldBuilder {
                owner: table_id,
                arena: fields,
                allocated: Vec::new(),
            };
            build_fields(&mut table_fields);
            TableDecl::new(
                span,
                name,
                name_span,
                schema_type,
                schema_type_span,
                table_fields.allocated,
            )
        });
        self.table_order.push(table_id);
        table_id
    }

    #[must_use]
    pub fn finish(self) -> Ast {
        let root = SourceFile::new(self.table_order);
        Ast::new(root, self.tables, self.fields)
    }
}

/// Allocates the fields owned by one table during [`AstBuilder::alloc_table`].
pub struct TableFieldBuilder<'ast> {
    owner: TableId,
    arena: &'ast mut Arena<FieldId, FieldDecl>,
    allocated: Vec<FieldId>,
}

impl TableFieldBuilder<'_> {
    /// Allocates one field and records it on its owning table in call order.
    ///
    /// `name_span` and `source_type.span()` must use the same source as `span` and
    /// be contained by it.
    pub fn alloc_field<Name>(
        &mut self,
        span: SourceSpan,
        name: Name,
        name_span: SourceSpan,
        source_type: SourceType,
    ) -> FieldId
    where
        Name: Into<String>,
    {
        let id = self.arena.alloc(FieldDecl::new(
            span,
            name,
            name_span,
            source_type,
            self.owner,
        ));
        self.allocated.push(id);
        id
    }
}

impl Default for AstBuilder {
    fn default() -> Self {
        Self::new()
    }
}
