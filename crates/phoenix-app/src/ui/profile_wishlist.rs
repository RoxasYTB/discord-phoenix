use gpui::SharedString;
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WishItem {
    pub name: SharedString,
    pub image: Option<SharedString>,
}

pub fn first_id(profile: &Value) -> Option<String> {
    profile.get("wishlist_settings")?.as_object()?.keys().next().cloned()
}

fn str_at<'a>(v: &'a Value, paths: &[&str]) -> Option<&'a str> {
    paths.iter().find_map(|p| v.pointer(p).and_then(Value::as_str).filter(|s| !s.is_empty()))
}

pub fn items(v: &Value) -> Vec<WishItem> {
    v.get("wishlist_items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|it| {
            let name = str_at(it, &["/sku_name", "/sku/name"]).unwrap_or_default();
            let sku = str_at(it, &["/sku_id", "/sku/id"])?;
            let image = Some(SharedString::from(format!("https://cdn.discordapp.com/media/v1/collectibles-shop/{sku}/static")));
            Some(WishItem { name: name.to_owned().into(), image })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn reads_first_wishlist_id_and_items() {
        assert_eq!(super::first_id(&json!({"wishlist_settings": {"42": {}}})).as_deref(), Some("42"));
        let v = json!({"wishlist_items": [{"sku_id": "7", "sku_name": "Crown"}]});
        assert_eq!(super::items(&v)[0].name.as_ref(), "Crown");
    }
}
