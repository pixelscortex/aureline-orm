use crate::arena::ArenaId;

macro_rules! define_arena_id {
    ($vis:vis struct $name:ident;) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis struct $name(u32);

        impl ArenaId for $name {
            #[cfg(feature = "unstable-test-normalization")]
            const KIND: &'static str = stringify!($name);

            fn from_index(index: usize) -> Self {
                let index = u32::try_from(index).expect(concat!(
                    stringify!($name),
                    " arena exceeded u32::MAX nodes"
                ));

                Self(index)
            }

            fn into_index(self) -> usize {
                self.0 as usize
            }
        }

        #[cfg(feature = "unstable-test-normalization")]
        impl serde::Serialize for $name {
            fn serialize<SerializerType>(
                &self,
                serializer: SerializerType,
            ) -> Result<SerializerType::Ok, SerializerType::Error>
            where
                SerializerType: serde::Serializer,
            {
                use serde::ser::SerializeStruct;

                let mut reference = serializer.serialize_struct("$Ref", 2)?;
                reference.serialize_field("kind", Self::KIND)?;
                reference.serialize_field("index", &self.0)?;
                reference.end()
            }
        }
    };
}

define_arena_id!(
    pub struct ItemId;
);
