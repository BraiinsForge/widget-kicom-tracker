use crate::model::finished;
use bmc_wasm_sdk::{JsonDoc, JsonKind, parse_datetime};

#[derive(Clone)]
pub(crate) struct Person {
    pub(crate) name: String,
    pub(crate) position: String,
    pub(crate) avatar: String,
}

#[derive(Clone)]
pub(crate) struct Bet {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) deadline: String,
    pub(crate) subject: String,
    pub(crate) people: Vec<Person>,
    pub(crate) stake: String,
    pub(crate) outcome: String,
    pub(crate) metrics: Vec<String>,
    pub(crate) closed: Option<i64>,
}

pub(crate) fn date(value: &str) -> Option<i64> {
    parse_datetime(value).or_else(|| parse_datetime(&format!("{value}T00:00:00Z")))
}

pub(crate) fn parse(body: &[u8]) -> Option<(Vec<Bet>, String)> {
    let doc = JsonDoc::parse(body);
    if doc.f64("/data/schema_version") != Some(1.0) {
        return None;
    }
    let generated = doc.str("/data/generated_at")?;
    date(&generated)?;
    let count = doc.len("/data/wagers")?;
    if doc.kind("/data/wagers") != Some(JsonKind::Array) || count > 256 || count == 0 {
        return None;
    }
    // Validate all required identifiers before changing persistent state.
    let mut ids = std::collections::BTreeSet::new();
    for i in 0..count {
        let p = format!("/data/wagers/{i}");
        let id = doc.str(&format!("{p}/id"))?;
        if id.is_empty() || id.len() > 100 || !ids.insert(id) {
            return None;
        }
        doc.str(&format!("{p}/title"))?;
        doc.str(&format!("{p}/status"))?;
        doc.str(&format!("{p}/claim"))?;
        doc.str(&format!("{p}/outcome_note"))?;
        date(&doc.str(&format!("{p}/deadline"))?)?;
    }
    let mut bets = Vec::new();
    for i in 0..count {
        let p = format!("/data/wagers/{i}");
        let get = |field: &str| doc.str(&format!("{p}/{field}")).unwrap_or_default();
        let closed = match doc.kind(&format!("{p}/closed_at")) {
            None | Some(JsonKind::Null) => None,
            _ => Some(date(&doc.str(&format!("{p}/closed_at"))?)?),
        };
        let mut metrics = Vec::new();
        let value = get("metric/value");
        if !value.is_empty() {
            metrics.push(format!(
                "{}: {} · {}",
                get("metric/label"),
                value,
                get("metric/detail")
            ));
        }
        let mut people = vec![Person {
            name: doc
                .str(&format!("{p}/kicom/name"))
                .unwrap_or_else(|| "Kicom".into()),
            position: get("kicom/position"),
            avatar: get("kicom/avatar"),
        }];
        for j in 0..doc.len(&format!("{p}/counterparties")).unwrap_or(0).min(16) {
            people.push(Person {
                name: get(&format!("counterparties/{j}/name")),
                position: get(&format!("counterparties/{j}/position")),
                avatar: get(&format!("counterparties/{j}/avatar")),
            });
        }
        bets.push(Bet {
            id: get("id"),
            title: get("title"),
            status: get("status"),
            deadline: get("deadline"),
            subject: get("claim"),
            people,
            stake: get("stake"),
            outcome: get("outcome_note"),
            metrics,
            closed,
        });
    }
    bets.sort_by(|a, b| {
        finished(&a.status)
            .cmp(&finished(&b.status))
            .then(a.deadline.cmp(&b.deadline))
            .then(a.id.cmp(&b.id))
    });
    Some((bets, generated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{avatar_url, visible};

    const FEED: &str = include_str!("../tests/fixtures/wagers-v1.json");

    #[test]
    fn partner_feed_preserves_content_participants_and_retention() {
        let (bets, generated) = parse(FEED.as_bytes()).unwrap();
        assert_eq!(bets.len(), 13);
        assert_eq!(generated, "2026-11-22T09:04:32Z");
        let coin = bets.iter().find(|b| b.id == "vzorkoin-cena-2026").unwrap();
        assert_eq!(coin.title, "Vzorkoin nad 2 000 Kč");
        assert_eq!(coin.deadline, "2026-12-31");
        assert!(coin.subject.starts_with("Fiktivní token Vzorkoin"));
        assert!(coin.outcome.starts_with("Rozhoduje závěrečný kurz"));
        assert_eq!(coin.stake, "10 000 Kč na charitativní účely");
        assert!(coin.metrics[0].contains("1 412 Kč"));
        assert_eq!(coin.people[0].position, "NE");
        assert!(avatar_url(&coin.people[0].avatar).is_some());
        let multi = bets.iter().find(|b| b.id == "drony-100-2026").unwrap();
        assert_eq!(multi.people.len(), 3);
        assert_eq!(multi.people[2].name, "Klára Pokusná");
        assert_eq!(multi.closed, date("2026-10-27"));
        let now = date("2026-11-23").unwrap();
        assert_eq!(
            bets.iter()
                .filter(|b| visible(&b.status, b.closed, now, 3))
                .count(),
            6
        );
    }

    #[test]
    fn nullable_fields_and_review_remain_supported() {
        let input = FEED
            .replace("\"status\": \"open\"", "\"status\": \"review\"")
            .replace("\"position\": \"NE\"", "\"position\": null")
            .replace(
                "\"avatar\": \"/api/v1/data/kicomtracker/avatar/kicom-jpg.jpg\"",
                "\"avatar\": null",
            )
            .replace("\"stake\": \"Nedoloženo\"", "\"stake\": null");
        let (bets, _) = parse(input.as_bytes()).unwrap();
        let review = bets.iter().find(|b| b.status == "review").unwrap();
        assert!(visible(&review.status, review.closed, i64::MAX, 0));
        assert!(review.people[0].avatar.is_empty());
        assert!(review.people[0].position.is_empty());
        let draw = bets.iter().find(|b| b.id == "kolo-dodavky-2023").unwrap();
        assert!(draw.stake.is_empty());
        assert!(draw.metrics.is_empty());
    }

    #[test]
    fn incompatible_or_malformed_snapshots_do_not_replace_cache() {
        for input in [
            FEED.replace("\"schema_version\": 1", "\"schema_version\": 2"),
            FEED.replace("\"schema_version\": 1", "\"schema_version\": null"),
            FEED.replace("\"schema_version\": 1", "\"schema_version\": \"1\""),
            FEED.replace("\"outcome_note\":", "\"outcomeNote\":"),
            FEED.replace(
                "\"closed_at\": \"2024-11-13\"",
                "\"closed_at\": \"invalid\"",
            ),
            FEED.replace(
                "\"id\": \"vzorkoin-top10\"",
                "\"id\": \"vzorkoin-cena-2026\"",
            ),
            FEED.replace("\"data\": {", "\"feed\": {"),
        ] {
            assert!(parse(input.as_bytes()).is_none());
        }
    }
}
