use aureline_test::aurl_test;

#[test]
fn parses_schemafull_table() {
    aurl_test!("table User schemafull {}").parses_as("(SourceFile (Table User Schemafull))");
}
