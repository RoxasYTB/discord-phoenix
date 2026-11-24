use super::{DiscordBackend, api_err};
use crate::{backend::BoxFuture, ui::settings::collectibles::{Collectible, ShopCatalog, parse_product, parse_shop, promote}};

impl DiscordBackend {
    pub(super) fn collectibles_shop_impl(&self) -> BoxFuture<ShopCatalog> {
        self.with_session(move |_, s| async move {
            let v = s.rest.get_collectibles_categories().await.map_err(api_err)?;
            let mut cat = parse_shop(&v);
            if let Ok(rec) = s.rest.get_recommended_collectibles().await {
                promote(&mut cat.shop, &rec);
            }
            Ok(cat)
        })
    }

    pub(super) fn shop_storefront_impl(&self) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move { s.rest.get_collectibles_categories().await.map_err(api_err) })
    }

    pub(super) fn orbs_balance_impl(&self) -> BoxFuture<Option<u64>> {
        self.with_session(move |_, s| async move { s.rest.get_orbs_balance().await.map_err(api_err) })
    }

    pub(super) fn collectible_product_impl(&self, sku: u64) -> BoxFuture<Option<Collectible>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.get_collectibles_product(sku).await.map_err(api_err)?;
            Ok(parse_product(&v))
        })
    }
}
