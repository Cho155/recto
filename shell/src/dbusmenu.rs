//! com.canonical.dbusmenu, the menus behind tray items. Parsed into a tree and
//! flattened into a plain list: submenus become headed groups, so the whole menu
//! fits in one quiet panel without nested popups.

use gtk::glib;

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub separator: bool,
    /// Some(on) for check and radio items.
    pub toggle: Option<bool>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Item { id: i32, label: String, enabled: bool, toggle: Option<bool>, depth: usize },
    Heading { label: String, depth: usize },
    Separator,
}

/// "_Open" -> "Open"; "Save__As" -> "Save_As".
pub fn strip_mnemonic(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if chars.peek() == Some(&'_') {
                out.push('_');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse one `(ia{sv}av)` layout node.
pub fn parse(node: &glib::Variant) -> Option<Node> {
    if node.type_().as_str() != "(ia{sv}av)" {
        return None;
    }
    let id = node.child_value(0).get::<i32>()?;
    let props = glib::VariantDict::new(Some(&node.child_value(1)));
    let get_bool = |key: &str, default: bool| {
        props.lookup_value(key, None).and_then(|v| v.get::<bool>()).unwrap_or(default)
    };
    let get_str = |key: &str| {
        props.lookup_value(key, None).and_then(|v| v.get::<String>()).unwrap_or_default()
    };
    let toggle = match get_str("toggle-type").as_str() {
        "checkmark" | "radio" => Some(
            props.lookup_value("toggle-state", None).and_then(|v| v.get::<i32>()).unwrap_or(0) == 1,
        ),
        _ => None,
    };
    let children_value = node.child_value(2);
    let children = (0..children_value.n_children())
        .filter_map(|i| parse(&children_value.child_value(i).as_variant()?))
        .collect();
    Some(Node {
        id,
        label: strip_mnemonic(&get_str("label")),
        enabled: get_bool("enabled", true),
        visible: get_bool("visible", true),
        separator: get_str("type") == "separator",
        toggle,
        children,
    })
}

/// The visible entries under a root node, in order, with no doubled or dangling separators.
pub fn flatten(root: &Node) -> Vec<Entry> {
    fn walk(node: &Node, depth: usize, out: &mut Vec<Entry>) {
        for child in node.children.iter().filter(|c| c.visible) {
            if child.separator {
                out.push(Entry::Separator);
            } else if !child.children.is_empty() {
                out.push(Entry::Heading { label: child.label.clone(), depth });
                walk(child, depth + 1, out);
            } else {
                out.push(Entry::Item {
                    id: child.id,
                    label: child.label.clone(),
                    enabled: child.enabled,
                    toggle: child.toggle,
                    depth,
                });
            }
        }
    }
    let mut raw = Vec::new();
    walk(root, 0, &mut raw);
    let mut out: Vec<Entry> = Vec::with_capacity(raw.len());
    for entry in raw {
        let is_sep = entry == Entry::Separator;
        if is_sep && (out.is_empty() || out.last() == Some(&Entry::Separator)) {
            continue;
        }
        out.push(entry);
    }
    if out.last() == Some(&Entry::Separator) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::glib::prelude::*;
    use std::collections::HashMap;

    fn node(id: i32, props: &[(&str, glib::Variant)], children: Vec<glib::Variant>) -> glib::Variant {
        let map: HashMap<String, glib::Variant> = props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        // Vec<Variant> serializes as "av", boxing each child once, as dbusmenu does.
        (id, map, children).to_variant()
    }

    #[test]
    fn mnemonics() {
        assert_eq!(strip_mnemonic("_Open"), "Open");
        assert_eq!(strip_mnemonic("Save__As"), "Save_As");
    }

    #[test]
    fn parses_and_flattens() {
        let tree = node(0, &[], vec![
            node(1, &[("label", "_Enable Wi-Fi".to_variant()), ("toggle-type", "checkmark".to_variant()),
                      ("toggle-state", 1i32.to_variant())], vec![]),
            node(2, &[("type", "separator".to_variant())], vec![]),
            node(3, &[("type", "separator".to_variant())], vec![]),
            node(4, &[("label", "VPN".to_variant())], vec![
                node(5, &[("label", "Office".to_variant())], vec![]),
            ]),
            node(6, &[("label", "Hidden".to_variant()), ("visible", false.to_variant())], vec![]),
            node(7, &[("type", "separator".to_variant())], vec![]),
        ]);
        let root = parse(&tree).expect("valid layout");
        let entries = flatten(&root);
        assert_eq!(entries, vec![
            Entry::Item { id: 1, label: "Enable Wi-Fi".into(), enabled: true, toggle: Some(true), depth: 0 },
            Entry::Separator,
            Entry::Heading { label: "VPN".into(), depth: 0 },
            Entry::Item { id: 5, label: "Office".into(), enabled: true, toggle: None, depth: 1 },
        ]);
    }
}
