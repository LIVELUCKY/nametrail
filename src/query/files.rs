//! `uses <file>`: the files that depend on a file through its top-level definitions, then the files
//! that depend on those. Name based, so it works in every indexed language and also finds users
//! that never import (same-package Kotlin, Swift modules).

use std::collections::{BTreeMap, BTreeSet};

use super::{Located, coverage, line, name_users, rank, tier_of};
use crate::index::store::Store;
use crate::model::{Hit, Role};
use crate::output::Answer;
use crate::workspace::Workspace;

#[cfg(test)]
#[path = "../../tests/unit/query/files.rs"]
mod tests;

type Opened = [(Option<String>, Store)];
/// A file of the workspace: its repository (`None` for the project) and path.
type File = (Option<String>, String);

/// Second level files listed at most.
const SECOND_LEVEL: usize = 10;
/// First level files whose users are looked up for the second level: the best ranked ones.
const SECOND_LEVEL_FROM: usize = 20;

/// `None` when `arg` names no indexed file, so `uses` looks it up as a name.
pub(crate) fn users(ws: &Workspace, opened: &Opened, arg: &str) -> Option<Answer> {
    // A bare word is a name, even when an extensionless file (`bin/tool`, `Makefile`) carries it.
    if !arg.contains(['.', '/']) {
        return None;
    }
    let arg = arg.strip_prefix("./").unwrap_or(arg);
    let (target, by_suffix) = match resolve(opened, arg) {
        Resolved::None => return None,
        Resolved::Many(paths) => {
            let mut answer = Answer::new("index");
            let shown = paths[..paths.len().min(5)].join(", ");
            answer
                .notes
                .push(format!("{arg} matches {} files: {shown}", paths.len()));
            return Some(answer);
        }
        Resolved::One(file, by_suffix) => (file, by_suffix),
    };
    let mut answer = Answer::new("index");
    if by_suffix {
        answer.notes.push(format!("file: {}", shown(&target)));
    }
    let defs = top_level(opened, &target);
    if defs.is_empty() {
        answer
            .notes
            .push(format!("no definitions in {}", shown(&target)));
        return Some(answer);
    }

    // A name used in many files is looked up once per call.
    let mut cache = BTreeMap::new();
    let first = users_of(ws, opened, &target, &defs, &mut cache);
    let first_files: BTreeSet<File> = first.iter().map(|u| u.file.clone()).collect();

    // Second level: who uses the first level files, counted by how many of them each one uses.
    let mut second: BTreeMap<File, (User, usize)> = BTreeMap::new();
    for user in first.iter().take(SECOND_LEVEL_FROM) {
        let defs = top_level(opened, &user.file);
        // The folder too: `mod.rs` or `index.ts` alone does not say which file.
        let via = match user.file.1.rsplitn(3, '/').collect::<Vec<_>>().as_slice() {
            [file, folder, ..] => format!("{folder}/{file}"),
            _ => user.file.1.clone(),
        };
        for mut next in users_of(ws, opened, &user.file, &defs, &mut cache) {
            if next.file == target || first_files.contains(&next.file) {
                continue;
            }
            match second.get_mut(&next.file) {
                Some((_, n)) => *n += 1,
                None => {
                    next.hit.1.symbol.flags = vec![format!("via:{via}")];
                    second.insert(next.file.clone(), (next, 1));
                }
            }
        }
    }
    let mut second: Vec<(User, usize)> = second.into_values().collect();
    second.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    second.truncate(SECOND_LEVEL);

    let hits: Vec<Located> = first
        .into_iter()
        .chain(second.into_iter().map(|(u, _)| u))
        .map(|u| u.hit)
        .collect();
    answer.tier = tier_of(&hits);
    answer.lines = hits.iter().map(|(repo, hit)| line(repo, hit)).collect();
    let found = !answer.lines.is_empty();
    coverage::add(&mut answer, ws, opened, true, found);
    Some(answer)
}

enum Resolved {
    One(File, bool),
    Many(Vec<String>),
    None,
}

/// An exact path, else the one path ending in `/arg`; the project first, then the siblings.
fn resolve(opened: &Opened, arg: &str) -> Resolved {
    let ending = format!("/{arg}");
    let mut by_suffix = Vec::new();
    for (repo, store) in opened {
        for row in store.files().unwrap_or_default() {
            if row.path == arg || shown(&(repo.clone(), row.path.clone())) == arg {
                return Resolved::One((repo.clone(), row.path), false);
            }
            if row.path.ends_with(&ending) {
                by_suffix.push((repo.clone(), row.path));
            }
        }
    }
    match by_suffix.len() {
        0 => Resolved::None,
        1 => Resolved::One(by_suffix.remove(0), true),
        _ => Resolved::Many(by_suffix.iter().map(shown).collect()),
    }
}

fn shown(file: &File) -> String {
    match &file.0 {
        Some(repo) => format!("{repo}/{}", file.1),
        None => file.1.clone(),
    }
}

/// The definitions of a file that are not inside another definition of it, and not private where
/// the language records that (Kotlin today): another file cannot use those.
fn top_level(opened: &Opened, file: &File) -> Vec<Hit> {
    let Some((_, store)) = opened.iter().find(|(repo, _)| *repo == file.0) else {
        return Vec::new();
    };
    let defs: Vec<Hit> = store
        .under(&file.1)
        .unwrap_or_default()
        .into_iter()
        .filter(|h| h.path == file.1 && h.symbol.role == Role::Definition)
        .filter(|h| {
            !h.symbol
                .flags
                .iter()
                .any(|f| f == "private" || f == "fileprivate")
        })
        .collect();
    let inside = |d: &Hit, o: &Hit| {
        o.symbol.start_line <= d.symbol.start_line
            && d.symbol.end_line <= o.symbol.end_line
            && (o.symbol.start_line, o.symbol.end_line) != (d.symbol.start_line, d.symbol.end_line)
    };
    defs.iter()
        .filter(|d| !defs.iter().any(|o| inside(d, o)))
        .cloned()
        .collect()
}

/// One user file: its first hit (rank order) carrying the used names, and how many hits it has.
struct User {
    file: File,
    hit: Located,
}

/// The files other than `target` that use any of `defs`, one per file, in rank order. The hit's
/// kind becomes `uses`, its name the names used, its flags the hit count.
fn users_of(
    ws: &Workspace,
    opened: &Opened,
    target: &File,
    defs: &[Hit],
    cache: &mut BTreeMap<String, Vec<Located>>,
) -> Vec<User> {
    let mut hits: Vec<Located> = Vec::new();
    let mut seen = BTreeSet::new();
    for def in defs {
        if !seen.insert(def.symbol.name.clone()) {
            continue;
        }
        let found = cache
            .entry(def.symbol.name.clone())
            .or_insert_with(|| name_users(ws, opened, &def.symbol.name))
            .clone();
        // A file that imports this name from somewhere else means that other definition.
        let elsewhere: BTreeSet<File> = found
            .iter()
            .filter(|(_, h)| {
                h.symbol.kind == "import"
                    && h.symbol
                        .flags
                        .first()
                        .is_some_and(|p| import_points_elsewhere(p, &target.1))
            })
            .map(|(repo, h)| (repo.clone(), h.path.clone()))
            .collect();
        for (repo, mut hit) in found {
            let file = (repo.clone(), hit.path.clone());
            if file == *target || elsewhere.contains(&file) {
                continue;
            }
            hit.symbol.name = def.symbol.name.clone();
            hits.push((repo, hit));
        }
    }
    rank(&mut hits);
    let mut order: Vec<File> = Vec::new();
    let mut per_file: BTreeMap<File, (Located, Vec<String>, usize)> = BTreeMap::new();
    for (repo, hit) in hits {
        let file = (repo.clone(), hit.path.clone());
        match per_file.get_mut(&file) {
            Some((_, names, count)) => {
                *count += 1;
                if !names.contains(&hit.symbol.name) {
                    names.push(hit.symbol.name.clone());
                }
            }
            None => {
                order.push(file.clone());
                let names = vec![hit.symbol.name.clone()];
                per_file.insert(file, ((repo, hit), names, 1));
            }
        }
    }
    order
        .into_iter()
        .map(|file| {
            let ((repo, mut hit), names, count) = per_file.remove(&file).expect("inserted above");
            hit.symbol.kind = "uses".into();
            hit.symbol.name = names.join(", ");
            hit.symbol.flags = vec![format!("{count}x")];
            User {
                file,
                hit: (repo, hit),
            }
        })
        .collect()
}

/// Whether an import's module path names some other file or package than `file`. `false` when it
/// cannot be judged, so an unclear import keeps its hit.
fn import_points_elsewhere(import_path: &str, file: &str) -> bool {
    let split = |s: &str| -> Vec<String> {
        s.split(['/', '.', ':'])
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect()
    };
    let wanted: Vec<String> = split(import_path)
        .into_iter()
        .filter(|p| !matches!(p.as_str(), "crate" | "super" | "self"))
        .collect();
    if wanted.is_empty() {
        return false;
    }
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    let have = split(stem);
    !have.windows(wanted.len()).any(|w| w == wanted.as_slice())
}
