/// ScraperEngine — in-process scraper orchestrator.
///
/// Uses interior mutability so `Arc<ScraperEngine>` can be cloned into Tauri
/// commands and scrape jobs run concurrently (bounded by `semaphore`) without
/// serializing on an outer mutex.
use super::boards::*;
use super::types::{BoardSearchInput, JobPosting, Scraper, ScrapeContext};
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScraperCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub mode: String,
}

#[derive(Debug, Clone)]
pub struct ScraperRuntimeHealth {
    pub mode: String,
    pub scrapers: Vec<ScraperCatalogEntry>,
    pub ready: bool,
}

pub struct ScraperEngine {
    /// Bounded concurrency — swapped on `set_performance_mode`. Holding an
    /// owned permit for the duration of a scrape lets us shrink the limit
    /// without cancelling running work.
    semaphore: ArcSwap<Semaphore>,
    /// Active jobs keyed by job_id. Used by `cancel(job_id)`.
    jobs: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl ScraperEngine {
    pub fn new() -> Self {
        Self {
            semaphore: ArcSwap::from_pointee(Semaphore::new(2)),
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn catalog(&self) -> Vec<ScraperCatalogEntry> {
        const ROWS: &[(&str, &str, &str)] = &[
            ("ycombinator", "Y Combinator", "http"),
            ("remotive", "Remotive", "http"),
            ("remoteok", "RemoteOK", "http"),
            ("wwr", "We Work Remotely", "http"),
            ("arbeitnow", "Arbeitnow", "http"),
            ("berlinstartupjobs", "Berlin Startup Jobs", "http"),
            ("germantechjobs", "German Tech Jobs", "http"),
            ("greenhouse", "Greenhouse", "http"),
            ("lever", "Lever", "http"),
            ("stepstone", "StepStone", "http"),
            ("smartrecruiters", "SmartRecruiters", "http"),
            ("personio", "Personio", "http"),
            ("recruitee", "Recruitee", "http"),
            ("workday", "Workday", "http"),
            ("ashby", "Ashby", "http"),
            ("arbeitsagentur", "Arbeitsagentur", "http"),
            ("linkedin", "LinkedIn", "http"),
            ("indeed", "Indeed", "browser"),
            ("xing", "Xing", "browser"),
            ("glassdoor", "Glassdoor", "browser"),
        ];
        ROWS.iter()
            .map(|(id, name, mode)| ScraperCatalogEntry {
                id: (*id).to_string(),
                display_name: (*name).to_string(),
                mode: (*mode).to_string(),
            })
            .collect()
    }

    pub fn health(&self) -> ScraperRuntimeHealth {
        ScraperRuntimeHealth {
            mode: "in-process".to_string(),
            scrapers: self.catalog(),
            ready: true,
        }
    }

    pub async fn scrape_board(
        &self,
        board: &str,
        input: BoardSearchInput,
        job_id: String,
        on_progress: Option<Box<dyn Fn(f32) + Send>>,
        on_item: Option<Box<dyn Fn(JobPosting) + Send>>,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Bound concurrency — acquire owned permit so the limit can be reduced
        // mid-flight without aborting this job.
        let sem = self.semaphore.load_full();
        let _permit = sem
            .acquire_owned()
            .await
            .map_err(|_| anyhow::anyhow!("scraper engine semaphore closed"))?;

        // Register cancellation token under job_id so `cancel(job_id)` works.
        let token = CancellationToken::new();
        {
            let mut jobs = self.jobs.lock().await;
            jobs.insert(job_id.clone(), token.clone());
        }

        let ctx = ScrapeContext {
            signal: token,
            on_progress,
            on_item,
        };

        let result = match board {
            "ycombinator" => Scraper::search(&YCombinatorScraper, input, ctx).await,
            "remotive" => Scraper::search(&RemotiveScraper, input, ctx).await,
            "remoteok" => Scraper::search(&RemoteOkScraper, input, ctx).await,
            "wwr" => Scraper::search(&WeWorkRemotelyScraper, input, ctx).await,
            "arbeitnow" => Scraper::search(&ArbeitnowScraper, input, ctx).await,
            "berlinstartupjobs" => Scraper::search(&BerlinStartupJobsScraper, input, ctx).await,
            "germantechjobs" => Scraper::search(&GermanTechJobsScraper, input, ctx).await,
            "greenhouse" => Scraper::search(&GreenhouseScraper, input, ctx).await,
            "lever" => Scraper::search(&LeverScraper, input, ctx).await,
            "stepstone" => Scraper::search(&StepStoneScraper, input, ctx).await,
            "smartrecruiters" => Scraper::search(&SmartRecruitersScraper, input, ctx).await,
            "personio" => Scraper::search(&PersonioScraper, input, ctx).await,
            "recruitee" => Scraper::search(&RecruiteeScraper, input, ctx).await,
            "workday" => Scraper::search(&WorkdayScraper, input, ctx).await,
            "ashby" => Scraper::search(&AshbyScraper, input, ctx).await,
            "arbeitsagentur" => Scraper::search(&ArbeitsagenturScraper, input, ctx).await,
            "linkedin" => Scraper::search(&LinkedInScraper, input, ctx).await,
            "indeed" => Scraper::search(&IndeedScraper, input, ctx).await,
            "xing" => Scraper::search(&XingScraper, input, ctx).await,
            "glassdoor" => Scraper::search(&GlassdoorScraper, input, ctx).await,
            _ => Err(anyhow::anyhow!("Unknown board: {}", board)),
        };

        // Always clear the token slot, even on error.
        self.jobs.lock().await.remove(&job_id);
        result
    }

    /// Signal cancellation to a running job by id. No-op if the id is unknown.
    pub async fn cancel(&self, job_id: &str) {
        let mut jobs = self.jobs.lock().await;
        if let Some(token) = jobs.remove(job_id) {
            token.cancel();
        }
    }

    /// Register a job token so it can be reached by `cancel(job_id)`. Used by
    /// the apply flow, which manages its own token outside `scrape_board`.
    pub async fn register_token(&self, job_id: &str, token: CancellationToken) {
        self.jobs.lock().await.insert(job_id.to_string(), token);
    }

    /// Remove a registered token. Idempotent.
    pub async fn unregister_token(&self, job_id: &str) {
        self.jobs.lock().await.remove(job_id);
    }

    /// Resize the concurrency limit. Already-running jobs keep their permits
    /// until they finish; new jobs are bounded by the new value.
    pub fn set_performance_mode(&self, mode: &str) {
        let n = match mode {
            "low-memory" => 1,
            "performance" => 4,
            _ => 2,
        };
        self.semaphore.store(Arc::new(Semaphore::new(n)));
    }

    /// Cancel every in-flight job. Called on app exit.
    #[allow(dead_code)]
    pub async fn shutdown(&self) {
        let mut jobs = self.jobs.lock().await;
        for token in jobs.values() {
            token.cancel();
        }
        jobs.clear();
    }
}

impl Default for ScraperEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test;
