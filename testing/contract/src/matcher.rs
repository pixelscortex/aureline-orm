use crate::{diff, sexpr::SExpr};

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
