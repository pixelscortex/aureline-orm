mod table;

use aureline_ast::{
    AstBuilder,
    ast::{Ast, Comment, SchemaType, SourceType, TypeArgument},
    source::{SourceId, SourceSpan},
    tokens::Token,
};
use chumsky::{
    error::Cheap,
    extra,
    input::{Input as _, MappedInput},
    prelude::*,
};

use crate::grammar::table::table_parser;
use crate::lexer::TokenOccurrence;

pub(crate) type TokenInput<'tokens, 'src> =
    MappedInput<'tokens, Token<'src>, SimpleSpan, &'tokens [TokenOccurrence<'src>]>;

/// A malformed form consumed during the construction walk so parsing can
/// select the earliest precise problem and then discard any partially built AST.
#[derive(Clone, Copy)]
pub(crate) enum GrammarProblem {
    IdentifierWhitespace(SimpleSpan),
    IdentifierStartsWithDigit(SimpleSpan),
    IdentifierPunctuation(char, SimpleSpan),
    EmptyTypeArguments(SimpleSpan),
    TrailingTypeArgumentComma(SimpleSpan),
    PostfixOptionalType(SimpleSpan),
    MissingUnionMember(SimpleSpan),
    Unexpected(SimpleSpan),
}

impl GrammarProblem {
    pub(crate) const fn span(self) -> SimpleSpan {
        match self {
            Self::IdentifierWhitespace(span)
            | Self::IdentifierStartsWithDigit(span)
            | Self::IdentifierPunctuation(_, span)
            | Self::EmptyTypeArguments(span)
            | Self::TrailingTypeArgumentComma(span)
            | Self::PostfixOptionalType(span)
            | Self::MissingUnionMember(span)
            | Self::Unexpected(span) => span,
        }
    }
}

pub(crate) enum ParseTokensError {
    Parser(Vec<Cheap<SimpleSpan>>),
    Problem(GrammarProblem),
}

#[derive(Clone, Copy)]
struct ApplicationMark {
    opening: SimpleSpan,
    joined_to_name: bool,
}

impl ApplicationMark {
    const fn into_declared_name_problem(self) -> GrammarProblem {
        if self.joined_to_name {
            GrammarProblem::IdentifierPunctuation('<', self.opening)
        } else {
            GrammarProblem::Unexpected(self.opening)
        }
    }
}

/// Lets recursive parsing consume a malformed type without constructing an
/// invalid public [`SourceType`], while retaining an application opener for
/// contextual declared-name recovery.
pub(crate) struct ParsedSourceType {
    outcome: Result<SourceType, GrammarProblem>,
    application: Option<ApplicationMark>,
}

impl ParsedSourceType {
    fn valid(source_type: SourceType) -> Self {
        Self {
            outcome: Ok(source_type),
            application: None,
        }
    }

    fn recovered(problem: GrammarProblem) -> Self {
        Self {
            outcome: Err(problem),
            application: None,
        }
    }

    fn application(
        outcome: Result<SourceType, GrammarProblem>,
        name: SimpleSpan,
        opening: SimpleSpan,
    ) -> Self {
        Self {
            outcome,
            application: Some(ApplicationMark {
                opening,
                joined_to_name: name.end == opening.start,
            }),
        }
    }

    fn with_postfix(mut self, question: Option<Spanned<Token<'_>>>) -> Self {
        if let (Ok(_), Some(question)) = (&self.outcome, question) {
            self.outcome = Err(GrammarProblem::PostfixOptionalType(question.span));
        }
        self
    }

    fn declared_name_problem(&self) -> Option<GrammarProblem> {
        self.application
            .map(ApplicationMark::into_declared_name_problem)
    }

    pub(crate) fn into_result(self) -> Result<SourceType, GrammarProblem> {
        self.outcome
    }
}

/// Recursive types already own Aureline spans or a recovered problem; integer
/// spelling keeps its parser-relative span until application construction can
/// attach the source identity without leaking parser-library locations into the AST.
enum ParsedTypeArgument {
    Type(ParsedSourceType),
    Integer(Spanned<String>),
}

impl ParsedTypeArgument {
    fn into_result(self, state: &ParserState) -> Result<TypeArgument, GrammarProblem> {
        match self {
            Self::Type(source_type) => source_type.into_result().map(TypeArgument::Type),
            Self::Integer(integer) => Ok(TypeArgument::integer(
                integer.inner,
                state.source_span(integer.span),
            )),
        }
    }
}

pub(crate) struct ParserState {
    ast: AstBuilder,
    source: SourceId,
    inline_whitespace: Vec<SimpleSpan>,
}

impl ParserState {
    fn new(source: SourceId, comments: Vec<Comment>, inline_whitespace: Vec<SimpleSpan>) -> Self {
        Self {
            ast: AstBuilder::new(comments),
            source,
            inline_whitespace,
        }
    }

    pub(crate) fn source_span(&self, span: SimpleSpan) -> SourceSpan {
        super::source_span(self.source, span)
    }

    pub(crate) fn inline_whitespace_between(
        &self,
        left: SimpleSpan,
        right: SimpleSpan,
    ) -> Option<SimpleSpan> {
        let gap = SimpleSpan::from(left.end..right.start);
        self.inline_whitespace.contains(&gap).then_some(gap)
    }
}

pub(crate) type ParserExtra = extra::Full<Cheap<SimpleSpan>, extra::SimpleState<ParserState>, ()>;

pub(crate) fn source_file_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Option<GrammarProblem>, ParserExtra> {
    let newlines = just(Token::Newline).repeated();

    let items = table_parser()
        .then_ignore(newlines.clone())
        .repeated()
        .collect::<Vec<_>>();

    // `ignored()` would put child parsers in Chumsky's check-only mode and skip
    // the `map_with` calls that allocate the AST. Inspect emitted problems only
    // after the single construction walk has run.
    newlines
        .ignore_then(items)
        .then_ignore(end())
        .map(|problems| {
            problems
                .into_iter()
                .flatten()
                .min_by_key(|problem| problem.span().start)
        })
}

/// Parses a token stream into an arena-backed AST.
///
/// # Errors
///
/// Returns [`ParseTokensError::Parser`] when the token stream cannot be consumed,
/// or [`ParseTokensError::Problem`] when a consumed recovery form carries the
/// earliest precise grammar problem.
pub(crate) fn parse_tokens<'tokens, 'src: 'tokens>(
    tokens: &'tokens [TokenOccurrence<'src>],
    comments: Vec<Comment>,
    inline_whitespace: Vec<SimpleSpan>,
    source: SourceId,
    source_len: usize,
) -> Result<Ast, ParseTokensError> {
    let mut state = extra::SimpleState(ParserState::new(source, comments, inline_whitespace));
    let input = tokens.split_spanned(SimpleSpan::from(source_len..source_len));

    let problem = source_file_parser()
        .parse_with_state(input, &mut state)
        .into_result()
        .map_err(ParseTokensError::Parser)?;

    if let Some(problem) = problem {
        return Err(ParseTokensError::Problem(problem));
    }

    Ok(state.0.ast.finish())
}

pub(crate) fn ident<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<String>, ParserExtra> {
    select_ref! {
        Token::Ident(name) => *name,
    }
    .map(str::to_owned)
    .spanned()
}

pub(crate) fn schema_type_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<SchemaType>, ParserExtra> {
    choice((
        just(Token::Schemafull).to(SchemaType::Full),
        just(Token::Schemaless).to(SchemaType::Less),
    ))
    .spanned()
}

pub(crate) fn integer<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<String>, ParserExtra> {
    select_ref! {
        Token::Integer(raw) => *raw,
    }
    .map(str::to_owned)
    .spanned()
}

pub(crate) fn identifier_punctuation<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<char>, ParserExtra> {
    choice((
        just(Token::LAngle).to('<'),
        just(Token::RAngle).to('>'),
        just(Token::Comma).to(','),
        just(Token::Question).to('?'),
        just(Token::Pipe).to('|'),
    ))
    .spanned()
}

/// Consumes a token spelling that can continue a lexical identifier candidate.
///
/// Integer and structural-word tokens are not valid declared names themselves,
/// but their original spellings can follow punctuation inside the malformed
/// candidate reconstructed by [`punctuated_identifier`].
pub(crate) fn identifier_fragment<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, Spanned<()>, ParserExtra> {
    choice((
        ident().ignored(),
        integer().ignored(),
        just(Token::Table).ignored(),
        just(Token::Schemafull).ignored(),
        just(Token::Schemaless).ignored(),
    ))
    .spanned()
}

pub(crate) struct PunctuatedIdentifier {
    punctuation: char,
    span: SimpleSpan,
    adjacent: bool,
}

impl PunctuatedIdentifier {
    pub(crate) const fn into_problem(self) -> GrammarProblem {
        if self.adjacent {
            GrammarProblem::IdentifierPunctuation(self.punctuation, self.span)
        } else {
            GrammarProblem::Unexpected(self.span)
        }
    }
}

/// Reconstructs a declared-name candidate split by structural type punctuation.
///
/// The complete shape is consumed so compound violations retain the first
/// punctuation. Only the first punctuation group determines the problem;
/// later whitespace cannot mask an earlier violation. Adjacency remains explicit
/// because whitespace is absent from the grammatical token stream and changes an
/// identifier problem into syntax.
pub(crate) fn punctuated_identifier<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, PunctuatedIdentifier, ParserExtra> {
    let punctuation_run = identifier_punctuation()
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .boxed();
    let suffix = punctuation_run.clone().then(identifier_fragment());

    ident()
        .then(suffix.repeated().at_least(1).collect::<Vec<_>>())
        .then(punctuation_run.or_not())
        .map(|((first_name, suffixes), _)| {
            let (first_run, next_name) = suffixes
                .first()
                .expect("a punctuated identifier has a punctuation suffix");
            let first_punctuation = first_run.first().expect("a punctuation run is non-empty");
            let mut previous_end = first_name.span.end;
            let mut adjacent = true;
            for punctuation in first_run {
                adjacent &= previous_end == punctuation.span.start;
                previous_end = punctuation.span.end;
            }
            adjacent &= previous_end == next_name.span.start;
            PunctuatedIdentifier {
                punctuation: first_punctuation.inner,
                span: first_punctuation.span,
                adjacent,
            }
        })
}

/// Consumes a complete recursive application shape, including a recovered
/// malformed one, and returns its declared-name problem rather than a source type.
/// The outer `<` is identifier punctuation only when joined to the applied name;
/// otherwise it is unexpected syntax. Bare source type names do not match.
pub(crate) fn applied_source_type_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, GrammarProblem, ParserExtra> {
    source_type_parser().try_map(|source_type, span| {
        source_type
            .declared_name_problem()
            .ok_or_else(|| Cheap::new(span))
    })
}

fn finish_application(
    name: Spanned<String>,
    opening: SimpleSpan,
    first_argument: ParsedTypeArgument,
    remaining_arguments: Vec<ParsedTypeArgument>,
    application_span: SimpleSpan,
    state: &ParserState,
) -> ParsedSourceType {
    let mut arguments = std::iter::once(first_argument)
        .chain(remaining_arguments)
        .map(|argument| argument.into_result(state));
    let first_argument = match arguments.next().expect("application is non-empty") {
        Ok(argument) => argument,
        Err(problem) => {
            return ParsedSourceType::application(Err(problem), name.span, opening);
        }
    };
    let remaining_arguments = match arguments.collect::<Result<Vec<_>, _>>() {
        Ok(arguments) => arguments,
        Err(problem) => {
            return ParsedSourceType::application(Err(problem), name.span, opening);
        }
    };
    ParsedSourceType::application(
        Ok(SourceType::application(
            name.inner,
            state.source_span(name.span),
            first_argument,
            remaining_arguments,
            state.source_span(application_span),
        )),
        name.span,
        opening,
    )
}

fn finish_trailing_application(
    name: &Spanned<String>,
    opening: SimpleSpan,
    first_argument: ParsedTypeArgument,
    remaining_arguments: Vec<ParsedTypeArgument>,
    comma: SimpleSpan,
    state: &ParserState,
) -> ParsedSourceType {
    let nested_problem = std::iter::once(first_argument)
        .chain(remaining_arguments)
        .find_map(|argument| argument.into_result(state).err());
    // Preserve source-order reporting: an earlier malformed nested argument is
    // more relevant than this application's later comma.
    ParsedSourceType::application(
        Err(nested_problem.unwrap_or(GrammarProblem::TrailingTypeArgumentComma(comma))),
        name.span,
        opening,
    )
}

fn finish_empty_application(
    name: &Spanned<String>,
    opening: SimpleSpan,
    closing: SimpleSpan,
) -> ParsedSourceType {
    ParsedSourceType::application(
        Err(GrammarProblem::EmptyTypeArguments(SimpleSpan::from(
            opening.start..closing.end,
        ))),
        name.span,
        opening,
    )
}

/// Converts members in source order so the earliest recovered problem wins.
/// Successful members remain structurally unchanged—including nested unions—
/// before construction of the outer union.
fn finish_union(
    first_member: ParsedSourceType,
    remaining_members: Vec<ParsedSourceType>,
    union_span: SimpleSpan,
    state: &ParserState,
) -> ParsedSourceType {
    let mut members = std::iter::once(first_member)
        .chain(remaining_members)
        .map(ParsedSourceType::into_result);
    let first_member = match members.next().expect("a union has at least two members") {
        Ok(member) => member,
        Err(problem) => {
            return ParsedSourceType {
                outcome: Err(problem),
                application: None,
            };
        }
    };
    let second_member = match members.next().expect("a union has at least two members") {
        Ok(member) => member,
        Err(problem) => {
            return ParsedSourceType {
                outcome: Err(problem),
                application: None,
            };
        }
    };
    let remaining_members = match members.collect::<Result<Vec<_>, _>>() {
        Ok(members) => members,
        Err(problem) => {
            return ParsedSourceType {
                outcome: Err(problem),
                application: None,
            };
        }
    };
    ParsedSourceType::valid(SourceType::union(
        first_member,
        second_member,
        remaining_members,
        state.source_span(union_span),
    ))
}

/// Preserves an earlier recovered member problem over the later missing-member
/// pipe; the pipe is reported only when every consumed member is valid.
fn finish_missing_union_member(
    first_member: ParsedSourceType,
    remaining_members: Vec<ParsedSourceType>,
    missing: SimpleSpan,
) -> ParsedSourceType {
    let earlier_problem = std::iter::once(first_member)
        .chain(remaining_members)
        .find_map(|member| member.into_result().err());
    ParsedSourceType::recovered(
        earlier_problem.unwrap_or(GrammarProblem::MissingUnionMember(missing)),
    )
}

fn missing_union_member(span: SimpleSpan) -> ParsedSourceType {
    ParsedSourceType::recovered(GrammarProblem::MissingUnionMember(span))
}

pub(crate) fn source_type_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, TokenInput<'tokens, 'src>, ParsedSourceType, ParserExtra> {
    recursive(|source_type| {
        let integer_argument = integer().map(ParsedTypeArgument::Integer);
        let argument = choice((
            source_type.clone().map(ParsedTypeArgument::Type),
            integer_argument,
        ))
        .boxed();
        let arguments = argument.clone().then(
            just(Token::Comma)
                .ignore_then(argument)
                .repeated()
                .collect::<Vec<_>>(),
        );
        let application = ident()
            .then(just(Token::LAngle).spanned())
            .then(arguments.clone())
            .then_ignore(just(Token::RAngle))
            .map_with(|((name, opening), (first, rest)), context| {
                finish_application(
                    name,
                    opening.span,
                    first,
                    rest,
                    context.span(),
                    &context.state().0,
                )
            });

        let trailing_comma_application = ident()
            .then(just(Token::LAngle).spanned())
            .then(arguments.then(just(Token::Comma).spanned()))
            .then_ignore(just(Token::RAngle))
            .map_with(|((name, opening), ((first, rest), comma)), context| {
                finish_trailing_application(
                    &name,
                    opening.span,
                    first,
                    rest,
                    comma.span,
                    &context.state().0,
                )
            });

        let empty_application = ident()
            .then(just(Token::LAngle).spanned())
            .then(just(Token::RAngle).spanned())
            .map(|((name, opening), closing)| {
                finish_empty_application(&name, opening.span, closing.span)
            });

        let name = ident().map_with(|name, context| {
            ParsedSourceType::valid(SourceType::name(
                name.inner,
                context.state().0.source_span(name.span),
            ))
        });

        // All forms begin with a name. Try recovered applications and the
        // non-empty application shape before the bare form so their angle tokens are
        // consumed rather than left behind as an unrelated parser failure.
        let primary = choice((
            empty_application,
            trailing_comma_application,
            application,
            name,
        ));

        let member = primary
            .then(just(Token::Question).spanned().or_not())
            // `with_postfix` preserves an earlier malformed primary and its
            // application mark instead of letting a later `?` mask either fact.
            .map(|(source_type, question)| source_type.with_postfix(question))
            .boxed();

        let leading_missing_member = just(Token::Pipe)
            .spanned()
            .then_ignore(member.clone())
            .map(|pipe| missing_union_member(pipe.span));
        let lone_missing_member = just(Token::Pipe)
            .spanned()
            .map(|pipe: Spanned<Token<'src>>| missing_union_member(pipe.span));
        // Claim `| member` before the lone-pipe fallback; reversing these would
        // consume only `|` and strand the following member outside recovery.
        let union_member =
            choice((leading_missing_member, lone_missing_member, member.clone())).boxed();
        let separated_members = union_member
            .clone()
            .then(
                just(Token::Pipe)
                    .ignore_then(union_member.clone())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .boxed();
        let trailing_union = separated_members
            .clone()
            .then(just(Token::Pipe).spanned())
            .map(|((first, rest), pipe)| finish_missing_union_member(first, rest, pipe.span));
        let union = union_member
            .clone()
            .then(
                just(Token::Pipe)
                    .ignore_then(union_member.clone())
                    .repeated()
                    .at_least(1)
                    .collect::<Vec<_>>(),
            )
            .map_with(|(first, rest), context| {
                finish_union(first, rest, context.span(), &context.state().0)
            });

        // Applications and postfix recovery finish before lower-precedence `|`.
        // Trailing recovery and complete unions precede a single member so pipe
        // syntax is consumed and classified precisely.
        choice((trailing_union, union, union_member)).boxed()
    })
}
