use super::ast::{Expr, Predicate, TextField, TextMatchMode, TextMatcher};
use crate::entry::FsEntry;
use std::path::Path;

pub fn matches(entry: &FsEntry, expr: &Expr) -> bool {
    match expr {
        Expr::Predicate(p) => matches_predicate(entry, p),
        Expr::Not(inner) => !matches(entry, inner),
        Expr::And(parts) => parts.iter().all(|part| matches(entry, part)),
        Expr::Or(parts) => parts.iter().any(|part| matches(entry, part)),
    }
}

fn matches_predicate(entry: &FsEntry, pred: &Predicate) -> bool {
    match pred {
        Predicate::Hidden(v) => entry.hidden == *v,
        Predicate::Readonly(v) => entry.read_only == *v,
        Predicate::Text { field, matcher } => match field {
            TextField::Name => match_text(&entry.name, matcher),
            TextField::Filename => {
                (entry.kind == "file" || entry.kind == "link") && match_text(&entry.name, matcher)
            }
            TextField::Folder => {
                let parent = Path::new(&entry.path)
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                match_text(&parent, matcher)
            }
            TextField::Path => match_text(&entry.path, matcher),
        },
    }
}

fn match_text(value: &str, matcher: &TextMatcher) -> bool {
    let value_lc = value.to_lowercase();
    let needle_lc = matcher.raw.to_lowercase();
    match matcher.mode {
        TextMatchMode::Contains => value_lc.contains(&needle_lc),
        TextMatchMode::Exact => value_lc == needle_lc,
        TextMatchMode::Wildcard => wildcard_match(&value_lc, &needle_lc),
    }
}

fn wildcard_match(value: &str, pattern: &str) -> bool {
    let s: Vec<char> = value.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let (mut si, mut pi) = (0usize, 0usize);
    let (mut star_idx, mut match_idx) = (None::<usize>, 0usize);

    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            si += 1;
            pi += 1;
            continue;
        }
        if pi < p.len() && p[pi] == '*' {
            star_idx = Some(pi);
            match_idx = si;
            pi += 1;
            continue;
        }
        if let Some(star) = star_idx {
            pi = star + 1;
            match_idx += 1;
            si = match_idx;
            continue;
        }
        return false;
    }

    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::matches;
    use crate::commands::search::query::ast::{
        Expr, Predicate, TextField, TextMatchMode, TextMatcher,
    };
    use crate::entry::FsEntry;

    fn sample_entry(name: &str, path: &str, kind: &str) -> FsEntry {
        FsEntry {
            name: name.to_string(),
            path: path.to_string(),
            kind: kind.to_string(),
            ext: None,
            size: None,
            items: None,
            modified: None,
            original_path: None,
            trash_id: None,
            icon_id: 0,
            starred: false,
            hidden: false,
            network: false,
            read_only: false,
            read_denied: false,
            capabilities: None,
        }
    }

    #[test]
    fn matches_filename_only_for_files_and_links() {
        let dir = sample_entry("src", "/home/chris/src", "dir");
        let file = sample_entry("main.rs", "/home/chris/src/main.rs", "file");
        let expr = Expr::Predicate(Predicate::Text {
            field: TextField::Filename,
            matcher: TextMatcher {
                raw: "*.rs".into(),
                mode: TextMatchMode::Wildcard,
            },
        });
        assert!(!matches(&dir, &expr));
        assert!(matches(&file, &expr));
    }

    #[test]
    fn matches_folder_against_parent_path() {
        let file = sample_entry("main.rs", "/home/chris/Projects/app/main.rs", "file");
        let expr = Expr::Predicate(Predicate::Text {
            field: TextField::Folder,
            matcher: TextMatcher {
                raw: "projects".into(),
                mode: TextMatchMode::Contains,
            },
        });
        assert!(matches(&file, &expr));
    }

    #[test]
    fn matches_exact_case_insensitive() {
        let file = sample_entry("Main.RS", "/tmp/Main.RS", "file");
        let expr = Expr::Predicate(Predicate::Text {
            field: TextField::Name,
            matcher: TextMatcher {
                raw: "main.rs".into(),
                mode: TextMatchMode::Exact,
            },
        });
        assert!(matches(&file, &expr));
    }
}
