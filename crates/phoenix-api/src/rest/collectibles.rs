use reqwest::Method;
use serde_json::Value;

use super::{Body, RestClient};
use crate::error::Result;

pub const CATEGORIES_V2: &str = "/collectibles-categories/v2?include_bundles=true&variants_return_style=2&skip_num_categories=0";
pub const SHOP_HOME: &str = "/collectibles-shop?include_bundles=true&variants_return_style=2&include_dynamic_blocks=true";
pub const PURCHASES: &str = "/users/@me/collectibles-purchases?variants_return_style=2";

pub const COLLECTIBLES_APPLICATION_ID: &str = "1096190356233670716";
pub const RECOMMENDED_ITEMS: &str = "/storefront/recommended-items?application_ids=1096190356233670716&limit=100";

pub const PREMIUM_TYPE_NITRO: u64 = 2;

pub const STOREFRONT_SKUS: &str = "/storefront/products/skus";
pub const SOURCE_TYPE_SHOP: u64 = 1;
pub const SKU_FLAG_AVAILABLE: u64 = 1 << 2;

pub fn storefront_skus_path(skus: &[u64], locale: &str) -> String {
    let ids: String = skus.iter().map(|s| format!("sku_ids={s}&")).collect();
    format!("{STOREFRONT_SKUS}?{ids}locale={locale}&with_bundled_skus=true&include_google_sku_ids=false&ignore_cache=false")
}

#[derive(Clone, Debug, PartialEq)]
pub struct LookSku {
    pub sku: u64,
    pub kind: u64,
    pub name: String,
    pub shoppable: bool,
    pub item: Value,
    pub summary: String,
    pub background: Option<String>,
    pub prices: Value,
}

pub fn look_skus(v: &Value, wanted: &[u64]) -> Vec<LookSku> {
    let products: Vec<&Value> = v.get("products").and_then(Value::as_array).into_iter().flatten().collect();
    let id_of = |s: &Value| s.get("id").and_then(Value::as_str).and_then(|x| x.parse::<u64>().ok());
    wanted
        .iter()
        .filter_map(|&w| {
            let (p, s) = products.iter().find_map(|p| {
                let s = p.get("skus").and_then(Value::as_array)?.iter().find(|s| id_of(s) == Some(w))?;
                Some((*p, s))
            })?;
            let meta = s.get("tenant_metadata")?.get("collectibles")?;
            let item = meta.get("item").cloned().unwrap_or(Value::Null);
            let kind = meta.get("type").and_then(Value::as_u64).or_else(|| item.get("type").and_then(Value::as_u64))?;
            let flags = s.get("flags").and_then(Value::as_u64).unwrap_or(0);
            let source = meta.get("source_type").and_then(Value::as_u64);
            let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            Some(LookSku {
                sku: w,
                kind,
                name: text(s, "name"),
                shoppable: flags & SKU_FLAG_AVAILABLE != 0 && source == Some(SOURCE_TYPE_SHOP),
                item,
                summary: Some(text(p, "summary")).filter(|x| !x.trim().is_empty()).unwrap_or_else(|| text(s, "description")),
                background: p
                    .get("tenant_metadata")
                    .and_then(|t| t.get("collectibles"))
                    .and_then(|c| c.get("primary_collection_pdp_bg_url"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                prices: s.get("prices").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

pub fn products(v: &Value) -> impl Iterator<Item = &Value> {
    v.get("categories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("products").and_then(Value::as_array))
        .flatten()
}

pub fn recommended_skus(v: &Value) -> Vec<u64> {
    v.get("recommended_items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|i| i.get("sku_id")?.as_str()?.parse().ok())
        .collect()
}

impl RestClient {
    pub async fn get_orbs_balance(&self) -> Result<Option<u64>> {
        let v: Value = self.call(Method::GET, "/users/@me/virtual-currency/balance", Body::None).await?;
        Ok(v.get("balance").and_then(Value::as_u64))
    }

    pub async fn get_collectibles_categories(&self) -> Result<Value> {
        self.call(Method::GET, CATEGORIES_V2, Body::None).await
    }

    pub async fn get_collectibles_shop_home(&self) -> Result<Value> {
        self.call(Method::GET, SHOP_HOME, Body::None).await
    }

    pub async fn get_recommended_collectibles(&self) -> Result<Vec<u64>> {
        let v: Value = self.call(Method::GET, RECOMMENDED_ITEMS, Body::None).await?;
        Ok(recommended_skus(&v))
    }

    pub async fn get_collectibles_product(&self, sku: u64) -> Result<Value> {
        self.call(Method::GET, &format!("/collectibles-products/{sku}"), Body::None).await
    }

    pub async fn get_storefront_skus(&self, skus: &[u64]) -> Result<Value> {
        self.call(Method::GET, &storefront_skus_path(skus, &self.locale()), Body::None).await
    }

    pub async fn get_collectibles_purchases(&self) -> Result<Value> {
        self.call(Method::GET, PURCHASES, Body::None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flattens_products() {
        let v = json!({"categories": [{"products": [{"sku_id": "1"}, {"sku_id": "2"}]}, {"products": []}, {}]});
        assert_eq!(products(&v).count(), 2);
        assert_eq!(products(&json!({})).count(), 0);
    }

    #[test]
    fn reads_recommendations() {
        let v = json!({"recommended_items": [{"sku_id": "5"}, {"sku_id": "x"}, {"sku_id": "3"}]});
        assert_eq!(recommended_skus(&v), vec![5, 3]);
    }

    #[test]
    fn shop_this_look_keeps_the_profile_order_and_flags_unavailable_skus() {
        let sku = |id: &str, flags: u64, source: u64, ty: u64| {
            json!({"id": id, "name": format!("n{id}"), "flags": flags,
                "tenant_metadata": {"collectibles": {"type": ty, "source_type": source, "item": {"type": ty, "sku_id": id, "asset": "a"}}}})
        };
        let v = json!({"products": [
            {"skus": [sku("1", 4, 1, 0), sku("9", 4, 1, 0)]},
            {"skus": [sku("2", 0, 1, 2)]},
            {"skus": [sku("3", 4, 2, 1)]}
        ]});
        let got = look_skus(&v, &[3, 1, 2, 7]);
        assert_eq!(got.iter().map(|l| l.sku).collect::<Vec<_>>(), vec![3, 1, 2]);
        assert_eq!(got.iter().map(|l| l.shoppable).collect::<Vec<_>>(), vec![false, true, false]);
        assert_eq!(got[0].kind, 1);
        assert_eq!(got[1].name, "n1");
        let p = storefront_skus_path(&[1, 2], "fr");
        assert!(p.starts_with("/storefront/products/skus?sku_ids=1&sku_ids=2&locale=fr&with_bundled_skus=true"));
    }

    #[test]
    fn shop_this_look_keeps_the_product_details_for_the_pdp() {
        let v = json!({"products": [{
            "name": "Bunny", "summary": "Gives your avatar a new look.",
            "tenant_metadata": {"collectibles": {"primary_collection_pdp_bg_url": "https://cdn/bg"}},
            "skus": [{"id": "5", "name": "Bunny", "description": "desc", "flags": 4,
                "prices": {"0": {"country_prices": {"prices": [{"amount": 599, "currency": "eur", "exponent": 2}]}}},
                "tenant_metadata": {"collectibles": {"type": 0, "source_type": 1, "item": {"type": 0, "asset": "a"}}}}]
        }]});
        let got = look_skus(&v, &[5]);
        assert_eq!(got[0].summary, "Gives your avatar a new look.");
        assert_eq!(got[0].background.as_deref(), Some("https://cdn/bg"));
        assert_eq!(got[0].prices["0"]["country_prices"]["prices"][0]["amount"], 599);
        let blank = json!({"products": [{"summary": " ", "skus": [{"id": "6", "description": "fallback", "flags": 4,
            "tenant_metadata": {"collectibles": {"type": 1, "source_type": 1}}}]}]});
        assert_eq!(look_skus(&blank, &[6])[0].summary, "fallback");
    }
}
