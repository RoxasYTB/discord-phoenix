//! Read-only collectibles catalogue, as the live client fetches it when a
//! profile collectible picker opens:
//! `GET /collectibles-categories/v2?include_bundles=true&variants_return_style=2&skip_num_categories=0`
//! (`{ categories: [{ products: [{ type, premium_type, items: [...] }] }] }`)
//! and the owned items, `GET /users/@me/collectibles-purchases`. Nothing here
//! buys, claims or equips anything.

use reqwest::Method;
use serde_json::Value;

use super::{Body, RestClient};
use crate::error::Result;

/// `COLLECTIBLES_CATEGORIES_V2` with the query string the client sends.
pub const CATEGORIES_V2: &str = "/collectibles-categories/v2?include_bundles=true&variants_return_style=2&skip_num_categories=0";
/// `COLLECTIBLES_PURCHASES`.
pub const PURCHASES: &str = "/users/@me/collectibles-purchases?variants_return_style=2";

/// Collectibles shop application (client constant `FYj`).
pub const COLLECTIBLES_APPLICATION_ID: &str = "1096190356233670716";
/// Picker recommendations, as the profile editor preloads them.
pub const RECOMMENDED_ITEMS: &str = "/storefront/recommended-items?application_ids=1096190356233670716&limit=100";

/// Product `premium_type` of the "Nitro exclusive" picker section.
pub const PREMIUM_TYPE_NITRO: u64 = 2;

/// Flattens the catalogue into its products, in category order.
pub fn products(v: &Value) -> impl Iterator<Item = &Value> {
    v.get("categories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("products").and_then(Value::as_array))
        .flatten()
}

/// `recommended_items[].sku_id` of a `/storefront/recommended-items` reply.
pub fn recommended_skus(v: &Value) -> Vec<u64> {
    v.get("recommended_items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|i| i.get("sku_id")?.as_str()?.parse().ok())
        .collect()
}

impl RestClient {
    /// Orbs balance (`VIRTUAL_CURRENCY_USER_BALANCE`, `{ balance }`), as the
    /// Shop header pill fetches it. Read-only.
    pub async fn get_orbs_balance(&self) -> Result<Option<u64>> {
        let v: Value = self.call(Method::GET, "/users/@me/virtual-currency/balance", Body::None).await?;
        Ok(v.get("balance").and_then(Value::as_u64))
    }

    /// The shop catalogue (raw JSON; CDN asset URLs are read at runtime).
    pub async fn get_collectibles_categories(&self) -> Result<Value> {
        self.call(Method::GET, CATEGORIES_V2, Body::None).await
    }

    /// Recommended shop SKUs, best first (`recommended_items[].sku_id`). The
    /// pickers list these first in their "Shop" section.
    pub async fn get_recommended_collectibles(&self) -> Result<Vec<u64>> {
        let v: Value = self.call(Method::GET, RECOMMENDED_ITEMS, Body::None).await?;
        Ok(recommended_skus(&v))
    }

    /// One product by SKU (`COLLECTIBLES_PRODUCTS`), as the client fetches
    /// the applied effect / frame of a profile it shows.
    pub async fn get_collectibles_product(&self, sku: u64) -> Result<Value> {
        self.call(Method::GET, &format!("/collectibles-products/{sku}"), Body::None).await
    }

    /// Owned collectibles (raw JSON array of purchases).
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
}
