use aureline_checker::{Findings, TypeResolution};

#[derive(Debug, PartialEq, Eq)]
enum Finding {
    Root,
    Independent,
    Warning,
}

#[test]
fn reporting_keeps_root_and_independent_findings_in_fifo_order() {
    let mut findings = Findings::new();
    let proof = findings.report(Finding::Root);
    let dependent: TypeResolution<()> = TypeResolution::Invalid(proof);
    assert!(matches!(dependent, TypeResolution::Invalid(_)));
    let unknown: TypeResolution<()> = TypeResolution::Unknown;
    assert!(matches!(unknown, TypeResolution::Unknown));
    findings.report(Finding::Independent);

    assert_eq!(findings.as_slice(), &[Finding::Root, Finding::Independent]);
    assert!(!findings.generation_allowed());
}

#[test]
fn warning_only_reporting_keeps_generation_open_and_repeats_values() {
    let mut findings = Findings::new();
    findings.report_warning(Finding::Warning);
    findings.report_warning(Finding::Warning);

    assert_eq!(findings.as_slice(), &[Finding::Warning, Finding::Warning]);
    assert!(findings.generation_allowed());
}
