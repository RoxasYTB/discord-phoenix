use parking_lot::RwLock;
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Source {
    pub brand: String,
    pub sub: String,
    pub default: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Payment {
    pub date: String,
    pub description: String,
    pub amount: String,
    pub orbs: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Billing {
    pub country: Option<String>,
    pub sources: Vec<Source>,
    pub payments: Vec<Payment>,
}

static DATA: RwLock<Option<Billing>> = RwLock::new(None);

pub fn get() -> Option<Billing> {
    DATA.read().clone()
}

pub fn set(b: Billing) {
    *DATA.write() = Some(b);
}

pub fn fetch(app: &crate::app::PhoenixApp, cx: &mut gpui::Context<crate::app::PhoenixApp>) {
    if DATA.read().is_some() {
        return;
    }
    let fut = app.backend.billing();
    cx.spawn(async move |this, cx| {
        if let Ok(Some(b)) = fut.await {
            set(b);
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned()
}

pub fn parse_sources(v: &Value) -> Vec<Source> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let ty = p.get("type").and_then(Value::as_u64).unwrap_or(0);
            let (brand, sub) = if ty == 2 {
                ("PayPal".to_owned(), s(p, "email"))
            } else {
                let mut b = s(p, "brand");
                if let Some(f) = b.get(0..1) {
                    b = f.to_uppercase() + &b[1..];
                }
                (b, format!("•••• {}", s(p, "last_4")))
            };
            Source { brand, sub, default: p.get("default").and_then(Value::as_bool).unwrap_or(false) }
        })
        .collect()
}

pub fn source_country(v: &Value, fallback: Option<String>) -> Option<String> {
    let arr = v.as_array()?;
    let pick = arr.iter().find(|p| p.get("default").and_then(Value::as_bool) == Some(true)).or(arr.first());
    pick.and_then(|p| p.get("billing_address")?.get("country")?.as_str().map(str::to_owned)).or(fallback)
}

pub fn mark_default(mut v: Vec<Source>) -> Vec<Source> {
    if v.len() == 1 {
        v[0].default = true;
    }
    v
}

fn group(n: u64) -> String {
    let d = n.to_string();
    let mut out = String::new();
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(ch);
    }
    out
}

pub fn parse_payments(v: &Value) -> Vec<Payment> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let date = chrono::DateTime::parse_from_rfc3339(&s(p, "created_at")).map(|d| d.format("%m/%d/%Y").to_string()).unwrap_or_default();
            let cur = s(p, "currency").to_lowercase();
            let amount = p.get("amount").and_then(Value::as_u64).unwrap_or(0);
            let orbs = cur.len() != 3;
            let amount = if orbs { group(amount) } else { format!("{},{:02} {}", amount / 100, amount % 100, if cur == "eur" { "€" } else { "$" }) };
            let description = p
                .get("sku")
                .and_then(|k| k.get("name"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| s(p, "description"));
            Payment { date, description, amount, orbs }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn parses_sources_and_payments() {
        let s = super::parse_sources(&json!([{"type": 2, "email": "a@b.c", "default": true}, {"type": 1, "brand": "visa", "last_4": "4242"}]));
        assert_eq!(s[0].brand, "PayPal");
        assert!(s[0].default);
        assert_eq!(s[1].sub, "•••• 4242");
        assert_eq!(s[1].brand, "Visa");
        let p = super::parse_payments(&json!([{"created_at": "2026-10-18T10:00:00+00:00", "amount": 4100, "currency": "discord_orb", "description": "x"}]));
        assert_eq!(p[0].date, "10/18/2026");
        assert_eq!(p[0].amount, "4\u{202f}100");
        assert!(p[0].orbs);
    }
}
