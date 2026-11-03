//! Notifications as the shell keeps them: which are on screen, which are unread,
//! and which are held during focus. Plain data, no GTK, no D-Bus.

use std::collections::VecDeque;

pub const MAX_POPUPS: usize = 3;
pub const MAX_HISTORY: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

impl Urgency {
    /// The freedesktop `urgency` hint: 0 low, 1 normal, 2 critical.
    pub fn from_hint(hint: Option<u8>) -> Self {
        match hint {
            Some(0) => Urgency::Low,
            Some(2) => Urgency::Critical,
            _ => Urgency::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    /// (key, label) pairs. The "default" action is a click on the note itself.
    pub actions: Vec<(String, String)>,
    pub urgency: Urgency,
    /// Unix seconds.
    pub arrived: i64,
    pub read: bool,
}

impl Note {
    pub fn has_default_action(&self) -> bool {
        self.actions.iter().any(|(key, _)| key == "default")
    }

    /// Actions shown as buttons (everything but the default one).
    pub fn buttons(&self) -> impl Iterator<Item = &(String, String)> {
        self.actions.iter().filter(|(key, _)| key != "default")
    }
}

/// The spec sends actions as a flat list: key, label, key, label, ...
pub fn pair_actions(flat: &[String]) -> Vec<(String, String)> {
    flat.chunks_exact(2).map(|pair| (pair[0].clone(), pair[1].clone())).collect()
}

/// How long a popup stays, in milliseconds. `None` means until dismissed.
pub fn timeout_ms(urgency: Urgency, requested: i32) -> Option<u32> {
    match urgency {
        Urgency::Critical => None,
        _ if requested > 0 => Some((requested as u32).clamp(3_000, 15_000)),
        Urgency::Low => Some(4_000),
        Urgency::Normal => Some(6_000),
    }
}

/// Notifications are shown as plain text. Strip tags and the common entities,
/// in case an app sends markup the server never asked for.
pub fn plain_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_owned()
}

#[derive(Debug, Default, PartialEq)]
pub struct Added {
    pub id: u32,
    /// Shown on screen now (false when held by focus).
    pub popup: bool,
    /// Older popups pushed off screen to make room.
    pub evicted: Vec<u32>,
}

#[derive(Debug, Default)]
pub struct Inbox {
    next_id: u32,
    /// Ids on screen, newest first.
    popups: Vec<u32>,
    /// Newest first.
    history: VecDeque<Note>,
    /// Unix seconds when focus ends; `None` when not focusing.
    pub focus_until: Option<i64>,
    pub held: usize,
}

impl Inbox {
    pub fn add(&mut self, mut note: Note, replaces: u32) -> Added {
        let replacing = replaces != 0 && self.history.iter().any(|n| n.id == replaces);
        note.id = if replacing {
            replaces
        } else {
            self.next_id = self.next_id.wrapping_add(1).max(1);
            self.next_id
        };
        note.read = false;
        let id = note.id;
        self.history.retain(|n| n.id != id);

        let mut added = Added { id, ..Added::default() };
        if self.focus_until.is_some() && note.urgency != Urgency::Critical {
            self.popups.retain(|&p| p != id);
            if !replacing {
                self.held += 1;
            }
        } else {
            added.popup = true;
            self.popups.retain(|&p| p != id);
            self.popups.insert(0, id);
            while self.popups.len() > MAX_POPUPS {
                if let Some(old) = self.popups.pop() {
                    added.evicted.push(old);
                }
            }
        }
        self.history.push_front(note);
        self.history.truncate(MAX_HISTORY);
        added
    }

    pub fn get(&self, id: u32) -> Option<&Note> {
        self.history.iter().find(|n| n.id == id)
    }

    pub fn is_popup(&self, id: u32) -> bool {
        self.popups.contains(&id)
    }

    /// The popup went away on its own. It stays unread.
    pub fn expire(&mut self, id: u32) -> bool {
        let was = self.is_popup(id);
        self.popups.retain(|&p| p != id);
        was
    }

    /// The person dealt with it: dismissed it, clicked it, or used an action.
    pub fn dismiss(&mut self, id: u32) -> bool {
        let was = self.is_popup(id);
        self.popups.retain(|&p| p != id);
        if let Some(note) = self.history.iter_mut().find(|n| n.id == id) {
            note.read = true;
        }
        was
    }

    /// The app took it back.
    pub fn withdraw(&mut self, id: u32) -> bool {
        let was = self.is_popup(id) || self.get(id).is_some();
        self.popups.retain(|&p| p != id);
        self.history.retain(|n| n.id != id);
        was
    }

    pub fn unread(&self) -> usize {
        self.history.iter().filter(|n| !n.read && !self.is_popup(n.id)).count()
    }

    pub fn mark_all_read(&mut self) {
        for note in self.history.iter_mut() {
            note.read = true;
        }
    }

    /// Clear the history; notes still on screen stay.
    pub fn clear(&mut self) {
        let popups = self.popups.clone();
        self.history.retain(|n| popups.contains(&n.id));
        self.held = 0;
    }

    pub fn recent(&self, count: usize) -> Vec<&Note> {
        self.history.iter().take(count).collect()
    }

    pub fn start_focus(&mut self, until: i64) {
        self.focus_until = Some(until);
    }

    /// End focus; what was held stays in the history as unread.
    pub fn end_focus(&mut self) {
        self.focus_until = None;
        self.held = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(summary: &str, urgency: Urgency) -> Note {
        Note {
            id: 0,
            app: "Test".into(),
            summary: summary.into(),
            body: String::new(),
            actions: vec![],
            urgency,
            arrived: 0,
            read: false,
        }
    }

    #[test]
    fn at_most_three_popups_newest_first() {
        let mut inbox = Inbox::default();
        let ids: Vec<u32> = (0..4).map(|i| inbox.add(note(&i.to_string(), Urgency::Normal), 0).id).collect();
        assert_eq!(ids, vec![1, 2, 3, 4]);
        assert!(!inbox.is_popup(1) && inbox.is_popup(4));
        // The pushed-off note was never seen, so it counts as unread.
        assert_eq!(inbox.unread(), 1);
    }

    #[test]
    fn eviction_is_reported() {
        let mut inbox = Inbox::default();
        for i in 0..3 {
            inbox.add(note(&i.to_string(), Urgency::Normal), 0);
        }
        assert_eq!(inbox.add(note("4", Urgency::Normal), 0).evicted, vec![1]);
    }

    #[test]
    fn expired_stays_unread_dismissed_is_read() {
        let mut inbox = Inbox::default();
        let a = inbox.add(note("a", Urgency::Normal), 0).id;
        let b = inbox.add(note("b", Urgency::Normal), 0).id;
        inbox.expire(a);
        inbox.dismiss(b);
        assert_eq!(inbox.unread(), 1);
        inbox.mark_all_read();
        assert_eq!(inbox.unread(), 0);
    }

    #[test]
    fn focus_holds_all_but_critical() {
        let mut inbox = Inbox::default();
        inbox.start_focus(100);
        assert!(!inbox.add(note("quiet", Urgency::Normal), 0).popup);
        assert!(inbox.add(note("battery", Urgency::Critical), 0).popup);
        assert_eq!(inbox.held, 1);
        inbox.end_focus();
        assert_eq!(inbox.held, 0);
        assert_eq!(inbox.unread(), 1);
    }

    #[test]
    fn replacing_keeps_the_id() {
        let mut inbox = Inbox::default();
        let id = inbox.add(note("downloading", Urgency::Normal), 0).id;
        let again = inbox.add(note("done", Urgency::Normal), id);
        assert_eq!(again.id, id);
        assert_eq!(inbox.recent(10).len(), 1);
        assert_eq!(inbox.get(id).unwrap().summary, "done");
    }

    #[test]
    fn withdraw_removes_everywhere() {
        let mut inbox = Inbox::default();
        let id = inbox.add(note("x", Urgency::Normal), 0).id;
        assert!(inbox.withdraw(id));
        assert!(inbox.get(id).is_none() && !inbox.is_popup(id));
    }

    #[test]
    fn timeouts() {
        assert_eq!(timeout_ms(Urgency::Critical, 5000), None);
        assert_eq!(timeout_ms(Urgency::Normal, -1), Some(6000));
        assert_eq!(timeout_ms(Urgency::Low, 0), Some(4000));
        assert_eq!(timeout_ms(Urgency::Normal, 60_000), Some(15_000));
        assert_eq!(timeout_ms(Urgency::Normal, 500), Some(3_000));
    }

    #[test]
    fn markup_becomes_plain_text() {
        assert_eq!(plain_text("<b>Lin</b> &amp; <i>you</i>"), "Lin & you");
        assert_eq!(plain_text("a &lt;tag&gt;"), "a <tag>");
    }

    #[test]
    fn actions_pair_up() {
        let flat: Vec<String> = ["default", "Open", "reply", "Reply", "odd"].iter().map(|s| s.to_string()).collect();
        let pairs = pair_actions(&flat);
        assert_eq!(pairs.len(), 2);
        let n = Note { actions: pairs, ..note("x", Urgency::Normal) };
        assert!(n.has_default_action());
        assert_eq!(n.buttons().count(), 1);
    }
}
