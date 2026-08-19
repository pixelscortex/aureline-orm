mod diff;
mod matcher;
mod normalizer;
mod sexpr;

/// Fluent entry point for an Aureline contract assertion.
pub struct AurlTest {
    source: String,
}

impl AurlTest {
    #[doc(hidden)]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }

    /// Parses the source and compares its logical structure exactly with an S-expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use aureline_test::aurl_test;
    ///
    /// aurl_test!("table User schemafull {}")
    ///     .parses_as("(SourceFile (Table User Schemafull))");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when the source cannot be parsed, the expectation is not a complete
    /// S-expression, or the parsed logical structure differs from the expectation.
    pub fn parses_as(self, expected: &str) {
        let ast = aureline_parser::parse(&self.source).unwrap_or_else(|errors| {
            panic!(
                "source did not parse:\n{}\n\nparser errors:\n{errors:#?}",
                self.source
            )
        });
        let actual = normalizer::normalize(&ast)
            .unwrap_or_else(|error| panic!("could not normalize parser output: {error}"));

        matcher::assert_matches(expected, &actual);
    }
}

#[macro_export]
macro_rules! aurl_test {
    ($source:expr $(,)?) => {
        $crate::AurlTest::new($source)
    };
}
