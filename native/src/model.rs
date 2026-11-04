use chrono::{Datelike, Local, NaiveDate};
use serde_json::Value;
pub fn rows(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    }
}
pub fn field(fields: &Value, key: &str) -> String {
    rows(fields)
        .iter()
        .find(|f| f["id"] == key)
        .map(|f| {
            text(
                f.get("value_id")
                    .filter(|v| !v.is_null())
                    .unwrap_or(&f["value"]),
            )
        })
        .unwrap_or_default()
}
pub fn display(fields: &Value, key: &str) -> String {
    rows(fields)
        .iter()
        .find(|f| f["id"] == key)
        .map(|f| {
            text(
                f.get("value")
                    .filter(|v| !v.is_null())
                    .unwrap_or(&f["value_id"]),
            )
        })
        .unwrap_or_default()
}
pub fn holder(card: &Value) -> String {
    let mut names = vec![];
    for f in rows(&card["holder"]) {
        if f["id"] == "holder.name" {
            let name = text(&f["value"]).replace('\0', "").trim().to_owned();
            if !name.is_empty() && !name.chars().all(|c| c == '0') && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if names.is_empty() {
        "O teu passe".into()
    } else {
        names.join(" ")
    }
}
pub fn money(cents: i64) -> String {
    format!("{},{:02} €", cents / 100, cents % 100)
}
pub fn date(value: &str) -> String {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|d| d.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|_| value.to_owned())
}
pub fn product_name(item: &Value) -> String {
    let name = display(&item["contract"], "contract.name");
    if name.is_empty() {
        text(&item["product_detail"]["product"]["name"])
    } else {
        name
    }
}
pub fn stored(item: &Value) -> bool {
    let name = product_name(item).to_lowercase();
    item["product_detail"]["product"]["id"] == "id-prod-zapping"
        || name.contains("zapping")
        || name.contains("pré-pago")
        || rows(&item["contract"])
            .iter()
            .any(|f| f["id"] == "contract.amount")
}
pub fn monthly(id: &str, name: &str) -> bool {
    id.strip_prefix("id-prod-navegante-").is_some_and(|city| {
        matches!(
            city,
            "metro"
                | "alcochete"
                | "almada"
                | "amadora"
                | "barreiro"
                | "cascais"
                | "lisboa"
                | "loures"
                | "mafra"
                | "moita"
                | "montijo"
                | "odivelas"
                | "oeiras"
                | "palmela"
                | "seixal"
                | "sesimbra"
                | "setubal"
                | "sintra"
                | "vfxira"
        )
    }) || name.to_lowercase().contains("metropolitano")
        || name.to_lowercase().contains("municipal")
}
pub fn period(item: &Value) -> Option<(i32, u32)> {
    let f = &item["contract"];
    let year = field(f, "contract.temporal_validity.year");
    let month = field(f, "contract.temporal_validity.month");
    if let (Ok(y), Ok(m)) = (year.get(..4).unwrap_or("").parse(), month.parse::<u32>())
        && (1..=12).contains(&m)
    {
        return Some((y, m));
    }
    let id = text(&item["product_detail"]["product"]["id"]);
    if monthly(&id, &product_name(item)) {
        let d = NaiveDate::parse_from_str(&field(f, "contract.temporal_validity.date"), "%Y-%m-%d")
            .ok()?;
        return Some((d.year(), d.month()));
    }
    None
}
pub fn title_state(item: &Value, today: NaiveDate) -> &'static str {
    let status = field(&item["contract"], "contract.temporal_validity.status");
    if let Some(p) = period(item) {
        if p > (today.year(), today.month()) {
            return "Próximo mês";
        }
        if p < (today.year(), today.month()) {
            return "Expirado";
        }
    }
    if status == "3" || status.eq_ignore_ascii_case("EXPIRED") {
        return "Expirado";
    }
    let expiry = field(&item["contract"], "contract.temporal_validity.date");
    if let Ok(d) = NaiveDate::parse_from_str(&expiry, "%Y-%m-%d")
        && d < today
    {
        return "Expirado";
    }
    if matches!(status.as_str(), "1" | "2" | "VALID" | "EXPIRING") {
        "Válido"
    } else {
        "Validade por confirmar"
    }
}
pub fn products(group: &Value, out: &mut Vec<Value>) {
    out.extend(rows(&group["products"]).iter().cloned());
    for g in rows(&group["groups"]) {
        products(g, out)
    }
}
pub fn option_label(details: &Value, price: &Value) -> String {
    let mut names = vec![];
    for a in rows(&details["attributes"]) {
        for v in rows(&a["values"]) {
            if v["id"] == price["id"] {
                let value = text(&v["value"]);
                if a["id"] == "product_monthly"
                    && let Ok(month) = value.parse::<usize>()
                    && (1..=12).contains(&month)
                {
                    names.push(
                        [
                            "janeiro",
                            "fevereiro",
                            "março",
                            "abril",
                            "maio",
                            "junho",
                            "julho",
                            "agosto",
                            "setembro",
                            "outubro",
                            "novembro",
                            "dezembro",
                        ][month - 1]
                            .to_owned(),
                    );
                    continue;
                }
                if a["id"] == "product_occasional"
                    && let Ok(cents) = value.parse::<i64>()
                {
                    names.push(format!("Saldo de {}", money(cents)));
                    continue;
                }
                let label = text(
                    v.get("name")
                        .or_else(|| v.get("description"))
                        .unwrap_or(&v["value"]),
                );
                if !label.is_empty() {
                    names.push(label);
                }
            }
        }
    }
    if names.is_empty() {
        let s = text(&price["description"]);
        if !s.is_empty() {
            names.push(s)
        }
    }
    names.join(" · ")
}
pub fn blocked(
    card: &Value,
    pid: &str,
    details: &Value,
    variant: &Value,
    today: NaiveDate,
) -> Option<String> {
    let attr = rows(&details["attributes"])
        .iter()
        .find(|a| a["id"] == "product_monthly")?;
    let option = rows(&attr["values"]).iter().find(|v| v["id"] == *variant)?;
    let value = text(&option["value"]);
    let target = if value.len() <= 2 {
        let month = value.parse::<u32>().ok()?;
        if !(1..=12).contains(&month) {
            return None;
        }
        let year = (today.year() - 1..=today.year() + 1).min_by_key(|y| {
            ((y - today.year()) * 12 + month as i32 - today.month() as i32).abs()
        })?;
        (year, month)
    } else {
        let d =
            NaiveDate::parse_from_str(&(value.get(..7)?.to_owned() + "-01"), "%Y-%m-%d").ok()?;
        (d.year(), d.month())
    };
    for item in rows(&card["contracts"]) {
        if matches!(
            field(&item["contract"], "contract.temporal_validity.status")
                .to_uppercase()
                .as_str(),
            "3" | "EXPIRED"
        ) || period(item) != Some(target)
        {
            continue;
        }
        let name = product_name(item);
        let existing = text(&item["product_detail"]["product"]["id"]);
        let same = (!existing.is_empty() && existing == pid)
            || (!name.is_empty() && name.eq_ignore_ascii_case(&text(&details["name"])));
        let metro =
            existing == "id-prod-navegante-metro" || name.to_lowercase().contains("metropolitano");
        if same || (metro && monthly(pid, &text(&details["name"]))) {
            return Some(
                "Já tens um passe que cobre este período. Escolhe outro período ou adiciona saldo."
                    .into(),
            );
        }
    }
    None
}
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn future_not_valid() {
        let item = json!({"product_detail":{"product":{"id":"id-prod-navegante-metro"}},"contract":[{"id":"contract.temporal_validity.date","value_id":"2026-12-31"},{"id":"contract.temporal_validity.status","value_id":"1"}]});
        assert_eq!(
            title_state(&item, NaiveDate::from_ymd_opt(2026, 11, 4).unwrap()),
            "Próximo mês"
        );
    }
    #[test]
    fn placeholder_holder() {
        assert_eq!(
            holder(
                &json!({"holder":[{"id":"holder.name","value":"Tiago Dias"},{"id":"holder.name","value":"0"}]})
            ),
            "Tiago Dias"
        );
    }
    #[test]
    fn exact_cents() {
        assert_eq!(money(1235), "12,35 €");
    }
    #[test]
    fn metro_blocks_same_month_municipal_but_not_next_month_or_balance() {
        let card = json!({"contracts":[{"product_detail":{"product":{"id":"id-prod-navegante-metro","name":"navegante Metropolitano"}},"contract":[{"id":"contract.temporal_validity.date","value_id":"2026-11-30"},{"id":"contract.temporal_validity.status","value_id":"1"}]}]});
        let details = json!({"name":"navegante Municipal Lisboa","attributes":[{"id":"product_monthly","values":[{"id":"nov","value":"11"},{"id":"dec","value":"12"}]}]});
        let now = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        assert!(
            blocked(
                &card,
                "id-prod-navegante-lisboa",
                &details,
                &json!("nov"),
                now
            )
            .is_some()
        );
        assert!(
            blocked(
                &card,
                "id-prod-navegante-lisboa",
                &details,
                &json!("dec"),
                now
            )
            .is_none()
        );
        assert!(blocked(&card, "id-prod-zapping", &json!({}), &json!("ten"), now).is_none());
    }
    #[test]
    fn january_rollover_and_month_label() {
        let card = json!({"contracts":[{"product_detail":{"product":{"id":"id-prod-navegante-metro"}},"contract":[{"id":"contract.temporal_validity.month","value_id":"1"},{"id":"contract.temporal_validity.year","value_id":"2027"}]}]});
        let details = json!({"name":"navegante Metropolitano","attributes":[{"id":"product_monthly","values":[{"id":"jan","value":"1"}]}]});
        assert!(
            blocked(
                &card,
                "id-prod-navegante-metro",
                &details,
                &json!("jan"),
                NaiveDate::from_ymd_opt(2026, 12, 28).unwrap()
            )
            .is_some()
        );
        assert_eq!(option_label(&details, &json!({"id":"jan"})), "janeiro");
    }
}
