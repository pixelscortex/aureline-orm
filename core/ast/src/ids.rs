use crate::arena::ArenaId;

macro_rules! define_arena_id {
    ($vis:vis struct $name:ident;) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis struct $name(u32);

        impl ArenaId for $name {
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
    };
}

define_arena_id!(
    pub struct ItemId;
);
