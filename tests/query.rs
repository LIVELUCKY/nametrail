//! Queries against the fixture workspace; see `common` for how it is materialised.

mod common;

use common::{lines, workspace};
use nametrail::output::render;
use nametrail::query;

#[test]
fn def_lists_project_before_siblings() {
    let f = workspace(None);
    let answer = query::def(&f.ws, "BookRoute");
    let got = lines(&answer);
    assert_eq!(
        got[0],
        "src/commonMain/kotlin/app/BookRoute.kt:12 class BookRoute [main]"
    );
    assert_eq!(
        got[1],
        "../server/src/main/kotlin/server/Routes.kt:4 class BookRoute [main]"
    );
    assert_eq!(answer.source, "index");
    assert_eq!(answer.tier, "grammar");
}

#[test]
fn uses_ranks_main_before_test_and_counts_per_file() {
    let f = workspace(None);
    let answer = query::uses(&f.ws, "BookPort", false);
    let got = lines(&answer);
    assert!(
        got[0].starts_with("src/commonMain/kotlin/app/BookRoute.kt:7 "),
        "{got:?}"
    );
    assert!(
        got.iter()
            .any(|l| l.starts_with("src/commonTest/kotlin/app/FakeBookPort.kt:3 ")),
        "{got:?}"
    );
    assert!(
        got.iter().position(|l| l.contains("commonTest"))
            > got
                .iter()
                .position(|l| l.contains("commonMain/kotlin/app/BookRoute.kt:7"))
    );
    assert_eq!(
        answer
            .lines
            .iter()
            .filter(|l| l.path.ends_with("BookRoute.kt"))
            .count(),
        1,
        "one line per file with a count"
    );
    assert!(
        answer.lines[0].flags.contains("1x") || answer.lines[0].flags.contains("x"),
        "{:?}",
        answer.lines[0].flags
    );
}

#[test]
fn impl_includes_test_fakes_last() {
    let f = workspace(None);
    let got = lines(&query::impl_(&f.ws, "BookPort"));
    assert_eq!(
        got,
        vec![
            "src/commonMain/kotlin/app/BookRoute.kt:7 class BookRepositoryImpl [main]",
            "src/commonTest/kotlin/app/FakeBookPort.kt:3 class FakeBookPort [test]"
        ]
    );
}

#[test]
fn variants_puts_expect_first_then_each_actual() {
    let f = workspace(None);
    let answer = query::variants(&f.ws, "platformName");
    let got = lines(&answer);
    assert_eq!(got.len(), 3, "{got:?}");
    assert!(
        got[0].starts_with("src/commonMain/") && answer.lines[0].flags.contains("expect"),
        "{got:?}"
    );
    assert!(
        got[1].starts_with("src/androidMain/") && got[2].starts_with("src/iosMain/"),
        "{got:?}"
    );
}

#[test]
fn link_groups_by_repository_with_enclosing_definition() {
    let f = workspace(None);
    let answer = query::link(&f.ws, "/api/v1/books/{id}");
    let got = lines(&answer);
    assert_eq!(got.len(), 3, "{got:?}");
    assert!(
        got[0].starts_with("src/commonMain/kotlin/app/BookRoute.kt:11 literal"),
        "project first: {got:?}"
    );
    assert!(
        got[1].starts_with("../server/") && got[2].starts_with("../web/"),
        "{got:?}"
    );
    assert!(answer.lines[0].flags.contains("in:BookRoute"));
    assert_eq!(
        query::link(&f.ws, "api/v1/books/{id}").lines.len(),
        3,
        "without the leading slash too"
    );
}

#[test]
fn a_lookup_routes_paths_and_names_among_words_by_themselves() {
    use nametrail::query::show::lookup;
    let f = workspace(None);
    let route = lookup(&f.ws, "api/v1/books/{id}");
    assert!(
        route.contains("BookRoute.kt:11 literal") && route.contains("../server/"),
        "{route}"
    );
    let named = lookup(&f.ws, "platformName expect declaration and implementations");
    for file in [
        "Platform.kt:3",
        "Platform.android.kt:3",
        "Platform.ios.kt:3",
    ] {
        assert!(named.contains(file), "{file}: {named}");
    }
    let words = lookup(&f.ws, "where is the book route defined");
    assert!(words.starts_with("# fts"), "{words}");
}

#[test]
fn outline_groups_by_configured_role() {
    let f = workspace(Some(r#"{"roles": {"Port": ["*Port"]}}"#));
    let answer = query::outline(&f.ws, "src/commonMain");
    let got = lines(&answer);
    assert!(
        got[0].starts_with("src/commonMain/kotlin/app/BookRoute.kt:3 Port BookPort"),
        "{got:?}"
    );
    assert!(
        got[1].starts_with("src/commonMain/kotlin/app/BookRoute.kt:7 Port BookRepositoryImpl"),
        "{got:?}"
    );
    assert!(
        got.iter().any(|l| l.contains(" class BookRoute ")),
        "{got:?}"
    );
    assert!(!got.iter().any(|l| l.contains("commonTest")));
}

#[test]
fn cap_cuts_at_the_configured_limit_and_reports_the_rest() {
    let f = workspace(None);
    let limit = f.ws.config.settings.limits.answer_lines;
    assert_eq!(limit, 25);
    let mut answer = query::def(&f.ws, "same");
    assert_eq!(answer.lines.len(), 30);
    query::cap(&mut answer, false, limit);
    assert_eq!((answer.lines.len(), answer.cut), (25, 5));
    assert!(render(&answer).contains("... 5 more (--all)"));
    let mut all = query::def(&f.ws, "same");
    query::cap(&mut all, true, limit);
    assert_eq!((all.lines.len(), all.cut), (30, 0));
    let small = workspace(Some(r#"{"settings": {"limits": {"answerLines": 3}}}"#));
    assert_eq!(
        small.ws.config.settings.limits.answer_lines, 3,
        "a repository override applies"
    );
}

#[test]
fn no_match_is_explicit() {
    let f = workspace(None);
    let answer = query::def(&f.ws, "Nope");
    assert!(answer.lines.is_empty());
    assert!(render(&answer).contains("no match"));
}

#[test]
fn render_matches_exact_format() {
    let f = workspace(None);
    let answer = query::def(&f.ws, "FakeBookPort");
    assert_eq!(
        render(&answer),
        "# index (grammar)\nsrc/commonTest/kotlin/app/FakeBookPort.kt:3 class FakeBookPort [test]\n"
    );
}

#[test]
fn uses_reports_bridged_hits_with_label() {
    let f = workspace(None);
    let answer = query::uses(&f.ws, "platformName", true);
    let swift = answer
        .lines
        .iter()
        .find(|l| l.path == "ios/App.swift")
        .expect("the Swift call site");
    assert_eq!(swift.line, 5);
    assert!(
        swift.flags.contains("bridge:kmp-swift"),
        "{:?}",
        swift.flags
    );
    let grouped = query::uses(&f.ws, "platformName", false);
    let swift = grouped
        .lines
        .iter()
        .find(|l| l.path == "ios/App.swift")
        .expect("grouped Swift line");
    assert!(
        swift.flags.contains("bridge:kmp-swift"),
        "{:?}",
        swift.flags
    );
}

#[test]
fn render_carries_sibling_prefix_notes_and_flags() {
    let f = workspace(None);
    let answer = query::variants(&f.ws, "platformName");
    let text = render(&answer);
    assert!(text.starts_with("# index (grammar)\n"));
    assert!(
        text.contains("src/\n  commonMain/kotlin/app/Platform.kt:3 function platformName expect\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "  androidMain/kotlin/app/Platform.android.kt:3 function platformName actual\n"
        ),
        "{text}"
    );
}

#[test]
fn uses_says_a_light_sibling_has_no_uses_indexed() {
    let f = workspace(None);
    let text = render(&query::uses(&f.ws, "BookRoute", false));
    assert!(
        text.contains("../server: definitions only, its uses are not indexed\n"),
        "{text}"
    );
}

#[test]
fn a_code_file_over_the_limit_is_named_when_def_finds_nothing() {
    let f = workspace(Some(
        r#"{"settings": {"limits": {"indexFileBytes": 1000}}}"#,
    ));
    let root = &f.ws.project.root;
    std::fs::write(
        root.join("src/Big.kt"),
        format!("class Hidden\n{}", "// x\n".repeat(400)),
    )
    .unwrap();
    common::git(root, &["add", "-A"]);
    common::git(root, &["commit", "-q", "-m", "big"]);
    let text = render(&query::def(&f.ws, "Hidden"));
    assert!(
        text.contains("1 code files too large to parse: src/Big.kt\n"),
        "{text}"
    );
}

#[test]
fn an_answer_without_gaps_has_no_note() {
    let f = workspace(Some(r#"{"siblings": []}"#));
    let text = render(&query::uses(&f.ws, "BookRoute", false));
    let second = text.lines().nth(1).unwrap_or_default();
    assert!(
        second.contains(".kt") || second.contains(".swift") || second.ends_with('/'),
        "the line after the header is a hit, not a note: {text}"
    );
}

fn commit(f: &common::Fixture, files: &[(&str, &str)]) {
    let root = &f.ws.project.root;
    for (path, text) in files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }
    common::git(root, &["add", "-A"]);
    common::git(root, &["commit", "-q", "-m", "files"]);
}

#[test]
fn uses_of_a_file_lists_its_users_then_theirs() {
    let f = workspace(Some(r#"{"siblings": []}"#));
    commit(
        &f,
        &[
            (
                "src/data/Store.kt",
                "package data\nclass Shelf\nclass Ledger { fun inner() {} }\n",
            ),
            (
                "src/ui/Screen.kt",
                "package ui\nimport data.Shelf\nclass Screen(val s: Shelf)\n",
            ),
            (
                "src/data/Same.kt",
                "package data\nclass Same(val l: Ledger)\n",
            ),
            (
                "src/app/Main.kt",
                "package app\nimport ui.Screen\nfun main() { Screen(TODO()) }\n",
            ),
        ],
    );
    let text = render(&query::uses(&f.ws, "Store.kt", false));
    assert!(text.contains("file: src/data/Store.kt\n"), "{text}");
    assert!(text.contains("Screen.kt:3 uses Shelf"), "{text}");
    assert!(
        text.contains("Same.kt:2 uses Ledger"),
        "same-package user without an import: {text}"
    );
    assert!(
        text.contains("Main.kt:3 uses Screen via:ui/Screen.kt"),
        "{text}"
    );
    assert!(
        !text.contains("Store.kt:"),
        "the file is not its own user: {text}"
    );
}

#[test]
fn an_import_from_another_package_does_not_count() {
    let f = workspace(Some(r#"{"siblings": []}"#));
    commit(
        &f,
        &[
            ("src/data/store.ts", "export class Shelf {}\n"),
            ("src/other/shelf.ts", "export class Shelf {}\n"),
            (
                "src/ui/wrong.ts",
                "import { Shelf } from \"../other/shelf\";\nexport const w = new Shelf();\n",
            ),
            (
                "src/ui/right.ts",
                "import { Shelf } from \"../data/store\";\nexport const r = new Shelf();\n",
            ),
        ],
    );
    let text = render(&query::uses(&f.ws, "src/data/store.ts", false));
    assert!(text.contains("right.ts:1 uses Shelf"), "{text}");
    assert!(!text.contains("wrong.ts"), "{text}");
}

#[test]
fn only_a_whole_file_name_is_a_file_and_a_name_stays_a_name() {
    let f = workspace(Some(r#"{"siblings": []}"#));
    commit(
        &f,
        &[
            ("src/BookRepo.kt", "class BookRepo\n"),
            ("src/Use.kt", "val r = BookRepo()\n"),
        ],
    );
    assert!(query::uses(&f.ws, "Repo.kt", false).lines.is_empty());
    let text = render(&query::uses(&f.ws, "BookRepo", false));
    assert!(!text.contains("file: "), "{text}");
}

#[test]
fn a_file_without_definitions_says_so() {
    let f = workspace(Some(r#"{"siblings": []}"#));
    commit(&f, &[("scripts/run.kt", "// nothing here\n")]);
    let text = render(&query::uses(&f.ws, "scripts/run.kt", false));
    assert!(
        text.contains("no definitions in scripts/run.kt\n"),
        "{text}"
    );
}

#[test]
fn a_private_definition_has_no_users_in_other_files() {
    let f = workspace(Some(r#"{"siblings": []}"#));
    commit(
        &f,
        &[
            (
                "src/data/Store.kt",
                "package data\nclass Shelf\nprivate fun insert() {}\n",
            ),
            (
                "src/ui/Other.kt",
                "package ui\nfun go(m: MutableMap<Int, Int>) { insert() }\n",
            ),
        ],
    );
    let text = render(&query::uses(&f.ws, "src/data/Store.kt", false));
    assert!(!text.contains("Other.kt"), "{text}");
}

#[test]
fn a_path_as_printed_resolves_with_dot_slash_or_sibling_prefix() {
    let f = workspace(None);
    commit(
        &f,
        &[
            ("src/data/Store.kt", "package data\nclass Shelf\n"),
            ("src/ui/Use.kt", "package data\nval s = Shelf()\n"),
        ],
    );
    let file_mode = |arg: &str| {
        let answer = query::uses(&f.ws, arg, false);
        !answer.lines.is_empty() && answer.lines.iter().all(|l| l.kind == "uses")
    };
    assert!(file_mode("src/data/Store.kt"), "baseline");
    assert!(file_mode("./src/data/Store.kt"));
    assert!(file_mode("../server/src/main/kotlin/server/Routes.kt"));
}

#[test]
fn a_bare_word_stays_a_name_even_when_an_extensionless_file_has_it() {
    let f = workspace(Some(r#"{"siblings": []}"#));
    commit(
        &f,
        &[
            ("tools/Shelf", "#!/bin/sh\necho hi\n"),
            ("src/Shelf.kt", "class Shelf\n"),
            ("src/Use.kt", "val s = Shelf()\n"),
        ],
    );
    let text = render(&query::uses(&f.ws, "Shelf", false));
    assert!(text.contains("Use.kt"), "{text}");
    assert!(!text.contains("no definitions"), "{text}");
}

#[test]
fn the_too_large_note_shows_only_when_uses_finds_nothing() {
    let f = workspace(Some(
        r#"{"siblings": [], "settings": {"limits": {"indexFileBytes": 1000}}}"#,
    ));
    commit(
        &f,
        &[(
            "src/Big.kt",
            &format!("class Hidden\n{}", "// x\n".repeat(400)),
        )],
    );
    let found = render(&query::uses(&f.ws, "BookRoute", false));
    assert!(!found.contains("too large"), "{found}");
    let missed = render(&query::uses(&f.ws, "Hidden", false));
    assert!(
        missed.contains("1 code files too large to parse: src/Big.kt"),
        "{missed}"
    );
}
