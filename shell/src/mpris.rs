//! Now playing: any MPRIS player on the session bus. Read when Today opens and
//! after one of its controls is used; nothing is watched while Today is closed.

use gtk::{gio, glib, prelude::*};

const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PREFIX: &str = "org.mpris.MediaPlayer2.";

#[derive(Debug, Clone, PartialEq)]
pub struct Playing {
    /// The player's bus name, to send controls to.
    pub player: String,
    pub title: String,
    pub artist: Option<String>,
    pub playing: bool,
}

/// One player as read from the bus.
#[derive(Debug, Default)]
pub struct Seen {
    pub player: String,
    /// "Playing", "Paused" or "Stopped".
    pub status: String,
    pub title: Option<String>,
    pub artists: Vec<String>,
}

/// The player to show: one that is playing, else one that is paused.
/// Stopped players, and players with no title, say nothing worth reading.
pub fn choose(seen: &[Seen]) -> Option<Playing> {
    let titled = |s: &&Seen| s.title.as_deref().is_some_and(|t| !t.trim().is_empty());
    let pick = seen
        .iter()
        .filter(titled)
        .find(|s| s.status == "Playing")
        .or_else(|| seen.iter().filter(titled).find(|s| s.status == "Paused"))?;
    let artists: Vec<&str> = pick.artists.iter().map(|a| a.trim()).filter(|a| !a.is_empty()).collect();
    Some(Playing {
        player: pick.player.clone(),
        title: pick.title.clone().unwrap_or_default().trim().to_owned(),
        artist: (!artists.is_empty()).then(|| artists.join(", ")),
        playing: pick.status == "Playing",
    })
}

fn ty(text: &str) -> Option<&glib::VariantTy> {
    glib::VariantTy::new(text).ok()
}

async fn read(bus: &gio::DBusConnection, player: String) -> Option<Seen> {
    let reply = bus
        .call_future(
            Some(&player),
            PATH,
            "org.freedesktop.DBus.Properties",
            "GetAll",
            Some(&(PLAYER,).to_variant()),
            ty("(a{sv})"),
            gio::DBusCallFlags::NONE,
            1_000,
        )
        .await
        .ok()?;
    let props = glib::VariantDict::new(Some(&reply.child_value(0)));
    let status = props.lookup::<String>("PlaybackStatus").ok().flatten().unwrap_or_default();
    let mut seen = Seen { player, status, ..Seen::default() };
    if let Some(meta) = props.lookup_value("Metadata", ty("a{sv}")) {
        let meta = glib::VariantDict::new(Some(&meta));
        seen.title = meta.lookup::<String>("xesam:title").ok().flatten();
        // The spec says a list of artists; some players send one string.
        seen.artists = meta
            .lookup::<Vec<String>>("xesam:artist")
            .ok()
            .flatten()
            .or_else(|| meta.lookup::<String>("xesam:artist").ok().flatten().map(|a| vec![a]))
            .unwrap_or_default();
    }
    Some(seen)
}

/// What is playing right now, if anything.
pub async fn current() -> Option<Playing> {
    let bus = gio::bus_get_future(gio::BusType::Session).await.ok()?;
    let reply = bus
        .call_future(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "ListNames",
            None,
            ty("(as)"),
            gio::DBusCallFlags::NONE,
            1_000,
        )
        .await
        .ok()?;
    let (names,) = reply.get::<(Vec<String>,)>()?;
    let mut seen = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with(PREFIX)) {
        if let Some(s) = read(&bus, name).await {
            seen.push(s);
        }
    }
    choose(&seen)
}

/// "PlayPause", "Next" or "Previous".
pub async fn control(player: &str, method: &str) {
    if let Ok(bus) = gio::bus_get_future(gio::BusType::Session).await {
        let _ = bus
            .call_future(Some(player), PATH, PLAYER, method, None, None, gio::DBusCallFlags::NONE, 1_000)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(player: &str, status: &str, title: Option<&str>, artists: &[&str]) -> Seen {
        Seen {
            player: player.into(),
            status: status.into(),
            title: title.map(Into::into),
            artists: artists.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn playing_wins_over_paused() {
        let list = [
            seen("org.mpris.MediaPlayer2.a", "Paused", Some("Aria"), &["J. S. Bach"]),
            seen("org.mpris.MediaPlayer2.b", "Playing", Some("Clair de lune"), &["Debussy"]),
        ];
        let p = choose(&list).unwrap();
        assert_eq!(p.title, "Clair de lune");
        assert!(p.playing);
    }

    #[test]
    fn paused_is_shown_and_stopped_is_not() {
        let list = [
            seen("a", "Stopped", Some("Old"), &[]),
            seen("b", "Paused", Some("Aria"), &["J. S. Bach", " "]),
        ];
        let p = choose(&list).unwrap();
        assert_eq!((p.title.as_str(), p.artist.as_deref(), p.playing), ("Aria", Some("J. S. Bach"), false));
        assert_eq!(choose(&[seen("a", "Stopped", Some("Old"), &[])]), None);
    }

    #[test]
    fn untitled_players_say_nothing() {
        assert_eq!(choose(&[seen("a", "Playing", Some("  "), &["Someone"])]), None);
        assert_eq!(choose(&[seen("a", "Playing", None, &[])]), None);
    }
}
