//! Dense, typed storage used by one parsed AST.
//!
//! An `Arena<TableId, TableDecl>` conceptually maps the first allocated table to
//! `TableId(0)`, the second to `TableId(1)`, and so on;
//! [`Arena::get`] converts the typed ID back to its vector index. The typed ID
//! prevents accidentally using a [`FieldId`](crate::ids::FieldId) to read the
//! table arena, while the arena keeps values densely stored in source order.
//!
//! `Arena::alloc_with` supports parent/child construction inside this crate:
//!
//! ```text
//! next table slot is 0
//!   -> pass TableId(0) into the builder closure
//!   -> closure builds fields whose owner is TableId(0)
//!   -> store the completed table in slot 0
//! ```
//!
//! The reserved ID never escapes unless the completed value is stored.

use std::marker::PhantomData;

pub trait ArenaId: Copy {
    fn from_index(index: usize) -> Self;
    fn into_index(self) -> usize;
}

#[derive(Debug)]
pub struct Arena<Id, T> {
    values: Vec<T>,
    _id: PhantomData<Id>,
}

impl<Id, T> Arena<Id, T>
where
    Id: ArenaId,
{
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            _id: PhantomData,
        }
    }

    pub fn alloc(&mut self, value: T) -> Id {
        let id = Id::from_index(self.values.len());
        self.values.push(value);
        id
    }

    pub(crate) fn alloc_with(&mut self, build: impl FnOnce(Id) -> T) -> Id {
        // Reserve the next identity before building the value so its children can
        // record a parent reference during the same atomic construction step. The
        // value is stored before this ID can escape the call.
        let id = self.next_id();
        self.values.push(build(id));
        id
    }

    fn next_id(&self) -> Id {
        Id::from_index(self.values.len())
    }

    pub fn get(&self, id: Id) -> Option<&T> {
        self.values.get(id.into_index())
    }
}

impl<Id, T> Default for Arena<Id, T>
where
    Id: ArenaId,
{
    fn default() -> Self {
        Self::new()
    }
}
