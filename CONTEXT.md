# Context

Domain glossary. Terms and one-line meanings only; behaviour lives in the code and the ADRs.

## Ghost-job signals

- **Ghost signals**: Advisory hints that a posting may be stale or recycled. They never filter or hide a posting.
- **Open long**: The posting has been seen for a long time, measured from the first time we saw it.
- **Repost**: A posting that disappeared from a source that could have shown it, then came back there.
- **Source**: One board plus one search; the unit a repost is judged against.

## Export

- **ATS view**: What an applicant-tracking system extracts from an exported résumé, judged per field as found, missing or suspicious. Advisory only.

## Outcome insights

- **Positive reply**: An applied application that ever reached screening, interviewing, offer or accepted, whatever its status is now.
- **Rejected**: An applied application closed as rejected without ever reaching a positive stage.
- **No reply**: An applied application that has sat unanswered at applied past the cutoff (`NO_REPLY_AFTER_DAYS` in `insights/mod.rs`).
- **Pending**: An applied application not yet decided: still within the no-reply cutoff, or in progress without a positive stage.
- **Tailored**: Generated for that posting, as opposed to a résumé or letter reused as-is.
- **Volume nudge**: A gentle, advisory "slow down and tailor" signal when many recent applications were untailored or near-copies. Thresholds live in `insights/mod.rs`.
