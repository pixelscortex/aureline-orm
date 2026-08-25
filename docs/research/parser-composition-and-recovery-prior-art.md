# Parser composition and recovery prior art

**Researched:** 2026-08-25  
**Question:** How do established language and schema parsers compose declarations, parse names, recover malformed input, and scale when attributes are added?

This note compares the official parsers or grammars for Gleam, Go, TypeScript, Rust, CPython, and Prisma Schema Language. The comparison is about parser architecture, not language popularity or surface-syntax preference. Prisma is especially relevant because its field spelling is almost the proposed Aureline spelling: `name type @attribute*`.

## Conclusion

A parser does not normally decide what each source character “is.” The lexer or scanner turns characters into tokens. The parser then decides what those tokens mean *together in a grammar position* and builds syntax nodes.

For Aureline:

```text
source:  first string @unique
tokens:  Ident(first) Ident(string) At Ident(unique) Newline
grammar: field-name   type          attribute       field-end
```

The valid field grammar is small and scales directly:

```text
field = declared-identifier type-expression field-attribute* field-end
```

Adding `@attr` does not make name parsing harder. `@` is an unmistakable boundary: it cannot be confused with another identifier or a type member.

The present complexity around `first name string` comes from a diagnostic policy, not from the valid grammar. That input produces three identifier tokens and has at least two plausible interpretations:

1. field `first`, custom type `name`, accidental extra token `string`;
2. malformed multiword field name `first name`, followed by type `string`.

No parser can recover the author's intention from those tokens alone. Aureline currently chooses interpretation 2 to satisfy the bespoke “whitespace inside an identifier” requirement in issue #63. The surveyed parsers do not make that inference: they parse one name, move to the next production, and report the extra or unexpected token at the declaration boundary.

The recommended architecture is therefore:

- keep one small parser for a valid declared identifier;
- let each declaration parser visibly compose its keyword, header, body, attributes, terminator, and construction step;
- parse field attributes as a repeated suffix after the type;
- keep recovery local to the grammar position where the parser has enough context;
- either replace the `first name string` heuristic with an extra-token field diagnostic, or isolate it as a field/table-specific invalid production rather than making it the shared name parser for future functions and events.

## The normal pipeline

The surveyed implementations all preserve a boundary resembling:

```text
source text
  -> lexer / scanner
  -> tokens
  -> declaration parser composed from smaller grammar parsers
  -> syntax tree
  -> later validation / semantics
```

Gleam's public entrypoint creates a tokenizer and passes it to `Parser`; the module parser repeatedly invokes `parse_definition`. TypeScript's parser controls a separate scanner and turns its tokens into syntax nodes. Rust's item parser receives tokens, gathers outer attributes, dispatches to an item-kind parser, and constructs the item. These are different implementations of the same separation.

This matters for punctuation. A lexer can always emit `At`, `LessThan`, `Question`, or `Pipe`. Whether a token is valid in a declared name, a type expression, an attribute, or another construct is parser context.

## How the parsers compose large constructs

### Gleam

Gleam's module parser is a series of `parse_definition` calls. A definition parser first collects attributes, examines the declaration-leading tokens, and delegates to focused parsers for imports, constants, functions, custom types, and aliases. Its naming convention explicitly distinguishes `parse_x`, `expect_x`, and `maybe_x` by whether absence is allowed or is an error ([parser terminology and entrypoint](https://github.com/gleam-lang/gleam/blob/f37ad93db459b5fd3ed8a5b720a22cefe4e253f9/compiler-core/src/parse.rs#L4-L31), [module and definition composition](https://github.com/gleam-lang/gleam/blob/f37ad93db459b5fd3ed8a5b720a22cefe4e253f9/compiler-core/src/parse.rs#L258-L410)).

The function parser is still the readable assembly point: it expects the function name, parses parameters, parses the optional return annotation, parses the body, and creates the function node. Those pieces are not hidden behind one-use “recognized function” wrappers ([function composition](https://github.com/gleam-lang/gleam/blob/f37ad93db459b5fd3ed8a5b720a22cefe4e253f9/compiler-core/src/parse.rs#L2232-L2319)).

Gleam parses one declaration name with `expect_name`, which can produce position-specific errors for the wrong token category. It does not supply the entire rest of the declaration to a generic name parser ([name parser](https://github.com/gleam-lang/gleam/blob/f37ad93db459b5fd3ed8a5b720a22cefe4e253f9/compiler-core/src/parse.rs#L4261-L4291)).

### Go

Go's recursive-descent parser follows its published grammar. The file parser switches on declaration-leading keywords and delegates to the corresponding declaration parser. The struct parser owns `struct { ... }` and repeatedly calls the field parser ([top-level dispatch](https://github.com/golang/go/blob/49178db21a45a4cd6dbed533da2c05475a38574f/src/cmd/compile/internal/syntax/parser.go#L405-L485), [struct and field parser](https://github.com/golang/go/blob/49178db21a45a4cd6dbed533da2c05475a38574f/src/cmd/compile/internal/syntax/parser.go#L1618-L1686)).

The official grammar says a field is `(IdentifierList Type | EmbeddedField) [Tag]`. The parser accordingly assembles names, type, and optional tag in one field parser ([Go specification: struct fields](https://go.dev/ref/spec#Struct_types)). Go's tag is attribute-like suffix metadata, and it does not complicate parsing the preceding name or type because its string-literal token is a distinct boundary.

### TypeScript

The current native TypeScript parser follows the same shape. `parseDeclaration` collects decorators/modifiers and delegates to `parseDeclarationWorker`, whose switch selects variable, function, class, interface, type, enum, module, import, or export parsing. If modifiers are not followed by a declaration, recovery constructs a missing declaration node ([declaration composition and recovery](https://github.com/microsoft/TypeScript/blob/8ac035a394c79e693a3a7d74cb170448503ee894/tsc/internal/parser/parser.go#L1124-L1194)).

A class-element parser collects modifiers, distinguishes constructors, index signatures, accessors, methods, and properties, and then delegates. The property path composes a name, optional marker, type annotation, initializer, and terminator ([class element composition](https://github.com/microsoft/TypeScript/blob/8ac035a394c79e693a3a7d74cb170448503ee894/tsc/internal/parser/parser.go#L1894-L2029)). Decorators are accumulated in the modifier loop rather than built into identifier recognition ([decorator/modifier loop](https://github.com/microsoft/TypeScript/blob/8ac035a394c79e693a3a7d74cb170448503ee894/tsc/internal/parser/parser.go#L3904-L3968)).

### Rust

Rust composes a crate from a module, a module from repeated items, and an item from outer attributes, visibility, item-kind dispatch, and final node construction. `parse_item` is not merely a last `choice`; it reveals those phases directly ([crate, module, and item composition](https://github.com/rust-lang/rust/blob/e7769602aca3770e8d8ea55716becb22e839a579/compiler/rustc_parse/src/parser/item.rs#L29-L190)).

For a record struct, `parse_item_struct` owns the name, generics, where clause, and body choice. `parse_record_struct_body` owns braces and repeated fields. `parse_field_def` owns field attributes and visibility before delegating to `parse_name_and_ty`, which visibly parses identifier, colon, and type ([struct composition](https://github.com/rust-lang/rust/blob/e7769602aca3770e8d8ea55716becb22e839a579/compiler/rustc_parse/src/parser/item.rs#L1897-L2029), [field composition](https://github.com/rust-lang/rust/blob/e7769602aca3770e8d8ea55716becb22e839a579/compiler/rustc_parse/src/parser/item.rs#L2147-L2337)). The Rust grammar makes the boundary explicit: `OuterAttribute* Visibility? IDENTIFIER : Type` ([Rust Reference: structs](https://doc.rust-lang.org/reference/items/structs.html)).

### CPython

CPython's PEG grammar makes the composition visible in grammar rules: a file contains statements; a decorated class or function consists of repeated decorators followed by the raw declaration; the raw function rule owns keyword, exact `NAME`, parameters, optional return type, colon, and block ([statement grammar](https://github.com/python/cpython/blob/ee521e8ac19ad012ebc4e1b3e71b369988a9b9f8/Grammar/python.gram#L86-L145), [decorator, name, and body composition](https://github.com/python/cpython/blob/ee521e8ac19ad012ebc4e1b3e71b369988a9b9f8/Grammar/python.gram#L260-L309)).

This parser has a useful separation for diagnostics: the first pass excludes `invalid_*` productions; after failure, a second pass enables them to produce a better error, but successful syntax still follows the small valid grammar ([invalid-rule contract](https://github.com/python/cpython/blob/ee521e8ac19ad012ebc4e1b3e71b369988a9b9f8/Grammar/python.gram#L30-L43), [two-pass implementation](https://github.com/python/cpython/blob/ee521e8ac19ad012ebc4e1b3e71b369988a9b9f8/Parser/pegen.c#L996-L1029)).

### Prisma Schema Language

Prisma is the closest surface-syntax comparison. Its documentation defines a field as a name, type, optional type modifiers, and optional attributes, with examples such as `id Int @id @default(autoincrement())` ([Prisma fields and attributes](https://docs.prisma.io/docs/orm/prisma-schema/data-model/models#defining-fields)).

Its official PEG grammar is direct:

```text
field_declaration =
    identifier
    ~ LEGACY_COLON?
    ~ field_type?
    ~ field_attribute*
    ~ trailing_comment?
    ~ NEWLINE
```

The same grammar defines a field attribute as `"@" ~ path ~ arguments_list?` and uses a block-level catch-all for malformed lines ([Prisma field grammar](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/schema-ast/src/parser/datamodel.pest#L29-L57), [attribute and catch-all grammar](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/schema-ast/src/parser/datamodel.pest#L107-L151)).

The AST conversion walks the parsed field's children once, collecting one name, one type, attributes, and comments. If name or type is absent, it returns a general invalid-field diagnostic. It has no alternative parser for integer names, marked-type-shaped names, punctuated names, and split names ([Prisma field conversion](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/schema-ast/src/parser/parse_field.rs#L9-L60)). The model parser separately owns the repeated field/block-attribute body and gives a whole-line diagnostic for anything matching the catch-all ([Prisma model conversion](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/schema-ast/src/parser/parse_model.rs#L11-L65)).

For an analogue of `first name string`, Prisma greedily parses `first` as the field name and `name` as the field type; the remaining `string` prevents the line from matching a field declaration, so the line catch-all reports that it is not a valid field or attribute definition. It does not guess that `first name` was intended as one name.

## What attributes change in Aureline

For valid syntax, only the field result and one sequence step change:

```text
before: field = name type field-end
after:  field = name type attribute* field-end
```

A staged field can grow naturally:

```text
ParsedField {
    name,
    source_type,
    attributes,
    span,
}
```

The attribute parser should own `@`, its path/name, and its optional payload. Static semantics should own whether an attribute exists, accepts its arguments, or is legal on a field, table, function, or event. This matches the existing Arc 2 research and the Gleam/TypeScript/Rust pattern of parsing metadata before validating attachment.

Representative flows are straightforward:

```text
email string @unique @index(name: "email_idx")
  -> declared identifier: email
  -> type expression: string
  -> attributes: [unique, index(...)]
  -> newline
  -> one staged field

owner record<User | Bot> @index
  -> declared identifier: owner
  -> complete recursive type expression: record<User | Bot>
  -> attribute: index
  -> newline
```

Useful suffix-specific negative tests are:

- bare trailing `@`;
- an attribute with an incomplete payload;
- an attribute before the field type, if prefix placement is unsupported;
- an identifier after a completed type but before `@` or newline;
- an attribute after a line break, where it may instead be a table-level attribute;
- a complex type immediately followed by `@`, proving that the type parser stops cleanly.

## How to handle `first name string`

There are three honest design choices.

### 1. Prefer a field-boundary diagnostic

Parse exactly one name and one type. When another identifier appears where only `@attribute` or the physical field end is legal, report it directly:

```text
unexpected `string` after field type `name`
field syntax is `<name> <type> [@attribute ...]`
```

This is the simplest and most scalable choice. It matches Prisma and the programming-language parsers: do not claim to know which earlier token the author intended differently.

This choice requires amending the #63 interpretation that `first name string` must be diagnosed as whitespace *inside* an identifier. Whitespace terminates an identifier lexeme; treating it as internal is recovery policy.

### 2. Keep a targeted invalid field production

If the precise whitespace diagnostic is a product requirement, keep the small valid field grammar first and try the multiword-name recovery only after it fails. Scope that recovery to field/table headers whose following grammar makes the guess plausible. Do not make future function and event parsers use it automatically.

This resembles CPython's separation of valid grammar from `invalid_*` diagnostic productions. It keeps the uncommon heuristic visibly secondary.

### 3. Add an explicit delimiter

`first: string` removes the ambiguity completely, as Rust and TypeScript demonstrate. It is the strongest grammar simplification but changes Aureline's chosen surface and is not necessary for attributes—Prisma proves `name type @attr*` can remain simple when diagnostics do not guess multiword intent.

## Recommended Aureline composition

The source-level structure should remain visible at each owning parser:

```text
document_parser
  = repeated(table_parser | future_function_parser | future_event_parser)

table_parser
  = "table" table_header_parser table_body_parser
    -> choose problem or atomically commit table

table_header_parser
  = declared_identifier schema_mode

table_body_parser
  = "{" repeated(field_parser | table_attribute_parser) "}"

field_parser
  = declared_identifier type_expression_parser field_attribute_parser* field_end
```

This suggests the following code organization rules:

1. A module's main `parser()` is the visible composition root for the complete source construct. It should show how meaningful subregions such as the header and body are sequenced and where recovery/commit occurs.
2. Extract mini parsers for reusable grammar concepts or independently meaningful source regions. Do not extract one-use wrappers merely to make `parser()` short.
3. Share `declared_identifier`, not “declared name plus an arbitrary parser for the rest of the declaration.” A function name followed by `(`, an event name followed by its event grammar, and a field name followed by a type have different recovery evidence.
4. Keep valid grammar and diagnostic-only invalid shapes distinguishable in names and comments.
5. Keep AST staging/atomic allocation separate from token recognition. Aureline may retain its no-partial-table guarantee without making name recognition own allocation policy.

The current staged table commit remains defensible because issue #62 requires both arena ownership edges to be established together and the public tree to remain immutable. That constraint explains `ParsedField` and delayed allocation. It does not require the shared name parser to reparse every declaration-specific tail through every malformed-name alternative.

## Recommended next decision

Before expanding the parser, decide one narrow point explicitly:

> Must `first name string` specifically mean “identifier contains whitespace,” or may it mean “a field has unexpected material after its parsed type”?

If the latter is acceptable, simplify the valid field/name path now and make the field boundary diagnostic precise. If the former is binding, document it as an intentional heuristic and isolate it from the valid parser and from future declaration families.

Either decision lets `@attr` scale cleanly. The attribute is not the source of the current complexity.
