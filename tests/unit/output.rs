use super::*;

fn line(repo: Option<&str>, path: &str, number: u32, name: &str) -> Line {
    Line {
        repo: repo.map(str::to_string),
        path: path.into(),
        line: number,
        kind: "class".into(),
        name: name.into(),
        scope: "main".into(),
        flags: String::new(),
    }
}

#[test]
fn render_without_lines_says_no_match() {
    let answer = Answer::new("index");
    assert_eq!(render(&answer), "# index (-)\nno match\n");
}

#[test]
fn render_prefixes_sibling_lines_and_joins_flags() {
    let mut answer = Answer::new("index");
    answer.tier = "grammar".into();
    answer.notes.push("a note".into());
    let mut a = line(Some("../server"), "src/A.kt", 4, "A");
    a.flags = "sealed data".into();
    answer.lines.push(a);
    answer.cut = 2;
    assert_eq!(
        render(&answer),
        "# index (grammar)\na note\n../server/src/A.kt:4 class A sealed data\n... 2 more (--all)\n"
    );
}

#[test]
fn only_a_scope_other_than_main_is_printed_and_grep_lines_read_like_grep() {
    let mut test = line(None, "src/ATest.kt", 3, "ATest");
    test.scope = "test".into();
    let mut grep = line(None, "lib/b.rs", 7, "let b = 1;");
    grep.kind = String::new();
    let mut answer = Answer::new("grep");
    answer.lines = vec![test, grep];
    assert_eq!(
        render(&answer),
        "# grep (-)\nsrc/ATest.kt:3 class ATest [test]\nlib/b.rs:7: let b = 1;\n"
    );
}

#[test]
fn lines_in_one_folder_name_it_once() {
    let mut answer = Answer::new("index");
    answer.lines = vec![
        line(None, "src/deep/app/A.kt", 1, "A"),
        line(None, "src/deep/app/B.kt", 2, "B"),
        line(Some("../web"), "src/deep/app/C.ts", 3, "C"),
        line(None, "README.md", 4, "D"),
        line(None, "README.md", 5, "E"),
    ];
    assert_eq!(
        render(&answer),
        "# index (-)\n\
         src/deep/app/\n  A.kt:1 class A\n  B.kt:2 class B\n\
         ../web/src/deep/app/C.ts:3 class C\n\
         README.md\n  :4 class D\n  :5 class E\n",
        "another repository is another folder; a file with two lines is named once"
    );
}

#[test]
fn a_folder_is_named_once_even_when_its_lines_are_apart_and_subfolders_nest() {
    let mut answer = Answer::new("index");
    answer.lines = vec![
        line(None, "src/query/a.rs", 1, "A"),
        line(None, "tests/t.rs", 2, "T"),
        line(None, "src/query/b.rs", 3, "B"),
        line(None, "src/agent/hook/c.rs", 4, "C"),
        line(None, "src/agent/hook/d.rs", 5, "D"),
        line(None, "src/e.rs", 6, "E"),
    ];
    assert_eq!(
        render(&answer),
        "# index (-)\n\
         src/\n\
         \x20 query/\n    a.rs:1 class A\n    b.rs:3 class B\n\
         \x20 agent/hook/\n    c.rs:4 class C\n    d.rs:5 class D\n\
         \x20 e.rs:6 class E\n\
         tests/t.rs:2 class T\n",
        "folders keep the order of their best line; a folder with one entry joins it"
    );
}

#[test]
fn a_name_that_repeats_its_file_name_is_left_out() {
    let mut answer = Answer::new("fts");
    let mut readme = line(None, "README.md", 1, "README.md");
    readme.kind = "text".into();
    answer.lines = vec![readme];
    assert_eq!(render(&answer), "# fts (-)\nREADME.md:1 text\n");
}
