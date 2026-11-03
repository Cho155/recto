//! Input method indicator, backed by fcitx5's D-Bus controller.
//! The bar shows two letters ("EN", "中", "あ"), and only when there is a choice.

use gtk::{gio, glib, prelude::*};

const BUS: &str = "org.fcitx.Fcitx5";
const PATH: &str = "/controller";
const IFACE: &str = "org.fcitx.Fcitx.Controller1";

/// Two letters for an fcitx5 input method name.
pub fn label_for(name: &str) -> String {
    if let Some(layout) = name.strip_prefix("keyboard-") {
        let base = layout.split('-').next().unwrap_or(layout);
        return match base {
            "us" | "gb" | "cn" | "au" | "ca" => "EN".to_owned(),
            other => other.chars().take(2).collect::<String>().to_uppercase(),
        };
    }
    match name {
        "pinyin" | "shuangpin" | "wbx" | "wbpy" | "rime" | "cangjie" | "quick" | "erbi" => "中".into(),
        "chewing" | "zhuyin" => "注".into(),
        "mozc" | "anthy" | "kkc" | "skk" => "あ".into(),
        "hangul" => "한".into(),
        "unikey" | "bamboo" => "VI".into(),
        other => other.chars().filter(|c| c.is_alphanumeric()).take(2).collect::<String>().to_uppercase(),
    }
}

async fn call(connection: &gio::DBusConnection, method: &str, args: Option<&glib::Variant>) -> Option<glib::Variant> {
    connection
        .call_future(Some(BUS), PATH, IFACE, method, args, None, gio::DBusCallFlags::NO_AUTO_START, 1_000)
        .await
        .ok()
}

/// The label to show, or `None` when fcitx5 is not running or offers only one input method.
pub async fn current(connection: &gio::DBusConnection) -> Option<String> {
    let group = call(connection, "CurrentInputMethodGroup", None).await?.child_value(0).get::<String>()?;
    let info = call(connection, "InputMethodGroupInfo", Some(&(group,).to_variant())).await?;
    // (s layout, a(ss) items): each item is (input method, layout).
    let items = info.child_value(1);
    if items.n_children() < 2 {
        return None;
    }
    let first = items.child_value(0).child_value(0).get::<String>()?;
    let current = call(connection, "CurrentInputMethod", None)
        .await
        .and_then(|v| v.child_value(0).get::<String>())
        .filter(|s| !s.is_empty())
        // No text field has focus: show the group's default.
        .unwrap_or(first);
    Some(label_for(&current))
}

/// Switch input method for the focused text field.
pub fn toggle(connection: &gio::DBusConnection) {
    let connection = connection.clone();
    glib::spawn_future_local(async move {
        call(&connection, "Toggle", None).await;
    });
}

#[cfg(test)]
mod tests {
    use super::label_for;

    #[test]
    fn labels() {
        assert_eq!(label_for("keyboard-us"), "EN");
        assert_eq!(label_for("keyboard-de"), "DE");
        assert_eq!(label_for("keyboard-fr-oss"), "FR");
        assert_eq!(label_for("pinyin"), "中");
        assert_eq!(label_for("mozc"), "あ");
        assert_eq!(label_for("hangul"), "한");
    }
}
