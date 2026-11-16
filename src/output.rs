//! The one output shape every command renders: a header naming the source, compact lines, a cap.

#[derive(Debug, Clone, Default)]
pub struct Answer {
    /// `index`, `grep`, `fts` or `bridge:<name>`.
    pub source: String,
    /// `grammar`, `regex`, `mixed`, or `-` when nothing was found.
    pub tier: String,
    pub lines: Vec<Line>,
    /// How many lines the cap removed.
    pub cut: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The path of a sibling repository from the project (`../server`); `None` for the project.
    pub repo: Option<String>,
    pub path: String,
    pub line: u32,
    /// Empty for a grep line, which then reads like grep's own `path:line: text`.
    pub kind: String,
    pub name: String,
    /// `main` is the default and is not printed, so an unconfigured test folder is never mislabelled.
    pub scope: String,
    /// Space-joined flags, possibly empty.
    pub flags: String,
}

impl Answer {
    pub fn new(source: &str) -> Answer {
        Answer {
            source: source.into(),
            tier: "-".into(),
            ..Answer::default()
        }
    }
}

/// `path` is what is printed before `:line`; `file` is the file's own name, which the name repeats
/// for a hit on the file itself and is then left out.
fn hit(l: &Line, path: &str, file: &str) -> String {
    let flags = if l.flags.is_empty() {
        String::new()
    } else {
        format!(" {}", l.flags)
    };
    let scope = match l.scope.as_str() {
        "" | "main" => String::new(),
        scope => format!(" [{scope}]"),
    };
    if l.kind.is_empty() {
        format!("{path}:{}: {}{scope}{flags}\n", l.line, l.name)
    } else if l.name == file {
        format!("{path}:{} {}{scope}{flags}\n", l.line, l.kind)
    } else {
        format!("{path}:{} {} {}{scope}{flags}\n", l.line, l.kind, l.name)
    }
}

/// The lines as a tree of folders and files, each in the order of its best ranked line.
enum Entry<'a> {
    Dir(String, Vec<Entry<'a>>),
    File(String, Vec<&'a Line>),
}

fn insert<'a>(entries: &mut Vec<Entry<'a>>, parts: &[&str], line: &'a Line) {
    let (&first, rest) = parts.split_first().expect("a path has a file name");
    let found = entries.iter().position(|e| match e {
        Entry::Dir(name, _) => !rest.is_empty() && name == first,
        Entry::File(name, _) => rest.is_empty() && name == first,
    });
    let i = found.unwrap_or_else(|| {
        entries.push(if rest.is_empty() {
            Entry::File(first.into(), Vec::new())
        } else {
            Entry::Dir(first.into(), Vec::new())
        });
        entries.len() - 1
    });
    match &mut entries[i] {
        Entry::Dir(_, children) => insert(children, rest, line),
        Entry::File(_, lines) => lines.push(line),
    }
}

/// A folder with a single entry is joined to it, so a lone deep hit is still one line.
fn print(entries: &[Entry], prefix: &str, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    for entry in entries {
        match entry {
            Entry::Dir(name, children) if children.len() == 1 => {
                print(children, &format!("{prefix}{name}/"), depth, out);
            }
            Entry::Dir(name, children) => {
                out.push_str(&format!("{pad}{prefix}{name}/\n"));
                print(children, "", depth + 1, out);
            }
            Entry::File(name, lines) if lines.len() == 1 => {
                out.push_str(&pad);
                out.push_str(&hit(lines[0], &format!("{prefix}{name}"), name));
            }
            Entry::File(name, lines) => {
                out.push_str(&format!("{pad}{prefix}{name}\n"));
                for l in lines {
                    out.push_str(&format!("{pad}  {}", hit(l, "", name)));
                }
            }
        }
    }
}

/// ```text
/// # index (grammar)
/// <note>
/// [../sibling/]path:line kind name [scope] flags
/// folder/
///   sub/
///     file:line: grep line [scope]
///   file
///     :line kind name
/// ... N more (--all)
/// ```
/// Every folder and file is named once, so a deep path costs its tokens once. A line under a file
/// reads as the file's line with the file name left out.
pub fn render(answer: &Answer) -> String {
    let mut out = format!("# {} ({})\n", answer.source, answer.tier);
    for note in &answer.notes {
        out.push_str(note);
        out.push('\n');
    }
    if answer.lines.is_empty() {
        out.push_str("no match\n");
    }
    let mut tree = Vec::new();
    for l in &answer.lines {
        let parts: Vec<&str> = l
            .repo
            .as_deref()
            .into_iter()
            .chain(l.path.split('/'))
            .collect();
        insert(&mut tree, &parts, l);
    }
    print(&tree, "", 0, &mut out);
    if answer.cut > 0 {
        out.push_str(&format!("... {} more (--all)\n", answer.cut));
    }
    out
}

#[cfg(test)]
#[path = "../tests/unit/output.rs"]
mod tests;
