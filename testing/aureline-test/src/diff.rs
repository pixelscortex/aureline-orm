use crate::sexpr::SExpr;

pub(crate) fn mismatch(expected: &SExpr, actual: &SExpr) -> String {
    let expected_lines = expected.pretty_lines();
    let actual_lines = actual.pretty_lines();

    format!(
        "S-expression mismatch\n\nexpected:\n{}\n\nactual:\n{}\n\ndiff:\n{}",
        expected.compact(),
        actual.compact(),
        line_diff(&expected_lines, &actual_lines)
    )
}

fn line_diff(expected: &[String], actual: &[String]) -> String {
    let mut shared_suffix_lengths = vec![vec![0; actual.len() + 1]; expected.len() + 1];

    for expected_index in (0..expected.len()).rev() {
        for actual_index in (0..actual.len()).rev() {
            shared_suffix_lengths[expected_index][actual_index] =
                if expected[expected_index] == actual[actual_index] {
                    shared_suffix_lengths[expected_index + 1][actual_index + 1] + 1
                } else {
                    shared_suffix_lengths[expected_index + 1][actual_index]
                        .max(shared_suffix_lengths[expected_index][actual_index + 1])
                };
        }
    }

    let mut lines = Vec::new();
    let (mut expected_index, mut actual_index) = (0, 0);

    while expected_index < expected.len() && actual_index < actual.len() {
        if expected[expected_index] == actual[actual_index] {
            lines.push(format!(" {}", expected[expected_index]));
            expected_index += 1;
            actual_index += 1;
        } else if shared_suffix_lengths[expected_index + 1][actual_index]
            >= shared_suffix_lengths[expected_index][actual_index + 1]
        {
            lines.push(format!("-{}", expected[expected_index]));
            expected_index += 1;
        } else {
            lines.push(format!("+{}", actual[actual_index]));
            actual_index += 1;
        }
    }

    lines.extend(
        expected[expected_index..]
            .iter()
            .map(|line| format!("-{line}")),
    );
    lines.extend(actual[actual_index..].iter().map(|line| format!("+{line}")));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::mismatch;
    use crate::sexpr::SExpr;

    #[test]
    fn mismatch_is_copyable_and_marks_the_changed_structure() {
        let expected = SExpr::parse("(SourceFile (Table User Schemaless))").unwrap();
        let actual = SExpr::parse("(SourceFile (Table User Schemafull))").unwrap();

        assert_eq!(
            mismatch(&expected, &actual),
            concat!(
                "S-expression mismatch\n\n",
                "expected:\n(SourceFile (Table User Schemaless))\n\n",
                "actual:\n(SourceFile (Table User Schemafull))\n\n",
                "diff:\n",
                " (SourceFile\n",
                "   (Table\n",
                "     User\n",
                "-    Schemaless\n",
                "+    Schemafull\n",
                "   )\n",
                " )",
            )
        );
    }
}
