use aureline_checker::{Findings, TypeResolution};
use aureline_test::aurl_test;
use serde::Serialize;

#[derive(Serialize)]
enum Finding {
    Root,
    Independent,
    Warning,
}

#[derive(Serialize)]
struct ReportView {
    findings: Vec<Finding>,
    dependent_invalid: bool,
    unknown_distinct: bool,
    generation_allowed: bool,
}

#[test]
fn reporting_keeps_root_and_independent_findings_in_fifo_order() {
    aurl_test!("table User schemafull {} ").reports_with(
        |ast| {
            assert_eq!(ast.root().tables().len(), 1);

            let mut findings = Findings::new();
            let proof = findings.report(Finding::Root);
            let dependent: TypeResolution<()> = TypeResolution::Invalid(proof);
            let dependent_invalid = matches!(dependent, TypeResolution::Invalid(_));
            let unknown: TypeResolution<()> = TypeResolution::Unknown;
            let unknown_distinct = matches!(unknown, TypeResolution::Unknown);
            findings.report(Finding::Independent);
            let generation_allowed = findings.generation_allowed();

            ReportView {
                findings: findings.into_vec(),
                dependent_invalid,
                unknown_distinct,
                generation_allowed,
            }
        },
        "(ReportView Root Independent true true false)",
    );
}

#[test]
fn warning_only_reporting_keeps_generation_open_and_repeats_values() {
    aurl_test!("table User schemafull {}").reports_with(
        |_| {
            let mut findings = Findings::new();
            findings.report_warning(Finding::Warning);
            findings.report_warning(Finding::Warning);
            let generation_allowed = findings.generation_allowed();

            ReportView {
                findings: findings.into_vec(),
                dependent_invalid: false,
                unknown_distinct: true,
                generation_allowed,
            }
        },
        "(ReportView Warning Warning false true true)",
    );
}
