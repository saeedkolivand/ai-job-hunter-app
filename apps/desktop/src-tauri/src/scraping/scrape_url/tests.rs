//! `scrape_url` tests, split by concern: dispatch/SSRF gates, URL-shape +
//! generic-HTML parsing, the `[data-ajh-job-root]` hint and JSON-LD/
//! `__NEXT_DATA__` structured-data paths, Personio/`job_identity`, and
//! `canonical_job_url`/redirect re-dispatch.

mod canonical_url_and_redirect;
mod dispatch_and_ssrf;
mod html_fallback_ats;
mod html_fallback_basic;
mod html_fallback_greenhouse_page;
mod html_fallback_job_root_hint;
mod html_fallback_literal_newline;
mod html_fallback_structured_data;
mod html_fallback_thin_json_ld;
mod personio_and_identity;
mod url_parsing_and_generic_html;
