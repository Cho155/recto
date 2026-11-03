//! How Search ranks what you type against apps and actions. Plain data, tested.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    App,
    Action,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// App id (desktop file) or action name.
    pub key: String,
    pub name: String,
    pub detail: String,
    pub keywords: Vec<String>,
    pub kind: Kind,
}

/// Letters of `needle` appear in `hay` in order.
fn subsequence(needle: &str, hay: &str) -> bool {
    let mut hay = hay.chars();
    needle.chars().all(|c| hay.any(|h| h == c))
}

/// Higher is better; `None` means no match.
pub fn score(query: &str, candidate: &Candidate) -> Option<u32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(0);
    }
    let name = candidate.name.to_lowercase();
    if name.starts_with(&q) {
        return Some(1000);
    }
    if name.split(|c: char| !c.is_alphanumeric()).any(|word| word.starts_with(&q)) {
        return Some(800);
    }
    if name.contains(&q) {
        return Some(600);
    }
    let in_extra = candidate.keywords.iter().any(|k| k.to_lowercase().contains(&q))
        || candidate.detail.to_lowercase().contains(&q);
    if in_extra {
        return Some(400);
    }
    if q.chars().count() >= 2 && subsequence(&q, &name) {
        return Some(200);
    }
    None
}

/// The best matches, at most `limit`. Ties go to actions first when the query is
/// empty (they are few), otherwise alphabetically by name.
pub fn rank<'a>(query: &str, candidates: &'a [Candidate], limit: usize) -> Vec<&'a Candidate> {
    let mut hits: Vec<(u32, &Candidate)> = candidates
        .iter()
        .filter_map(|c| score(query, c).map(|s| (s, c)))
        .collect();
    let empty = query.trim().is_empty();
    hits.sort_by(|(sa, a), (sb, b)| {
        sb.cmp(sa)
            .then_with(|| if empty { (a.kind == Kind::App).cmp(&(b.kind == Kind::App)) } else { std::cmp::Ordering::Equal })
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    hits.into_iter().take(limit).map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, keywords: &[&str]) -> Candidate {
        Candidate {
            key: name.to_lowercase(),
            name: name.into(),
            detail: String::new(),
            keywords: keywords.iter().map(|k| k.to_string()).collect(),
            kind: Kind::App,
        }
    }

    #[test]
    fn prefix_beats_word_beats_substring() {
        let list = vec![app("Text Editor", &[]), app("Terminal", &[]), app("Contacts", &[])];
        let names: Vec<&str> = rank("te", &list, 10).iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Terminal", "Text Editor"]);
        let names: Vec<&str> = rank("edit", &list, 10).iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Text Editor"]);
        let names: Vec<&str> = rank("tac", &list, 10).iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Contacts"]);
    }

    #[test]
    fn keywords_and_fuzzy() {
        let list = vec![app("Files", &["folder", "explorer"]), app("Settings", &[])];
        assert_eq!(rank("folder", &list, 10)[0].name, "Files");
        assert_eq!(rank("stg", &list, 10)[0].name, "Settings");
        assert!(rank("zzz", &list, 10).is_empty());
    }

    #[test]
    fn empty_query_lists_actions_first() {
        let mut action = app("Lock screen", &[]);
        action.kind = Kind::Action;
        let list = vec![app("Alpha", &[]), action];
        assert_eq!(rank("", &list, 10)[0].name, "Lock screen");
        assert_eq!(rank("", &list, 1).len(), 1);
    }
}
