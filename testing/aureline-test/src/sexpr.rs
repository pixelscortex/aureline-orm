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
            Self::Atom(atom) => atom.clone(),
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
            Self::Atom(atom) => lines.push(format!("{indent}{atom}")),
            Self::List(items) if items.is_empty() => lines.push(format!("{indent}()")),
            Self::List(items) => {
                let (head, tail) = items.split_first().expect("non-empty list");
                match head {
                    Self::Atom(atom) => lines.push(format!("{indent}({atom}")),
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
    let atom = none_of("()")
        .filter(|character: &char| !character.is_whitespace())
        .repeated()
        .at_least(1)
        .to_slice()
        .map(|atom: &str| SExpr::Atom(atom.to_owned()));

    recursive(|expression| {
        let list = expression
            .padded()
            .repeated()
            .collect::<Vec<_>>()
            .map(SExpr::List)
            .delimited_by(just('('), just(')'));

        list.or(atom)
    })
    .padded()
    .then_ignore(end())
}
