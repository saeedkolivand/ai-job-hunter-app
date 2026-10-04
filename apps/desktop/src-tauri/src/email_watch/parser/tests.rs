//! Tests for the pure email parser: the subject fingerprint and sender-domain gates
//! ([`fingerprint`]), the `Authentication-Results`/DMARC write gate ([`dmarc`]), and company/title
//! candidate extraction plus header/body decoding ([`candidates`]).

use super::*;

mod candidates;
mod dmarc;
mod fingerprint;
