//! Which countries an IANA timezone is used in, from the tzdb's own tables
//! (`data/zone.tab` + `data/zone1970.tab`, public domain, vendored so the
//! answer doesn't depend on the machine). Used to catch a city lookup that
//! matched a place in another country ("Londn, GB" → America/New_York).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const ZONE_TAB: &str = include_str!("../data/zone.tab");
const ZONE1970_TAB: &str = include_str!("../data/zone1970.tab");

fn table() -> &'static HashMap<&'static str, HashSet<&'static str>> {
    static TABLE: OnceLock<HashMap<&'static str, HashSet<&'static str>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map: HashMap<&str, HashSet<&str>> = HashMap::new();
        // Both files: `codes<TAB>coordinates<TAB>zone[<TAB>comment]`, where
        // zone.tab has one code and zone1970.tab a comma-separated list.
        for line in ZONE_TAB.lines().chain(ZONE1970_TAB.lines()) {
            if line.starts_with('#') {
                continue;
            }
            let mut cols = line.split('\t');
            let (Some(codes), Some(_), Some(zone)) = (cols.next(), cols.next(), cols.next())
            else {
                continue;
            };
            map.entry(zone).or_default().extend(codes.split(','));
        }
        map
    })
}

/// `Some(true)` when `zone` is used in the country with ISO code
/// `country_code`, `Some(false)` when it is only used elsewhere, `None` when
/// the tables don't know the zone (aliases like "Asia/Calcutta", "UTC") —
/// callers treat that as "can't tell", never as a mismatch.
pub fn zone_in_country(zone: &str, country_code: &str) -> Option<bool> {
    let countries = table().get(zone)?;
    Some(countries.contains(country_code.to_ascii_uppercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_zones() {
        assert_eq!(zone_in_country("Europe/London", "GB"), Some(true));
        assert_eq!(zone_in_country("Asia/Dhaka", "BD"), Some(true));
        assert_eq!(zone_in_country("Asia/Kolkata", "in"), Some(true));
        // Countries with several zones.
        assert_eq!(zone_in_country("America/Chicago", "US"), Some(true));
        assert_eq!(zone_in_country("Asia/Makassar", "ID"), Some(true));
        // Zones shared across borders (zone1970.tab): Büsingen, Germany.
        assert_eq!(zone_in_country("Europe/Zurich", "DE"), Some(true));
    }

    #[test]
    fn mismatched_zones() {
        assert_eq!(zone_in_country("America/New_York", "GB"), Some(false));
        assert_eq!(zone_in_country("Asia/Kolkata", "BD"), Some(false));
        assert_eq!(zone_in_country("Asia/Karachi", "IN"), Some(false));
    }

    #[test]
    fn unknown_zones_are_not_a_mismatch() {
        assert_eq!(zone_in_country("Asia/Calcutta", "IN"), None);
        assert_eq!(zone_in_country("UTC", "GB"), None);
        assert_eq!(zone_in_country("", "GB"), None);
    }
}
