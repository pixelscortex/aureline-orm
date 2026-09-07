mod diff;
mod matcher;
mod normalizer;
mod sexpr;

use serde::Serialize;

/// Fluent entry point for an Aureline contract assertion.
pub struct AurlTest {
    source: String,
    source_id: aureline_ast::source::SourceId,
}

impl AurlTest {
    #[doc(hidden)]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            source_id: aureline_ast::source::SourceId::new(0),
        }
    }

    /// Selects the source identity used by location-focused contract assertions.
    #[must_use]
    pub fn with_source_id(mut self, source_id: aureline_ast::source::SourceId) -> Self {
        self.source_id = source_id;
        self
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
        let ast = aureline_parser::parse_with_source(self.source_id, &self.source).unwrap_or_else(
            |errors| {
                panic!(
                    "source did not parse:\n{}\n\nparser errors:\n{errors:#?}",
                    self.source
                )
            },
        );
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
        let ast = aureline_parser::parse_with_source(self.source_id, &self.source).unwrap_or_else(
            |errors| {
                panic!(
                    "source did not parse:\n{}\n\nparser errors:\n{errors:#?}",
                    self.source
                )
            },
        );
        let view = analyze(&ast);
        let actual = normalizer::normalize(&view)
            .unwrap_or_else(|error| panic!("could not normalize semantic output: {error}"));

        matcher::assert_matches(expected, &actual);
    }

    /// Parses source, runs the table semantic checker, and compares its typed
    /// Findings as a logical S-expression.
    ///
    /// The envelope keeps an empty Finding list as one logical root while the
    /// checker-provided contract adapter projects source metadata out of the
    /// ordinary logical expectation. Tests that need exact spans can inspect
    /// the real Findings through [`AurlTest::reports_with`].
    ///
    /// # Panics
    ///
    /// Panics when the source does not parse, the Findings cannot be
    /// normalized, the expectation is not a complete S-expression, or the
    /// semantic result differs from the expectation.
    pub fn findings(self, expected: &str) {
        let ast = aureline_parser::parse_with_source(self.source_id, &self.source).unwrap_or_else(
            |errors| {
                panic!(
                    "source did not parse:\n{}\n\nparser errors:\n{errors:#?}",
                    self.source
                )
            },
        );
        let findings = aureline_checker::check(&ast).findings().to_vec();
        let view = Findings {
            findings: &findings,
        };
        let actual = normalizer::normalize(&view)
            .unwrap_or_else(|error| panic!("could not normalize semantic Findings: {error}"));

        matcher::assert_matches(expected, &actual);
    }

    /// Parses source, checks it, and compares its valid semantic program as a
    /// logical S-expression.
    ///
    /// This assertion crosses the same generation gate as a consumer: an
    /// invalid source panics instead of exposing recovery data as a checked
    /// program. The checker contract serializer projects private storage into
    /// source-ordered tables, fields, and resolved semantic types.
    ///
    /// # Panics
    ///
    /// Panics when the source cannot be parsed or checked, when the checked
    /// program cannot be normalized, or when the expectation does not match.
    pub fn checks_as(self, expected: &str) {
        let ast = aureline_parser::parse_with_source(self.source_id, &self.source).unwrap_or_else(
            |errors| {
                panic!(
                    "source did not parse:\n{}\n\nparser errors:\n{errors:#?}",
                    self.source
                )
            },
        );
        let checked = aureline_checker::check(&ast)
            .into_checked()
            .unwrap_or_else(|analysis| panic!("source did not check:\n{:?}", analysis.findings()));
        let actual = normalizer::normalize(&checked)
            .unwrap_or_else(|error| panic!("could not normalize checked program: {error}"));

        matcher::assert_matches(expected, &actual);
    }
}

#[derive(Serialize)]
struct Findings<'findings> {
    findings: &'findings [aureline_checker::Finding],
}

#[macro_export]
macro_rules! aurl_test {
    ($source:expr $(,)?) => {
        $crate::AurlTest::new($source)
    };
}
