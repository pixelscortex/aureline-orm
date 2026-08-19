use std::marker::PhantomData;

#[cfg(feature = "unstable-test-normalization")]
use serde::{Serialize, Serializer, ser::SerializeStruct};

pub trait ArenaId: Copy {
    #[cfg(feature = "unstable-test-normalization")]
    const KIND: &'static str;

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

#[cfg(feature = "unstable-test-normalization")]
impl<Id, T> Serialize for Arena<Id, T>
where
    Id: ArenaId,
    T: Serialize,
{
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        let mut arena = serializer.serialize_struct("$Arena", 2)?;
        arena.serialize_field("kind", Id::KIND)?;
        arena.serialize_field("values", &self.values)?;
        arena.end()
    }
}
