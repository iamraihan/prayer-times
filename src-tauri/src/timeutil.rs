//! Timezone-aware helpers. All scheduling math happens in the *location's*
//! IANA timezone (from the Aladhan API), not the device timezone.

use chrono::{DateTime, LocalResult, NaiveDate, NaiveTime};
use chrono_tz::Tz;

pub fn parse_tz(name: &str) -> Option<Tz> {
    name.parse().ok()
}

/// Combines a date + wall-clock time into an instant in `tz`, tolerating
/// DST gaps/overlaps (first candidate wins on ambiguity).
pub fn resolve(tz: Tz, date: NaiveDate, time: NaiveTime) -> Option<DateTime<Tz>> {
    match date.and_time(time).and_local_timezone(tz) {
        LocalResult::Single(dt) => Some(dt),
        LocalResult::Ambiguous(dt, _) => Some(dt),
        LocalResult::None => None,
    }
}

/// Like [`resolve`] but rolls to the next day when `time` is earlier than
/// `earlier_than` (e.g. Isha ending at Midnight "00:30" of the next day).
pub fn resolve_after(
    tz: Tz,
    date: NaiveDate,
    time: NaiveTime,
    earlier_than: NaiveTime,
) -> Option<DateTime<Tz>> {
    if time >= earlier_than {
        resolve(tz, date, time)
    } else {
        let next_day = date.succ_opt()?;
        resolve(tz, next_day, time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONDON: Tz = chrono_tz::Europe::London;

    fn d(ymd: &str) -> NaiveDate {
        NaiveDate::parse_from_str(ymd, "%Y-%m-%d").unwrap()
    }

    fn t(hm: &str) -> NaiveTime {
        NaiveTime::parse_from_str(hm, "%H:%M").unwrap()
    }

    #[test]
    fn parses_iana_names_only() {
        assert_eq!(parse_tz("Asia/Dhaka"), Some(chrono_tz::Asia::Dhaka));
        assert_eq!(parse_tz("Not/AZone"), None);
    }

    #[test]
    fn resolves_plain_wall_clock() {
        let dt = resolve(LONDON, d("2026-11-09"), t("12:00")).unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-11-09T12:00:00+00:00");
    }

    #[test]
    fn dst_gap_has_no_instant() {
        // 2026-03-29 01:30 does not exist in London (clocks jump 01:00 → 02:00).
        assert!(resolve(LONDON, d("2026-03-29"), t("01:30")).is_none());
    }

    #[test]
    fn dst_overlap_takes_first_instant() {
        // 2026-10-25 01:30 happens twice in London; BST (+01:00) comes first.
        let dt = resolve(LONDON, d("2026-10-25"), t("01:30")).unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-10-25T01:30:00+01:00");
    }

    #[test]
    fn resolve_after_rolls_to_next_day_only_when_earlier() {
        let same = resolve_after(LONDON, d("2026-06-20"), t("23:30"), t("22:55")).unwrap();
        assert_eq!(same.date_naive(), d("2026-06-20"));
        let next = resolve_after(LONDON, d("2026-06-20"), t("01:05"), t("22:55")).unwrap();
        assert_eq!(next.date_naive(), d("2026-06-21"));
    }
}
