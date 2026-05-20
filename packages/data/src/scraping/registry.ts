import type { Scraper } from './base.js';
import { LinkedInScraper } from './boards/linkedin.js';
import { IndeedScraper } from './boards/indeed.js';
import { StepStoneScraper } from './boards/stepstone.js';
import { GreenhouseScraper } from './boards/greenhouse.js';
import { LeverScraper } from './boards/lever.js';
import { WorkdayScraper } from './boards/workday.js';
import { AshbyScraper } from './boards/ashby.js';
import { SmartRecruitersScraper } from './boards/smartrecruiters.js';
import { RecruiteeScraper } from './boards/recruitee.js';
import { PersonioScraper } from './boards/personio.js';
import { RemoteOkScraper } from './boards/remoteok.js';
import { RemotiveScraper } from './boards/remotive.js';
import { ArbeitnowScraper } from './boards/arbeitnow.js';
import { WeWorkRemotelyScraper } from './boards/wwr.js';
import { YCombinatorScraper } from './boards/ycombinator.js';
import { ArbeitsagenturScraper } from './boards/arbeitsagentur.js';
import { BerlinStartupJobsScraper } from './boards/berlinstartupjobs.js';
import { GermanTechJobsScraper } from './boards/germantechjobs.js';
import { XingScraper } from './boards/xing.js';

export class ScraperRegistry {
  private readonly scrapers = new Map<string, Scraper>();

  constructor() {
    this.registerDefaults();
  }

  private registerDefaults(): void {
    [
      // Browser-required (authenticated)
      new IndeedScraper(),
      new XingScraper(),
      // Major boards (HTTP)
      new LinkedInScraper(),
      new StepStoneScraper(),
      // German / DACH
      new ArbeitsagenturScraper(),
      new BerlinStartupJobsScraper(),
      new GermanTechJobsScraper(),
      // ATS platforms
      new GreenhouseScraper(),
      new LeverScraper(),
      new AshbyScraper(),
      new SmartRecruitersScraper(),
      new RecruiteeScraper(),
      new PersonioScraper(),
      new WorkdayScraper(),
      // Remote-first / aggregators
      new RemoteOkScraper(),
      new RemotiveScraper(),
      new ArbeitnowScraper(),
      new WeWorkRemotelyScraper(),
      new YCombinatorScraper(),
    ].forEach((s) => this.register(s));
  }

  register(s: Scraper): void {
    this.scrapers.set(s.id, s);
  }

  get(id: string): Scraper | undefined {
    return this.scrapers.get(id);
  }

  list(): Scraper[] {
    return [...this.scrapers.values()];
  }

  /** Convenience for the UI / command palette. */
  catalog(): Array<{ id: string; displayName: string; mode: 'http' | 'browser' }> {
    return this.list().map((s) => ({ id: s.id, displayName: s.displayName, mode: s.mode }));
  }
}
