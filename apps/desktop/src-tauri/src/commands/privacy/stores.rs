//! `Resettable` impls for every managed persistent store (factory reset).

use std::sync::Arc;

use parking_lot::Mutex;

use crate::ai_config::AiConfigStore;
use crate::ai_generations::AiGenerationStore;
use crate::applications::ApplicationStore;
use crate::autopilot::AutopilotStore;
use crate::contact_profile::ContactProfileStore;
use crate::credentials::CredentialStore;
use crate::data_store::Resettable;
use crate::dedup::DedupStore;
use crate::discovered::DiscoveredCompanyStore;
use crate::documents::DocumentStore;
use crate::email_watch::EmailWatchStore;
use crate::error::AppResult;
use crate::job_preferences::JobPreferencesStore;
use crate::jobs::JobTracker;
use crate::notifications::NotificationStore;
use crate::pipeline::cache::KvCache;
use crate::pipeline::runs::PipelineRunStore;
use crate::portable_prefs::PortablePrefsStore;
use crate::postings::{InteractionStore, PostingsCache};
use crate::referrals::ReferralStore;
use crate::spend::SpendStore;
use crate::sync::state::SyncStateStore;

impl Resettable for Mutex<PostingsCache> {
    fn reset(&self) -> AppResult<()> {
        self.lock().clear_all();
        Ok(())
    }
}
impl Resettable for Mutex<InteractionStore> {
    fn reset(&self) -> AppResult<()> {
        self.lock().clear_all();
        Ok(())
    }
}
impl Resettable for Mutex<JobTracker> {
    fn reset(&self) -> AppResult<()> {
        self.lock().clear();
        Ok(())
    }
}
impl Resettable for Mutex<CredentialStore> {
    fn reset(&self) -> AppResult<()> {
        // Clears every stored secret (AI/provider keys incl. the Ollama account
        // key, board passwords), driven off the credential metadata index.
        self.lock().clear_all()
    }
}
impl Resettable for Arc<Mutex<AutopilotStore>> {
    fn reset(&self) -> AppResult<()> {
        self.lock().clear_all();
        Ok(())
    }
}
impl Resettable for DocumentStore {
    fn reset(&self) -> AppResult<()> {
        self.clear_all();
        Ok(())
    }
}
impl Resettable for AiGenerationStore {
    fn reset(&self) -> AppResult<()> {
        self.clear_all();
        Ok(())
    }
}
impl Resettable for ApplicationStore {
    fn reset(&self) -> AppResult<()> {
        self.clear_all();
        Ok(())
    }
}
impl Resettable for JobPreferencesStore {
    fn reset(&self) -> AppResult<()> {
        self.clear()
    }
}
impl Resettable for DedupStore {
    fn reset(&self) -> AppResult<()> {
        // Wipe every "not a duplicate" verdict — after a factory reset the
        // clusterer starts from a clean slate (ADR-029).
        self.clear_all();
        Ok(())
    }
}
impl Resettable for std::sync::Arc<crate::scraping::BoardHealthStore> {
    fn reset(&self) -> AppResult<()> {
        // Forget every board's reliability history — after a factory reset the
        // chips start from "unknown", not from the previous install's grudges.
        self.clear_all();
        Ok(())
    }
}
impl Resettable for crate::posting_history::PostingHistoryStore {
    fn reset(&self) -> AppResult<()> {
        // Forget every first/last-seen observation (ghost-job signals).
        self.clear_all();
        Ok(())
    }
}
impl Resettable for DiscoveredCompanyStore {
    fn reset(&self) -> AppResult<()> {
        // Wipe every passively-harvested ATS slug + watched-company star (ADR-030).
        self.clear_all();
        Ok(())
    }
}
impl Resettable for ContactProfileStore {
    fn reset(&self) -> AppResult<()> {
        self.clear()
    }
}
impl Resettable for PortablePrefsStore {
    fn reset(&self) -> AppResult<()> {
        self.clear()
    }
}
impl Resettable for AiConfigStore {
    fn reset(&self) -> AppResult<()> {
        self.clear();
        Ok(())
    }
}
impl Resettable for ReferralStore {
    fn reset(&self) -> AppResult<()> {
        self.clear_all();
        Ok(())
    }
}
impl Resettable for NotificationStore {
    fn reset(&self) -> AppResult<()> {
        self.clear_all();
        Ok(())
    }
}
impl Resettable for KvCache {
    fn reset(&self) -> AppResult<()> {
        self.clear();
        Ok(())
    }
}
impl Resettable for PipelineRunStore {
    fn reset(&self) -> AppResult<()> {
        // Both tables: the run rows AND their per-stage event trail. A reset
        // that left the events behind would keep a searchable record of which
        // postings the user ran, which is the thing being wiped.
        //
        // `clear_all` attempts both tables and logs a refusal itself (it has no
        // error return), so this store always reports `Ok`.
        self.clear_all();
        Ok(())
    }
}
impl Resettable for SpendStore {
    fn reset(&self) -> AppResult<()> {
        self.clear_all();
        Ok(())
    }
}
impl Resettable for EmailWatchStore {
    fn reset(&self) -> AppResult<()> {
        // Account row back to defaults (including `auto_write_enabled`,
        // unconditionally — see `EmailWatchStore::clear`'s own doc for why
        // this store no longer has a separate `factory_reset`: an earlier
        // round split the two on the reasoning that disconnect/reconnect
        // of the SAME mailbox should preserve the opt-in, but `clear()`
        // NULLs the address in the same statement that would preserve it,
        // so nothing survives to verify "same mailbox" by the time a later
        // `connect()` runs — the split hid an unverifiable premise instead
        // of fixing it) + every `seen` row gone. The keychain app password
        // is cleared separately by `Mutex<CredentialStore>`'s own `reset`
        // (registered under the "credentials" label), which wipes every
        // stored secret including this store's `email-imap` slot.
        self.clear()
    }
}

impl Resettable for SyncStateStore {
    fn reset(&self) -> AppResult<()> {
        // Sync settings, cursors, baselines and held envelopes. The token is
        // in the keychain and goes with "credentials". The device id stays.
        SyncStateStore::reset(self)
    }
}
