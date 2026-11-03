//! StatusNotifierItem (tray) data: where an item lives and what to call it.
//! No D-Bus calls here, so it can be tested on its own.

use crate::inbox::plain_text;
use gtk::glib;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address {
    pub bus: String,
    pub path: String,
}

impl Address {
    /// What an item passes to RegisterStatusNotifierItem: a bus name, a bus name with a
    /// path, or just an object path on the caller's own connection.
    pub fn from_registration(service: &str, sender: &str) -> Address {
        if service.starts_with('/') {
            Address { bus: sender.to_owned(), path: service.to_owned() }
        } else if let Some((bus, path)) = service.split_once('/') {
            Address { bus: bus.to_owned(), path: format!("/{path}") }
        } else {
            Address { bus: service.to_owned(), path: "/StatusNotifierItem".to_owned() }
        }
    }

    /// The string the watcher announces and lists for this item.
    pub fn key(&self) -> String {
        format!("{}{}", self.bus, self.path)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub status: String,
    pub icon_name: String,
    pub tooltip_title: String,
    pub tooltip_body: String,
    /// Object path of the item's dbusmenu, if it has one.
    pub menu: Option<String>,
    /// The item has no action of its own; a click should open its menu.
    pub item_is_menu: bool,
}

fn string(props: &HashMap<String, glib::Variant>, key: &str) -> String {
    props.get(key).and_then(|v| v.get::<String>()).unwrap_or_default()
}

impl Item {
    pub fn from_props(props: &HashMap<String, glib::Variant>) -> Item {
        let (tooltip_title, tooltip_body) = props
            .get("ToolTip")
            .filter(|v| v.type_().as_str() == "(sa(iiay)ss)")
            .map(|v| {
                (
                    v.child_value(2).get::<String>().unwrap_or_default(),
                    v.child_value(3).get::<String>().unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        let menu = props
            .get("Menu")
            .and_then(|v| v.get::<glib::variant::ObjectPath>())
            .map(|p| p.as_str().to_owned())
            .filter(|p| p != "/");
        Item {
            id: string(props, "Id"),
            title: string(props, "Title"),
            status: string(props, "Status"),
            icon_name: string(props, "IconName"),
            tooltip_title: plain_text(&tooltip_title),
            tooltip_body: plain_text(&tooltip_body),
            menu,
            item_is_menu: props.get("ItemIsMenu").and_then(|v| v.get::<bool>()).unwrap_or(false),
        }
    }

    /// The name shown in the tray list.
    pub fn label(&self) -> String {
        if !self.title.trim().is_empty() {
            return self.title.trim().to_owned();
        }
        if !self.id.trim().is_empty() {
            let words = self.id.trim().replace(['_', '-'], " ");
            let mut chars = words.chars();
            return match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            };
        }
        "Unnamed".to_owned()
    }

    /// A few quiet words beside the name, taken from the tooltip.
    pub fn detail(&self) -> String {
        let label = self.label();
        let title = self.tooltip_title.trim();
        if !title.is_empty() && title != label && title != self.title {
            return title.to_owned();
        }
        self.tooltip_body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_owned()
    }

    /// Passive items ask to be left out.
    pub fn shown(&self) -> bool {
        self.status != "Passive"
    }

    /// The input method's own tray item; the bar shows it as the input method indicator.
    pub fn is_input_method(&self) -> bool {
        let id = self.id.to_ascii_lowercase();
        id == "fcitx" || id.starts_with("fcitx") || self.icon_name.starts_with("fcitx")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::glib::prelude::*;

    #[test]
    fn registration_forms() {
        assert_eq!(
            Address::from_registration("org.kde.StatusNotifierItem-12-1", ":1.5"),
            Address { bus: "org.kde.StatusNotifierItem-12-1".into(), path: "/StatusNotifierItem".into() }
        );
        assert_eq!(
            Address::from_registration("/org/ayatana/NotificationItem/nm", ":1.7"),
            Address { bus: ":1.7".into(), path: "/org/ayatana/NotificationItem/nm".into() }
        );
        assert_eq!(Address::from_registration(":1.9/Item", ":1.9").key(), ":1.9/Item");
    }

    #[test]
    fn labels_fall_back_politely() {
        let item = Item { id: "nm-applet".into(), ..Item::default() };
        assert_eq!(item.label(), "Nm applet");
        let item = Item { title: "Sync".into(), id: "x".into(), ..Item::default() };
        assert_eq!(item.label(), "Sync");
        assert_eq!(Item::default().label(), "Unnamed");
    }

    #[test]
    fn detail_comes_from_the_tooltip() {
        let item = Item { title: "VPN".into(), tooltip_title: "Connected".into(), ..Item::default() };
        assert_eq!(item.detail(), "Connected");
        let item = Item { title: "Sync".into(), tooltip_title: "Sync".into(), tooltip_body: "\nUp to date".into(), ..Item::default() };
        assert_eq!(item.detail(), "Up to date");
    }

    #[test]
    fn reads_properties() {
        let mut props = HashMap::new();
        props.insert("Id".to_owned(), "Fcitx".to_variant());
        props.insert("Status".to_owned(), "Passive".to_variant());
        props.insert("ItemIsMenu".to_owned(), true.to_variant());
        let tooltip = ("icon", Vec::<(i32, i32, Vec<u8>)>::new(), "Input Method", "<b>English</b>").to_variant();
        props.insert("ToolTip".to_owned(), tooltip);
        let item = Item::from_props(&props);
        assert!(item.is_input_method() && !item.shown() && item.item_is_menu);
        assert_eq!(item.tooltip_body, "English");
    }
}
