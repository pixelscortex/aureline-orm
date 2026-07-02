use aureline_lang::inspect_schema_name;

fn main() {
    let result = inspect_schema_name("aureline");
    println!(
        "Aureline schema '{}' has {} diagnostics.",
        result.schema.name,
        result.diagnostics.len()
    );
}
