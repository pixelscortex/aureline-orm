# Aureline DSL — Attribute & Field Grammar (archive `aureline-orm-arc2`)

Repo root: `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2`

Primary sources consulted:

| Area | Path |
| --- | --- |
| Attribute AST | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-ast/src/schema.rs` |
| Attribute parser | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-parser/src/parser/grammar/attr.rs` |
| Argument parser | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-parser/src/parser/grammar/args.rs` |
| Value/literal parser | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-parser/src/parser/grammar/literal.rs` |
| Field parser | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-parser/src/parser/grammar/field.rs` |
| Table / relation / body / relate | `.../grammar/table.rs`, `.../grammar/relation.rs`, `.../grammar/body.rs`, `.../grammar/relate.rs` |
| Lexer | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-parser/src/parser/lexer.rs` |
| SurQL slot routing | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-parser/src/parser/surql/mod.rs` |
| Attribute catalog | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-semantic/src/attributes/catalog.rs` |
| Attribute validation | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-semantic/src/attributes/validate/**` |
| Parser test cases | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-parser/tests/cases/{field,table,permissions,literal,surql,func,event,analyzer}.rs` |
| Semantic test cases | `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-semantic/tests/{attribute_catalog,field_attr_typecheck}.rs` |

---

## 0. Lexical foundations (needed to read everything below)

From `lexer.rs`:

- `@name` is **one token**: `just("@").ignore_then(text::ident()).map(Token::Attr)`. The `@` and the identifier must be adjacent; `@` + space + ident does not lex as an attribute.
- Backtick payloads are `Token::RawString`. The raw lexer treats `"..."` and `'...'` as opaque while scanning for the closing backtick, so SurQL bodies may contain backticks inside quoted strings:
  ```
  `THROW "field `num` must be even"`
  ```
- Newlines are **real tokens** (`Token::Newline`); the field/table grammar uses them as terminators.
- Reserved words (cannot be plain identifiers, therefore cannot be field names or attribute path segments): `table`, `relation`, `schemafull`, `schemaless`, `func`, `event`, `analyzer`, `true`, `false`. Everything else lexes as `Token::Ident` — including `relate` and `permissions`.
- Numbers keep their source slice: `-?int(.digits)?`. Strings accept both `"..."` and `'...'`.
- Arrow tokens: `->`, `<-`, `<->` (longest-match ordering enforced in the `symbol` choice).

Span nuance (`grammar/mod.rs`): `attr_ident()` narrows the `@index` token span to just `index` (excludes the `@`), and `raw_surql()` narrows the backtick token span to the *content* only (strips both backticks). Both matter for diagnostics.

---

## 1. Attribute syntax forms

### 1.1 Grammar (verbatim from `grammar/attr.rs`)

```text
<attr> = "@" <ident> [ "(" <attr-args>? ")" | <raw-string> ]
<attr-args> = <attr-arg> ("," <attr-arg>)* [","]
<attr-arg> = <ident> ":" <value> | <value>
```

The implementation additionally allows a dotted path after the head:

```rust
let attr_path = attr_ident()
    .then(
        just(Token::Dot)
            .ignore_then(ident())
            .repeated()
            .collect::<Vec<_>>(),
    )
```

so the real shape is:

```text
<attr>      = "@" <ident> ("." <ident>)* <payload>?
<payload>   = "(" <attr-args>? ")" | <raw-string>
```

Key facts:

- `payload` is `choice((args, raw_arg)).or_not()` — an attribute has **either** a parenthesised argument list **or** exactly one backtick payload, **or** nothing. The two forms are never combined, and `@attr()` (empty parens) is legal syntax producing zero args.
- A backtick payload is stored as **one** `AttrArg::Embedded`.
- Path tail segments are parsed with `ident()`, i.e. only `Token::Ident`. A reserved word cannot be a path segment (`@perm.table` will not parse).
- Path depth is unbounded at the parser level (`.repeated()`); the catalog restricts it later.
- The parser is context-free about names: **any** `@name` parses. `@source(...)`, `@asset`...`` etc. are syntactically valid and only rejected by the semantic catalog.

### 1.2 AST (verbatim from `aureline-ast/src/schema.rs`)

```rust
pub type AttrDecl<'src> = Spanned<AttrDeclKind<'src>>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrDeclKind<'src> {
    /// Attribute path without the leading `@`, e.g. `index` or `perm.select`.
    pub path: Vec<Ident<'src>>,
    /// Parenthesized arguments or one embedded SurQL/raw argument.
    pub args: Vec<AttrArg<'src>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrArg<'src> {
    /// Positional argument, e.g. `@hnsw(1536)`.
    Positional(Expr<'src>),
    /// Named argument, e.g. `@index(name: "email_idx")`.
    Named { key: Ident<'src>, value: Expr<'src> },
    /// Embedded SurQL payload, e.g. ``@assert`string::is_email($value)` ``.
    Embedded(ast_surql::Expr<'src>),
}
```

Outline rendering (`aureline-test/src/outline/actual/schema.rs`) — needed to read every test expectation below:

- attribute path joins with `::` → `@perm.select` renders `(Attr perm::select)`
- field path joins with `.` → `profile.email` renders `(Field profile.email ...)`
- `AttrArg::Positional` → `(Arg <expr>)`; `Named` → `(Named <key> <expr>)`; `Embedded` → `(Embedded <expr|Raw>)`

### 1.3 Form A — bare marker `@name`

```
source: "@perm"
expect: "(Attr perm)"
```
(`aureline-parser/tests/cases/permissions.rs`)

```
source: r#"email string @unique @index(name: "email_idx")"#
expect: r#"
    (Field email
      (Type string)
      (Attr unique)
      (Attr index
        (Named name (String "email_idx"))))
"#
```
(`aureline-parser/tests/cases/field.rs`, `parses_inline_field_attrs`)

Other bare markers appearing in tests: `@count`, `@fulltext`, `@index` (no parens), `@perm`.

```
source: r#"relation Likes schemafull {
  relate user -> post
  created_at datetime @index
  @count
}"#
```
(`table.rs`, `parses_relation_table_with_relate_clause` — bare `@index` inline on a field, bare `@count` as a table-level body item)

### 1.4 Form B — dotted path `@name.sub.path`

Two-segment:

```
source: "@perm.select`WHERE owner = $auth.id`"
expect: r#"
    (Attr perm::select
      (Embedded
        (Binary Equal
          (Path owner)
          (FieldAccess id
            (Param auth)))))
"#
```
(`permissions.rs`)

Also `@perm.create`, `@perm.update`, `@perm.delete`, `@ftxt.basic_text_search`, `@fulltext.basic_text_search`, `@hnsw.cosf32`.

Three-segment (from `aureline-semantic/tests/attribute_catalog.rs`):

```
source: "table Article schemafull {\n  embedding array<float> @hnsw.cosf32(dimension: 1536)\n  related array<float> @hnsw.cosine.f32(dimension: 1536)\n}\n"
```

### 1.5 Form C — parenthesised arguments `@name(args)`

Named-only:

```
source: r#"email string @index(name: "email_idx")"#
```

Mixed value kinds (`table.rs`, `parses_field_and_table_attrs`):

```
source: r#"table User schemafull {
  email string @index(name: "email_idx")
  @index(fields: [email], pair: (123, 435), active: true, value: 291.0)
  @count
}"#
expect: r#"
    (Document
      (Table User
        (Kind Schemafull)
        (Field email
          (Type string)
          (Attr index
            (Named name (String "email_idx"))))
        (Attr index
          (Named fields (List (Ident email)))
          (Named pair (Tuple (Int 123) (Int 435)))
          (Named active (Bool true))
          (Named value (Float 291.0)))
        (Attr count)))
"#
```

Dotted paths used **as values** (`aureline-parser/tests/cases/literal.rs`, the whole file):

```
source: r#"table User schemafull {
  email string @source(path: profile.email, fallbacks: [profile.name, account.email], pair: (owner.id, user))
}"#
expect: r#"
    (Document
      (Table User
        (Kind Schemafull)
        (Field email
          (Type string)
          (Attr source
            (Named path (Path profile::email))
            (Named fallbacks
              (List
                (Path profile::name)
                (Path account::email)))
            (Named pair
              (Tuple
                (Path owner::id)
                (Ident user)))))))
"#
```

Note: `@source` is not a catalog attribute — this case exercises the *value grammar* only.

Positional arguments are documented in the parser but have no parser test case; `grammar/attr.rs` doc comment shows:

```
/// @index
/// @index(name: "email_idx")
/// @assert`string::is_email($value)`
/// @hnsw(dimension: 1536, dist: cosine)
```
and `AttrArg::Positional` is documented as `@hnsw(1536)`. See §6 for the fact that **no catalog attribute currently accepts a positional argument** — every validator rejects them.

#### Accepted argument value kinds

From `grammar/literal.rs` (verbatim grammar comment):

```text
<literal> = <path>
          | <ident>
          | <string>
          | <number>
          | <bool>
          | "[" <literal-list>? "]"
          | "(" <literal-list>? ")"

<path> = <ident> "." <ident> ("." <ident>)*
<literal-list> = <literal> ("," <literal>)* [","]
```

| Value kind | Syntax | AST | Outline | Notes |
| --- | --- | --- | --- | --- |
| String | `"email_idx"` or `'email_idx'` | `ExprKind::Literal(Literal::String)` | `(String "email_idx")` | both quote styles lex identically |
| Integer | `123`, `-4` | `Literal::Number` w/ `NumericKind::Int` | `(Int 123)` | source slice preserved |
| Float | `291.0` | `Literal::Number` w/ float kind | `(Float 291.0)` | `-?int "." digits` |
| Bool | `true`, `false` | `Literal::Bool` | `(Bool true)` | lexed as `Token::Bool` |
| Identifier | `email`, `user`, `cosine` | `ExprKind::Ident` | `(Ident email)` | bare ident, no dot |
| Dotted path | `profile.email` | `ExprKind::Path(Vec)` | `(Path profile::email)` | **requires ≥1 dot**, else it is an `Ident` |
| List | `[email]`, `[profile.name, account.email]` | `ExprKind::List` | `(List ...)` | trailing comma allowed |
| Tuple | `(123, 435)`, `(owner.id, user)` | `ExprKind::Tuple` | `(Tuple ...)` | trailing comma allowed |

Lists and tuples are recursive (`recursive(|value| ...)`), so arbitrary nesting is grammatically permitted. Attribute-argument lists themselves allow a trailing comma (`.allow_trailing()` in `args::mixed`).

Named vs positional disambiguation (`args::mixed`): `choice((named, positional))` where `named = ident ":" value`. Attributes are the only construct using `mixed`; function parameters use `named_optional` (named-only, with a trailing `?` marker), which is why `func get_user(uuid)` is a parse error.

### 1.6 Form D — backtick escape-hatch payload ``@name`surql` ``

```
source: r#"email string {
  @unique
  @assert`string::is_email($value)`
  @index(name: "email_idx") @fulltext
}"#
```
(`field.rs`, `parses_field_attr_block`) — with `resolve_surql: false`, the payload stays raw:
```
(Attr assert
  (Embedded (Raw "string::is_email($value)")))
```

With SurQL resolution on (`field.rs`, `lowers_assert_attr_when_surql_is_enabled`):

```
source: "email string @assert`string::is_email($value)`"
expect: r#"
    (Field email
      (Type string)
      (Attr assert
        (Embedded
          (Call string::is_email
            (Param value)))))
"#
```

`aureline-parser/tests/cases/surql.rs` (entire file) is the wiring contract for payload lowering:

```
lowers_assert_attr_payload_with_surql_resolver {
    source: "email string @assert`string::is_email($value)`",
}
lowers_value_attr_payload_with_surql_resolver {
    source: "display_name string @value`$value OR 'Anonymous'`",
    expect: r#"
        (Field display_name
          (Type string)
          (Attr value
            (Embedded
              (Binary Or
                (Param value)
                (String "Anonymous")))))
    "#,
}
lowers_default_attr_payload_with_surql_resolver {
    source: "created_at datetime @default`time::now()`",
    expect: r#"
        (Field created_at
          (Type datetime)
          (Attr default
            (Embedded
              (Call time::now))))
    "#,
}
```

**Payload slot routing** (`aureline-parser/src/parser/surql/mod.rs::lower_attr`):

```rust
let slot = match attr.path.first().map(|path| path.as_ref()) {
    Some("perm") => Slot::Permission,
    _ => Slot::Expr,
};
```

- `@perm…` payloads are parsed with SurrealDB's **`Permission`** grammar: `FULL`, `NONE`, or `WHERE <expr>`. `FULL`/`NONE` are re-parsed as expressions; `WHERE <expr>` lowers to just the predicate expression.
- Every other attribute payload is parsed as a SurQL **expression**.
- `run` blocks (func/event) use `Slot::Query`; event `when` uses `Slot::Expr`. Those are not attributes but share the mechanism.

Permission payload forms, verbatim from `permissions.rs`:

```
@perm                                    -> (Attr perm)
@perm`FULL`                              -> (Attr perm (Embedded (Path FULL)))
@perm`NONE`                              -> (Attr perm (Embedded (None)))
@perm`WHERE owner = $auth.id`
@perm.select`WHERE owner = $auth.id`
@perm.create`WHERE $auth.role = 'admin'`
@perm.update`WHERE org = $auth.org AND status != 'paid'`
@perm.delete`NONE`
@perm.select`WHERE owner = $auth.id AND $value != NONE`
@perm.select`WHERE (SELECT VALUE id FROM membership WHERE user = $auth.id LIMIT 1) != NONE`
```

The last one lowers to a full subquery outline:

```
(Attr perm::select
  (Embedded
    (Binary NotEqual
      (Subquery
        (Select
          (Value (Path id))
          (From (Table membership))
          (Where
            (Binary Equal
              (Path user)
              (FieldAccess id (Param auth))))
          (Limit (Int 1))))
      (None))))
```

---

## 2. The full attribute catalog

Source of truth: `aureline-semantic/src/attributes/catalog.rs::resolve_attr`, matching on the path segment slice.

```rust
match segments.as_slice() {
    ["assert"] => Ok(AttributeKind::Assert),
    ["value"] => Ok(AttributeKind::Value),
    ["default"] => Ok(AttributeKind::Default),
    ["index"] => Ok(AttributeKind::Index),
    ["unique"] => Ok(AttributeKind::Unique),
    ["count"] => Ok(AttributeKind::Count),
    ["ftxt"] | ["fulltext"] => Err(AttrPathError::MissingAnalyzer),
    ["ftxt", analyzer] => Ok(AttributeKind::FullText { spelling: Ftxt, analyzer }),
    ["fulltext", analyzer] => Ok(AttributeKind::FullText { spelling: FullText, analyzer }),
    ["hnsw", compact] => parse_compact_hnsw(compact) ... .ok_or(AttrPathError::Unknown),
    ["hnsw", metric, element] => match (parse_long_metric(metric), parse_element(element)) { ... },
    ["perm"] => Ok(AttributeKind::Permission(PermissionKind::Bare)),
    ["perm", "select"] => ...Select,
    ["perm", "create"] => ...Create,
    ["perm", "update"] => ...Update,
    ["perm", "delete"] => ...Delete,
    _ => Err(AttrPathError::Unknown),
}
```

### 2.1 Complete recognised-path table

| Written form | Kind | Payload shape | Notes |
| --- | --- | --- | --- |
| `@assert` | `Assert` | one backtick expr | expr must return `bool` |
| `@value` | `Value` | one backtick expr | expr must match field type |
| `@default` | `Default` | one backtick expr | expr must match field type |
| `@index` | `Index` | `(name: "…")` on fields; `(fields: […], name: "…")` on table/relation | no payload also legal on a field |
| `@unique` | `Unique` | no args on fields; `(fields: […], name: "…")` on table/relation | |
| `@count` | `Count` | none | table/relation only |
| `@fulltext` | — | — | **error**: `MissingAnalyzer` (bare form is reserved but invalid) |
| `@ftxt` | — | — | **error**: `MissingAnalyzer` |
| `@ftxt.<analyzer>` | `FullText { spelling: Ftxt }` | none | `<analyzer>` is any identifier; must resolve |
| `@fulltext.<analyzer>` | `FullText { spelling: FullText }` | none | long spelling, same semantics |
| `@hnsw.<compact>` | `Hnsw` | `(dimension: N, efc: N?, m: N?)` | compact = metric prefix + element, see below |
| `@hnsw.<metric>.<element>` | `Hnsw` | same | long form |
| `@perm` | `Permission(Bare)` | none, or one backtick permission payload | function only |
| `@perm.select` | `Permission(Select)` | one backtick permission payload | |
| `@perm.create` | `Permission(Create)` | one backtick permission payload | |
| `@perm.update` | `Permission(Update)` | one backtick permission payload | |
| `@perm.delete` | `Permission(Delete)` | one backtick permission payload | |

Anything else — including `@hnsw` bare, `@hnsw.<bad>`, `@ftxt.a.b` (3 segments), `@perm.foo`, `@source`, `@asset` — resolves to `AttrPathError::Unknown` → diagnostic `unknown_attribute`, message `unknown attribute @<path>`.

### 2.2 Full-text spelling variants

| Spelling | Enum | Example |
| --- | --- | --- |
| `ftxt` | `FullTextSpelling::Ftxt` | `@ftxt.basic_text_search` |
| `fulltext` | `FullTextSpelling::FullText` | `@fulltext.basic_text_search` |

Both spellings, both directions, in one test (`attribute_catalog.rs`):

```
source: "analyzer basic_text_search {}\ntable Article schemafull {\n  bio string @ftxt.basic_text_search\n  title string @fulltext.basic_text_search\n}\n"
// asserts: report.diagnostics().is_empty()
```

The analyzer segment is *not* a fixed vocabulary — it is any identifier, resolved against declared `analyzer` declarations.

### 2.3 HNSW metric and element segments

```rust
fn parse_compact_hnsw(value: &str) -> Option<(HnswMetric, HnswElement)> {
    for (prefix, metric) in [
        ("cos", HnswMetric::Cosine),
        ("eucl", HnswMetric::Euclidean),
        ("manh", HnswMetric::Manhattan),
    ] {
        if let Some(element) = value.strip_prefix(prefix).and_then(parse_element) {
            return Some((metric, element));
        }
    }
    None
}

fn parse_long_metric(value: &str) -> Option<HnswMetric> {
    match value {
        "cosine" => Some(HnswMetric::Cosine),
        "euclidean" => Some(HnswMetric::Euclidean),
        "manhattan" => Some(HnswMetric::Manhattan),
        _ => None,
    }
}

fn parse_element(value: &str) -> Option<HnswElement> {
    match value {
        "f64" => Some(HnswElement::F64),
        "f32" => Some(HnswElement::F32),
        "i64" => Some(HnswElement::I64),
        "i32" => Some(HnswElement::I32),
        "i16" => Some(HnswElement::I16),
        _ => None,
    }
}
```

| Metric | Long segment | Compact prefix |
| --- | --- | --- |
| Cosine | `cosine` | `cos` |
| Euclidean | `euclidean` | `eucl` |
| Manhattan | `manhattan` | `manh` |

| Element | Segment / compact suffix |
| --- | --- |
| F64 | `f64` |
| F32 | `f32` |
| I64 | `i64` |
| I32 | `i32` |
| I16 | `i16` |

**All 15 compact spellings** (`@hnsw.<compact>`):

`cosf64`, `cosf32`, `cosi64`, `cosi32`, `cosi16`,
`euclf64`, `euclf32`, `eucli64`, `eucli32`, `eucli16`,
`manhf64`, `manhf32`, `manhi64`, `manhi32`, `manhi16`

**All 15 long spellings** (`@hnsw.<metric>.<element>`):

`cosine.f64`, `cosine.f32`, `cosine.i64`, `cosine.i32`, `cosine.i16`,
`euclidean.f64`, `euclidean.f32`, `euclidean.i64`, `euclidean.i32`, `euclidean.i16`,
`manhattan.f64`, `manhattan.f32`, `manhattan.i64`, `manhattan.i32`, `manhattan.i16`

Edge behaviours worth preserving:
- `@hnsw` (one segment) → `Unknown` (no `["hnsw"]` arm).
- `@hnsw.cosine` (two segments, long metric only) → compact parse strips `cos` → `"ine"` → not an element → `Unknown`.
- `@hnsw.cos.f32` → long-metric arm rejects `cos` → `Unknown`. Compact prefixes are only valid *fused* with the element.
- The compact loop returns on the first prefix that yields a valid element; the prefixes are disjoint, so ordering is not load-bearing.

Test evidence (`attribute_catalog.rs`):

```
source: "table Article schemafull {\n  embedding array<float> @hnsw.cosf32(dimension: 1536)\n  related array<float> @hnsw.cosine.f32(dimension: 1536)\n}\n"
// asserts: report.diagnostics().is_empty()
```

### 2.4 Permission operation segments

| Segment | Kind |
| --- | --- |
| *(none)* — `@perm` | `PermissionKind::Bare` |
| `select` | `PermissionKind::Select` |
| `create` | `PermissionKind::Create` |
| `update` | `PermissionKind::Update` |
| `delete` | `PermissionKind::Delete` |

No abbreviations exist for permission operations.

---

## 3. Placement rules — attribute × location matrix

From `catalog.rs::AttributeKind::is_allowed_at`:

```rust
Self::Assert | Self::Value | Self::Default => matches!(location, AttrLocation::Field),
Self::Index | Self::Unique => matches!(location, Field | Table | Relation),
Self::Count => matches!(location, Table | Relation),
Self::FullText { .. } | Self::Hnsw { .. } => matches!(location, Field),
Self::Permission(Bare) => matches!(location, Function),
Self::Permission(Select | Create | Update) => matches!(location, Table | Relation | Field),
Self::Permission(Delete) => matches!(location, Table | Relation),
```

`AttrLocation` variants: `Field`, `Table`, `Relation`, `Function`, `Event`.

| Attribute | Field | Table | Relation | Function | Event |
| --- | :---: | :---: | :---: | :---: | :---: |
| `@assert` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `@value` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `@default` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `@index` | ✅ | ✅ | ✅ | ❌ | ❌ |
| `@unique` | ✅ | ✅ | ✅ | ❌ | ❌ |
| `@count` | ❌ | ✅ | ✅ | ❌ | ❌ |
| `@ftxt.<analyzer>` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `@fulltext.<analyzer>` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `@hnsw.<compact>` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `@hnsw.<metric>.<element>` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `@perm` (bare path) | ❌ | ❌ | ❌ | ✅ | ❌ |
| `@perm.select` | ✅ | ✅ | ✅ | ❌ | ❌ |
| `@perm.create` | ✅ | ✅ | ✅ | ❌ | ❌ |
| `@perm.update` | ✅ | ✅ | ✅ | ❌ | ❌ |
| `@perm.delete` | ❌ | ✅ | ✅ | ❌ | ❌ |

**No attribute of any kind is allowed at `AttrLocation::Event`.** This is a live inconsistency in the archive worth flagging for the rewrite: the parser test `parses_event_when_and_run_blocks` (`cases/event.rs`) happily parses

```
source: "event new_user on user when`$event = \"CREATE\"` {\n  @perm\n  run`\n    INSERT INTO user_log SET user = $after.id\n  `\n}"
expect: "... (Attr perm) ..."
```

but the semantic catalog rejects any attribute on an event, and `validate/mod.rs` has a blanket `(AttrOwner::Event(event), _) => disallowed(...)` arm.

Diagnostic wording depends on the owner (`validate/resolve.rs`):
- Function/event owners → `NotAllowedOn` → `"@index is not allowed on function `visible`"` / `"@count is not allowed on event `created` on table `User`"`.
- Field/table/relation owners → `NotAllowedHere` → `"@perm.delete is not allowed here"`.
- Both render under code `attribute_not_allowed_here`.

Verbatim placement tests (`aureline-semantic/tests/attribute_catalog.rs`):

```
invalid_permission_locations_are_reported_by_catalog:
source: "table User schemafull {\n  email string @perm.delete`NONE`\n}\n"
// code == "attribute_not_allowed_here"

invalid_function_and_event_attribute_locations_name_the_owner_context:
source: r#"func visible(id: uuid) -> bool {
  @index
  run`RETURN true`
}
event created on User when`true` {
  @count
  run`RETURN true`
}
"#
// diagnostics[0]: "@index is not allowed on function `visible`"
// diagnostics[1]: "@count is not allowed on event `created` on table `User`"

relation_permissions_are_table_like_body_attrs:
source: r#"relation Purchased schemafull {
  relate User -> Product
  @perm.select`WHERE true`
  @perm.delete`NONE`
}
"#
// asserts: report.diagnostics().is_empty()
```

---

## 4. Field grammar

### 4.1 Grammar (verbatim from `grammar/field.rs`)

```text
<field> = <path> <type-expr> <attr>* (<newline> | <field-attr-block>)
<field-attr-block> = "{" <newline>* (<attr>+ <newline>*)* "}"
```

with the field path built as:

```rust
let field_path = ident()
    .then(just(Token::Dot).ignore_then(ident()).repeated().collect::<Vec<_>>())
```

Structural facts:

- A field is `path type inline-attrs* tail`, where `tail` is **either** a newline **or** an attribute block. A field with no newline and no block does not parse inside a document.
- `Parser::field` (surgical entry point) appends a synthetic `Token::Newline` so `email string` parses standalone — this is a test affordance, not grammar.
- Inline attrs and block attrs are **flattened into one ordered `attrs` list**; `FieldDecl` cannot tell them apart afterwards.
- A block "row" is `attr+` followed by zero or more newlines, so multiple attributes may share a line, and a whole block may be on one line.
- The closing `}` of an attribute block does **not** consume a following newline; the enclosing table body's `.then_ignore(newlines)` does.
- Field names come from `ident()`, so reserved words cannot be field names (but see §5 for `relate`/`permissions`, which are not reserved).

AST (`schema.rs`):

```rust
pub struct FieldDeclKind<'src> {
    /// Dotted field path, e.g. `email` or `profile.name`.
    pub path: Vec<Ident<'src>>,
    /// Field type expression, e.g. `string`, `option<uuid>`, or `{ name: string }`.
    pub ty: TypeExpr<'src>,
    /// Field attributes, either inline or in a field attribute block.
    pub attrs: Vec<AttrDecl<'src>>,
}
```

### 4.2 Simple field

```
source: "email string",
resolve_surql: false,
expect: "(Field email (Type string))",
```

### 4.3 Dotted / nested field path

```
source: "profile.email string",
resolve_surql: false,
expect: "(Field profile.email (Type string))",
```

(Semantic indexing keeps the dotted text: `aureline-semantic/tests/field.rs::indexes_dotted_field_paths` looks up `"profile.email"` and asserts `path()[0] == "profile"`, `path()[1] == "email"`.)

### 4.4 Inline attributes after the type

```
source: r#"email string @unique @index(name: "email_idx")"#,
resolve_surql: false,
expect: r#"
    (Field email
      (Type string)
      (Attr unique)
      (Attr index
        (Named name (String "email_idx"))))
"#,
```

### 4.5 Attribute *block* form

```
source: r#"email string {
  @unique
  @assert`string::is_email($value)`
  @index(name: "email_idx") @fulltext
}"#,
resolve_surql: false,
expect: r#"
    (Field email
      (Type string)
      (Attr unique)
      (Attr assert
        (Embedded (Raw "string::is_email($value)")))
      (Attr index
        (Named name (String "email_idx")))
      (Attr fulltext))
"#,
```

Note the third row carries **two** attributes on one line.

### 4.6 Combining inline and block attributes

(`cases/table.rs`, `combines_inline_and_block_field_attrs`)

```
source: r#"table User schemafull {
  email string @unique {
    @index(name: "email_idx")
  }
}"#,
resolve_surql: false,
expect: r#"
    (Document
      (Table User
        (Kind Schemafull)
        (Field email
          (Type string)
          (Attr unique)
          (Attr index
            (Named name (String "email_idx"))))))
"#,
```

Ordering: inline attrs first, block attrs appended (`attrs.extend(block_attrs)`).

The same block form inside a table (`cases/table.rs`, `parses_field_attrs_in_block`):

```
source: r#"table User schemafull {
  email string {
    @unique
    @assert`string::is_email($value)`
    @index(name: "email_idx") @fulltext
  }
}"#,
```

### 4.7 Inline object literal type

```
source: "profile { name: string, tags: set<string>, location: geometry<point> }",
resolve_surql: false,
expect: r#"
    (Field profile
      (ObjectType
        (Prop name
          (Type string))
        (Prop tags
          (SetType
            (Type string)))
        (Prop location
          (GeometryType point))))
"#,
```

Object type keys (`grammar/ty/object.rs`) accept `Token::Ident`, `Token::String`, and — unusually — the reserved words `table`, `relation`, `func`, `event`, `analyzer`, `schemafull`, `schemaless`. So `{ table: string, "schemafull": bool }` is a valid object type (see `cases/ty.rs::object_keyword_key_type_spec`).

### 4.8 Object literal type followed by an attribute block

```
source: "profile { name: string } {\n  @unique\n}",
resolve_surql: false,
expect: r#"
    (Field profile
      (ObjectType
        (Prop name
          (Type string)))
      (Attr unique))
"#,
```

This is the one genuinely delicate spot in the field grammar: **two adjacent brace groups**, the first consumed greedily by the type parser as an object literal, the second by `field_tail` as the attribute block.

### 4.9 SurQL-resolving field case

```
lowers_assert_attr_when_surql_is_enabled {
    source: "email string @assert`string::is_email($value)`",
    expect: r#"
        (Field email
          (Type string)
          (Attr assert
            (Embedded
              (Call string::is_email
                (Param value)))))
    "#,
}
```

(no `resolve_surql: false`, so the default `true` applies)

---

## 5. Table and relation body grammar

### 5.1 Grammar (verbatim from `grammar/table.rs` / `grammar/relation.rs` / `grammar/relate.rs`)

```text
<table-decl> = "table" <ident> <table-kind> "{" <body-items>* "}"
<table-kind> = "schemafull" | "schemaless"
<body-item>  = <field> | <table-attr>
```

```text
<relation-decl> = "relation" <ident> <relation-table-kind> "{" <body-items>* "}"
<relation-table-kind> = "schemafull" | "schemaless"
<body-item> = <field> | <table-attr> | <relate-clause>
```

```text
<relate-clause>  = "relate" <ident> <relation-arrow> <ident>
<relation-arrow> = "->" | "<-" | "<->"
```

```text
<attr-item> = @<name> [ "(" <args> ")" ] <newline>
```

### 5.2 How table-level attributes are distinguished from field-level ones

Purely **positional**, decided by the body-item `choice` (`grammar/body.rs`):

- `body::field_item()` = `field::parser()` — an attribute is field-level if it appears *after a field's type* (inline) or inside that field's `{ … }` attribute block. `field::parser()` consumes the terminating newline itself.
- `body::attr_item()` = `attr::parser().then_ignore(just(Token::Newline))` — an attribute is table-level if it starts a body line on its own. The attribute parser itself never consumes a newline (because inline field attrs must not); the wrapper adds the required newline.
- Chumsky tries `field_item` before `attr_item` in `table.rs`, and `relate_item` → `field_item` → `attr_item` in `relation.rs`. Since a field must start with an `Ident` and an attribute with `Token::Attr`, there is no real ambiguity.
- `BodyItems::from_items` splits the parsed items into `fields` / `attrs` / `relates` buckets, so **relative source ordering between fields and table attrs is lost** in the AST (and the outline always prints kind → relates → fields → attrs).

Combined example (`cases/table.rs`, `parses_field_and_table_attrs`) — `@index(name: …)` is field-level, the following `@index(fields: …)` and `@count` are table-level:

```
source: r#"table User schemafull {
  email string @index(name: "email_idx")
  @index(fields: [email], pair: (123, 435), active: true, value: 291.0)
  @count
}"#
```

### 5.3 Relation `relate` clauses and arrow directions

```
parses_relation_table_with_relate_clause {
    source: r#"relation Likes schemafull {
  relate user -> post
  created_at datetime @index
  @count
}"#,
    resolve_surql: false,
    expect: r#"
        (Document
          (Relation Likes
            (Kind Schemafull)
            (Relate "user -> post")
            (Field created_at
              (Type datetime)
              (Attr index))
            (Attr count)))
    "#,
}
```

```
parses_relation_table_relate_arrow_directions {
    source: r#"relation Likes schemafull { relate user -> post }
relation Owned schemafull { relate user <- post }
relation Friends schemaless { relate user <-> user }"#,
    resolve_surql: false,
    expect: r#"
        (Document
          (Relation Likes
            (Kind Schemafull)
            (Relate "user -> post"))
          (Relation Owned
            (Kind Schemafull)
            (Relate "user <- post"))
          (Relation Friends
            (Kind Schemaless)
            (Relate "user <-> user")))
    "#,
}
```

| Arrow | `RelationDirection` | Doc comment |
| --- | --- | --- |
| `->` | `Right` | `from -> to` |
| `<-` | `Left` | `from <- to` |
| `<->` | `Both` | `from <-> to` |

Notes from `grammar/relate.rs`:
- Newlines are optional *between* the clause pieces (`relate`, from, arrow, to) and one trailing newline run is consumed.
- The arrow direction is preserved verbatim in the AST; normalisation to SurrealDB `IN`/`OUT` is deferred to later passes.
- `relates` is a `Vec`, so multiple clauses parse; "exactly one" is explicitly declared future semantic work.
- Normal `table` bodies deliberately exclude `relate_item`.

### 5.4 Structural keywords usable as field names

`relate` is matched by a `select_ref!` guard on `Token::Ident(name) if *name == "relate"`, not by a lexer keyword. So when the relate-clause parser fails (no arrow follows), the body item falls through to `field_item` and `relate` becomes a field name:

```
allows_relate_as_a_field_name_when_not_a_clause {
    source: r#"relation Links schemafull {
  relate string
  relate user -> post
}"#,
    resolve_surql: false,
    expect: r#"
        (Document
          (Relation Links
            (Kind Schemafull)
            (Relate "user -> post")
            (Field relate (Type string))))
    "#,
}
```

```
allows_non_structural_words_as_field_names {
    source: r#"table Config schemafull {
  permissions string
}"#,
    resolve_surql: false,
    expect: r#"
        (Document
          (Table Config
            (Kind Schemafull)
            (Field permissions (Type string))))
    "#,
}
```

By contrast the true lexer keywords (`table`, `relation`, `schemafull`, `schemaless`, `func`, `event`, `analyzer`, `true`, `false`) cannot be field names or attribute path segments.

### 5.5 Whitespace / layout tolerance (`cases/table.rs`)

```
parses_empty_schemafull_table  : "table User schemafull {\n}"        -> (Document (Table User (Kind Schemafull)))
parses_empty_schemaless_table  : "table User schemaless {\n}"        -> (Document (Table User (Kind Schemaless)))
allows_blank_lines_inside_table: "table User schemafull {\n\n}\n"    -> (Document (Table User (Kind Schemafull)))
allows_same_line_empty_table   : "table User schemafull { }\n"       -> (Document (Table User (Kind Schemafull)))
allows_trailing_top_level_newlines: "table User schemafull {\n}\n\n" -> (Document (Table User (Kind Schemafull)))
```

Document level (`grammar/mod.rs`): `<document> = <newline>* <decl>* EOF`, blank lines accepted before/between/after declarations.

---

## 6. Attribute validation beyond placement

Pipeline (`aureline-semantic/src/attributes/validate/mod.rs`): `collect::attrs(facts)` → `resolve::attr` (catalog + location) → per-`(owner, kind)` dispatch into **shape**, **reference**, and **type** checks.

### 6.1 Argument shape (`validate/shape.rs`)

Three shape primitives:

| Helper | Rule |
| --- | --- |
| `check_no_args` | any arg is an error: `Embedded` → `DoesNotAcceptEmbeddedPayload`, `Positional` → `DoesNotAcceptPositionalArguments`, `Named` → `DoesNotAcceptArguments` |
| `collect_named_args(allowed)` | only names in `allowed` accepted (`DoesNotAcceptArgument` otherwise); duplicates reported (`DuplicateArgument`) but **the last value wins**; any positional or embedded arg rejected |
| `exactly_one_embedded` | zero args → `RequiresEmbeddedPayload`; >1 arg → `AcceptsExactlyOneEmbeddedPayload`; one `Positional` → `RequiresEmbeddedPayloadFromPositional`; one `Named` → `RequiresEmbeddedPayloadFromNamed` |

Per-attribute shape rules:

| Attribute / owner | Allowed named args | Required | Value type rule |
| --- | --- | --- | --- |
| `@index` on **field** (`check_field_index_args`) | `name` | — | `name` must be a string literal |
| `@index` / `@unique` on **table/relation** (`check_index_or_unique_shape`) | `fields`, `name` | `fields` | `name` string literal; `fields` must be a **non-empty list** whose items are `Ident` or dotted `Path` (`field path list` / `non-empty field path list` / `field path` mismatches) |
| `@unique` on **field** | — | — | `check_no_args` |
| `@count` on table/relation | — | — | `check_no_args` |
| `@ftxt.*` / `@fulltext.*` on field | — | — | `check_no_args` |
| `@hnsw.*` (`check_hnsw_args`) | `dimension`, `efc`, `m` | `dimension` | each must be a **positive int** literal (`NumericKind::Int`, parses as `i64`, `> 0`) |
| `@assert` / `@value` / `@default` | — | one embedded | `exactly_one_embedded` |
| `@perm*` | — | one embedded, **except** bare `@perm` with zero args returns early with no findings | `exactly_one_embedded` |

Consequence worth recording: **no catalog attribute accepts a positional argument today**, even though the grammar and AST support them (`AttrArg::Positional`, doc example `@hnsw(1536)`).

### 6.2 Argument / field type checks (`validate/types.rs`)

- `check_fulltext_field_type` — the annotated field must be `string` or `option<…string>` (`is_stringish`), else `FieldTypeMismatch` with expected `"string"`.
- `check_hnsw_field_type` — the field must be `array<number|float|int>` or `option<…>` of that (`is_numeric_array`), else expected `"numeric array"`.
- `check_field_expression_attr` (for `@assert` / `@value` / `@default`):
  - payload must be a **parsed expression**; a parsed *query* → `UnsupportedFieldEmbedded { kind: "query" }`; still-raw → `kind: "raw"`.
  - analyzer references inside the expression are checked (see 6.3).
  - the expression is typechecked in the field's parameter scope (`$value`, etc.) using the `FunctionCatalog`.
  - expected type: `@assert` → `bool`; `@value` / `@default` → the field's declared type. Mismatch → `FieldExpressionTypeMismatch`, rendered as `field_assert_type_mismatch` / `field_value_type_mismatch` / `field_default_type_mismatch` / `field_attribute_type_mismatch`.
  - if the offending expression is a call to a schema `fn::name`, the diagnostic gains a **secondary label** on the function's `-> type` annotation plus a help line.
- `check_permission_attr`:
  - `FULL` (as `Ident` or single-segment `Path`) and `NONE` literals short-circuit with no typechecking (`is_permission_literal`).
  - otherwise, the expression is typechecked with `$auth: object` injected into scope (plus the field param scope when the owner is a field) and must be assignable to `bool`, else `PermissionExpressionTypeMismatch` → `"@perm.select must return bool, got string"`.
  - parsed-query payload → `UnsupportedPermissionEmbedded { kind: "query" }`; raw → `kind: "raw"`.

### 6.3 Reference resolution (`validate/references/`)

- `check_analyzer_exists` (`analyzers.rs`) — **yes, an analyzer named by a full-text attribute must exist.** The analyzer segment of `@ftxt.<a>` / `@fulltext.<a>` is looked up in `facts.analyzers().by_name(...)`; missing → `UnknownAnalyzer` (`unknown_analyzer`, `"unknown analyzer `basic_text_search`"`), spanned on the attribute.
- `check_analyzer_refs_in_expr` — additionally walks **every embedded expression** in field-expression attrs and permission attrs, finds `search::analyze(...)` calls, and resolves their **first string-literal argument** as an analyzer name. The walker (`references/walk.rs::for_each_call`) recurses through calls, objects, lists/tuples/blocks, ranges, record IDs, access chains, field access, method calls, graph traversals, closures, casts, unary/binary, `RETURN`/`THROW`, `if`, subqueries — so nested and block-scoped occurrences are caught. Span points at the string literal.
- `check_table_index_fields` / `check_relation_index_fields` (`index_fields.rs`) — every path in `@index(fields: [...])` / `@unique(fields: [...])` on a table or relation must resolve to a declared field of that same parent, else `UnknownField` → `"@unique on table `User` references unknown field `missing`"`, spanned on the individual list item where possible.

### 6.4 Diagnostic codes emitted for attributes

From `aureline-semantic/src/diagnostic/codes.rs` + `render.rs` + `messages.rs`:

| Finding | Code | Message template |
| --- | --- | --- |
| `MissingAnalyzerSegment` | `attribute_missing_analyzer` | `@{attr} requires an analyzer name` |
| `UnknownAttribute` | `unknown_attribute` | `unknown attribute @{attr}` |
| `NotAllowedHere` | `attribute_not_allowed_here` | `@{attr} is not allowed here` |
| `NotAllowedOn` | `attribute_not_allowed_here` | `@{attr} is not allowed on {context}` |
| `MissingArgument` | `attribute_missing_argument` | `@{attr} requires argument `{name}`` |
| `DoesNotAcceptArguments` | `attribute_unexpected_argument` | `@{attr} does not accept arguments` |
| `DoesNotAcceptArgument` | `attribute_unknown_argument` | `@{attr} does not accept argument `{name}`` |
| `DuplicateArgument` | `attribute_duplicate_argument` | `@{attr} repeats argument `{name}`` |
| `DoesNotAcceptPositionalArguments` | `attribute_unexpected_positional_argument` | `@{attr} does not accept positional arguments` |
| `DoesNotAcceptEmbeddedPayload` | `attribute_unexpected_embedded_argument` | `@{attr} does not accept an embedded payload` |
| `RequiresEmbeddedPayload` | `attribute_missing_argument` | `@{attr} requires an embedded payload` |
| `RequiresEmbeddedPayloadFromPositional` | `attribute_unexpected_positional_argument` | same message |
| `RequiresEmbeddedPayloadFromNamed` | `attribute_unknown_argument` | same message |
| `AcceptsExactlyOneEmbeddedPayload` | `attribute_unexpected_argument` | `@{attr} accepts exactly one embedded payload` |
| `ArgumentTypeMismatch` | `attribute_argument_type_mismatch` | `argument `{name}` to @{attr} expected {expected}` |
| `UnknownField` | `attribute_unknown_field` | `@{attr} on {context} references unknown field `{path}`` |
| `UnknownAnalyzer` | `unknown_analyzer` | `unknown analyzer `{analyzer}`` |
| `FieldTypeMismatch` | `attribute_field_type_mismatch` | `@{attr} requires field `{field}` to be {expected}, got {actual}` |
| `FieldExpressionTypeMismatch` | `field_{assert,value,default,attribute}_type_mismatch` | `@{attr} for field `{f}` must return {e}, but function `{fn}` returns {a}` / `@{attr} for field `{f}` expected {e}, got {a}` |
| `PermissionExpressionTypeMismatch` | `permission_expression_type_mismatch` | `@{attr} must return bool, got {actual}` |
| `UnsupportedFieldEmbedded` | `unsupported_field_attribute_expression` | `cannot typecheck {kind} embedded SurQL for @{attr} on field `{field}`` |
| `UnsupportedPermissionEmbedded` | `unsupported_permission_expression` | `cannot typecheck {kind} embedded SurQL for @{attr}` |

`{attr}` in messages is the dotted path (`diagnostics::attr_path` joins with `.`), which is why messages read `@perm.select` and `@hnsw.cosf32`.

---

## 7. Error cases

### 7.1 Parser-level `error,` cases

| Case | File | Source | Why it fails |
| --- | --- | --- | --- |
| `rejects_permission_without_where_keyword_for_now_spec` | `cases/permissions.rs` | ``@perm.select`owner = $auth.id` `` | `@perm*` payloads go through SurrealDB's `Permission` grammar, which requires `FULL`, `NONE`, or `WHERE <expr>` |
| `rejects_permission_where_without_predicate_spec` | `cases/permissions.rs` | ``@perm`WHERE` `` | `WHERE` with no predicate |
| `reports_parse_errors` | `cases/table.rs` | `"table User schemafull"` | table declaration with no body braces |
| `rejects_event_without_run_block` | `cases/event.rs` | ``"event new_user on user when`$event = \"CREATE\"` {}"`` | `run` block is required |
| `rejects_event_run_comma_expr_list_spec` | `cases/event.rs` | ``run`(CREATE …), (UPDATE …)` `` | comma-separated expression list is not an accepted `run` payload |
| `rejects_function_without_run_block_empty_body` | `cases/func.rs` | `"func get_user(id: uuid) {}"` | `run` block required |
| `rejects_function_without_run_block_no_body` | `cases/func.rs` | `"func get_user(id: uuid)"` | no body |
| `rejects_positional_function_params` | `cases/func.rs` | ``"func get_user(uuid) { run`SELECT * FROM user` }"`` | function params use `args::named_optional` (named-only) — **contrast with attributes, which use `args::mixed` and do accept positional syntax** |

Verbatim:

```rust
rejects_permission_without_where_keyword_for_now_spec {
    source: "@perm.select`owner = $auth.id`",
    error,
}

rejects_permission_where_without_predicate_spec {
    source: "@perm`WHERE`",
    error,
}

reports_parse_errors {
    source: "table User schemafull",
    resolve_surql: false,
    error,
}
```

(No `rejects_*` cases exist in `cases/field.rs`, `cases/literal.rs`, or `cases/surql.rs`.)

### 7.2 Semantic error cases (`aureline-semantic/tests/attribute_catalog.rs`)

```rust
// unknown attribute path
source: "table User schemafull {\n  email string @asset`true`\n}\n"
// unknown_attribute / Error / "unknown attribute @asset"

// bare fulltext missing its analyzer segment
source: "table Article schemafull {\n  bio string @fulltext\n}\n"
// attribute_missing_analyzer / "@fulltext requires an analyzer name"

// analyzer must exist
source: "table Article schemafull {\n  bio string @ftxt.basic_text_search\n}\n"
// unknown_analyzer / "unknown analyzer `basic_text_search`"

// fulltext requires a string field
source: "analyzer basic_text_search {}\ntable Article schemafull {\n  age int @ftxt.basic_text_search\n}\n"
// attribute_field_type_mismatch
// "@ftxt.basic_text_search requires field `age` to be string, got int"

// hnsw dimension must be a positive int
source: "table Article schemafull {\n  embedding array<float> @hnsw.cosf32(dimension: 0)\n}\n"
// attribute_argument_type_mismatch
// "argument `dimension` to @hnsw.cosf32 expected positive int"

// table index fields must resolve
source: "table User schemafull {\n  email string\n  @index(fields: [email])\n  @unique(fields: [missing])\n}\n"
// attribute_unknown_field, span on `missing`
// "@unique on table `User` references unknown field `missing`"

// permission payload must be bool
source: r#"table User schemafull {
  @perm.select`WHERE string::lowercase("x")`
}
"#
// permission_expression_type_mismatch / "@perm.select must return bool, got string"

// placement failures (see §3 for the two-diagnostic function/event case)
source: "table User schemafull {\n  email string @perm.delete`NONE`\n}\n"
// attribute_not_allowed_here
```

`search::analyze` analyzer-reference failures, all reporting `unknown_analyzer` spanned on the string literal:

```rust
// direct argument in @default
tokens array<string> @default`search::analyze("missing_analyzer", "hello world")`

// nested inside another call
token_count int @default`array::len(search::analyze("missing_analyzer", "hello world"))`

// inside a block expression in @value
tokens array<string> @value`{ LET $tokens = search::analyze("missing_analyzer", "hello world"); RETURN $tokens; }`

// inside a permission subquery
@perm.select`WHERE (SELECT VALUE search::analyze("missing_analyzer", "hello world") FROM Article LIMIT 1) != NONE`

// inside object/list/index nesting
payload any @default`{ tokens: [search::analyze("missing_analyzer", "hello world")][0] }`
```

### 7.3 Field-attribute type errors (`aureline-semantic/tests/field_attr_typecheck.rs`)

```rust
// array element type not assignable
scores array<number> @default`[true]`
// field_default_type_mismatch, span on `[true]`
// "@default for field `scores` expected array<number>, got array<bool>"

// assert must return bool
email string @assert`string::lowercase($value)`
// field_assert_type_mismatch
// "@assert for field `email` must return bool, but function `string::lowercase` returns string"

// value must match field type
username string @value`time::now()`
// field_value_type_mismatch
// "@value for field `username` must return string, but function `time::now` returns datetime"

// default must match field type
created_at datetime @default`"soon"`
// field_default_type_mismatch
// "@default for field `created_at` expected datetime, got string"

// unknown param inside a payload does NOT cascade into a contract error
email string @assert`string::is_email($anything_else)`
// exactly 1 diagnostic: unknown_parameter / "unknown parameter `$anything_else`"

// nested function arg errors are preserved
email string @assert`string::is_email(10)`
// function_argument_type_mismatch
// "argument `value` to `string::is_email` expected string, got int"

// schema function signatures are used
func accepts_string(value: string) -> bool { run`RETURN true` }
... email string @assert`fn::accepts_string(10)`
// "argument `value` to `fn::accepts_string` expected string, got int"

// schema function return type checked against the attr contract, with a
// secondary label on `-> string` and a help line — note the ATTRIBUTE BLOCK form:
email string {
  @assert`fn::accepts_string($value)`
}
// field_assert_type_mismatch + secondary label on "-> string"
// help: "change this return annotation to `bool` if the function body already returns `bool`"
```

Passing baselines (no diagnostics):

```
email string @assert`string::is_email($value)`
username string @value`string::lowercase($value)`
created_at datetime @default`time::now()`
scores array<number> @default`[1]`
```

---

## 8. Consolidated grammar for a rewrite

```text
document        = newline* decl* EOF
decl            = table-decl | relation-decl | analyzer-decl | func-decl | event-decl

table-decl      = "table" ident table-kind "{" newline* (table-body-item newline*)* "}"
relation-decl   = "relation" ident table-kind "{" newline* (relation-body-item newline*)* "}"
table-kind      = "schemafull" | "schemaless"

table-body-item    = field | attr-item
relation-body-item = relate-clause | field | attr-item

attr-item       = attr newline
relate-clause   = "relate" newline* ident newline* arrow newline* ident newline*
arrow           = "->" | "<-" | "<->"

field           = field-path type-expr attr* field-tail
field-path      = ident ("." ident)*
field-tail      = attr-block | newline
attr-block      = "{" newline* (attr+ newline*)* "}"

attr            = "@" ident ("." ident)* attr-payload?
attr-payload    = "(" attr-args? ")" | raw-string
attr-args       = attr-arg ("," attr-arg)* ","?
attr-arg        = ident ":" value | value

value           = list | tuple | path | string | number | bool | ident
list            = "[" (value ("," value)* ","?)? "]"
tuple           = "(" (value ("," value)* ","?)? ")"
path            = ident "." ident ("." ident)*
```

Non-obvious invariants to carry over:

1. Attribute payload is `(args)` **xor** `` `raw` ``, never both, and always optional.
2. A backtick payload becomes exactly one `AttrArg::Embedded`; catalog attributes that accept it accept exactly one.
3. The SurQL slot for a payload is chosen by the **first path segment only**: `perm` → permission grammar; everything else → expression grammar.
4. Inline and block field attributes are flattened into one ordered list (inline first).
5. A field must terminate with a newline or an attribute block; table-level attributes must terminate with a newline.
6. Field/table-attr/relate items lose their interleaved ordering when bucketed into the AST.
7. `relate` and `permissions` are ordinary identifiers; only the nine lexer keywords are reserved. Object-type keys additionally allow those keywords and quoted strings.
8. The parser accepts any `@name`; unknown-attribute rejection is exclusively a semantic-catalog concern.
9. Positional attribute arguments parse but are rejected by every current catalog validator.
10. `AttrLocation::Event` currently permits nothing, though the parser accepts attributes in event bodies.