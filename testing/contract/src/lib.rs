//! Contract tests for the parser, checker, and migration seams.
//!
//! The harness deliberately exposes a small fluent API so integration tests can
//! describe a source fixture once and assert the stage they care about. Parser
//! assertions compare a normalized logical tree; compilation assertions run the
//! same parser/checker/migration pipeline used by the product and then compare
//! the generated DDL exactly.

mod diff;
mod matcher;
mod normalizer;
mod sexpr;

/// Fluent entry point for an Aureline contract assertion.
///
/// A value owns its source fixture and consumes itself at the assertion that
/// completes the contract. This keeps parser and migration expectations tied to
/// one input while preventing a test from accidentally mixing stages.
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

    /// Parses, checks, and generates a first migration from empty history.
    ///
    /// The returned [`Compiled`] value contains the generated artifact without
    /// touching a database. Contract tests can therefore pin the offline
    /// generator's output and its ordering independently of runtime execution.
    ///
    /// # Panics
    ///
    /// Panics when parsing, static checking, or migration generation rejects the source.
    #[must_use]
    pub fn compiles(self) -> Compiled {
        let ast = aureline_parser::parse(&self.source).unwrap_or_else(|errors| {
            panic!(
                "source did not parse:\n{}\n\nparser errors:\n{errors:#?}",
                self.source
            )
        });
        let checked = aureline_checker::check(&ast)
            .into_checked()
            .unwrap_or_else(|analysis| {
                panic!(
                    "source did not check:\n{}\n\nfindings:\n{:#?}",
                    self.source,
                    analysis.findings()
                )
            });
        let generation = aureline_migration::generate(&checked, None).unwrap_or_else(|errors| {
            panic!(
                "source did not generate:\n{}\n\ngeneration errors:\n{errors:#?}",
                self.source
            )
        });
        Compiled {
            ddl: generation.script,
        }
    }
}

/// Generated artifacts from one source compilation, available for exact assertions.
pub struct Compiled {
    ddl: String,
}

impl Compiled {
    /// Compares the first migration script byte-for-byte with an inline expectation.
    ///
    /// # Panics
    ///
    /// Panics with a readable expected-versus-actual diff when the script differs.
    pub fn ddl(self, expected: &str) {
        assert!(
            self.ddl == expected,
            "{}",
            diff::artifact_mismatch(expected, &self.ddl)
        );
    }
}

#[macro_export]
macro_rules! aurl_test {
    ($source:expr $(,)?) => {
        $crate::AurlTest::new($source)
    };
}
