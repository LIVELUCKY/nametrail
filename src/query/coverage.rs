//! Notes for what an answer could not look at: a sibling not indexed yet, a sibling indexed without
//! its references, code files too large to parse. Added only when there is such a gap.

use crate::index::parse::Language;
use crate::index::store::Store;
use crate::output::Answer;
use crate::workspace::Workspace;

/// `references`: the answer is about uses, which a light sibling does not hold. `found`: the answer
/// found what it was asked for; only when it did not are files too large to parse named, as the
/// reason it may have missed.
pub(crate) fn add(
    answer: &mut Answer,
    ws: &Workspace,
    opened: &[(Option<String>, Store)],
    references: bool,
    found: bool,
) {
    let mut notes = Vec::new();
    let mut too_large = Vec::new();
    let limit = ws.config.settings.limits.index_file_bytes as u64;
    for (repo, store) in opened {
        if let Some(name) = repo {
            if store.files().map(|f| f.is_empty()).unwrap_or(true) {
                notes.push(format!("{name}: not indexed yet"));
                continue;
            }
            if references && store.light_files().unwrap_or(0) > 0 {
                notes.push(format!(
                    "{name}: definitions only, its uses are not indexed"
                ));
            }
        }
        if found {
            continue;
        }
        let root = match repo {
            None => &ws.project.root,
            Some(name) => match ws.siblings.iter().find(|s| &s.name == name) {
                Some(sibling) => &sibling.root,
                None => continue,
            },
        };
        for (path, language) in store.empty_blobs().unwrap_or_default() {
            let code = Language::from_name(&language).is_some();
            let size = std::fs::metadata(root.join(&path)).map(|m| m.len());
            if code && size.is_ok_and(|s| s > limit) {
                let shown = match repo {
                    Some(name) => format!("{name}/{path}"),
                    None => path,
                };
                too_large.push(shown);
            }
        }
    }
    if !too_large.is_empty() {
        let mut listed = too_large[..too_large.len().min(3)].join(", ");
        if too_large.len() > 3 {
            listed.push_str(", ...");
        }
        notes.push(format!(
            "{} code files too large to parse: {listed}",
            too_large.len()
        ));
    }
    answer.notes.extend(notes);
}
