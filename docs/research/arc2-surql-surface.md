# Aureline embedded-SurQL surface: complete reconstruction report

Archive root: `/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2`

---

## 0. Pipeline, entry points, and how to read the corpus

### 0.1 Where SurQL enters the DSL

`/Volumes/mypixel/conductor/workspaces/aureline-orm/spokane/.repo/aureline-orm-arc2/aureline-parser/src/parser/surql/mod.rs` defines three *slots*, chosen by the containing Aureline node:

| Slot | Aureline syntax | SurrealDB parser entry | Result |
|---|---|---|---|
| `Slot::Expr` | any attribute payload (`@assert`…``, `@value`…``, `@default`…``) and `event … when`…`` | `Parser::enter_parse::<surrealdb_ast::Expr>` | `ParsedExpr::Expr(ast::Expr)` |
| `Slot::Query` | `func … { run`…` }` and `event … { run`…` }` | `Parser::enter_parse::<surrealdb_ast::Query>` | `ParsedExpr::Query(Vec<ast::Expr>)` — one lowered expr per top-level statement |
| `Slot::Permission` | any attribute whose first path segment is `perm` (`@perm`, `@perm.select`, …) | `Parser::enter_parse::<surrealdb_ast::Permission>` | `ParsedExpr::Expr` — `FULL`/`NONE` are re-parsed as ordinary expressions, `WHERE <expr>` lowers only the predicate |

Before lowering, the payload is `ast_surql::Expr::Raw { raw, span }`; after, `ast_surql::Expr::Parsed { parsed, span }` (`/…/aureline-ast/src/ast_surql.rs`). Tests with `resolve_surql: false` show the un-lowered form, e.g. `(When (Raw "$event = \"CREATE\""))`.

Test entry points (`/…/aureline-parser/src/parser/mod.rs`): `Parser::surql_expr` (used by `expr_cases!`), `Parser::surql_ty` (`ty_cases!` with `parsers: [aureline_ty, surql_ty]`), `Parser::surql_query`, `Parser::attr`, `Parser::field`, `Parser::document`.

### 0.2 Spans

`LowerCx` (`/…/aureline-parser/src/parser/surql/cx.rs`) holds `raw` (the backtick payload only), `base_offset` (byte offset of the payload inside the `.aurl` file), and the SurrealDB arena. Every SurrealDB relative span is re-based into document coordinates by `source_span()`. `span_for_expr` has a special case: a `Binary` node's span is computed as `left ∪ op ∪ right` rather than trusting the parser node span.

### 0.3 S-expression conventions in the corpus

`/…/aureline-test/src/outline/actual/{expr,query,statement,ty}.rs` render the AST; the test `expect:` strings are those outlines. Label with payload renders as `(Label payload children…)`, e.g. `Call(string::is_email)` prints `(Call string::is_email …)`. `(Opaque <kind>)` is the opaque rendering. A case with `error,` asserts a hard parse failure; a case with only `source:` and no `expect:` asserts "this must parse" without pinning a shape.

Per `/…/aureline-parser/tests/cases/AGENTS.md`, the corpus deliberately encodes **desired** shapes rather than current `Opaque` behaviour — so some expectations in `expr.rs` marked `_spec` are aspirational/failing specs. They are still the authoritative statement of intended surface and are documented below as such.

---

## 1. Expressions — `aureline-parser/tests/cases/expr.rs`

Dispatcher: `/…/aureline-parser/src/parser/surql/lower/expr/mod.rs`. Every arm below is a `surrealdb_ast::Expr` variant routed to a specific Aureline node; anything not in the table becomes `Opaque("unsupported expression")`.

### 1.1 Parameters, paths, identifiers

```
source: "id"                              →  (Path id)
source: "string::is_email($value)"        →  (Call string::is_email (Param value))
```
- `$value` → `ExprKind::Param("value")` — **`$` is stripped**, name stored bare.
- Bare/dotted SurQL `Path` → `ExprKind::Path(Vec<segment>)`, rendered with `::` joins. `cx.path_segments()` returns `None` when a path contains a non-`Ident` segment (version segments such as `fn::foo<1.0.0>`), producing `Opaque("unsupported path")`.
- `ExprKind::Ident` is **never produced by the SurQL lowerer**. It comes only from the Aureline-native attribute literal grammar (`/…/aureline-parser/src/parser/grammar/literal.rs`), see §5.5.

### 1.2 Binary operators

```
source: "$value > 0"
expect: "(Binary GreaterThan (Param value) (Int 0))"

source: "$value OR 'GBR'"
expect: (Binary Or (Param value) (String "GBR"))

source: "first_name + ' ' + last_name"
expect: (Binary Add (Binary Add (Path first_name) (String " ")) (Path last_name))

source: "$value != NONE AND string::is_email($value)"
expect: (Binary And (Binary NotEqual (Param value) (None)) (Call string::is_email (Param value)))

source: "$value IN [\"draft\", \"published\", \"archived\"]"
expect: (Binary In (Param value) (List (String "draft") (String "published") (String "archived")))
```
`WHERE host == $parent.id` (subquery corpus) yields `(Binary ExactEqual …)`; `WHERE id NOT IN (…)` yields `(Binary NotIn …)`.

**Complete operator mapping** (`binary_op()` in `/…/lower/expr/operator.rs`) — SurrealDB `BinaryOperator` → `ast::BinaryOp`:

| SurQL spelling | `BinaryOp` |
|---|---|
| `=` | `Equal` |
| `==` | `ExactEqual` |
| `!=` | `NotEqual` |
| `>` | `GreaterThan` |
| `>=` | `GreaterThanEqual` |
| `<` | `LessThan` |
| `<=` | `LessThanEqual` |
| `AND`, `&&` | `And` |
| `OR`, `\|\|` | `Or` |
| `+` | `Add` |
| `-` | `Subtract` |
| `*`, `×` | `Multiply` |
| `/` | `Divide` |
| `%` | `Remainder` |
| `IN`, `∈` (`Inside`) | `In` |
| `NOT IN`, `∉` (`NotInside`) | `NotIn` |
| `..`, `..=` | *not a binary node* — become `ExprKind::Range` (§1.7) |
| **everything else** | **`BinaryOp::Other`** |

`BinaryOp::Other` swallows, with the operator identity **lost entirely** (no raw text retained — this is a real fidelity hole, not an `Opaque`): `**` (Power), `*=` (AllEqual), `?=` (AnyEqual), `??` (NullCoalescing), `?:` (TernaryCondition), `CONTAINS`/`∋`, `CONTAINSNOT`/`∌`, `CONTAINSALL`/`⊇`, `CONTAINSANY`/`⊃`, `CONTAINSNONE`/`⊅`, `ALLINSIDE`/`⊆`, `ANYINSIDE`/`⊂`, `NONEINSIDE`/`⊄`, `OUTSIDE`, `INTERSECTS`, `>..` / `>..=` (RangeSkip / RangeSkipInclusive), `@@` (Matches, with reference/operator payload), `<|k,dist|>` KNN, KTree, KApproximate.

### 1.3 Unary operators

```
source: "(SELECT * FROM product WHERE !(SELECT * FROM order_line WHERE …))"
expect: … (Where (Unary Not (Subquery (Select …))))
```
`UnaryOp` has exactly **one** variant: `Not` (SurQL `NOT expr` / `!expr`). `PrefixOperator::Negate` (`-x`) and `Positive` (`+x`) are **not** modeled — they become `Opaque("unsupported prefix expression")`. (Negative *literals* still work because SurrealDB folds the sign into the `Integer`/float token; see §5.2.)

### 1.4 Function calls, method calls, parenthesised callees

```
source: "time::now()"            →  (Call time::now)
source: "(time::now)()"          →  (Call time::now)          // Covered wrapper peeled
source: "$value.is_email()"      →  (MethodCall is_email (Param value))
source: "($value).is_email()"    →  (MethodCall is_email (Param value))
source: "$value.all(|$val| $val IN 0..=255)"
expect: (MethodCall all (Param value) (Closure (Param val) (Binary In (Param val) (RangeInclusive (Int 0) (Int 255)))))
```
`call_operator()` in `/…/lower/expr/access.rs`: an `IdiomOperator::Call` whose left side is `x.method` becomes `MethodCall{target, method, args}`; otherwise the left side must `uncover()` to a `Path`, giving `Call{path, args}`. Any other callee (`$fn()`, `(|$x| …)(1)`, versioned path) → `Opaque("unsupported call target")`.

### 1.5 Field access, index access, access chains

```
source: "$auth.id = id"
expect: (Binary Equal (FieldAccess id (Param auth)) (Path id))

source: "$products[0].org"
expect: (Access (Base (Param products)) (Index (Int 0)) (Field org))
```
Rules (`access.rs`):
- `.field` on a plain base → `FieldAccess{base, field}`.
- `.field` on an existing `GraphField` → appended to the graph path.
- `.field` on an existing `Access` → appended as `AccessStep::Field`.
- `[expr]` → starts or extends a **flat** `Access{base, steps}` chain (`AccessStep::Index`), never nested `Index(Field(...))`.
- `.method(args)` on an `Access` chain → `AccessStep::MethodCall`; otherwise a standalone `MethodCall` node.
- `AccessStep::Graph { direction, query }` **exists in the AST but is never constructed** by the lowerer (graph steps always produce a top-level `GraphTraversal`). Dead variant.

### 1.6 Conditionals, blocks, return, throw

```
source:
    IF $input % 2 = 0 {
        RETURN true
    } ELSE {
        THROW "must be even"
    }
expect:
    (If
      (Condition (Binary Equal (Binary Remainder (Param input) (Int 2)) (Int 0)))
      (Then (Block (Return (Bool true))))
      (Else (Block (Throw (String "must be even")))))
```
The same body wrapped in `{ … }` produces an outer `(Block (If …))`. `otherwise` is optional (`(Else …)` omitted when absent). `RETURN expr` → `ExprKind::Return`; `THROW expr` → `ExprKind::Throw` (SurrealDB stores `Throw` as a bare `NodeId<Expr>`, handled by `control::lower_throw_expr`).

### 1.7 Ranges

```
source: "$value IN 0..=255"  →  (Binary In (Param value) (RangeInclusive (Int 0) (Int 255)))
source: "..10"               →  (Range (Int 10))        // open start
source: "1.."                →  (Range (Int 1))         // open end
source: ".."                 →  (Range)                 // unbounded
```
Sources: `BinaryOperator::Range`/`RangeInclusive` (both bounds), `PrefixOperator::Range`/`RangeInclusive` (open start), `PostfixOperator::Range` (open end), `Expr::UnboundedRange` (`..`). Note that the S-expression is ambiguous between open-start and open-end: `(Range (Int 10))` vs `(Range (Int 1))` — the outline only prints the present bounds, so `Range{start, end, inclusive}` distinguishes them but the printed form does not. `PostfixOperator::RangeSkip` (`>..`) is **not** handled → `Opaque("unsupported postfix expression")`.

### 1.8 Closures

```
source: "$value.all(|$val| $val IN 0..=255)"
expect: … (Closure (Param val) (Binary In …))
```
`ExprKind::Closure{params: Vec<name>, body}`; parameter names stored without `$`. **Typed closures are refused**: if `closure.output_ty.is_some()` or any parameter has a type, the whole closure becomes `Opaque("unsupported typed closure")` (`/…/lower/expr/control.rs`).

### 1.9 Graph traversal and graph projection

Three and only three graph shapes lower structurally (`GraphTraversalSource` in `access.rs`):

**(a) `->(SELECT …)` — explicit select lookup:**
```
source:
    (SELECT
        ->(SELECT like_strength FROM likes WHERE like_strength > 10 ORDER BY like_strength DESC) AS strong_likes
    FROM person)
expect: … (Alias strong_likes (GraphTraversal Out (Select (Fields (Path like_strength)) (From (Table likes)) (Where …) (Order (OrderItem (Path like_strength) (Direction Desc))))))
```

**(b) Range subject — synthesised `SELECT * FROM …`:**
```
source: "(SELECT * FROM person:tobie->likes:1..10)"
expect:
    (From (GraphTraversal Out
            (Base (RecordId person (KeyString tobie)))
            (Select (Fields (All)) (From (RecordId likes (KeyRange (KeyInt 1) (KeyInt 10)))))))
```
and the explicitly-nested equivalent `(SELECT * FROM person:tobie->(SELECT * FROM likes:1..10))` produces the identical outline.

**(c) Document-rooted table subject → `GraphField` projection:**
```
source: "(SELECT count() AS number_of_reviews, math::mean(<float> rating) AS avg_review, ->product.id AS product_id, ->product.name AS product_name FROM review GROUP BY product_id, product_name)"
expect: … (Alias product_id (GraphField product.id)) (Alias product_name (GraphField product.name)) …
```
`GraphField{path}` renders dot-joined. Only produced when the traversal base uncovers to `sdb::Expr::Document` (the implicit `@` document) and the subject is a `LookupSubject::Table`.

**Boundary:** a `LookupSubject::Table` with a *non-document* base — i.e. the everyday `person:tobie->likes->post` — returns `None` and becomes `Opaque("unsupported graph traversal")`. Likewise `Lookup::Any` (`->?`), `Lookup::Basic` (`->likes WHERE … LIMIT …`), `IdiomOperator::Reference` (`<~`) and `IdiomOperator::Recurse` are not lowered. `GraphDirection` covers `In` (`<-`), `Out` (`->`), `Both` (`<->`).

### 1.10 Containers, parenthesisation, subqueries

```
source: "($value > 0)"                      →  (Binary GreaterThan (Param value) (Int 0))   // Covered peeled
source: "[\"draft\", \"published\"]"        →  (List (String "draft") (String "published"))
source: "{ active: true, created_at: time::now() }"
expect: (Object (Prop active (Bool true)) (Prop created_at (Call time::now)))
source: "(SELECT VALUE id FROM user LIMIT 1) != NONE"
expect: (Binary NotEqual (Subquery (Select (Value (Path id)) (From (Table user)) (Limit (Int 1)))) (None))
```
Object keys are decoded arena strings (quoted keys normalise: `"schemafull": bool` → `Prop schemafull`). `ExprKind::Tuple` is **not** produced from SurQL — only from the Aureline attribute grammar (§5.5).

### 1.11 Full expression-form inventory (dispatcher arms)

| SurrealDB `Expr` variant | Aureline result |
|---|---|
| `Covered` | transparent — inner value, outer span |
| `Param` | `Param` |
| `Path` | `Path` (or `Opaque("unsupported path")`) |
| `Builtin` | `Literal::{None,Null,Bool}` |
| `Integer`, `Float`, `Decimal` | `Literal::Number` |
| `Duration`, `DateTime`, `Uuid`, `Regex`, `File`, `String` | corresponding `Literal` |
| `Array` | `List` |
| `Object` | `Object` |
| `RecordId` | `RecordId` |
| `Binary` | `Binary` or `Range` |
| `Prefix` | `Cast`, `Unary(Not)`, `Range`, else `Opaque` |
| `Postfix` | `Range`, else `Opaque` |
| `UnboundedRange` | `Range{None,None,false}` |
| `Idiom` | `FieldAccess`/`Access`/`MethodCall`/`Call`/`GraphTraversal`/`GraphField`, else `Opaque` |
| `Closure` | `Closure` (untyped only) |
| `Block` | `Block` |
| `If` | `If` |
| `Let`, `For` | `Statement(Let/For)` |
| `Create`,`Update`,`Upsert`,`Delete`,`Insert`,`Relate` | `Statement(...)` |
| `Select` | `Subquery(QueryExpr::Select)` |
| `Return` | `Return` |
| `Throw` | `Throw` |
| *all others* | `Opaque("unsupported expression")` |

"All others" is large in the pinned parser (`surrealdb-ast` rev `039e87c8`): `Document`, `Bytes`, `Point`, `Set`, `Mock`, `JsFunction`, `Info`, `Rebuild`, `Access`, `Sleep`, `Continue`, `Break`, `Explain`, and every `Define*` / `Remove*` / `Alter*` statement variant.

---

## 2. Record IDs — `aureline-parser/tests/cases/record_id.rs`

Lowerer: `/…/aureline-parser/src/parser/surql/lower/expr/record_id.rs`. Shape is always `RecordId{table, key}` → `(RecordId <table> <key>)`.

| Key form | verbatim `source:` | expected S-expression |
|---|---|---|
| bare string | `person:one` | `(RecordId person (KeyString one))` |
| quoted string | `person:"one"` | `(RecordId person (KeyString one))` — quotes decoded, identical to bare |
| integer | `person:123` | `(RecordId person (KeyInt 123))` |
| uuid literal | `person:u"a8f30d8b-db67-47ec-8b38-ef703e05ad1b"` | `(RecordId person (KeyUuid a8f30d8b-db67-47ec-8b38-ef703e05ad1b))` |
| generated `rand()` | `person:rand()` | `(RecordId person (KeyCall rand))` |
| generated `uuid()` | `person:uuid()` | `(RecordId person (KeyCall uuid))` |
| generated `ulid()` | `person:ulid()` | `(RecordId person (KeyCall ulid))` |
| array, scalars | `person:["tenant", 123]` | `(RecordId person (KeyArray (String "tenant") (Int 123)))` |
| array, typed literals | `person:[d"2024-01-01T00:00:00Z", u"a8f30d8b-…"]` | `(RecordId person (KeyArray (Datetime "2024-01-01T00:00:00Z") (Uuid "a8f30d8b-…")))` |
| array, function calls | `person:[time::now(), rand::uuid()]` | `(RecordId person (KeyArray (Call time::now) (Call rand::uuid)))` |
| array, params | `person:[$tenant, $id]` | `(RecordId person (KeyArray (Param tenant) (Param id)))` |
| object | `person:{ tenant: "acme", id: 1 }` | `(RecordId person (KeyObject (Prop tenant (String "acme")) (Prop id (Int 1))))` |
| object with params | `person:{ tenant: $tenant, id: $id }` | `(RecordId person (KeyObject (Prop tenant (Param tenant)) (Prop id (Param id))))` |
| exclusive range | `person:1..10` | `(RecordId person (KeyRange (KeyInt 1) (KeyInt 10)))` |
| inclusive range | `person:1..=10` | `(RecordId person (KeyRangeInclusive (KeyInt 1) (KeyInt 10)))` |
| `r"…"` prefix form | `r"person:one"` | `(RecordId person (KeyString one))` (from `casting/record.rs`) |

Contextual cases:
```
source: "$value = person:[\"tenant\", 123]"
expect: (Binary Equal (Param value) (RecordId person (KeyArray (String "tenant") (Int 123))))

source: "[person:one, person:[\"tenant\", 123]]"
expect: (List (RecordId person (KeyString one)) (RecordId person (KeyArray (String "tenant") (Int 123))))

source: "type::record('user', $id)"
expect: (Call type::record (String "user") (Param id))     // a call, NOT a RecordId

source: "person:$id"
error,                                                     // rejected by SurrealDB's parser
```

Fidelity caveats:
- `RecordIdKey::Number` is always constructed with `NumericKind::Int` and `value.value.to_string()` — the original spelling is not preserved and a non-integer record key would still be labelled `Int`.
- `RecordIdKey::Uuid` stores the **normalised** `uuid.to_string()`, unlike `Literal::Uuid` which preserves source text between the quotes.
- Range bounds: `inclusive` is derived only from `Bound::Included` on the **end**; `Bound::Excluded` and `Bound::Included` on the start are treated identically (both simply lower the key), so `>..` style skip-start record ranges lose their exclusivity.
- `RecordIdKey::Unknown(Cow<str>)` exists in the AST but is **never constructed**. Dead variant.

---

## 3. Casts — `aureline-parser/tests/cases/casting/`

Cast form is uniform: `PrefixOperator::Cast(ty)` → `ExprKind::Cast{ty: TypeExpr, expr}` rendered `(Cast <type> <expr>)`. Type lowering is the shared SurQL→`TypeExpr` pipeline (`/…/lower/ty/`), so casts get the *whole* type grammar, including parameterised and union types.

### 3.1 generic.rs
```
"<string>$this"            → (Cast (Type string) (Param this))
"<bool>\"true\""           → (Cast (Type bool) (String "true"))
"<int>\"123\""             → (Cast (Type int) (String "123"))
"<string> <int>\"123\""    → (Cast (Type string) (Cast (Type int) (String "123")))
```

### 3.2 numeric.rs
```
"<float>\"13.5\""                                  → (Cast (Type float) (String "13.5"))
"<decimal>\"13.5729484672938472938410938456\""     → (Cast (Type decimal) (String …))
"<number>\"13.5729484672938472938410938456\""      → (Cast (Type number) (String …))
```

### 3.3 datetime.rs / duration.rs / uuid.rs / regex.rs
```
"<datetime>\"2024-01-01T00:00:00Z\""   → (Cast (Type datetime) (String "2024-01-01T00:00:00Z"))
"<duration>\"1h30m\""                  → (Cast (Type duration) (String "1h30m"))
"<uuid>\"a8f30d8b-db67-47ec-8b38-ef703e05ad1b\""  → (Cast (Type uuid) (String "a8f30d8b-…"))
"<regex>\"a|b\""                       → (Cast (Type regex) (String "a|b"))
"<regex> \"a|b\" = \"a\""              → (Binary Equal (Cast (Type regex) (String "a|b")) (String "a"))
"<regex> \"col(o|ou)r\" = \"colour\""  → (Binary Equal (Cast (Type regex) (String "col(o|ou)r")) (String "colour"))
"<regex> \"((?i)col(o|ou)r|couleur)\" = \"COULEUR\""  → (Binary Equal (Cast (Type regex) (String "((?i)col(o|ou)r|couleur)")) (String "COULEUR"))
```

### 3.4 record.rs / file_table.rs — parameterised and union type targets
```
"<record>\"person:one\""                 → (Cast (RecordType) (String "person:one"))
"<record<user | person>>\"user:one\""    → (Cast (RecordType (Table user) (Table person)) (String "user:one"))
"<table>\"person\""                      → (Cast (TableType) (String "person"))
"<file>\"avatars:/profile.png\""         → (Cast (FileType) (String "avatars:/profile.png"))
```

### 3.5 collection.rs — collections, unions, and the range-normalisation workaround
```
"<array>1..=3"
→ (Cast (ArrayType (Type any)) (RangeInclusive (Int 1) (Int 3)))

"<array<int>>[\"42\", \"314\", \"271\"]"
→ (Cast (ArrayType (Type int)) (List (String "42") (String "314") (String "271")))

"<array<bool | string | float>>[\"9.1\", \"true\", 15h]"
→ (Cast (ArrayType (EitherType (Type bool) (Type string) (Type float)))
        (List (String "9.1") (String "true") (Duration 15h)))

"<set<datetime | string>>[\"2020-09-09\", \"21 Jan 2020\"]"
→ (Cast (SetType (EitherType (Type datetime) (Type string)))
        (List (String "2020-09-09") (String "21 Jan 2020")))
```
`<array>1..=3` requires an explicit fix-up: SurrealDB parses it as `Range(start = Cast<array>(1), end = 3)`. `RangeSource` in `operator.rs` detects `normalize_array_cast_start` and rewrites to `Cast(array, Range(1..=3))`, recomputing the range span as `expr.span ∪ end.span`. This only fires when the start is a prefix cast **to an array type**; `<set>1..=3` would *not* be normalised.

### 3.6 Full cast-target type surface

From `/…/lower/ty/` and the `parsers: [aureline_ty, surql_ty]` cases in `tests/cases/ty.rs` — anything expressible as a `TypeExpr` is a legal cast target:

- Builtins: `any bool bytes datetime decimal duration float int number object range regex string uuid` → `(Type <name>)`
- `none` → `(NoneType)`, `null` → `(NullType)`
- `option<string>` → `(OptionType (Type string))`; `option<int | float>` → `(OptionType (EitherType …))`; SurrealDB encodes options as a prime list starting with a synthetic `None` marker
- Unions: `string | number` → `(EitherType (Type string) (Type number))`
- `array`, `array<any>`, `array<string>`, `array<array<string>>`, `array<option<record<user>>>` → `(ArrayType …)`; bare `array`/`set` normalise to item `(Type any)`
- Fixed lengths: `array<int, 640>` → `(ArrayType (Type int) (Length 640))`; `array<array<int, 3>, 5>`; `array<record<employee>, 5>`
- `set`, `set<any>`, `set<string>`, `set<float, 10>` → `(SetType …)`
- `record`, `record<user>`, `record<person | animal>` → `(RecordType)`, `(RecordType user)`, `(RecordType (Table person) (Table animal))`
- `table`, `table<person>`, `table<person | animal>`; `file`, `file<avatar>`, `file<avatar | document>`
- `geometry`, `geometry<point|line|polygon|multipoint|multiline|multipolygon|collection>` and unions such as `geometry<polygon | multipolygon | collection>`
- Literal types: `"regular"` → `(StringLiteralType "regular")`; `9 | 1 | 1.5` → `(EitherType (NumberLiteralType 9) …)`; `true | false` → `(EitherType (BoolLiteralType true) (BoolLiteralType false))`; mixed `datetime | uuid | "N/A"`
- Object literal types: `{ error: 'RetryWithId', id: string }` → `(ObjectType (Prop error (StringLiteralType "RetryWithId")) (Prop id (Type string)))`
- Tuple literal types: `[string, int, bool]` → `(TupleType …)`; `array<{ rate: float, set_at: datetime }>`
- `TypeExpr::Custom(&str)` (Aureline type aliases, e.g. `Email`) exists but has **no SurQL source** — the `surql_ty` parser is deliberately omitted for that case.

**Cast failure mode:** if `lower.ty(...)` returns `Err` (unsupported prime type, negative/oversized container length such as `array<int, -1>`), the cast becomes `Opaque("unsupported cast")`. A defensive `Opaque("unsupported cast expression")` guards a non-`Cast` prefix reaching the cast adapter.

---

## 4. Statements and subqueries — `aureline-parser/tests/cases/subquery/`

All statement forms live in expression position: `ExprKind::Statement(Box<StatementExpr>)`, rendered `(Statement (Create …))` etc. `SELECT` is different: it renders `(Subquery (Select …))` via `ExprKind::Subquery(QueryExpr::Select)`.

### 4.1 SELECT — `subquery/select.rs`

Field-by-field mapping (`/…/lower/expr/query/select.rs` ↔ `/…/aureline-ast/src/query.rs`); **every** field of the pinned `surrealdb_ast::Select` is carried across — nothing is dropped:

| SurQL clause | AST field | Outline | Interpretation status |
|---|---|---|---|
| `SELECT *` / list | `fields: SelectFields::List` | `(Fields (All) …)` | full |
| `SELECT VALUE expr` | `SelectFields::Value` | `(Value …)` | full — note `SELECT VALUE *` degrades to `List([All])` |
| `expr AS alias` | `SelectField::Expr{alias}` | `(Alias <name> …)` | alias name is a flattened `Place` string; an indexed alias place renders `foo[]` (index expression discarded) |
| `OMIT a, b` | `omit` | `(Omit …)` | full |
| `FROM …` | `from: Vec<SelectTarget>` | `(From …)` | full; single-segment `Path` → `Table(name)`, everything else → `Expr` |
| `FROM ONLY` | `only: bool` | `(Only)` | preserved flag |
| `WITH INDEX a, b` / `WITH NOINDEX` | `with_index: IndexHint` | `(WithIndex (a) (b))` / `(WithNoIndex)` | preserved, not interpreted |
| `WHERE` | `where_` | `(Where …)` | full |
| `SPLIT [ON] …` | `split` | `(Split …)` | preserved |
| `GROUP BY …` / `GROUP ALL` | `group: SelectGroup` | `(Group …)` / `(GroupAll)` | full |
| `ORDER BY …` / `ORDER BY RAND()` | `order: OrderBy` | `(Order (OrderItem …))` / `(Order Rand)` | full; per-item `collate`, `numeric`, `direction` |
| `START [AT] …` | `start` | `(Start …)` | full |
| `LIMIT [BY] …` | `limit` | `(Limit …)` | full |
| `FETCH …` | `fetch` | `(Fetch …)` | preserved |
| `VERSION …` | `version` | `(Version …)` | **preserved, not semantically interpreted** |
| `TIMEOUT …` | `timeout` | `(Timeout …)` | **preserved, not semantically interpreted** |
| `TEMPFILES` | `tempfiles: bool` | `(Tempfiles)` | **preserved, not interpreted** |
| `EXPLAIN` / `EXPLAIN FULL` | `explain: Explain` | `(Explain Base)` / `(Explain Full)` | **preserved, not interpreted** |

Verbatim corpus highlights:
```
"(SELECT * FROM user)"                      → (Subquery (Select (Fields (All)) (From (Table user))))
"(SELECT VALUE id FROM user LIMIT 1)"       → (Subquery (Select (Value (Path id)) (From (Table user)) (Limit (Int 1))))
"(SELECT id, email FROM user:bob WHERE active = true LIMIT 5)"
     → … (From (RecordId user (KeyString bob))) (Where (Binary Equal (Path active) (Bool true))) (Limit (Int 5))
"(SELECT country, count() AS total FROM user GROUP BY country LIMIT 10)"
"(SELECT count() AS total FROM user GROUP ALL)"           → … (GroupAll)
"(SELECT * FROM user LIMIT $limit)"                       → … (Limit (Param limit))
"(SELECT * FROM type::record('user', $id))"               → (From (Call type::record (String "user") (Param id)))
"(SELECT * OMIT password, token FROM ONLY user WHERE email = $email)"
     → (Fields (All)) (Omit (Path password) (Path token)) (From (Table user)) (Only) (Where …)
"(SELECT * FROM order SPLIT ON items, tags START AT $offset LIMIT BY $limit FETCH customer, owner)"
     → … (Split (Path items) (Path tags)) (Start (Param offset)) (Limit (Param limit)) (Fetch (Path customer) (Path owner))
"(SELECT * FROM user ORDER BY name COLLATE ASC, age NUMERIC DESC, created_at)"
     → (Order (OrderItem (Path name) (Collate) (Direction Asc))
              (OrderItem (Path age) (Numeric) (Direction Desc))
              (OrderItem (Path created_at)))
"(SELECT * FROM user ORDER BY rand() LIMIT 1)"            → (Order Rand) (Limit (Int 1))
"(SELECT * FROM person:alice FETCH address VERSION $t1)"  → (Fetch (Path address)) (Version (Param t1))
"(SELECT * FROM user WITH INDEX idx_user_email, idx_user_name WHERE email = $email)"
     → (WithIndex (idx_user_email) (idx_user_name)) (Where …)
"(SELECT * FROM user WITH NOINDEX WHERE active = true)"   → (WithNoIndex) (Where …)
"(SELECT * FROM user TIMEOUT 5s TEMPFILES EXPLAIN)"       → (Timeout (Duration 5s)) (Tempfiles) (Explain Base)
"(SELECT * FROM user EXPLAIN FULL)"                       → (Explain Full)
```
Nested / correlated selects stay fully structured:
```
"(SELECT *, (SELECT * FROM events WHERE type = \"activity\" LIMIT 5) AS history FROM user)"
"(SELECT name, (SELECT VALUE name FROM user LIMIT 5) AS sample_names FROM user)"
"(SELECT *, (SELECT * FROM events WHERE host == $parent.id ORDER BY time DESC LIMIT 10) AS hosted_events FROM user)"
"(SELECT name, (SELECT VALUE name FROM user WHERE member_of = $parent.member_of) AS group_members FROM user WHERE name = \"User1\")"
"(SELECT * FROM product WHERE id IN (SELECT VALUE product_id FROM order_line WHERE customer_id = \"customer:1\"))"
"(SELECT * FROM product WHERE id NOT IN (SELECT VALUE …))"
"(SELECT * FROM product WHERE !(SELECT * FROM order_line WHERE order_line.product_id = product.id))"
```
The last one produces `(Where (Unary Not (Subquery …)))` with `order_line.product_id` lowering to `(FieldAccess product_id (Path order_line))`.

**`SelectLookup` (graph-lookup) reduction:** when a `SELECT` appears as a graph lookup, SurrealDB uses the smaller `SelectLookup` node. The lowerer fills `omit=[]`, `only=false`, `with_index=None`, `fetch=[]`, `version=None`, `timeout=None`, `tempfiles=false`, `explain=None` — documented as intentional defaults, not evidence those clauses were written.

### 4.2 LET / FOR — `subquery/let_stmt.rs`

```
"LET $user = user:one"
→ (Statement (Let user (RecordId user (KeyString one))))

"LET $email: string = string::lowercase($value)"
→ (Statement (Let email (Type string) (Call string::lowercase (Param value))))

"LET $user_id = (SELECT VALUE id FROM user WHERE email = $email LIMIT 1)"
→ (Statement (Let user_id (Subquery (Select (Value (Path id)) (From (Table user)) (Where …) (Limit (Int 1))))))

"LET $patch = { name: $name, tags: ['new'], updated_at: time::now() }"
→ (Statement (Let patch (Object (Prop name (Param name)) (Prop tags (List (String "new"))) (Prop updated_at (Call time::now)))))

"LET $user = type::record('user', $id)"
→ (Statement (Let user (Call type::record (String "user") (Param id))))

"{ LET $normalized = string::lowercase($email); RETURN $normalized; }"
→ (Block (Statement (Let normalized (Call string::lowercase (Param email)))) (Return (Param normalized)))

"FOR $item IN $items { LET $id = $item.id; RETURN $id; }"
→ (Statement (For item (Param items) (Block (Statement (Let id (FieldAccess id (Param item)))) (Return (Param id)))))
```
`LetStatement{name (no `$`), ty: Option<TypeExpr>, value}`. **If the type annotation fails to lower, the whole `LET` becomes `Opaque("unsupported let type annotation")` with `raw` = the `LET` node's source slice.** `ForStatement{name, range, body}`: SurrealDB stores the body as an expression list; the lowerer wraps it in `ExprKind::Block` and gives the block the body node's span.

### 4.3 CREATE — `subquery/create.rs`
```
"CREATE user SET name = $name, active = true RETURN AFTER"
→ (Statement (Create (Target (Table user))
                     (Set (Assign name (Param name)) (Assign active (Bool true)))
                     (Output After)))

"CREATE user CONTENT { id: $id, name: $name, created_at: time::now() } RETURN AFTER"
→ (Statement (Create (Target (Table user)) (Content (Object …)) (Output After)))

"CREATE user:bob SET email = string::lowercase($email) RETURN AFTER"
→ (Statement (Create (Target (RecordId user (KeyString bob))) (Set (Assign email (Call string::lowercase (Param email)))) (Output After)))
```
Two cases assert only that the source parses (no `expect:`) — nested subqueries and a nested `CREATE … ).id`:
```
CREATE order CONTENT {
  created_at: time::now(),
  user: (SELECT VALUE id FROM user WHERE email = "a@b.com" LIMIT 1),
  items: (SELECT VALUE id FROM product WHERE category = "books" LIMIT 5)
}

CREATE user CONTENT {
  name: "Alice",
  profile: ( CREATE profile CONTENT { bio: "Hello", avatar: "x.png" } ).id
}
```
`Create{only, targets, data, output, version, timeout}` — `only`, `version`, `timeout` are **preserved but uninterpreted**.

### 4.4 UPDATE / UPSERT — `subquery/update.rs`, `subquery/upsert.rs`
```
"UPDATE user SET active = true WHERE id = $id RETURN DIFF"
→ (Statement (Update (Target (Table user)) (Set (Assign active (Bool true))) (Where (Binary Equal (Path id) (Param id))) (Output Diff)))

"UPDATE user:bob MERGE { name: $name, updated_at: time::now() } RETURN AFTER"
→ (Statement (Update (Target (RecordId user (KeyString bob))) (Merge (Object …)) (Output After)))

"UPDATE stats:emails SET count += 1 RETURN AFTER"
→ (Statement (Update (Target (RecordId stats (KeyString emails))) (Set (AssignAdd count (Int 1))) (Output After)))

"UPSERT user:bob CONTENT { name: $name, active: true } RETURN AFTER"
"UPSERT user SET login_count += 1 WHERE id = $id RETURN AFTER"
```
`Update`/`Upsert` share the field set `{only, targets, with_index, data, where_, output, timeout, explain}`; `with_index`, `timeout`, `explain`, `only` are **preserved but uninterpreted**.

### 4.5 DELETE — `subquery/delete.rs`
```
"DELETE user WHERE active = false RETURN BEFORE"  → (Statement (Delete (Target (Table user)) (Where …) (Output Before)))
"DELETE user:bob RETURN BEFORE"                   → (Statement (Delete (Target (RecordId user (KeyString bob))) (Output Before)))
"DELETE session RETURN NONE"                      → (Statement (Delete (Target (Table session)) (Output None)))
```
`Delete{only, targets, with_index, where_, output, timeout, explain}` — no `data` clause.

### 4.6 INSERT — `subquery/insert.rs`
```
"INSERT INTO user (id, name) VALUES ($id, $name) RETURN AFTER"
→ (Statement (Insert (Into (Table user))
                     (Columns (Path id) (Path name))
                     (Values (Row (Param id) (Param name)))
                     (Output After)))

"INSERT RELATION INTO likes [{ in: $user, out: $post, created_at: time::now() }] RETURN AFTER"
→ (Statement (Insert (Relation) (Into (Table likes))
                     (Data (List (Object (Prop in (Param user)) (Prop out (Param post)) (Prop created_at (Call time::now)))))
                     (Output After)))
```
`Insert{relation, ignore, into, data, on_duplicate, output, version, timeout}`. `into` is `InsertInto::Table(name)` or `InsertInto::Param(name)` (`INTO $param`, `$` stripped). `data` is `InsertData::Expr` or `InsertData::Tuples{columns, rows}` — columns are `Place`s converted to `ExprKind::Path` (so `(Path id)`), rows are expression lists. `ignore` (`INSERT IGNORE`), `on_duplicate` (`ON DUPLICATE KEY UPDATE …` → `(OnDuplicate …)`), `version`, `timeout` are represented but have **no corpus case** and are uninterpreted.

### 4.7 RELATE — `subquery/relate.rs`
```
"RELATE $user->likes->$post SET created_at = time::now() RETURN AFTER"
→ (Statement (Relate (From (Param user)) (Through (Table likes)) (To (Param post))
                     (Set (Assign created_at (Call time::now))) (Output After)))

"RELATE user:one->follows->user:two CONTENT { since: time::now(), source: 'import' } RETURN AFTER"
→ (Statement (Relate (From (RecordId user (KeyString one))) (Through (Table follows)) (To (RecordId user (KeyString two)))
                     (Content (Object (Prop since (Call time::now)) (Prop source (String "import")))) (Output After)))
```
`Relate{only, from, through: Target, to, data, output, timeout}`. Note `through` uses the `Target` (table-or-expr) discrimination while `from`/`to` are plain expressions.

### 4.8 Shared mutation clauses (`/…/lower/expr/statement/mutation/common.rs`)

**`Data`:** `Set(Vec<Assignment>)`, `Unset(Vec<String>)`, `Content(Expr)`, `Patch(Expr)`, `Merge(Expr)`, `Replace(Expr)` → `(Set …) (Unset …) (Content …) (Patch …) (Merge …) (Replace …)`. `PATCH`, `REPLACE`, `UNSET` are represented but have no corpus case.

**`Assignment`:** `{place: Cow<str>, op, value}`. `AssignmentOp` = `Assign` (`=`, prints `Assign`), `Add` (`+=`, prints `AssignAdd`), `Subtract` (`-=`), `Extend` (SurrealDB's extend operator, kept distinct from `Add`).
The `place` is **a flattened string, not structured AST**: `Place::Field` → `"name"`, `Place::Member` → `"profile.name"`, and **`Place::Index` → the raw source slice verbatim** (e.g. `tags[0]`). This is explicitly documented as source-level syntax. Same flattening applies to `UNSET` targets; `SELECT … AS alias` uses the sibling `place_name` in `select.rs`, which instead renders an indexed place as `lhs[]` — index expression dropped.

**`Output`:** `RETURN NONE|NULL|DIFF|AFTER|BEFORE` → `(Output None|Null|Diff|After|Before)`; `RETURN field, other AS alias` → `Output::Fields(SelectFields)` → `(Output (Fields …))`.

**`Target`:** single-segment `Path` → `Target::Table(name)` → `(Table user)`; anything else (`user:ada`, `$records`, subquery, dotted path) → `Target::Expr`.

### 4.9 Statement lists in `run` blocks — `tests/cases/event.rs`

```
event email_changed on user when`$event = "UPDATE"` {
  run`
    CREATE audit SET user = $after.id, before = $before.email, after = $after.email;
    UPDATE stats:emails SET count += 1;
  `
}
```
renders `(Run (Query (Statement (Create …)) (Statement (Update …))))` — one lowered expression per `;`-separated top-level statement. Each `TopLevelExpr` that is not `TopLevelExpr::Expr` (i.e. `Transaction`/`BEGIN…COMMIT`, `USE`, `OPTION`, `KILL`, `SHOW`) becomes `Opaque("unsupported top-level query expression")` whose `raw` is the **entire snippet**, not just the offending statement.

A comma-separated expression list in `run` is a hard error:
```
run`(CREATE audit SET user = $after.id), (UPDATE stats:email SET count += 1)`   → error,
```

### 4.10 Permission payloads — `tests/cases/permissions.rs`
```
"@perm"                                          → (Attr perm)                     // no payload
"@perm`FULL`"                                    → (Attr perm (Embedded (Path FULL)))
"@perm`NONE`"                                    → (Attr perm (Embedded (None)))
"@perm`WHERE owner = $auth.id`"                  → (Attr perm (Embedded (Binary Equal (Path owner) (FieldAccess id (Param auth)))))
"@perm.select`WHERE owner = $auth.id`"           → (Attr perm::select (Embedded …))
"@perm.create`WHERE $auth.role = 'admin'`"
"@perm.update`WHERE org = $auth.org AND status != 'paid'`"
"@perm.delete`NONE`"                             → (Attr perm::delete (Embedded (None)))
"@perm.select`WHERE (SELECT VALUE id FROM membership WHERE user = $auth.id LIMIT 1) != NONE`"
"@perm.select`owner = $auth.id`"                 → error,   // WHERE is mandatory
"@perm`WHERE`"                                   → error,
```
Quirk worth preserving: `FULL` lowers to `(Path FULL)` (an ordinary path expression) while `NONE` lowers to the `(None)` literal, because both are re-parsed through the *expression* grammar.

---

## 5. Literals

`Literal` (`/…/aureline-ast/src/literal.rs`) is leaf-only; arrays/objects/ranges/record-ids are separate `ExprKind`s.

### 5.1 Scalars, booleans, nulls
`NONE` → `(None)`, `NULL` → `(Null)`, `true`/`false` → `(Bool true)` / `(Bool false)` — from `surrealdb_ast::Builtin`.

### 5.2 Numbers
`NumericLiteral{kind: Int|Float|Decimal, raw}` — **raw source spelling is preserved**, including suffixes, exponent spelling and sign. Rendering is `(Int 0)`, `(Float 1.5)`, `(Decimal 98dec)`.
```
"1.5"              → (Float 1.5)
"1.5 + 2"          → (Binary Add (Float 1.5) (Int 2))
"98dec"            → (Decimal 98dec)
"98dec = 98dec"    → (Binary Equal (Decimal 98dec) (Decimal 98dec))
"+Infinity"        → (Float +Infinity)
"-Infinity"        → (Float -Infinity)
"NaN"              → (Float NaN)
```
Integers carry a `Sign`; if the raw slice omits a minus sign the lowerer re-prefixes `-`. If the raw slice is empty it falls back to formatting the parsed value. `NumericLiteral::from_source` (used by the Aureline-side grammar) infers `Decimal` on a `dec` suffix, `Float` on `.`/`e`/`E`, else `Int`.

### 5.3 Durations, datetimes, uuids, regexes, files, strings
```
"1h30m"                                  → (Duration 1h30m)          // raw text, trimmed
"[\"x\", 15h]"                           → (List (String "x") (Duration 15h))
"d\"2024-01-01T00:00:00Z\""              → (Datetime "2024-01-01T00:00:00Z")
"d'2024-01-01T00:00:00Z'"                → (Datetime "2024-01-01T00:00:00Z")   // single quotes equivalent
"d\"2024-01-01T00:00:00Z\" = $value"     → (Binary Equal (Datetime "…") (Param value))
"u\"a8f30d8b-db67-47ec-8b38-ef703e05ad1b\""  → (Uuid "a8f30d8b-…")
"u'a8f30d8b-db67-47ec-8b38-ef703e05ad1b'"    → (Uuid "a8f30d8b-…")
"$value = u\"a8f30d8b-…\""               → (Binary Equal (Param value) (Uuid "a8f30d8b-…"))
"/[A-Z0-9]{3}/"                          → (Regex "[A-Z0-9]{3}")
"/^a.*b$/"                               → (Regex ^a.*b$)
"$value != NONE AND $value = /[A-Z0-9]{3}/"  → (Binary And (Binary NotEqual (Param value) (None)) (Binary Equal (Param value) (Regex "[A-Z0-9]{3}")))
"f\"avatars:/profile.png\""              → (File "avatars:/profile.png")
```
Normalisation rules (`atom.rs`): `prefixed_quoted_text` strips the `d`/`u` affix and the surrounding quote (either `'` or `"`) and keeps the inner text; a UUID with an empty raw slice falls back to the parsed value's `to_string()`. `Regex` uses the arena `source` string (delimiting slashes removed); `File` uses the `FileLit.path` string; `String` uses the decoded arena text (escape processing done by SurrealDB).

### 5.4 Containers
Arrays → `ExprKind::List`; objects → `ExprKind::Object(Vec<Prop{key, value}>)`; tuples from SurQL are **not** produced (SurrealDB parenthesised groups are `Covered` and are peeled). `surrealdb_ast::Expr::Set` and `Bytes` and `Point` literals are **not lowered** → `Opaque("unsupported expression")`.

### 5.5 Aureline-native literal grammar (non-SurQL) — `tests/cases/literal.rs`

This one file is *not* SurQL; it covers attribute-argument values parsed by `/…/aureline-parser/src/parser/grammar/literal.rs`, which is the only producer of `ExprKind::Ident` and `ExprKind::Tuple`:
```
source:
table User schemafull {
  email string @source(path: profile.email, fallbacks: [profile.name, account.email], pair: (owner.id, user))
}
resolve_surql: false,
expect:
  (Document (Table User (Kind Schemafull)
    (Field email (Type string)
      (Attr source
        (Named path (Path profile::email))
        (Named fallbacks (List (Path profile::name) (Path account::email)))
        (Named pair (Tuple (Path owner::id) (Ident user)))))))
```
Grammar: `literal = path | ident | string | number | bool | "[" list "]" | "(" list ")"`, dotted paths require ≥1 dot (bare identifier stays `Ident`), trailing commas allowed.

### 5.6 Attribute integration — `tests/cases/surql.rs`, `tests/cases/field.rs`
```
"email string @assert`string::is_email($value)`"
→ (Field email (Type string) (Attr assert (Embedded (Call string::is_email (Param value)))))

"display_name string @value`$value OR 'Anonymous'`"
→ (Field display_name (Type string) (Attr value (Embedded (Binary Or (Param value) (String "Anonymous")))))

"created_at datetime @default`time::now()`"
→ (Field created_at (Type datetime) (Attr default (Embedded (Call time::now))))
```

---

## 6. The `Opaque` fallback

`OpaqueExpr{kind: &'static str, raw: Cow<str>}` — `kind` is a fixed category string, `raw` is the **exact source slice** (`cx.raw_for_expr` / `raw_for_node`, falling back to the whole snippet if the span is out of range). Rendered `(Opaque <kind>)`.

Complete inventory of construction sites:

| `kind` | Source file | Triggering syntax |
|---|---|---|
| `"unsupported expression"` | `lower/expr/mod.rs:74` | any `surrealdb_ast::Expr` variant outside the dispatch table: `Document` (`@`), `Bytes`, `Point`, `Set`, `Mock` (`\|user:1..10\|`), `JsFunction`, `Info`, `Rebuild`, `Access`, `Sleep`, `Continue`, `Break`, `Explain`, and all `Define*`/`Remove*`/`Alter*` statements |
| `"unsupported path"` | `lower/expr/atom.rs` | path containing a non-`Ident` segment (version segments, e.g. `fn::foo<1.0.0>`) |
| `"unsupported prefix expression"` | `lower/expr/operator.rs` | `PrefixOperator::Negate` (`-x`) and `Positive` (`+x`) |
| `"unsupported postfix expression"` | `lower/expr/operator.rs` | any `PostfixOperator` except `Range` — i.e. `RangeSkip` (`>..`), `MethodCall`, `Call` in postfix position |
| `"unsupported cast expression"` | `lower/expr/operator.rs` | defensive: non-`Cast` prefix reaching the cast adapter |
| `"unsupported cast"` | `lower/expr/operator.rs` | cast whose target type fails to lower (unsupported prime type, negative/oversized container length) |
| `"unsupported typed closure"` | `lower/expr/control.rs` | closure with a return type (`\|$x\| -> int …`) or any typed parameter (`\|$x: int\| …`) |
| `"unsupported idiom"` | `lower/expr/access.rs` | `IdiomOperator` other than `Field`/`Index`/`Call`/`Graph`: `All` (`.*`, `[*]`), `Last` (`[$]`), `Flatten` (`...`), `Where` (`[WHERE …]`, `[? …]`), `Option` (`.?`), `Repeat` (`.@`), `Destructure` (`.{ … }`), `Reference` (`<~`), `Recurse` |
| `"unsupported graph traversal"` | `lower/expr/access.rs` | `Graph` idiom whose lookup is not lowerable: `Lookup::Any` (`->?`), `Lookup::Basic` (`->edge WHERE … LIMIT …`), and — importantly — `Lookup::Subject(Table)` with a **non-document base**, i.e. plain `person:tobie->likes->post` |
| `"unsupported call target"` | `lower/expr/access.rs` | call whose callee does not uncover to a path (`$fn()`, immediately-invoked closure, versioned path) |
| `"unsupported let type annotation"` | `lower/expr/statement/mod.rs` | `LET $x: <unsupported type> = …` — the entire `LET` becomes opaque, not just the type |
| `"unsupported top-level query expression"` | `surql/mod.rs:192` | in a `run` block: `TopLevelExpr::{Transaction, Use, Option, Kill, Show}`; `raw` is the *whole* snippet |

Design rule (`lower/from_surql.rs` doc comments, `tests/cases/AGENTS.md`): **expression** lowering is infallible and degrades to `Opaque`; **type** lowering is fallible and surfaces a `LowerError`/diagnostic instead. Tests should assert the desired shape rather than `Opaque`, so `Opaque` appears in *no* corpus expectation.

Hard parse errors (not `Opaque`) seen in the corpus: `person:$id`, `@perm.select\`owner = $auth.id\``, `@perm\`WHERE\``, comma-separated `run` expression lists, event without `run`, function without `run`, positional function params.

---

## 7. AST cross-reference

### 7.1 `aureline-ast/src/expr.rs`

| Variant | Payload | Source syntax | Outline | Producer |
|---|---|---|---|---|
| `Ident(Cow)` | name | *(none in SurQL)* — Aureline attribute grammar bare identifier | `(Ident x)` | `grammar/literal.rs` |
| `Path(Vec<Cow>)` | segments | `id`, `user.profile.name`, `string::is_email` | `(Path a::b)` | SurQL `Expr::Path`, Aureline dotted literal, `place_expr` |
| `Param(Cow)` | name w/o `$` | `$value` | `(Param value)` | SurQL `Expr::Param` |
| `Literal(Literal)` | see §5 | all scalars | varies | `atom.rs` |
| `Object(Vec<ObjectProp>)` | key/value | `{ a: 1 }` | `(Object (Prop a …))` | `object.rs` |
| `List(Vec<Expr>)` | items | `[1, 2]` | `(List …)` | `atom.rs` |
| `Tuple(Vec<Expr>)` | items | *(none in SurQL)* — `(a, b)` in Aureline attribute args | `(Tuple …)` | `grammar/literal.rs` |
| `Range(Range<Expr>)` | start/end/inclusive | `1..10`, `1..=10`, `..10`, `1..`, `..` | `(Range …)` / `(RangeInclusive …)` | `operator.rs` |
| `RecordId{table, key}` | see §2 | `user:ada` | `(RecordId user …)` | `record_id.rs` |
| `Access{base, steps}` | flat chain | `$products[0].org` | `(Access (Base …) (Index …) (Field …))` | `access.rs` |
| `FieldAccess{base, field}` | single projection | `$auth.id` | `(FieldAccess id …)` | `access.rs`, `select.rs` lookup targets |
| `MethodCall{target, method, args}` | | `$value.is_email()` | `(MethodCall is_email …)` | `access.rs` |
| `GraphField{path}` | dotted | `->product.id` (document-rooted) | `(GraphField product.id)` | `access.rs` |
| `GraphTraversal{direction, base, query}` | | `->(SELECT …)`, `x->edge:1..10` | `(GraphTraversal Out (Base …) (Select …))` | `access.rs` |
| `Call(Call{path, args})` | | `time::now()` | `(Call time::now …)` | `access.rs` |
| `Closure{params, body}` | untyped only | `\|$val\| $val IN 0..=255` | `(Closure (Param val) …)` | `control.rs` |
| `Cast{ty, expr}` | | `<string>$value` | `(Cast (Type string) …)` | `operator.rs` |
| `Unary{op, expr}` | `Not` only | `NOT x`, `!x` | `(Unary Not …)` | `operator.rs` |
| `Binary{left, op, right}` | 16 ops + `Other` | see §1.2 | `(Binary Equal …)` | `operator.rs` |
| `Block(Vec<Expr>)` | | `{ … }`, `FOR` body | `(Block …)` | `control.rs`, `statement/mod.rs` |
| `If{condition, then, otherwise}` | | `IF … { } ELSE { }` | `(If (Condition …) (Then …) (Else …))` | `control.rs` |
| `Statement(Box<StatementExpr>)` | 8 kinds | `LET`, `FOR`, mutations | `(Statement …)` | `statement/` |
| `Subquery(Box<QueryExpr>)` | `Select` only | `(SELECT …)` | `(Subquery (Select …))` | `query/select.rs` |
| `Return(Box<Expr>)` | | `RETURN x` | `(Return …)` | `control.rs` |
| `Throw(Box<Expr>)` | | `THROW "…"` | `(Throw …)` | `control.rs` |
| `Opaque(OpaqueExpr)` | kind + raw | see §6 | `(Opaque <kind>)` | 12 sites |

Supporting enums: `GraphDirection{In,Out,Both}`; `AccessStep{Field, Index, MethodCall, Graph}` (**`Graph` never constructed**); `RecordIdKey{String, Number, Uuid, Call, Array, Object, Range, Unknown}` (**`Unknown` never constructed**); `UnaryOp{Not}`; `BinaryOp{Equal, ExactEqual, NotEqual, GreaterThan, GreaterThanEqual, LessThan, LessThanEqual, And, Or, Add, Subtract, Multiply, Divide, Remainder, In, NotIn, Other}`.

### 7.2 `aureline-ast/src/query.rs`
`QueryExpr::Select(Select)` — **only one query kind exists**. `Select` fields per §4.1. Supporting: `IndexHint{NoIndex, Index(Vec<name>)}`, `Explain{Base, Full}`, `SelectFields{Value, List}`, `SelectField{All, Expr{expr, alias}}`, `SelectTarget{Table, Expr}`, `SelectGroup{All, Fields}`, `OrderBy{Rand, List}`, `OrderItem{expr, collate, numeric, direction}`, `OrderDirection{Asc, Desc}`.

### 7.3 `aureline-ast/src/statement.rs`
`StatementExpr{Let, For, Create, Update, Upsert, Delete, Insert, Relate}`. Structs per §4.2–4.7. Supporting: `InsertInto{Table, Param}`, `InsertData{Expr, Tuples{columns, rows}}`, `Target{Table, Expr}`, `Data{Set, Unset, Content, Patch, Merge, Replace}`, `Assignment{place: String, op, value}`, `AssignmentOp{Assign, Add, Subtract, Extend}`, `Output{None, Null, Diff, After, Before, Fields}`.

### 7.4 Shared (`common.rs`, `types.rs`, `literal.rs`)
`SourceSpan{start,end}` + `cover()`, `Spanned<T>` (+ `WithSpan::at`), `Ident<'src> = Spanned<Cow<str>>`, `Range<T>{start, end, inclusive}`, `Prop<'src,T>{key, value}`, `Call{path, args}`. `TypeExpr` per §3.6. `Literal` / `NumericLiteral{kind, raw}` / `NumericKind{Int,Float,Decimal}` / `NumberLiteral{negative, value}` (the latter used only by literal *types*).

---

## 8. Boundary summary

**Fully represented and semantically usable**
Params; paths; scalar literals (all 10 kinds, source spelling preserved for numbers/durations); lists; objects; ranges (all four bound shapes); record IDs (all 8 key forms incl. typed literals, calls, params, objects, ranges); function calls; method calls; flat access chains with field/index steps; the 16 mapped binary operators; logical `NOT`; casts to the entire `TypeExpr` surface incl. parameterised, union, literal, object and tuple types; untyped closures; `IF/ELSE`; blocks; `RETURN`; `THROW`; `SELECT` with fields/value/alias/omit/from/where/split/group/order/start/limit; `LET` (incl. type annotation) and `FOR`; the six mutation statements with targets, `SET/UNSET/CONTENT/PATCH/MERGE/REPLACE`, and `RETURN` outputs; nested and correlated subqueries at arbitrary depth; three graph traversal shapes.

**Preserved but not semantically interpreted** (structure kept, semantics deferred)
`SELECT`: `VERSION`, `TIMEOUT`, `TEMPFILES`, `EXPLAIN`/`EXPLAIN FULL`, `WITH INDEX`/`WITH NOINDEX`, `FETCH`, `SPLIT`, `ONLY`. Mutations: `ONLY`, `VERSION`, `TIMEOUT`, `EXPLAIN`, `WITH INDEX`, `INSERT IGNORE`, `ON DUPLICATE KEY UPDATE`, tuple `VALUES` rows. Assignment/`UNSET`/alias `place`s are flattened **strings**, and indexed places degrade to raw source (`tags[0]`) or to `lhs[]` in aliases. Graph-lookup `SelectLookup` silently defaults six clauses it cannot carry. **`BinaryOp::Other`** is the worst case in this tier: ~20 operators (`??`, `?:`, `**`, `CONTAINS*`, `*INSIDE`, `OUTSIDE`, `INTERSECTS`, `@@`, KNN family, range-skip) keep their operand tree but lose the operator identity with no raw text retained. Record-ID numeric keys lose their spelling/kind; record-ID range bound exclusivity on the start side is lost; record-ID UUIDs are normalised.

**Falls back to `Opaque` (source text preserved, structure discarded)**
Unary `-`/`+`; range-skip postfix; all exotic idiom suffixes (`.*`, `[$]`, `...`, `[WHERE …]`, `.?`, `.@`, `.{…}`, `<~`, recursion); plain `a->edge->b` graph traversal and `Lookup::Any`/`Lookup::Basic`; non-path call targets; versioned paths; typed closures; casts to untranslatable types; `LET` with an untranslatable type annotation; `BEGIN`/`COMMIT`/`USE`/`OPTION`/`KILL`/`SHOW` in `run` blocks; and every unmapped `surrealdb_ast::Expr` variant (`@` document, bytes, points, sets, mocks, JS functions, `INFO`, `SLEEP`, `BREAK`, `CONTINUE`, `REBUILD`, `ACCESS`, `EXPLAIN`, and all `DEFINE`/`REMOVE`/`ALTER` statements).

**Hard parse errors (never reach lowering)**
`person:$id`; permission payloads without `WHERE` or with an empty `WHERE`; comma-separated expression lists in `run`; declaration-shape violations (missing `run` block, positional function params).

**Dead AST variants to drop or fill in a rewrite**
`AccessStep::Graph`, `RecordIdKey::Unknown` (never constructed); `ExprKind::Ident` and `ExprKind::Tuple` (never constructed from SurQL — Aureline attribute grammar only); `TypeExpr::Custom` (no SurQL source).