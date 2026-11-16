use super::*;

#[test]
fn an_import_path_is_judged_against_the_file() {
    assert!(!import_points_elsewhere(
        "data",
        "src/main/kotlin/data/Store.kt"
    ));
    assert!(import_points_elsewhere(
        "other",
        "src/main/kotlin/data/Store.kt"
    ));
    assert!(!import_points_elsewhere("./api", "web/src/api.ts"));
    assert!(import_points_elsewhere(
        "react-native",
        "web/src/Button.tsx"
    ));
    assert!(!import_points_elsewhere("crate::output", "src/output.rs"));
    assert!(
        !import_points_elsewhere("..", "src/output.rs"),
        "nothing to judge keeps the hit"
    );
}
