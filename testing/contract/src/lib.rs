mod diff;
mod matcher;
mod normalizer;
mod sexpr;

use serde::Serialize;

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

    /// Parses source, runs a test-only semantic view over the real AST, and
    /// compares that view as an exact logical S-expression.
    ///
    /// This keeps semantic contract cases on the same seam as parser cases:
    /// the closure receives the parser's AST, while the test may choose a
    /// small serializable view of a phase-local result. The view is private to
    /// the contract test and is not a compiler serialization format.
    ///
    /// # Panics
    ///
    /// Panics when the source cannot be parsed, the view cannot be normalized,
    /// the expectation is not a complete S-expression, or the view differs
    /// from the expectation.
    pub fn reports_with<Analyze, View>(self, analyze: Analyze, expected: &str)
    where
        Analyze: FnOnce(&aureline_ast::ast::Ast) -> View,
        View: Serialize,
    {
        let ast = aureline_parser::parse(&self.source).unwrap_or_else(|errors| {
            panic!(
                "source did not parse:\n{}\n\nparser errors:\n{errors:#?}",
                self.source
            )
        });
        let view = analyze(&ast);
        let actual = normalizer::normalize(&view)
            .unwrap_or_else(|error| panic!("could not normalize semantic output: {error}"));

        matcher::assert_matches(expected, &actual);
    }
}

#[macro_export]
macro_rules! aurl_test {
    ($source:expr $(,)?) => {
        $crate::AurlTest::new($source)
    };
}
