//! Assertion glue between inline contract text and normalized compiler output.
//!
//! Expectations are parsed into the same tree representation as actual output,
//! so whitespace and line wrapping remain presentation details while structure
//! remains exact.

use crate::{diff, sexpr::SExpr};

/// Parses an inline expectation and compares it with normalized output.
///
/// Invalid expectations are reported as test-author errors; structural
/// mismatches use the tree-aware diff so the changed constructor is visible.
pub(crate) fn assert_matches(expected: &str, actual: &SExpr) {
    let expected = SExpr::parse(expected)
        .unwrap_or_else(|error| panic!("invalid expected S-expression:\n{expected}\n\n{error}"));

    assert!(expected == *actual, "{}", diff::mismatch(&expected, actual));
}

#[cfg(test)]
mod tests {
    use super::assert_matches;
    use crate::sexpr::SExpr;

    #[test]
    fn matcher_ignores_s_expression_formatting() {
        let actual = SExpr::parse("(SourceFile (Table User Schemafull))").unwrap();

        assert_matches("(SourceFile\n  (Table User Schemafull)\n)", &actual);
    }
}
