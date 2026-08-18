// The scrape form's shape and defaults live at the FEATURE level
// (`features/jobs/types`) so the session store, which owns `jobs.scrapeForm`,
// imports a feature module instead of reaching into this component's internals.
// Re-exported here so the ScrapeForm subtree keeps its short relative import.
export { makeScrapeFormDefaults, type ScrapeFormState } from '../../types';
