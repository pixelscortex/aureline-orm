use std::fmt::Write as _;

use chumsky::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SExpr {
    Atom(String),
    List(Vec<Self>),
}

impl SExpr {
    pub(crate) fn parse(source: &str) -> Result<Self, String> {
        parser().parse(source).into_result().map_err(|errors| {
            errors
                .into_iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    pub(crate) fn compact(&self) -> String {
        match self {
            Self::Atom(atom) => render_atom(atom),
            Self::List(items) => {
                let contents = items
                    .iter()
                    .map(Self::compact)
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("({contents})")
            }
        }
    }

    pub(crate) fn pretty_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        self.write_pretty_lines(0, &mut lines);
        lines
    }

    fn write_pretty_lines(&self, depth: usize, lines: &mut Vec<String>) {
        let indent = "  ".repeat(depth);

        match self {
            Self::Atom(atom) => lines.push(format!("{indent}{}", render_atom(atom))),
            Self::List(items) if items.is_empty() => lines.push(format!("{indent}()")),
            Self::List(items) => {
                let (head, tail) = items.split_first().expect("non-empty list");
                match head {
                    Self::Atom(atom) => {
                        lines.push(format!("{indent}({}", render_atom(atom)));
                    }
                    Self::List(_) => {
                        lines.push(format!("{indent}("));
                        head.write_pretty_lines(depth + 1, lines);
                    }
                }
                for item in tail {
                    item.write_pretty_lines(depth + 1, lines);
                }
                lines.push(format!("{indent})"));
            }
        }
    }
}

fn parser<'source>() -> impl Parser<'source, &'source str, SExpr, extra::Err<Rich<'source, char>>> {
    let bare_atom = any()
        .filter(|character: &char| is_bare_atom_character(*character))
        .repeated()
        .at_least(1)
        .to_slice()
        .map(|atom: &str| SExpr::Atom(atom.to_owned()));

    let unicode_escape = any()
        .filter(char::is_ascii_hexdigit)
        .repeated()
        .at_least(1)
        .at_most(6)
        .to_slice()
        .delimited_by(just('{'), just('}'))
        .try_map(|digits: &str, span| {
            u32::from_str_radix(digits, 16)
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| Rich::custom(span, "invalid Unicode scalar escape"))
        });
    let escape = just('\\').ignore_then(choice((
        just('"').to('"'),
        just('\\').to('\\'),
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
        just('u').ignore_then(unicode_escape),
    )));
    let quoted_atom = choice((
        escape,
        none_of("\\\"").filter(|character: &char| !character.is_control()),
    ))
    .repeated()
    .collect::<String>()
    .delimited_by(just('"'), just('"'))
    .map(SExpr::Atom);

    recursive(|expression| {
        let list = expression
            .padded()
            .repeated()
            .collect::<Vec<_>>()
            .map(SExpr::List)
            .delimited_by(just('('), just(')'));

        list.or(quoted_atom).or(bare_atom)
    })
    .padded()
    .then_ignore(end())
}

fn render_atom(atom: &str) -> String {
    if !atom.is_empty() && atom.chars().all(is_bare_atom_character) {
        return atom.to_owned();
    }

    let mut rendered = String::with_capacity(atom.len() + 2);
    rendered.push('"');
    for character in atom.chars() {
        match character {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            character if character.is_control() => {
                write!(rendered, "\\u{{{:x}}}", u32::from(character))
                    .expect("writing to a String cannot fail");
            }
            character => rendered.push(character),
        }
    }
    rendered.push('"');
    rendered
}

fn is_bare_atom_character(character: char) -> bool {
    !character.is_whitespace()
        && !character.is_control()
        && !matches!(character, '(' | ')' | '"' | '\\')
}

#[cfg(test)]
mod tests {
    use super::SExpr;

    #[test]
    fn quoted_and_escaped_atoms_round_trip_through_inline_s_expressions() {
        let expression = SExpr::List(vec![
            SExpr::Atom("Finding".to_owned()),
            SExpr::Atom("two words (quoted)".to_owned()),
            SExpr::Atom("quote \" slash \\ line\nbreak".to_owned()),
        ]);
        let rendered = expression.compact();

        assert_eq!(
            (rendered.as_str(), SExpr::parse(&rendered)),
            (
                r#"(Finding "two words (quoted)" "quote \" slash \\ line\nbreak")"#,
                Ok(expression),
            )
        );
    }
}
