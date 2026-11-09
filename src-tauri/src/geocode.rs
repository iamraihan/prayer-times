//! City search for the location picker, via the Open-Meteo geocoding API
//! (https://open-meteo.com/en/docs/geocoding-api — free, no key, GeoNames
//! data). The user picks a real place from the results, so the app stores
//! exact coordinates + timezone instead of a free-typed city name that the
//! prayer-times API might match to the wrong place.

use serde::Deserialize;

use crate::models::CityMatchDto;
use crate::timeutil;

pub const BASE: &str = "https://geocoding-api.open-meteo.com/v1/search";
const MAX_RESULTS: usize = 8;

#[derive(Debug, Deserialize)]
struct RawResponse {
    #[serde(default)]
    results: Vec<RawPlace>,
}

#[derive(Debug, Deserialize)]
struct RawPlace {
    name: String,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    country_code: Option<String>,
    #[serde(default)]
    admin1: Option<String>,
    #[serde(default)]
    timezone: Option<String>,
    #[serde(default)]
    population: Option<u32>,
    /// GeoNames feature code: "PPL*" are populated places; the API also
    /// returns airports, mountains, … which aren't useful here.
    #[serde(default)]
    feature_code: Option<String>,
}

pub fn search(
    client: &reqwest::blocking::Client,
    query: &str,
    country_code: &str,
) -> Result<Vec<CityMatchDto>, String> {
    let query = query.trim();
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let body = client
        .get(BASE)
        .query(&[
            ("name", query),
            ("countryCode", country_code),
            ("count", "20"),
            ("language", "en"),
            ("format", "json"),
        ])
        .send()
        .map_err(|e| format!("network error: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP error: {e}"))?
        .text()
        .map_err(|e| format!("bad response: {e}"))?;
    parse(&body, country_code)
}

/// Keeps populated places in the requested country with a usable timezone,
/// biggest first, without duplicates.
fn parse(body: &str, country_code: &str) -> Result<Vec<CityMatchDto>, String> {
    let raw: RawResponse = serde_json::from_str(body).map_err(|e| format!("bad response: {e}"))?;
    let mut out: Vec<CityMatchDto> = Vec::new();
    for place in raw.results {
        let is_town = place
            .feature_code
            .as_deref()
            .is_some_and(|c| c.starts_with("PPL"));
        let in_country = place
            .country_code
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case(country_code));
        let Some(timezone) = place.timezone.filter(|tz| timeutil::parse_tz(tz).is_some()) else {
            continue;
        };
        if !is_town || !in_country {
            continue;
        }
        let duplicate = out
            .iter()
            .any(|m| m.name == place.name && m.region == place.admin1);
        if duplicate {
            continue;
        }
        out.push(CityMatchDto {
            name: place.name,
            region: place.admin1,
            country_code: country_code.to_ascii_uppercase(),
            latitude: place.latitude,
            longitude: place.longitude,
            timezone,
            population: place.population,
        });
    }
    // Stable: the API's relevance order breaks ties.
    out.sort_by_key(|m| std::cmp::Reverse(m.population.unwrap_or(0)));
    out.truncate(MAX_RESULTS);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HYDERABAD: &str = r#"{"results":[
        {"id":1,"name":"Hyderabad","latitude":17.38405,"longitude":78.45636,"feature_code":"PPLA",
         "country_code":"IN","timezone":"Asia/Kolkata","population":6993262,"admin1":"Telangana"},
        {"id":2,"name":"Begumpet Airport","latitude":17.45,"longitude":78.46,"feature_code":"AIRP",
         "country_code":"IN","timezone":"Asia/Kolkata","admin1":"Telangana"},
        {"id":3,"name":"Hyderabad","latitude":25.39,"longitude":68.37,"feature_code":"PPLA2",
         "country_code":"PK","timezone":"Asia/Karachi","population":1386330,"admin1":"Sindh"},
        {"id":4,"name":"Hyderabad","latitude":17.4,"longitude":78.5,"feature_code":"PPL",
         "country_code":"IN","timezone":"Asia/Kolkata","admin1":"Telangana"},
        {"id":5,"name":"Hyderabad Colony","latitude":17.0,"longitude":78.0,"feature_code":"PPL",
         "country_code":"IN","timezone":"Not/AZone","admin1":"Telangana"}
    ]}"#;

    #[test]
    fn keeps_towns_in_the_country_only() {
        let matches = parse(HYDERABAD, "in").unwrap();
        assert_eq!(matches.len(), 1, "{matches:?}");
        let m = &matches[0];
        assert_eq!(m.name, "Hyderabad");
        assert_eq!(m.region.as_deref(), Some("Telangana"));
        assert_eq!(m.country_code, "IN");
        assert_eq!(m.timezone, "Asia/Kolkata");
        assert!((m.latitude - 17.38405).abs() < 1e-9);
    }

    #[test]
    fn biggest_places_first() {
        let body = r#"{"results":[
            {"name":"Kolki","latitude":1.0,"longitude":1.0,"feature_code":"PPL","country_code":"IN",
             "timezone":"Asia/Kolkata","population":8054,"admin1":"Maharashtra"},
            {"name":"Kolkata","latitude":22.56,"longitude":88.36,"feature_code":"PPLA","country_code":"IN",
             "timezone":"Asia/Kolkata","population":4631392,"admin1":"West Bengal"},
            {"name":"Kolka","latitude":2.0,"longitude":2.0,"feature_code":"PPL","country_code":"IN",
             "timezone":"Asia/Kolkata","admin1":"Himachal Pradesh"}
        ]}"#;
        let names: Vec<_> = parse(body, "IN").unwrap().into_iter().map(|m| m.name).collect();
        assert_eq!(names, ["Kolkata", "Kolki", "Kolka"]);
    }

    #[test]
    fn no_results_is_an_empty_list() {
        // What the API answers for a typo like "Londn" in GB.
        assert!(parse(r#"{"generationtime_ms":0.2}"#, "GB").unwrap().is_empty());
    }
}
