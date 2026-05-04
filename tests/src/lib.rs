#[test]
#[cfg(not(feature = "dynamic-state"))]
fn dynamic_state_off_dsl_errors() {
    let t = trybuild::TestCases::new();
    t.compile_fail("compile_fail/without_feature/*.rs");
}

#[test]
#[cfg(feature = "dynamic-state")]
fn dynamic_state_dsl_errors() {
    let t = trybuild::TestCases::new();
    t.compile_fail("compile_fail/with_feature/*.rs");
}
