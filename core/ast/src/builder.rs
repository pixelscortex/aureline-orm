use crate::{
    arena::{Arena, ArenaId},
    ast::{Ast, Item, SourceFile, TableDecl},
    ids::ItemId,
};

pub trait AllocInAst {
    type Id: ArenaId;

    fn alloc_in(self, builder: &mut AstBuilder) -> Self::Id;
}

pub struct AstBuilder {
    items: Arena<ItemId, Item>,
}

impl AstBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Arena::new(),
        }
    }

    pub fn alloc<Node>(&mut self, node: Node) -> Node::Id
    where
        Node: AllocInAst,
    {
        node.alloc_in(self)
    }

    #[must_use]
    pub fn finish(self, root: SourceFile) -> Ast {
        Ast::new(root, self.items)
    }
}

impl Default for AstBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AllocInAst for TableDecl {
    type Id = ItemId;

    fn alloc_in(self, builder: &mut AstBuilder) -> Self::Id {
        builder.items.alloc(Item::Table(self))
    }
}
