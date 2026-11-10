//! Posting identity review on found jobs (issue #1445). A freshly resolved
//! posting that no longer matches a stored row's identity is NOT written onto
//! it: the row keeps its content and carries an [`IdentityReview`] until the
//! user dismisses it. Dismissing stores the resolved identity's fingerprint so
//! the same drift does not re-flag on the next refresh.

use serde::{Deserialize, Serialize};

use super::FoundJob;
use crate::db::now_ms;
use crate::scraping::scrape_url::{
    identity_fingerprint, identity_verdict, IdentityVerdict, PostingIdentity,
};

/// What the stored row's url resolved to this time, kept beside the old content
/// so the user can compare the two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityReview {
    pub resolved_title: String,
    pub resolved_company: String,
    pub resolved_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_external_id: Option<String>,
    pub detected_at: u64,
}

/// The identity of a freshly resolved posting, as handed in by a write-back
/// caller (the renderer's resolved posting or the LinkedIn enrichment pass).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedIdentity {
    pub title: String,
    pub company: String,
    pub url: String,
    #[serde(default)]
    pub external_id: Option<String>,
}

/// Per-call tally of a resolved-description write-back.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ResolveWrite {
    /// Rows that received the description.
    pub updated: u32,
    /// Rows held for identity review instead.
    pub drifted: u32,
}

impl ResolvedIdentity {
    pub fn as_posting(&self) -> PostingIdentity<'_> {
        PostingIdentity {
            title: &self.title,
            company: &self.company,
            url: &self.url,
        }
    }
}

impl IdentityReview {
    fn fingerprint(&self) -> String {
        identity_fingerprint(PostingIdentity {
            title: &self.resolved_title,
            company: &self.resolved_company,
            url: &self.resolved_url,
        })
    }
}

impl FoundJob {
    /// Run the identity verdict of `resolved` against this row. Same clears a
    /// stale review; Drift sets one unless the user already dismissed this
    /// exact drift. Returns whether the resolved posting is the same advert.
    pub(crate) fn check_resolved(&mut self, resolved: &ResolvedIdentity) -> bool {
        let stored = PostingIdentity {
            title: &self.title,
            company: &self.company,
            url: &self.url,
        };
        if identity_verdict(stored, resolved.as_posting()) == IdentityVerdict::Same {
            self.identity_review = None;
            return true;
        }
        let dismissed = self.identity_dismissed.as_deref()
            == Some(identity_fingerprint(resolved.as_posting()).as_str());
        if !dismissed {
            self.identity_review = Some(IdentityReview {
                resolved_title: resolved.title.clone(),
                resolved_company: resolved.company.clone(),
                resolved_url: resolved.url.clone(),
                resolved_external_id: resolved.external_id.clone(),
                detected_at: now_ms(),
            });
        }
        false
    }

    /// Dismiss the active review: the row is released and the fingerprint
    /// remembered. Returns whether there was a review to dismiss.
    pub(crate) fn dismiss_identity_review(&mut self) -> bool {
        match self.identity_review.take() {
            Some(review) => {
                self.identity_dismissed = Some(review.fingerprint());
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests;
