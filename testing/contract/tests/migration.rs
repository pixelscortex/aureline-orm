//! Exact offline migration contracts and target-enforcement rejection cases.
//!
//! The DDL assertions intentionally compare complete artifacts: ordering,
//! canonical type spelling, and generated assertions are part of the migration
//! contract. Error fixtures pin cases where a target cannot faithfully encode
//! a checked type, so generation fails before an unsafe artifact is emitted.

use aureline_test::aurl_test;

#[test]
fn composite_id_preserves_member_order_and_exact_arity() {
    aurl_test!("table item schemafull {\n  id [datetime, uuid, int, string]\n}")
        .compiles()
        .ddl(concat!(
            "DEFINE TABLE item TYPE NORMAL SCHEMAFULL;\n\n",
            "DEFINE FIELD id ON TABLE item TYPE [datetime, uuid, int, string];\n",
        ));
}

#[test]
fn empty_schema_has_no_migration_script() {
    aurl_test!("").compiles().ddl("");
}

#[test]
fn tables_precede_fields_in_exact_case_sensitive_identity_order() {
    aurl_test!("table zoo schemaless {\n Zebra int\n apple string\n}\ntable Alpha schemafull {\n Link record<zoo>\n}")
        .compiles()
        .ddl(concat!(
            "DEFINE TABLE Alpha TYPE NORMAL SCHEMAFULL;\n",
            "DEFINE TABLE zoo TYPE NORMAL SCHEMALESS;\n\n",
            "DEFINE FIELD Link ON TABLE Alpha TYPE record<zoo>;\n",
            "DEFINE FIELD Zebra ON TABLE zoo TYPE int;\n",
            "DEFINE FIELD apple ON TABLE zoo TYPE string;\n",
        ));
}

#[test]
fn nested_types_preserve_options_and_exact_array_lengths() {
    aurl_test!("table item schemafull {\n exact array<STRING, 2>\n maybe option<string>\n nested array<array<int, 0>, 2>\n unique set<int>\n}")
        .compiles()
        .ddl(concat!(
            "DEFINE TABLE item TYPE NORMAL SCHEMAFULL;\n\n",
            "DEFINE FIELD exact ON TABLE item TYPE array<string, 2>;\n",
            "DEFINE FIELD maybe ON TABLE item TYPE option<string>;\n",
            "DEFINE FIELD nested ON TABLE item TYPE array<array<int, 0>, 2>;\n",
            "DEFINE FIELD unique ON TABLE item TYPE set<int>;\n",
        ));
}

#[test]
fn sized_sets_enforce_maximum_distinct_count_without_native_exact_size() {
    aurl_test!("table item schemafull {\n bounded set<int, 2>\n empty set<string, 0>\n maybe option<set<int, 2>>\n nested array<set<int, 2>>\n}")
        .compiles()
        .ddl(concat!(
            "DEFINE TABLE item TYPE NORMAL SCHEMAFULL;\n\n",
            "DEFINE FIELD bounded ON TABLE item TYPE set<int> ASSERT set::len($value) <= 2;\n",
            "DEFINE FIELD empty ON TABLE item TYPE set<string> ASSERT set::len($value) <= 0;\n",
            "DEFINE FIELD maybe ON TABLE item TYPE option<set<int>> ASSERT ($value = NONE OR (set::len($value) <= 2));\n",
            "DEFINE FIELD nested ON TABLE item TYPE array<set<int>> ASSERT array::all($value, |$aurl_item_0| set::len($aurl_item_0) <= 2);\n",
        ));
}

#[test]
fn uncertain_sized_set_union_blocks_the_entire_first_migration() {
    let ast = aureline_parser::parse(
        "table item schemafull {\n valid string\n uncertain set<int, 2> | int\n}",
    )
    .unwrap();
    let checked = aureline_checker::check(&ast).into_checked().unwrap();
    assert_eq!(
        aureline_migration::generate(&checked, None).unwrap_err(),
        vec![aureline_migration::GenerationError {
            table: "item".into(),
            field: "uncertain".into(),
            reason: aureline_migration::TargetError::UnsupportedBoundedSetUnion,
        }],
    );
}

#[test]
fn nested_set_bound_beyond_target_integer_range_blocks_generation() {
    let ast = aureline_parser::parse(
        "table item schemafull {\n nested array<set<int, 9223372036854775808>>\n}",
    )
    .unwrap();
    let checked = aureline_checker::check(&ast).into_checked().unwrap();
    assert_eq!(
        aureline_migration::generate(&checked, None).unwrap_err(),
        vec![aureline_migration::GenerationError {
            table: "item".into(),
            field: "nested".into(),
            reason: aureline_migration::TargetError::UnsupportedSetBound {
                max_distinct: 9_223_372_036_854_775_808,
            },
        }],
    );
}

#[test]
fn maximum_supported_set_bound_remains_an_inclusive_maximum() {
    aurl_test!("table item schemafull {\n bounded set<int, 9223372036854775807>\n}")
        .compiles()
        .ddl(concat!(
            "DEFINE TABLE item TYPE NORMAL SCHEMAFULL;\n\n",
            "DEFINE FIELD bounded ON TABLE item TYPE set<int> ASSERT set::len($value) <= 9223372036854775807;\n",
        ));
}

#[test]
fn first_snapshot_preserves_composite_key_and_sized_set_contract() {
    let ast = aureline_parser::parse(
        "table item schemafull {\n tags set<string, 2>\n id [datetime, uuid, int, string]\n}",
    )
    .unwrap();
    let checked = aureline_checker::check(&ast).into_checked().unwrap();
    let generated = aureline_migration::generate(&checked, None).unwrap();
    let snapshot = generated.snapshot;
    assert_eq!(snapshot.id().len(), 16);
    assert!(
        snapshot
            .id()
            .chars()
            .all(|ch| "23456789abcdefghjkmnpqrstuvwxyz".contains(ch))
    );
    assert_eq!(
        snapshot.to_json(),
        format!(
            r#"{{
  "version": 1,
  "id": "{}",
  "prevIds": [],
  "entities": [
    {{
      "entityType": "table",
      "name": "item",
      "schemaMode": "schemafull"
    }},
    {{
      "entityType": "field",
      "table": "item",
      "name": "id",
      "type": "[datetime, uuid, int, string]",
      "recordKey": true
    }},
    {{
      "entityType": "field",
      "table": "item",
      "name": "tags",
      "type": "set<string, 2>"
    }}
  ]
}}
"#,
            snapshot.id()
        )
    );
    assert_eq!(
        aureline_migration::Snapshot::from_json(&snapshot.to_json()).unwrap(),
        snapshot
    );
    assert!(
        aureline_migration::Snapshot::from_json(&snapshot.to_json().replacen(
            "\"version\": 1",
            "\"version\": 2",
            1
        ))
        .is_err()
    );
}
