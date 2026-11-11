//! Place vocabulary: the tokens that name a location rather than an employer,
//! and the equivalences between spellings and languages of the same place.

/// Country/region/city tokens an employer's legal name — or an entry line's
/// location column — carries. Same argument as [`LEGAL_FORMS`]: "Deutschland"
/// or "Berlin" identifies no particular employer, so neither may be the SOLE
/// reason `validate::content` decides an entry survived, and a comma-tail made
/// of nothing but these is a location rather than a company (see
/// [`split_entry`]).
pub const GEOGRAPHY_TOKENS: &[&str] = &[
    "deutschland",
    "germany",
    "österreich",
    "austria",
    "schweiz",
    "switzerland",
    "europe",
    "europa",
    "emea",
    "apac",
    "dach",
    "international",
    "global",
    "worldwide",
    "berlin",
    "münchen",
    "munich",
    "hamburg",
    "wien",
    "zürich",
];

/// The same place under another spelling or language. The first entry of each
/// group is its canonical form; a translated résumé must not read as inventing
/// a place because it wrote "Germany" for "Deutschland" or "EU" for "Europe".
pub const GEOGRAPHY_EQUIVALENTS: &[&[&str]] = &[
    &["deutschland", "germany"],
    &["österreich", "austria"],
    &["schweiz", "switzerland"],
    &["europa", "europe", "eu"],
    &["münchen", "munich"],
    &["wien", "vienna"],
    &["zürich", "zurich"],
];

/// Canonical city to canonical country. A role that names a city has named its
/// country too, so "Berlin" in the source supports "Berlin, Germany".
pub const CITY_COUNTRY: &[(&str, &str)] = &[
    ("berlin", "deutschland"),
    ("münchen", "deutschland"),
    ("hamburg", "deutschland"),
    ("wien", "österreich"),
    ("zürich", "schweiz"),
];

/// Geography tokens that describe a company, not where a role was held.
pub const NON_PLACE_TOKENS: &[&str] = &[
    "international",
    "global",
    "worldwide",
    "emea",
    "apac",
    "dach",
];

/// `token` (lowercase) in its canonical spelling.
pub fn canonical_place(token: &str) -> String {
    GEOGRAPHY_EQUIVALENTS
        .iter()
        .find(|group| group.contains(&token))
        .map_or(token, |group| group[0])
        .to_string()
}

/// The canonical country of a canonical city.
pub fn country_of(city: &str) -> Option<&'static str> {
    CITY_COUNTRY
        .iter()
        .find(|(c, _)| *c == city)
        .map(|(_, country)| *country)
}
