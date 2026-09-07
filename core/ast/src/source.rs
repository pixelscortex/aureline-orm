use serde::Serialize;

/// Identity of one source document in a [`SourceRegistry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct SourceId(u32);

impl SourceId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A UTF-8 byte offset into an Aureline source document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct TextSize(u32);

impl TextSize {
    pub const MAX: Self = Self(u32::MAX);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Adds two byte offsets without truncating.
    ///
    /// # Errors
    ///
    /// Returns [`TextSizeOverflow`] when the sum exceeds [`TextSize::MAX`].
    pub fn checked_add(self, other: Self) -> Result<Self, TextSizeOverflow> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(TextSizeOverflow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSizeOverflow;

impl TryFrom<usize> for TextSize {
    type Error = TextSizeOverflow;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        u32::try_from(value).map(Self).map_err(|_| TextSizeOverflow)
    }
}

/// A half-open range of UTF-8 byte offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct TextRange {
    start: TextSize,
    end: TextSize,
}

impl TextRange {
    /// Creates a half-open byte range.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidTextRange`] when `end` precedes `start`.
    pub fn new(start: TextSize, end: TextSize) -> Result<Self, InvalidTextRange> {
        if start <= end {
            Ok(Self { start, end })
        } else {
            Err(InvalidTextRange { start, end })
        }
    }

    #[must_use]
    pub const fn start(self) -> TextSize {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> TextSize {
        self.end
    }

    /// Moves a payload-relative range into its containing source document.
    ///
    /// # Errors
    ///
    /// Returns [`TextSizeOverflow`] when either rebased endpoint exceeds
    /// [`TextSize::MAX`].
    pub fn rebase(self, payload_start: TextSize) -> Result<Self, TextSizeOverflow> {
        Ok(Self {
            start: payload_start.checked_add(self.start)?,
            end: payload_start.checked_add(self.end)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidTextRange {
    pub start: TextSize,
    pub end: TextSize,
}

/// A range associated with its source document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct SourceSpan {
    source: SourceId,
    range: TextRange,
}

impl SourceSpan {
    #[must_use]
    pub const fn new(source: SourceId, range: TextRange) -> Self {
        Self { source, range }
    }

    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceRegistrationError {
    SourceTooLarge,
    RegistryFull,
}

/// Owns source documents independently of the locations that refer to them.
#[derive(Debug, Default)]
pub struct SourceRegistry {
    sources: Vec<String>,
}

impl SourceRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Registers the exact text and rejects sources whose byte length cannot be located.
    ///
    /// # Errors
    ///
    /// Returns [`SourceRegistrationError::SourceTooLarge`] when the source exceeds
    /// [`TextSize::MAX`] bytes, or [`SourceRegistrationError::RegistryFull`] when no
    /// further [`SourceId`] can be allocated.
    pub fn register(
        &mut self,
        source: impl Into<String>,
    ) -> Result<SourceId, SourceRegistrationError> {
        let source = source.into();
        TextSize::try_from(source.len()).map_err(|_| SourceRegistrationError::SourceTooLarge)?;
        let id = u32::try_from(self.sources.len())
            .map(SourceId)
            .map_err(|_| SourceRegistrationError::RegistryFull)?;
        self.sources.push(source);
        Ok(id)
    }

    #[must_use]
    pub fn source(&self, id: SourceId) -> Option<&str> {
        self.sources.get(id.get() as usize).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceId, SourceRegistry, SourceSpan, TextRange, TextSize};

    #[test]
    fn location_values_preserve_half_open_utf8_byte_offsets() {
        let source = SourceId::new(7);
        let start = TextSize::try_from("é".len()).expect("UTF-8 byte length fits");
        let end = TextSize::try_from("éclair".len()).expect("UTF-8 byte length fits");
        let range = TextRange::new(start, end).expect("range is ordered");

        assert_eq!(range.start(), TextSize::new(2));
        assert_eq!(range.end(), TextSize::new(7));
        assert_eq!(SourceSpan::new(source, range).source(), source);
    }

    #[test]
    fn overflowing_offsets_and_reversed_ranges_are_rejected() {
        assert!(TextSize::try_from(usize::MAX).is_err());
        assert!(TextRange::new(TextSize::new(2), TextSize::new(1)).is_err());
    }

    #[test]
    fn nested_payload_ranges_rebase_by_checked_addition() {
        let relative =
            TextRange::new(TextSize::new(1), TextSize::new(3)).expect("range is ordered");

        assert_eq!(
            relative.rebase(TextSize::new(5)),
            Ok(TextRange::new(TextSize::new(6), TextSize::new(8)).expect("range is ordered"))
        );

        let ending_at_limit =
            TextRange::new(TextSize::new(0), TextSize::MAX).expect("range is ordered");
        assert!(ending_at_limit.rebase(TextSize::new(1)).is_err());
    }

    #[test]
    fn source_registry_owns_the_exact_supplied_text_separately() {
        let mut sources = SourceRegistry::new();
        let first = sources
            .register("table Café schemafull {}")
            .expect("source fits");
        let second = sources.register(String::new()).expect("source fits");

        assert_eq!(first, SourceId::new(0));
        assert_eq!(second, SourceId::new(1));
        assert_eq!(sources.source(first), Some("table Café schemafull {}"));
        assert_eq!(sources.source(second), Some(""));
        assert_eq!(sources.source(SourceId::new(2)), None);
    }

    #[test]
    fn location_types_are_serializable() {
        fn assert_serializable<T: serde::Serialize>() {}

        assert_serializable::<SourceId>();
        assert_serializable::<TextSize>();
        assert_serializable::<TextRange>();
        assert_serializable::<SourceSpan>();
    }
}
