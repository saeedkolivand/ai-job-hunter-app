//! The contact profile a résumé upload suggests, and the merge that seeds it. Pulled out of
//! `documents_import` so the seeding rules are unit-testable without an `AppHandle`.

use crate::contact_profile::{
    classify_contact_links, linkedin_from_text, ContactProfile, LocalizedText,
};
use crate::extraction::structured::StructuredResume;
use crate::extraction::types::{Confidence, ExtractedResume};

/// Links by NAME, a scheme-less plain-text LinkedIn the links pass can't see, a CONFIDENT
/// name (a Low/Medium guess would pollute an empty profile), and the structuring pass's
/// email / phone / location.
pub(super) fn suggest_contact(
    extraction: &ExtractedResume,
    structured: &StructuredResume,
) -> ContactProfile {
    let mut suggested = classify_contact_links(&extraction.links);
    if suggested.linkedin.is_none() {
        suggested.linkedin = linkedin_from_text(&extraction.text);
    }
    if structured.name.confidence == Confidence::High && !structured.name.value.trim().is_empty() {
        suggested.full_name = Some(structured.name.value.trim().to_string());
    }
    if let Some(email) = structured.email.as_ref().map(|f| f.value.clone()) {
        suggested.email.get_or_insert(email);
    }
    if let Some(phone) = structured.phone.as_ref().map(|f| f.value.clone()) {
        suggested.phone.get_or_insert(phone);
    }
    if let Some(loc) = structured.location.as_ref().map(|f| f.value.clone()) {
        if !loc.trim().is_empty() {
            suggested.location.get_or_insert_with(|| LocalizedText {
                default: loc,
                ..Default::default()
            });
        }
    }
    suggested
}

/// The profile to persist after an import: `current` with only its EMPTY fields filled from
/// `suggested`. `None` when there is nothing to seed. A name alone counts (it is not part of
/// `is_effectively_empty`, which gates the header).
pub(super) fn seeded_profile(
    current: &ContactProfile,
    suggested: &ContactProfile,
) -> Option<ContactProfile> {
    if suggested.is_effectively_empty() && suggested.full_name.is_none() {
        return None;
    }
    let mut merged = current.clone();
    merged.fill_empty_from(suggested);
    Some(merged)
}

#[cfg(test)]
mod tests;
