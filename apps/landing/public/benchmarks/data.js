window.BENCHMARK_DATA = {
  "lastUpdate": 1793278398593,
  "repoUrl": "https://github.com/saeedkolivand/ai-job-hunter-app",
  "entries": {
    "Export render": [
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c2df7bf6ea04914c8981ccf1961f4457a44ab228",
          "message": "fix(export): render projects from resumes imported as pdf or docx (#1072)\n\n* fix(export): recognize project titles in resumes imported from pdf or docx\n\nA project entry only opened on a leading bold run. Bold is markdown, and an\nimported resume has none - extraction keeps the words and drops the styling.\nSo a candidate's own CV carrying a perfectly formed project block\n\n    AI Job Hunter   aijobhunter.app\n    Tauri 2 - Rust - React 19 - TypeScript\n    Local-first Windows and macOS desktop application.\n\nstill rendered as six loose paragraphs, which is the exact flat output this\nfeature exists to remove. Reported from a real export.\n\nFall back to the shape when there is no bold: a short, non-sentence line whose\nNEXT line is a technology stack is a project title. The stack line is the\ndiscriminator - prose does not sit above a middot-separated list - and the\nline must not be a stack itself, or a two-stack sequence would open an entry\non the second one. Lookahead skips blanks, so a project's last description\nline sees the next project's title rather than stopping at the gap.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(pipeline): group imported project entries by shape, and ask for a tech line\n\nSeeding shared the export path's bold-or-bullet rule for \"where does a project\nentry begin\", so an imported resume - which has no markdown at all, because\npdf and docx extraction keeps the words and drops the styling - fell through\nto the \"first line of the section\" arm. Measured on a real user source: three\nprojects collapsed into ONE seed whose description swallowed the next\nproject's title and stack. seed_projects_for_normalize then correctly refused\nto normalize from it, which is why a generated Projects section kept whatever\nfree-form shape the model chose.\n\nTeach project_entry_starts the same shape signal the export adapter now uses,\ndefined ONCE in export::parser::is_project_title_shaped and shared by both, so\nthe two can never disagree about where an entry starts - the property that\npredicate's own doc comment exists to protect. The same source now seeds two\nclean projects with no bail reason.\n\nAlso tell the job-ad resume prompt to put a project's technologies on their\nown line. Nothing asked for it before, so the model merged the stack into the\ndescription sentence and the export had no meta line to style. The builder\nprompt already carried this rule; the job-ad path did not.\n\nLimits pinned as tests rather than left to be rediscovered: the signal needs a\nstack line, so a prose-only Projects section still collapses, and the\nwhole-bail guards do not catch that shape. Pre-existing, unchanged here.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(export): bound the project shape signal to its section and to paragraph starts\n\nBoth review findings reproduced before fixing, and both were real.\n\nCross-section lookahead: next_content_line scanned the whole document, so the\nlast line of Projects could be made a title by a separator-bearing HEADING in\nthe next section. \"SKILLS - TOOLS\" did exactly that, and the German and French\nheading pairs have the same shape. It now stops at a section header, which is\nhow the two sibling groupings were already scoped.\n\nSentence-like prose: any line under 100 characters could open an entry, so an\nunpunctuated body line above a SECOND stack line became a title - measured,\n\"Used by 200 teams\" above \"Go - gRPC - Redis\" split one project into two.\nA word count would not have caught that phrase; what separates a title from\nbody text is that entries are blank-separated and a description line is not.\nThe rule now requires the line to open a paragraph.\n\nBoth call sites in the pipeline compute that flag before the blank filter\ndestroys it, so all three groupings keep answering alike. Both guards\nmutation-checked: reverting either fix fails its test.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-08-22T02:13:21+02:00",
          "tree_id": "6a7b995f8e6a2aa5fe60eb04b496b20f71e20de3",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c2df7bf6ea04914c8981ccf1961f4457a44ab228"
        },
        "date": 1787359045796,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2325851,
            "range": "± 53470",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2847921,
            "range": "± 127113",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 318881,
            "range": "± 9503",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "52830b0c29885830bf7942af38865530167dba94",
          "message": "fix: make letter-spaced headings, plain-text links and placeholder sections behave (#1073)\n\n* fix(parser): collapse letter-spaced headings so their sections are visible\n\nDesigners set headings with wide tracking and some pdf producers bake it into\nthe text layer, so a real CV extracts its headings as\n\"S E L E C T E D   P R O J E C T S\". Every heading test in the parser, in\ndocuments::evidence and in SectionId is a string match, so that line matched\nnothing at all: the section was invisible. Measured on the reported document -\nseed_projects returned ZERO projects and the evidence extractor logged\nprojects=0, so nothing seeded, no links were collected, and the section\nrendered as body text. After the fix the same CV seeds four projects, each\nwith its own stack, and the normalize guards report no bail.\n\nSingle-character tokens are the signal; word gaps survive as runs of two or\nmore spaces, so the heading restores as \"SELECTED PROJECTS\" rather than one\nwelded word. The rewrite is gated on the collapsed form being a KNOWN heading,\nso an ordinary line can never be rewritten by accident, and it happens on the\nparsed text every consumer reads rather than in three separate matchers.\n\nWide tracking is an ATS hazard in its own right, so collapsing it is also what\na real applicant-tracking parser would need to see.\n\nAdds \"selected projects\" and \"core skills\", both from the reported CV, to\nSECTION_NAMES and to the shared TS parity fixture.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(extraction): recover plain-text links from a pdf with no annotation layer\n\nThe reference list a resume carries is built from a pdf's /Annot link layer.\nA CV typeset without real hyperlinks has none, so its urls survive only as\ncharacters - and every consumer downstream reads that list: the link injector\nthat re-attaches a project url to its item, and the resume seeder. Measured on\nthe reported document: zero links extracted, so the generated resume kept a\nproject url only when the model happened to copy one, and this time it did not.\n\nHarvest url-ish tokens from the text when the annotation layer is empty. The\nsame CV now yields three contact links and four body links where it yielded\nnone, and the injector has something to re-attach.\n\nA path-less host is only accepted for a TLD a technology name does not use.\n.io is excluded there on purpose - socket.io and crates.io are a library and a\nregistry, not the candidate's links, and nothing about their shape says\notherwise. A .io host WITH a path is unambiguous and kept.\n\nLocal to the extractor rather than a relaxation of factual::URL_RE: that regex\ngrades link Criticals, and widening it would change what counts as a claimed\nlink everywhere. Annotations still win when present, so an accurate anchor is\nnever replaced by a guessed one.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(export): drop a section whose only content is a parenthetical note\n\nA generator told to omit a section it has no content for sometimes writes the\nheading anyway plus a note explaining the absence. Reported from a real export:\ntwo headings advertising that the candidate has no awards and no publications,\nwhich is worse than printing neither.\n\npush_nonempty_section could not catch it - the section is not empty - so it\nreached the page as a real heading. Tested on the SHAPE, not on wording: the\nnote is written in the resume's own language, so a keyword list would be a\npermanent translation debt. A lone paragraph wrapped end to end in parentheses\nis a meta-comment about the document in any language; real resume content is\nnot written that way, and a section with real content that merely contains a\nparenthetical is untouched.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* test(egress): declare the extractor's scheme-prefixing literal\n\nThe link harvest writes a scheme onto a host the CV spelled out without one, to\nbuild the extractor's own markdown reference list. It builds a string and never\nfetches it, the same reason model/rich.rs is declared beside it.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(export): require a true single parenthetical, and keep balanced parens in a url\n\nBoth review findings reproduced first, and both were real.\n\nData loss: the placeholder-section test checked only the first and last\ncharacter, so \"(B.Sc.) Computer Science (2020)\" satisfied it and the whole\nEDUCATION section was silently deleted - measured, the section vanished from\nthe model. It now tracks depth and requires the opening paren to stay unclosed\nuntil the final character, so a paragraph that merely starts and ends with\nparens keeps its section.\n\nTruncated links: the url regex stopped at every \")\", so\n\"example.com/Function_(mathematics)\" lost its closing paren and became a dead\nlink. The regex now keeps \")\" and a balance check decides afterwards - a\nclosing paren survives only when an unclosed \"(\" inside the token is waiting\nfor it, so \"(see example.com/a).\" still trims correctly either way round.\n\nThe test module moved to model/adapter/tests.rs, matching model_docx.rs: the\nnew cases pushed adapter.rs past the R8 module cap.\n\nBoth guards mutation-checked - reverting either predicate fails its test.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-08-22T04:39:01+02:00",
          "tree_id": "92adb92a7835264fea3479de6d90bc8e02784471",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/52830b0c29885830bf7942af38865530167dba94"
        },
        "date": 1787367802838,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1712942,
            "range": "± 59439",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2042890,
            "range": "± 17196",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 188285,
            "range": "± 1832",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1d70c491c3f327d84da5a81059a65da4689ad1d3",
          "message": "feat(scraping): make freehire its own selectable board (#1074)\n\n* feat(scraping): make freehire its own selectable board\n\nfreehire was only reachable as the aggregator's keyless last tier, so a user\nwho wanted it had to select the aggregator and hope every keyed provider\nfailed or came back empty first. It is now a board in the catalog, chosen\nlike any other.\n\nMoved out of the aggregator entirely rather than kept in both places, so no\nsearch fetches it twice and each board does one thing. The consequences are\ndeliberate:\n\n- The aggregator is keyed providers only again, so needs_keys() reports true\n  without keys and the needs-keys skip means what it says. That skip was\n  unreachable while an always-on keyless tier sat underneath.\n- A failure is now REPORTED instead of degraded to an empty result. Silent\n  degradation existed because nobody had opted into an always-on tier; picking\n  the board in the catalog is opting in, and an empty result would read as \"no\n  such jobs\" rather than \"the source is down\".\n- supports_location stays false and no country is sent. freehire's geography\n  facets are one OR-group - measured, countries=gb returns 89,211 and\n  countries=gb+cities=London returns 89,617, so naming a place WIDENS the\n  result. Claiming server-side support would switch off the engine's central\n  post-filter, the only thing actually narrowing this board to the location.\n\nThe tier's own rules went with the tier: last-rung position, skip-on-real-\nfailure and merge-behind-sparse-hits are gone, along with their tests and the\nflag in primary_chain that existed only to feed the skip-on-failure guard. The\nclient tests moved with the client and still pass unchanged.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs(landing): sync the architecture map's board count to the registry\n\nThe copy-drift guard reads the SCRAPERS registry, and freehire's registration\nmade the map's 24 stale.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs(scraping): drop the board count that just went stale, and thin the freehire section\n\nThe page opened with \"board registry (24 active scrapers)\". Registering\nfreehire made that 25, so the number silently became a lie - which is the\nfailure rule 17 describes, and bumping it to 25 would only reset the same trap.\nReplaced with a pointer to SCRAPERS itself.\n\nThe freehire section this PR added also spelled out the endpoint path. The\nmodule doc owns that, so the section now names what lives there instead of\nrepeating it.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-08-22T08:06:28+02:00",
          "tree_id": "d50b95a9316fd1ab762389a8b2432743bd05d221",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/1d70c491c3f327d84da5a81059a65da4689ad1d3"
        },
        "date": 1787380100582,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1696227,
            "range": "± 55237",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2043786,
            "range": "± 10924",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 189252,
            "range": "± 3048",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fc339661d47229b12f5699d97933470e47d96c24",
          "message": "fix(scraping): send the requested location to freehire (#1075)\n\n* fix(scraping): send the requested location to freehire\n\nfreehire never received the location at all. `FreehireScraper::search` passed\n`None` for geography, so every search fetched the worldwide first page and left\nthe whole job to the engine's central post-filter — which can only narrow the\npage it is handed. A \"Berlin\" search fetched Johannesburg, Bengaluru and\nSingapore, then discarded 38 of 50 rows; the survivors were the ones with a\nblank or remote location the filter conservatively refuses to drop.\n\nThe old comment claimed freehire has no city parameter and that naming a place\nWIDENS the search. Half right. `cities` is documented, and geography is one\nOR-group — `countries=de&cities=Berlin` reads \"Germany or Berlin\" and is indeed\nwider. But a city ALONE narrows, which was never measured (live):\n\n  (none)                      1,348,012\n  countries=de                   41,154\n  countries=de&cities=Berlin     41,241   <- the widening that was measured\n  cities=Berlin                   8,000   <- never measured\n\nSo send exactly ONE geography parameter. The city is resolved through\nfreehire's own `/geo/cities` typeahead first, because the facet holds canonical\ndisplay names and matches nothing on a near miss (`Munich` -> 6,105,\n`Muenchen` -> 0). An unresolvable place falls back to the country, then to no\ngeography — the resolver is an optimisation, never a dependency, so its outage\ndegrades the filter instead of failing the search.\n\nTwo details the API forces:\n\n- Only the city segment is sent. The UI's location is a \"{city}, {country}\"\n  label and freehire splits a comma into two OR'd values, so passing it whole\n  widens (`cities=Berlin, Germany` -> 8,360).\n- The resolved city is re-checked against the country code client-side. City\n  names are not unique (\"London\" is both gb and ca) and the `cities` facet has\n  no country qualifier, so pairing it with `countries` would trip the OR-group.\n\n`supports_location` stays false on purpose: that flag switches OFF the central\npost-filter, and freehire's filter is coarser than the request (`cities=Berlin`\nlegitimately returns \"Remote (Karlsruhe, Berlin, Muenchen, Hamburg)\"). The\nserver-side parameter narrows the page; the post-filter still decides.\n\nLive after the fix, same query: 50 of 50 results in Berlin.\n\nFive tests added, each mutation-checked — reverting to no geography, sending\ncity and country together, dropping the country cross-check, and keeping the\ncomma all fail a distinct test.\n\nAlso corrects two docs that the fix turned into lies: ADR-0005 said freehire\n\"sends no location at all today\", and the scraping domain note pointed at\n`supports_location` for a geography rule that now lives on `fetch_freehire`.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs: keep the freehire scraping note a thin pointer\n\nThe paragraph opens by promising \"shape only, because the remaining rules are\nload-bearing and must not be paraphrased here\" — and then the location edit\nparaphrased one: it spelled out the geography facet names and the OR-group\nbehaviour, both of which live on `fetch_freehire`. Name the topic, point at the\nsymbol, and let the source own the rule.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-08-22T16:58:15+02:00",
          "tree_id": "a5873d7d574e61ed2005e9a45392ca5054e30667",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/fc339661d47229b12f5699d97933470e47d96c24"
        },
        "date": 1787412144968,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2281471,
            "range": "± 22489",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2708825,
            "range": "± 32956",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 309589,
            "range": "± 12719",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f2f28361247a3e57d5ff49d04909bd085e6043c8",
          "message": "fix(pipeline): stop a cover-letter-only run from generating a résumé (#1078)\n\n* fix(pipeline): stop a cover-letter-only run from generating a résumé\n\nPicking \"Cover letter\" in the Tailor wizard generated, validated, repaired\nand persisted a résumé, and then showed it instead of the letter. \"Both\"\nworked. Two defects stacked.\n\nThe choice was lost on the wire: useTailorPipeline encoded the three-way\noutputType as one boolean (includeCoverLetter: outputType !== 'resume'), so\n'cover' and 'both' sent a byte-identical request — there was no résumé flag\nto turn off. And the résumé stage was unconditional: CoverLetter::run had a\ngate, Draft::run had none.\n\nAdds includeResume (default true) to ResumePipelineRunSchema, threads it to\nQualityInput, and gates Draft::run exactly where CoverLetter::run gates its\nown — before the completer resolution, so a skip costs nothing. One stage\nlist, unchanged: paying_stages(), PIPELINE_STAGES and the renderer's step map\nare all derived from it.\n\nThree downstream readers needed scoping, two of which failed quietly:\n\n- validate no longer keeps a résumé report for a résumé that was never\n  written — an empty draft graded under DocKind::Resume is a dropped_role\n  Critical per employer, which parked the run at needsReview;\n- humanize no longer keys the WHOLE stage on the résumé's report. Its\n  `let Some(resume_report) = ctx.report … else { return }` head ran before\n  letter_flagged was computed, so a cover-only run skipped the letter's\n  polish pass and recorded a zero-flag artifact saying nothing was flagged;\n- save_verdict asks its two résumé questions only of a run that has one.\n  An empty draft returned Nothing (discarding the letter the run had just\n  paid four calls for), and is_persistable then called it Refused — a \"your\n  résumé came back without any of your work history\" banner on a run that\n  never asked for a résumé. persist_document passes report::build an Option,\n  so the wrapper omits the resume key and the merge leaves the posting's\n  stored slot, verdicts included, untouched.\n\nrepair needed no change: its ctx.report early return was already correct.\n\nRenderer: activeOut is derived from the run's target with a tab click as the\noverride (covering the cold remount, where no start() survives to seed it);\nthe tab list comes off target rather than off activeOut; a cover-only run on\na posting with an older tailored résumé still shows it, exportable but\nreview-inert — resume_pipeline_get reads report off the aggregate, so Fix\nsection would otherwise accept a click and rewrite a document this run never\nproduced.\n\nA cover-only run now costs 4 provider calls guaranteed and at most 6, down\nfrom 5 and up to 17.\n\nsave_verdict/is_persistable move to their own module: mod.rs crossed R8's\n1400-LOC cap.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(pipeline): close the two write paths a borrowed résumé tab left open\n\nBoth raised in review on #1078, both verified against the code first, and both\nintroduced by that PR: before it, a cover-letter-only run still produced its own\ndraft, so \"the aggregate has a résumé\" and \"this run wrote one\" were the same\nquestion.\n\nresume_pipeline_regenerate_section gated on a PROXY — ensure_latest_run plus a\nnon-empty ai_generations.resume_text. A cover-only run satisfies both: it IS the\nposting's newest run, and the aggregate still carries whatever an earlier run\nsaved. The command would splice, re-validate and persist a section of a résumé\nthis run never wrote, spending a provider call and overwriting the posting's\nreport on the way. execute now records metrics_json.resumeInRun — the same\nprovenance-in-metrics escape hatch sourceResumeId already documents, so no\nmigration — and the command refuses before it reads the aggregate. Absent reads\nas true: no migration touches the rows that predate includeResume, and every one\nof them wrote a résumé.\n\nuseTailorPipeline still handed Re-check the borrowed document. It re-validates\nwhatever is active and persistReports the merged wrapper back onto the\naggregate, so it would overwrite the posting's résumé report from a run with no\nrésumé of its own. Gated by withholding onReportChange rather than by a second\ncondition on recheck, so it reuses useQualityRecheck's own \"no session writer,\nno action\" rule instead of a parallel one that could drift.\n\nAudited every other activeOut reader in the hook rather than patching the two\nthat were reported: output and the export docType are read-only, and inline\nediting stays live deliberately — a hand edit is the user's own typing on a\ndocument they already have saved, which is the reason the tab is shown.\nresolve_fabrication needs no guard: it records a verdict, spends nothing, and\nlands in the same aggregate slot either way.\n\nThree of the new tests were mutation-verified the usual way. The fourth had to\nbe added because mutation testing found the hole: deleting the guard's CALL SITE\nleft both original tests green — they pinned run_wrote_a_resume and the metrics\nwrite, neither of which says the command consults either. A function nobody\ncalls is not a guard.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-08-24T22:02:31+02:00",
          "tree_id": "d4a1948e9f750684ecadeee34ee4a536d77d17fd",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f2f28361247a3e57d5ff49d04909bd085e6043c8"
        },
        "date": 1787603169723,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2328918,
            "range": "± 38890",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2701131,
            "range": "± 71099",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 306183,
            "range": "± 5562",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9bfb174a8d3daa84004183988c0463f46cb0f669",
          "message": "feat(autopilot): surface the strongest jobs across every autopilot in one place (#1083)\n\n* feat(autopilot): add cross-autopilot best-matches backend (phase 1)\n\nAdds the shared MATCH_TIER_CUTS constant (generated to Rust), the\ndismissed interaction type, the AutopilotContract.bestMatches wire\ncontract, and the autopilot_best_matches command that clusters every\nnon-archived autopilot's found jobs at query time and qualifies them\nagainst their own score kernel's tier cut. Renderer wiring is phase 2.\n\n* fix(shared): export the best-match wire types and harden the tier emitter\n\nThe contracts barrel re-exports a named list rather than `export *`, so the\nthree new AutopilotBestMatch* types never reached `@ajh/shared` and every\nconsumer failed to typecheck. Found by review, not by the pre-push hook, which\nruns gen:ipc:check but not typecheck from a consumer package.\n\ngenMatchTiers hardcoded four const names and built each f64 by concatenating\n\".0\", so a fractional cut point would emit invalid Rust and a new variant on\nMATCH_TIER_CUTS would be dropped silently — gen:ipc:check regenerates from the\nsame function, so it would compare the omission against itself and stay green.\nFormat with toFixed(1) and assert the variant set, verified by mutation.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(autopilot): stop found-job views colliding on the viewed badge\n\nInteractionStore::upsert keys on (job_id, interaction_type); scrape_persist_job\ndefaults job_id to \"\" when absent. AutopilotCard.handleJobClick never sent an\nid, so every autopilot found job collapsed onto the same (\"\", \"viewed\") slot\nand only the most-recently-opened job ever showed the Viewed badge. Pass\njob.url as the id, mirroring how the rest of the app keys interactions.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* refactor(autopilot): consume the shared match-tier cut points in scoretier\n\nmatch-band.tsx hardcoded the 55/30 and 75/50 band cut points; import the\nsingle shared MATCH_TIER_CUTS source (now also the Rust autopilot_best_matches\nqualification bar) instead of a second literal copy that could drift.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(autopilot): wire the best-matches ipc channel through the renderer\n\n- tauri-client autopilot namespace: bestMatches() -> invoke('autopilot_best_matches')\n- mock-client: bestMatches stub (unblocks typecheck against Phase 1's contract)\n- query-client: keys.autopilot.bestMatches, a deliberate CHILD of\n  keys.autopilot.all so every existing autopilot invalidation refreshes it by\n  prefix match with no extra wiring\n- use-autopilot: useBestMatches() service hook\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* refactor: point at the relocated salary-formatter import path\n\napps/desktop/src/renderer/lib/salary.ts moved to lib/format-salary.ts in\n7051055b (folded into that commit's staged changes from a shared worktree) so\na second feature (BestMatchRow) can reuse it without an illegal\nfeatures/applications -> features/best-matches cross-feature import. Update\nthe two references that still pointed at the old path.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(best-matches): add the cross-autopilot best-matches page and preview\n\nNew /best-matches route: sort by score (backend order, untouched) / newest /\nsalary, honest empty states (no autopilot has run vs. nothing has cleared the\ntier bar yet), a truncated notice when the backend capped the list, and a\nsalary-coverage caption on the salary sort. Row actions (View/Save/Apply/\nDismiss) and the \"Found by\" source chip -> /autopilot focus jump are shared\nwith the /autopilot top-3 preview strip via one BestMatchRow component\n(components/job/, since features/autopilot may not import features/best-matches)\nand one useBestMatchActions hook.\n\nDismiss hides the row optimistically (mirrors useRemoveAutopilot's own\noptimistic-delete pattern) with an inline \"Dismissed — Undo\" row, and its\npersistJob payload carries id + url + title + company — Rust matches a\ndismissal by deriving canonical_job_key(url, title, company) against every\ncluster member's own key, so a payload missing any of those never matches and\nthe row never disappears.\n\nApply resolves the source autopilot from sources[0].autopilotId against the\nuseAutopilots() cache and reuses use-apply-to-found-job.ts (lifted separately).\n\nAll scraped/LLM-generated fields (title, company, location, salaryCurrency,\nassistantNotes) render as plain JSX text nodes, never dangerouslySetInnerHTML;\nurl only ever reaches useOpenExternal, never a raw href.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(autopilot): surface the best-matches preview above the found-jobs list\n\nRenders <BestMatchesPreview /> after the error banner, before the card list\n(renders nothing itself while there are no qualifying matches). Also switches\nhandleApply to the newly-lifted useApplyToFoundJob hook, so /autopilot and\n/best-matches share exactly one Apply implementation instead of a second copy\nthat could drift.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(dashboard): exclude dismissed interactions from total tracked\n\nuseInteractions() with no filter returns every interaction type, so the\ndashboard's \"Total tracked\" tile was about to start counting the new\ndismissed type once best-matches ships it — inflating the headline number for\na job the user explicitly rejected, and silencing the empty-state hint\nwhenever only dismissals were present. Filter to an explicit allowlist\n(viewed/opened/applied/bookmarked) instead of \"everything except dismissed\",\nso a future sixth type needs a deliberate decision here too.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(documents): localize the dismissed activity-feed badge\n\nINTERACTION_TYPES had entries for applied/viewed/bookmarked only; a dismissed\nrecord fell through InteractionRow's forward-compat fallback and rendered the\nraw English word \"dismissed\" in every locale, once best-matches starts\nwriting that interaction type. Add the missing config entry (labelKey + icon\n+ colours), localized in en/de.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(i18n): add best-matches copy and the dismissed activity label\n\nNew bestMatches.* sub-tree + nav.bestMatches for the /best-matches page and\npreview strip, and resumes.activity.dismissed for the Activity feed's\ndismissed badge. Real German throughout, not English placeholders.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs: regenerate api.md for the autopilot best-matches method\n\nPhase 1 added the bestMatches contract method (with TSDoc) but did not run\npnpm gen:api; docs/API.md was stale. Regenerated, no hand edits.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(best-matches): stop the dismiss success invalidation from killing undo\n\nhandleDismiss invalidated keys.autopilot.all on a successful persist. That\ninvalidation is a prefix match on keys.autopilot.bestMatches, so it forced an\nimmediate refetch — and the dismiss write is a local JSON persist that\nresolves in milliseconds, long before a user could ever reach for undo.\ncompute_best_matches already excludes a dismissed job from that refetch, so\nthe row was evicted from the query cache entirely; undoDismiss then deleted a\nkey from dismissedKeys for a row that no longer existed anywhere to render.\nAn affordance that renders but does nothing is worse than not shipping one.\n\nThe optimistic local hide is sufficient on its own: the backend is already\nauthoritative, so any natural refetch this hook doesn't force (a remount, a\nroute change, another autopilot mutation) reflects the dismissal once it\nhappens. Local state and the backend only ever disagree about *when* the row\nleaves, never *whether* it does. Dropped the success invalidation entirely.\n\nChecked handleView/handleSave for the same shape of problem: neither\nparticipates in compute_best_matches's qualification predicate, so there was\nnothing to invalidate — confirmed with dedicated tests, not by omission.\n\nThe old dismiss test asserted only that dismissedKeys gained then lost a\nkey — true even with the feature fully broken (a pure useState-setter test).\nReplaced/added:\n  - a unit test asserting no success callback is wired to the dismiss\n    mutation at all\n  - use-best-match-actions.dismiss-undo.test.tsx: a real-QueryClient,\n    real-AppClient integration test that renders the row, dismisses it,\n    asserts it's gone, clicks undo, and asserts it's visible again, with a\n    mock backend that (like compute_best_matches) excludes a dismissed job\n    from any later fetch\n\nMutation-verified: reintroduced the exact success invalidation, reran the\nnew integration test — red, failing even earlier than expected (the\noptimistic placeholder assertion itself fails, since the invalidation-driven\nrefetch evicts the row before that assertion runs, not just before undo).\nRestored the fix, reran — green. Full desktop suite green.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(autopilot): fix cross-scale scoring and cluster-id collisions in best matches\n\nThree independent reviews of the best-matches command found the same two\nroot causes: best-member selection and the row sort both compared a\nCombined semantic score against a Keyword coverage score on one raw axis,\nand canonical_job_key collisions across title/company blocks let unrelated\npostings share a cluster id. Both are fixed structurally - member/row\nordering now goes through the shared ADR-020 two-block rule, and the union\nis deduped by canonical_job_key before clustering ever runs, which also\nremoves a second full-struct clone of every found job. Also fixes: the\nuntested dismissed interaction type, applied-status matching only the\ncanonical url instead of every board copy, the command blocking the IPC\nthread instead of running via spawn_blocking, and several undertested\ncontract clauses (foundAt earliest, autopilotCount, tombstone veto, exact\nscore-cut boundaries).\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs: close best-matches feature (adr-036, automation-domain update)\n\nAdd adr-036 documenting cross-autopilot best-matches query-time clustering\nwith two-block ranking (combined block first, then keyword), union dedup by\ncanonical_job_key, per-member qualification against own kernel's high cut,\npaused autopilot inclusion, and per-member dismissal identity.\n\nUpdate automation-domain.md with best matches subsection covering ipc contract,\nrust command (clustering, filtering, sorting), renderer surfaces (route, sort\noptions, dismiss action), and query-key strategy (prefix invalidation).\n\nAdd adr-036 to decision-records index in readme.md.\n\nAll validation gates pass: check:adr-citations ok, check:agent-system ok.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(autopilot): read best-matches assistant notes from the canonical member\n\nassistant_notes previously read whichever cluster member came first in\ninput order, which on a merged cross-autopilot row could surface a note\nwritten against a different autopilot's resume/provider context with no\nprovenance on the payload. Now prefers canonical, then best-scored, then\nany member, matching every other display field.\n\nAlso adds a regression test pinning is_degenerate_key's guard (deleting\nit does not fail any existing test) and corrects the clusterMembers\nTSDoc, which claimed the list excludes the canonical copy when the\nbackend always includes it -- the completeness dismiss-matching relies on.\n\n* docs: point at the payload cap and say non-archived, not active\n\nThe living automation-domain page copied the 100-row cap as a literal, which\nrule 17 exists to stop: tuning BEST_MATCHES_CAP would have left the page\nasserting a number the code no longer uses. Point at the constant instead.\n\nBoth pages also said \"active autopilots\" where the contract means non-archived\n— paused autopilots contribute, so \"active\" understated the source ceiling.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(best-matches): make undo actually reverse a dismissal\n\nUndo only removed a local dismissedKeys entry, so the persisted\ndismissed interaction survived every refetch/remount and the row kept\ndisappearing. Add InteractionStore::remove and a\nscrape.removeInteraction IPC round trip, then wire undoDismiss to call\nit (keyed on the same job url the dismiss wrote) and invalidate\nautopilot on success so the row genuinely comes back.\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-08-28T00:04:31+02:00",
          "tree_id": "e319d03d444cdd9da75e1bf6cdc3886e2bce5fd3",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9bfb174a8d3daa84004183988c0463f46cb0f669"
        },
        "date": 1787869683017,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2195056,
            "range": "± 18268",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2548474,
            "range": "± 18093",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 297096,
            "range": "± 5749",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "93b09cd68ad2e746dfff944378e91096adc0bd67",
          "message": "feat: agent-facing CLI so an AI agent can drive the app (#1085)\n\n* feat(bridge): serve a read-only agent.query surface over the loopback bridge\n\nIssue #1084 asked for a way for an AI agent to read the app, which has no\nsurface an agent can reach. Rather than a localhost HTTP server, this serves\nfive read-only resources over the loopback bridge the native host already\nrelays to, so there is no new port, listener, token or origin allowlist.\n\nResources: best-matches, job <url>, profile, automations, schema. `url` is the\ncross-resource key, not an id -- BestMatchRow.key is a cluster id, not a stable\nhandle. There is deliberately no apply or submit verb.\n\nEvery payload is an allowlist projection modeled on AutofillProfile, not a\ndelegated record. Autopilot carries resume_text, cover_letter, assistant_notes\nand full found_jobs descriptions; the projection structs have no field that can\nexpress them, so they are absent by construction rather than by omission.\n\nAgentQueryThrottle lives on BridgeState, not per-connection: every CLI\ninvocation is a fresh process and socket, so a per-connection bucket would be\nbypassed by construction. Two independently-sized buckets -- best-matches gets\nburst 1 / 30s against its measured 12.3s worst case, cheap reads burst 10 / 1s.\n\nKnown limit: best-matches calls the existing command unmodified, so the\nclustering compute stays unbounded per admitted call; the tight bucket bounds\nrepetition, not one call's cost. A compute-side cap belongs to the matching\ndomain.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(cli): add an agent mode that reads the app over the loopback bridge\n\nCompletes issue #1084 PR 1. `ajh-tauri agent <verb>` connects to the running\napp's bridge, sends one agent.query, prints JSON on stdout and exits. It is a\nmode of the existing binary, not a second [[bin]] -- the release upload globs\nonly read target/release/bundle/**, so a second binary would ship to nobody.\n\nThe sentinel sits below the native-host short-circuit and above run(): run()\nforks the minidump supervisor as its first act, and the single-instance plugin\nwould otherwise hand this argv to the running GUI, popping its window instead\nof ever printing.\n\nconnect_bridge returns on the first successful WS upgrade, before any protocol\nframe, so a port squatter would take the CLI down and read as a pairing\nfailure. This adds a separate port loop that carries the full v2 mutual\nhandshake per port and accepts only the port whose server proof verifies,\nleaving the native host's path untouched. verify_server_proof is the missing\nclient-side counterpart to the existing verify_client_proof.\n\nOn Windows the release build is windows_subsystem = \"windows\" with\npanic = \"abort\", so a bare interactive run has a NULL stdout handle and\nprintln! aborts. GetStdHandle is probed FIRST and an inherited pipe -- the\nagent case -- is left alone; only an invalid handle attaches to the parent\nconsole. Attaching unconditionally would replace the pipe.\n\nThe binary is not on PATH on Windows, macOS or Linux AppImage, so the app now\nwrites a pointer JSON carrying exePath and dataDir on every launch, on\nregister_native_host's existing best-effort lifecycle. home_dir() read only\n$HOME, which is unset on Windows, so the pointer would silently never have\nbeen written there; it now mirrors data_dir()'s USERPROFILE-then-HOME.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(cli): report a missing pointer as app_not_located, not app_not_running\n\nEnd-to-end verification against a live bridge reported `app_not_running` while\nthe app was demonstrably running and listening -- the pointer file simply had\nnot been written, and an unreadable token collapsed into the same sentinel.\nThree distinct causes shared one misleading name, and it sent this feature's\nown verification pass looking at the wrong thing for several minutes.\n\nA missing pointer is now `app_not_located` and an unreadable token is\n`pairing_token_unavailable`; the port-loop buckets are unchanged.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(cli): bound next_json by an absolute deadline instead of re-arming it\n\nnext_json re-armed its timeout on every loop pass while silently skipping\nping/pong frames, so a peer pinging faster than the budget kept it blocked\nindefinitely. That also defeated the caller's bound: send_agent_query\nrecomputes a shrinking `remaining`, but that recomputation never runs while\nnext_json is stuck inside its own loop. A squatter on any port in the range\ncould therefore stall a whole invocation -- exactly what the module doc\nclaimed it prevented.\n\nThe deadline is now computed once on entry and shrinks each pass, the pattern\nsend_agent_query already used. The ping-skip branch had zero coverage; the new\nregression test floods pings every 10ms against a 150ms budget and was\nconfirmed failing against the unfixed code before the fix landed.\n\nAlso pins two properties that no test held: the agent throttle is now driven\nthrough BridgeState across a simulated reconnect (mutation-checked -- making\ntry_acquire_agent always return true fails it), and the two new wire constants\njoin reserved_types_are_distinct. Their omission from the TS-parity test is\ncorrect and stays, but distinctness is a different invariant with no TS\ndependency and should not have inherited that exclusion.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs(cli): correct the console probe's claim that nothing can panic\n\nThe comment said this function \"never panics either way -- the caller degrades\nto a non-zero exit with no printed JSON, not a crash\". The first half is true\nand the second is not: when AttachConsole fails, stdout stays invalid and\nprintln! panics, and since release sets panic = \"abort\" and the CLI\nshort-circuits above run() (so crash reporting never initialised), that is a\nsilent abort with no message and no report.\n\nThe probe-before-attach design is right and unchanged; only the claim was\nwrong. The comment now says what it actually costs, and names where the fix\nbelongs -- the print paths, via writeln! with the Err ignored.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(bridge): fence scraped descriptions and close the security review findings\n\nAdds `ajh-tauri agent --help` (also -h and a bare `help`), generated from the\nsame table parse_verb matches on so it cannot advertise a verb that does not\nparse. It works with the app closed -- no pointer, token or socket -- because\nhelp that needs the thing you are trying to reach is useless.\n\nHIGH: job descriptions are third-party-authored text emitted to a consumer\nwhose whole purpose is \"an AI agent reads this\", and answer_assist.rs already\nfences the identical string. They now go through prompt_fence with the same\ntag and cap. Verified on real data: an 8697-char posting comes back wrapped\nand truncated.\n\n\"Absent by construction\" held only at the top level -- `trust` was the source\nstruct serialized whole, and both guards were blind to it (one matches a name\nlist, the other only top-level keys). Adds an AgentTrust allowlist and makes\nthe key assertions descend into nested objects.\n\nAlso: agent.query gets its own origin so the server can tell the CLI from the\nextension arriving via the relay, and is gated on it; it is spawned instead of\nrunning inline, so a multi-second call no longer stalls the read loop or its\ntoken.revoked observation; an unvalidated dataDir can no longer be a UNC path,\nwhich turned a local file read into outbound SMB; the timeout sentinel is\nreachable instead of dead code and a wrong-type reply on our own reqId fails\nfast as unsupported_by_app rather than burning 30s; and usage errors no longer\necho the typed argv, which could be a path with a username in it.\n\nmod.rs would have crossed the 1400-line cap, so the self-contained autofill\nsection moved to autofill_profile.rs unchanged.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(agents): wire cleanup and test-author to the skill contracts\n\nAn audit of the fleet found two agents whose skills could never reach them.\ncleanup holds Edit/Write and its job is DELETING code, yet it referenced no\nskill at all -- not author-contract, not token-efficiency -- so the repo's\nhighest-stakes writer operated with no smallest-diff discipline, no mandatory\nvalidation gate, and no never-approve-your-own-work rule, despite CLAUDE.md\nstating that every author loads author-contract.\n\ntest-author named testing-rules, author-contract and token-efficiency but\nnever as .claude/skills/<name>/SKILL.md paths, unlike the other 23.\n\nThis is the same defect class as the project-steward skill gap: a skill is\ninert unless an agent file names it, and naming it is not the same as naming\nwhere it lives. Audit command: for each agent, rg -q \"skills/\".\n\nNote CLAUDE.md files cleanup under \"Cross-cutting critics (no author)\" while\nit holds Edit/Write -- a writer shelved with the read-only agents, which is\nplausibly how it slipped the rule. Not changed here.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs(agents): reclassify cleanup as a writer, not a cross-cutting critic\n\ncleanup holds Edit/Write and deletes files, exports and dependencies, but it\nwas listed under \"Cross-cutting critics (no author)\". That grouping is\nplausibly why it loaded neither contract skill until the previous commit: the\nrule says every critic loads critic-contract and every author loads\nauthor-contract, and an agent filed under critics that is actually a writer\nfalls between them.\n\nIt now has a row in the author/critic table. Its critic is the owning domain\ncritic rather than a dedicated pair -- a deletion is reviewed by whoever owns\nthe code being deleted -- so no new agent file and no PAIRS entry.\n\nThe contract rule now states the test explicitly: holding Edit/Write makes an\nagent an author for that rule, wherever it sits in the table.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(agents): teach every critic how to state and hold a finding\n\nThe critic contract covered how to ATTACK a diff but nothing about the quality\nof the findings that come out, and all 13 critics load it. Three additions,\neach from measured evidence rather than taste:\n\nQuote the exact span. A finding that cannot name a file:line and the verbatim\ncode cannot be verified or refuted by anyone else, and it is what makes a\nrefutation stage possible at all.\n\nTake the UNKNOWN escape hatch. A model given no way to say \"I lack the\ninformation\" manufactures a justification instead -- which is how a nitpick\ngets written up as a CRITICAL. Related: assume most findings are wrong. The\nbest measured automated review system scores about 17% precision, so dropping\na weak finding costs nothing and shipping one costs credibility.\n\nOnly new evidence moves a severity. Models measurably abandon correct answers\nwhen merely challenged -- \"are you sure?\", \"that's intentional\" -- and do so\nfrom high confidence, so the pull toward conceding says nothing about the\nfinding's merit. A revision must name the line, result or spec clause that\nmoved it; \"the author explained it\" is not that, since the handoff was already\ncontext and never evidence.\n\nAlso separates functional from stylistic findings, with only functional ones\nblocking: reviewers score far better on logic and resource defects than on\norganisation or style, and mixing them buries what matters.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(agents): add the agent-cli author/critic pair, skill and review command\n\nThe agent CLI had no owner in the fleet. It now has the standard pair plus an\nagent-cli-standards skill carrying the rules the surface has already learned\nthe hard way, and a /review-agent-cli command.\n\nThe reviewer runs on Opus while the author runs on Sonnet deliberately, not for\ncapability: self-preference bias in LLM-as-judge is measured and directional,\nso a critic sharing the author's model family grades its own habits. The\ntiering paragraph in CLAUDE.md is the checker's single source, so it moved in\nlockstep.\n\nThe skill encodes what external research changed about the design. Fencing\nuntrusted text is attenuation, not a gate -- adaptive attacks defeat over 90%\nof published prompt-injection defenses -- so untrusted content may supply data\nbut never control flow: a job posting must not influence which verb runs, which\nid it targets, or whether a confirmation is satisfied. The threat is live\nrather than hypothetical: about 1% of 200,000 real resumes carried injections,\nrising sevenfold in 16 months, and employers have begun seeding postings.\n\nFor mutations it adds exit code 4 and a confirmation envelope instead of an\ninteractive prompt, because an autonomous caller cannot answer y/N and will\nreach for --force instead; the severe tier takes the resource's own name, which\nis the one confirmation a hallucinated argument cannot satisfy; an empty\nselector must error rather than mean \"all\"; and unknown verbs and flags are\nhard failures with no fuzzy matching or prefix abbreviation, which is the path\nfrom a typo to a destructive neighbour.\n\nWires all five integration points: the CLAUDE.md table, review-routes primary\nglobs and lessons domain, the checker's PAIRS list, and the public fleet\nexplainer on the landing site.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* chore: re-anchor two leak-guard allowlist entries after line shifts\n\nThe leak guard keys its allowlist by file:line, so inserting write_agent_pointer\nabove the current_exe() warn moved register.rs:114 to :138, and the bridge\nchanges moved mod.rs:483 to :507. Both sites are pre-existing and unchanged;\ntheir recorded reasons still hold verbatim -- current_exe() takes no input path\nto leak, and TcpListener::accept() carries no peer address.\n\nOnly the line numbers moved. Flagging the brittleness rather than fixing it\nhere: any edit above an allowlisted site breaks the guard in both directions at\nonce, reporting a fresh violation and a stale entry for what is one displaced\nline.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(landing): add the agent-cli pair to the fleet map, and address review\n\nCI Tests went red on two landing tests. The fleet explainer has TWO structures\nand I updated one: AUTHORS/CRITICS feed AGENT_COUNT, while the constellation\nrenders from PAIRS + CROSS_NODES. Adding the pair to the first moved\nAGENT_COUNT 25 -> 27 while the map still emitted 25 nodes, so both\n\"renders exactly AGENT_COUNT nodes\" assertions failed. Those tests are doing\ntheir job -- they pin the two structures to each other.\n\nI ran the landing typecheck before pushing but not its tests, which is what let\nthis through; a typecheck cannot catch a count that two structures disagree on.\n\nAlso addresses PR review findings in the agent files:\n\nThe completion gates listed fmt/clippy/test/architecture, but\ndocs/architecture-rules.md names deny, audit and machete too (R10-R13), plus\nthe EGRESS inventory as an arch test. Both the skill and the author now list\nthe full gate.\n\nmsg.rs, auth.rs and the mod.rs dispatch arm carry CLI frames but belong to the\nextension protocol. Rather than take those globs, the reviewer and command now\nscope the CLI's slice of them and hand off to extension-reviewer explicitly, so\na CLI frame added in msg.rs cannot end up with no reviewer.\n\nNot taking the MD041 suggestion to add H1 headings after front matter: zero of\n27 agent files have one, the repo has no markdownlint config and does not run\nit in CI, so adopting it would make four files inconsistent with twenty-three\nto satisfy a linter this repo does not use.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(cli): bound the connect and upgrade, and cancel a query on revocation\n\nSix review findings from PR #1085, each verified against the code first.\n\nThe two UNBOUNDED steps in an otherwise fully budgeted function: connect and\nthe WebSocket upgrade. A process that accepts on a PORT_RANGE port and never\ncompletes the upgrade -- a wedged previous instance whose listener is still\nbound but whose accept loop stopped -- parked the whole port loop forever. Both\nare now wrapped, and the whole invocation carries an outer deadline. Proven by\nstanding up a real TCP squatter: the built binary now returns app_not_running\nin ~15s instead of never returning.\n\nThe UNC guard only rejected a literal \\ or // prefix, but Windows treats the\nseparators interchangeably, so /\\host\\share and \\/host/share were absolute,\nparsed as a UNC root, and passed -- defeating the SMB/NTLM mitigation the doc\nclaimed. Now checks either separator in either position.\n\nThe unknown-flag branch still echoed argv into the exit-2 detail; that leak was\nfixed for the unknown-verb branch and missed one case over, with the test\npinning only the branch that was fixed. Usage errors now also carry a null\nresource so every exit-2 reply has the same shape.\n\nAn in-flight agent.query was not cancelled on token revocation -- a gap\nintroduced by the earlier fix that moved dispatch off the read loop, which is\nwhere fresh defects come from. It now races a per-connection cancellation token\ncancelled at the shared teardown site. Two honest limits: best-matches' blocking\nCPU pass has no cooperative checkpoint, so the reply is suppressed and the\nconnection freed but running compute is not preempted; and spawn_answer_assist\nhas the SAME stale-reply-after-cancellation gap, which was left unfixed rather\nthan copied, and needs its own follow-up.\n\nAccumulated docs pushed agent_cli.rs past R8's 1400-line cap, so the test module\nmoved to agent_cli/tests.rs, the split convention commands/autopilot.rs uses.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-08-31T20:44:22+02:00",
          "tree_id": "ba97f428169d1947a8245f3f2e84241f662b39be",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/93b09cd68ad2e746dfff944378e91096adc0bd67"
        },
        "date": 1788202484084,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1762074,
            "range": "± 30052",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2080785,
            "range": "± 15793",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 242118,
            "range": "± 5569",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "38b990d0e9491845313946ad7de304e8a492d15c",
          "message": "feat: generic command dispatch for the agent CLI, read effects first (#1087)\n\n* feat(cli): classify every command in a policy table the build enforces\n\nPhase 1 of ADR-038. Every registered command now carries a declared Effect --\nRead, Reversible, Irreversible, or NotExposed with a reason -- and a test\nasserts the table and generate_handler! agree in both directions. That is\nADR-014's static-allowlist invariant applied to inbound dispatch: a new command\ncannot ship without being classified, because the build fails first.\n\nNo dispatch, no new verb. The table is data; the phases that use it come next.\n\nEffects were assigned by reading each command body, not from its name.\nAnything that could not be verified was classified pessimistically, since an\nover-cautious row costs a ceremony and an over-permissive one costs data. Three\njudgment calls worth knowing: ai_set_embedding_config clears only recomputable\nderived caches so it is Reversible, while scrape_clear_postings destroys\nuser-visible interaction history and is not; and support_export_diagnostics\ntakes a caller-supplied destination straight to File::create, which truncates\nwhatever is already there, so it is Irreversible for the overwrite rather than\nfor the export.\n\nTwo stubs found while classifying, both relevant to the dispatch phase:\nai_unload_model ignores its argument and always reports success, and\nsupport_get_system_info returns null behind an implement-when-needed comment.\nWired through by name, a caller would get a convincing false success from both.\n\nThe guards are mutation-checked in both directions -- removing a row fails the\ncount and the set equality, adding a bogus one fails the other way. The count\nassertion is a hand-written literal rather than derived from either source,\nbecause a test that loops over the table it guards covers additions only.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(scraping): seed the company typeahead from a vendored slug directory\n\nADR-030 built passive slug harvesting and named this feeder in advance: the\nsource column is free text \"so future feeders (active probers, extension\nfingerprinting, community slug directories) require no schema changes\". Until a\nuser has organically scraped enough postings the typeahead is empty. This\nvendors ~27k slugs across the four platforms extract_ats_ref actually parses --\nbamboohr, greenhouse, lever, ashby. icims, paylocity and workday come from the\nsame upstream and are deliberately excluded: no extractor, so listing them\nwould advertise companies we cannot scrape.\n\nDeliberately NOT wired into the existing curated-seed list. boards/ats_seed.rs\nlooks like the natural home, but engine/mod.rs feeds it into seeded_companies,\nwhich the engine scrapes when the user leaves the company field blank --\nextending that to 27k entries turns one blank-field run into thousands of live\nrequests. The vendored directory is reachable only from the discovery typeahead\nand nothing else, and no rows are inserted: a slug becomes a discovered_companies\nrow only if the user stars it, through the pre-existing path.\n\nFormat follows geodata/cities.tsv.gz -- gzip embedded via include_bytes!, parsed\nonce, with recorded digests. 148 KB on disk, no new dependency, no runtime\nnetwork. Measured rather than assumed: index build ~2.9ms once, worst-case\nsingle-letter query ~1.8ms, typical ~0.5ms, against a 250ms UI debounce.\n\nLicensing is the part worth reading twice. Upstream's LICENSE is MIT, but its\nREADME carves the data/ datasets out under CC BY-NC 4.0, requiring attribution\nand forbidding commercial use without permission. This app is PolyForm\nNoncommercial (ADR-023) so there is no conflict today, but ADR-023 exists to\npreserve commercial licensing -- so this data is isolated in one directory that\ncan be deleted wholesale if that ever happens. Attribution to Riley Dorrington,\nthe upstream commit and per-file SHA-256 are recorded in ats-slugs/README.md.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(scraping): log the decode error kind, not its message\n\nThe leak guard flagged the new gunzip warn site. The error is an io::Error from\na reader over an include_bytes! slice compiled into the binary, so it carries no\npath or host and could have been allowlisted with that reasoning.\n\nLogging e.kind() instead removes the need for the argument entirely: a fixed\nenum cannot carry free text, so nothing has to stay true about the error's\nDisplay for the site to be safe. Fewer safety arguments beats stronger ones.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(scraping): stop the rate limiter underflowing its own window\n\nwait_for_slot sampled `now` BEFORE awaiting the requests lock, so while it\nwaited another task could record a timestamp later than `now`; the window sweep\nthen evaluated `now - t` with t > now and panicked with \"attempt to subtract\nwith overflow\". That is the intermittent failure that has been rotating around\nthe scraping tests and blocking pushes -- it is a real bug in a rate limiter,\nnot a flaky test.\n\n`now` is now sampled after the lock, which removes the race at its root. The\nsweep also uses saturating_sub, for an independent reason: timestamps are\nrecorded in a different critical section, so a backward wall-clock step can put\nt ahead of now even with correct lock ordering.\n\nThe regression test plants a future timestamp and was confirmed to reproduce\nthe exact panic before the fix.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(cli): dispatch read-effect commands generically through the bridge\n\nPhase 2 of ADR-038. `agent call <ns>:<command> --input '<json>'` reaches any\nregistered command whose policy row says Effect::Read; every other class\nrefuses with its own sentinel. Dispatch goes through Webview::on_message with\nAppHandle::invoke_key -- the route Tauri's own docs name for this (\"the invoke\nkey that must be referenced when using InvokeRequest\") -- so the real command\nbody runs, and the spend and rate limits living inside those bodies apply\nunchanged. No codegen, no service-layer shortcut that would have bypassed them.\n\nThe request URL is read live from the main window rather than hardcoded.\non_message compares it against the app's configured protocol origin, which\ndiffers across macOS, Windows and dev builds; reading the window's own URL is\ncorrect on all of them without special-casing any. The invoke key is read fresh\nper call and only ever placed in the request struct -- never logged, echoed or\nreturned, per Tauri's warning not to expose it.\n\n`dispatched`, not `ok`: a command's own in-band error becomes the data payload\nwith dispatched:true, because ~47 commands report failure inside a Value and\nthe dispatcher cannot honestly claim to know. The curated five keep a truthful\nok. Scraped text is still fenced -- verified against a live posting fetch, not\na fixture.\n\nThe two stubs found in Phase 1 are now NotExposed rather than Read.\nai_unload_model ignores its argument and support_get_system_info returns null,\nso classifying them by \"does this mutate state\" was right when nothing\ndispatched and wrong the moment something did: by name they would hand back a\nconvincing false success. Reclassifying reuses the mechanism every other\nNotExposed row uses instead of a bespoke blocklist.\n\nautopilot_best_matches shares the curated tier's tight throttle bucket rather\nthan getting its own, so alternating between the two tiers cannot double an\nallowance.\n\nAlso re-anchors a leak-guard allowlist line that moved again. Third time this\nbranch: the guard is keyed by file:line, so any edit above an allowlisted site\nbreaks it in both directions.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(docs): re-pad the index table row that broke the format check\n\nMy earlier edit shortened the adr-038 title without re-padding its table cell,\nleaving it four spaces wide. That landed on main, so `prettier --check` is\ncurrently failing there and every branch cut from it inherits the failure --\nincluding this pull request, where it surfaced as a red Lint & Format job.\n\nI initially concluded main was clean by checking a copy of the file in a temp\ndirectory; prettier resolves config by path, so that test used different\nsettings than the repo does and passed for the wrong reason.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(scripts): key the leak allowlist by message text, not line number\n\nThe allowlist was keyed by file:line, so any edit above a declared site broke\nthe guard in both directions at once -- a phantom \"undeclared\" violation and a\nstale entry, for one unchanged line that merely moved. It fired three times on\nthis branch alone, each time blocking a push and each time fixed by hand-editing\na number.\n\nThe mechanism already existed: a `sig` field holding the call site's own format\nstring, added earlier but wired to exactly one of 45 entries, with a header note\nsaying migrating the rest was deferred. That deferral is what kept breaking --\nboth entries that recurred were ones never migrated. All 45 now carry a `sig`,\ngenerated from the checker's own output rather than transcribed by hand.\n\nThe half that was missing: `sig` was only consulted when a line had moved, so\nediting a message in place kept passing because line-exact matching checked the\nkey and never the content. A declared entry is now satisfied only if the site\nat that key still carries the same signature, so a rewritten message falls\nthrough to the undeclared and stale checks exactly as a move would. Verified\nindependently: shifting a site five lines keeps the guard green, rewriting its\nmessage in place is caught.\n\nAlso replaces a literal NUL byte used as a bucket separator with the `\\0`\nescape -- same value, but the file stops registering as binary, which is what\nmade ripgrep refuse to search it.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs: make the vendored-slug note a pointer, not a copy\n\nThe section I added restated the upstream commit, the platform list, the\nlicense carve-out and the embedding mechanism -- all of which already live\nnext to the data in ats-slugs/README.md. Rule 17 exists for exactly this: the\nplatform list in particular would have gone stale the moment a parser was\nadded, since the real set is whatever extract_ats_ref can parse.\n\nIt now says what the data is for, that it needs no network, and where the\nprovenance and license terms actually live -- with the one warning worth\nrepeating at the call site, that the datasets carry different terms from the\nupstream repo's own code.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat(cli): dispatch reversible commands and gate irreversible ones on a withheld proof\n\nPhases 4 and 3 of adr-038. Reversible rows dispatch through the same path Read\nalready used. Irreversible rows demand a proof the refusal deliberately does\nnot contain.\n\nThere is no derivable dry-run -- no simulation path exists anywhere in the app\n-- so the entire safety property comes from where the proof lives. The refusal\nnames which read command yields it and withholds the value; a mismatch says\nonly that it did not match, never what would have. Satisfying it therefore\nrequires a second call to a different verb, and so requires having actually\nread the record. A one-hop ceremony that hands back its own answer stops\nnothing. Verified live rather than by unit test: the refusal bytes were grepped\nfor the real value and it does not appear, in either the initial refusal or the\nmismatch.\n\nA test pins every proof source to being itself a classified Read row, so a\nproof can never require something the caller is not allowed to fetch.\n\nNot every row gets a strong proof, and the weak ones say so in place rather\nthan looking strict. Deletes keyed by id ask for the record's own name and are\ngenuinely strong. Commands with no record to read fall back to a scalar --\nprivacy_reset_app is the weakest row in the table, and regenerate_token asks\nthe caller to prove reading a token it already holds locally. Those are\nrecorded as known-weak instead of dressed up.\n\nAlso closes four review findings: ai_embed is NotExposed until its missing\nspend charge lands, fencing now recurses the whole response tree rather than\none level, dispatch emits a span carrying only the command and outcome, and the\nhandler-list extraction skips comments before finding its terminator.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(ai): gate ai_embed against unbounded paid embedding spend\n\nai_embed reached a paid provider with no rate guard, no concurrency guard and\nno daily charge -- unlike ai_generate beside it, whose gate exists so \"a\nlooping/XSS'd renderer can't drive unbounded paid-API spend\". It was reachable\nfrom the renderer already; the agent CLI made it loop-friendly, which is how it\nsurfaced.\n\nThe charge lands on the EMBEDDING provider, resolved through the same\nembedding_config read documents::embed makes internally -- not on the\ngeneration provider, which would have silently throttled something unrelated.\n\nIts own bucket rather than reusing the generation caps: embedding is one cheap\nforward pass with no output tokens, so the generation ceiling would throttle\nordinary rapid embedding long before real abuse. The daily provider ceiling is\nshared, which is correct -- that bucket is already per day and per provider.\n\nAlso clamps the input. A request-count cap alone is not a cost cap here:\nembed_adaptive chunks oversized input into up to 32 provider round trips rather\nthan rejecting it, so one admitted call could bill dozens. The shared schema\ndeclares max(200_000) but that is zod, renderer-side only, and serde enforces\nnothing -- the same validation gap this repo has already documented twice\nelsewhere. It now clamps in Rust through the existing helper.\n\nBulk indexing and match scoring are unaffected: both call documents::embed\ndirectly and never pass through this command, confirmed by reading the callers\nrather than trusting a blast-radius tool.\n\nThe refusal test was mutation-checked -- removing the charge fails it, and the\nwrong-provider and normal-call tests correctly stay green.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(cli): replace two ceremonies that proved nothing\n\nBoth rows satisfied the letter of the confirmation gate and none of its\npurpose: each asked the caller to prove reading something it already held.\n\nregenerate_token is now NotExposed. Its status payload is {port, connected,\ntoken}; the CLI already knows port and token by construction, since it scans\nthe range and reads the token off disk before it can authenticate at all. And\nconnected is not a coin flip -- is_connected() is connected_count > 0, and the\nCLI's own authenticated socket increments it, so it reads true for the whole\nduration of every ceremony attempt. A hallucinated value could not lose. No\nother reachable read is bound to the pairing session, and rotating a pairing\ntoken from a shell has no job-hunting workflow behind it, so an honest refusal\nbeats a gate that cannot fail.\n\nprivacy_reset_app now counts applications rather than ai generations: a store\nthe reset actually wipes, holding the user's own tracked search rather than a\nsecondary table of model output. It keeps the property that its strength scales\nwith the stakes -- a populated store yields an unguessable number, and a user\nwith none has nothing to lose. Still partial, one of nineteen stores, and said\nso in place.\n\nThe exhaustive gate test added alongside proves less than its name suggested,\nand its doc now says so: it walks every row asserting the gate agrees with the\ndeclared effect, but the assertions key off that same declared effect, so a\nmis-classified row moves to a different self-consistent branch and passes.\nMutating the gate's own match arm is what fails it. Classifications remain\nguarded by reading, and by the count and set tests -- not by this.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(scraping): make the rate limiter monotonic and fill the typeahead page\n\nMy earlier fix here swapped an underflow panic for a stall. saturating_sub\nmakes a future timestamp yield zero, which is inside the window, so the entry\nwas RETAINED as a recent request and a full window then waited until that\ntimestamp plus a whole window. A backward clock step could delay every request\nfor the window plus the skew. My regression test left 29 of 30 slots free, so\nit never touched the blocking path at all -- it proved the panic was gone and\nnothing about what replaced it.\n\nThe timestamps are now Instant rather than SystemTime millis, which removes the\nclass rather than patching it: Instant is monotonic, so no clock step can put a\nstored entry ahead of a later sample. Verified first that they never leave the\nmodule -- no serialization, no cross-process reader -- so nothing blocked the\nchange. A min(window) cap on the computed wait makes the bound provable rather\nthan merely argued, and that is what the new full-window regression test\nexercises; removing the cap fails it with an hour-long wait.\n\nThe under-throttling direction is covered too, since over-throttling live\nscraping would be a user-facing regression: disabling the gate fails that test\nat 400ns.\n\nSeparately, the vendored typeahead returned short pages. The vendor pool was\nsized to the remaining capacity and dedup ran afterwards, so any collision\nsilently cost a row. The pool is now sized to absorb every possible collision\nand the cap is applied after dedup. Both halves are mutation-checked, the first\nreproducing the reported 49-instead-of-50 exactly.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(ui): surface the vendored slug directory's licence credit in the app\n\nThe branch compiles ~27k company slugs into the shipped binary, and that data\nis CC BY-NC 4.0 -- attribution is a condition of distribution, not a courtesy.\nIt existed only in source comments, the data directory's own readme and\ndocs/SCRAPING_ENDPOINTS.md. The data shipped to users; the credit did not.\n\nIt now rides the credits line that already carries the GeoNames and Photon-OSM\nattributions, rather than a parallel key or a new screen -- that surface exists\nprecisely for this, and the ensemble's suggestion to \"add\" the key was wrong,\nit needed extending. Both shipped locales carry it; en and de are the whole set.\n\nThe test asserts the credit renders in the DOM rather than that the string\nexists in a json file. A locale key nothing reads is exactly how this class of\nbug hides, so the check reads the same path the live screen does.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(cli): stop reporting failed calls as dispatched and repair three ceremonies\n\nSix blocking findings from the pre-PR ensemble, each independently verified.\n\nA tauri-level invoke error -- bad arguments, an acl denial, no such command --\nwas folded into the ok path, so a call whose body never ran came back\ndispatched:true and exit 0. Deleting an application while omitting a required\nargument told the caller it was deleted. It was not. I had inspected that arm\nearlier and approved it: an in-band error from a command that DID run is\ngenuinely dispatched, a transport-level rejection is not, and folding both\ntogether erased the distinction. It now has its own sentinel and a non-zero\nexit. The proof read needed the same fix, where a failed read could otherwise\nbe stringified into a resolved proof value.\n\nTwo ceremonies could never be satisfied. documents_list serialises its id as\n_id, and the rows matched on id, so documents_remove and resume_pipeline_run\nalways refused with text falsely saying the record may not exist -- the wire\nshape this repo already has a lesson about. The fixture is now built from the\nreal struct rather than hand-typed, which is what let it pass.\n\nWorse, two rows read their selector from the top level of the input while their\ncommands take a wrapped req, and tauri ignores unknown top-level keys. A caller\ncould satisfy the proof against one record while the command acted on another,\nso the property the whole ceremony exists for did not hold. Selectors are now\npaths, walked from the same place the command reads its target.\n\nFencing was keyed on command name and a field called description, so five other\nread paths returned the same attacker-authored posting text unfenced under\njobAd and jobDescription. It is now keyed on the field wherever it appears,\nwith a completeness test. Verified against the real store: every one of 144\napplications, 2416 found jobs and 115 generations comes back fenced.\n\nTwo rows changed class on evidence rather than instruction. match_resume and\nmatch_resume_text call embed_charged with no budget, documented as deliberately\nuncharged, so they are not dispatchable. And ai_embed became Irreversible with\na spend proof rather than Read as I had directed -- ai_generate carries the\nidentical gate and the same class, and making the one paid command freely\ndispatchable would have reintroduced what the gate was written to stop.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(cli): classify commands by where their input sends data, not just by writes\n\nSecurity review returned request-changes: two critical, three high. The\nunifying defect was the classification method, not the rows. Every command was\njudged on \"does this persist anything\", and three that answer no are\ndestination primitives whose caller-supplied input chooses where data goes.\n\nai_test_provider_key and ai_list_provider_models were Read while taking a\ncaller-supplied base_url, and the ssrf guard blocks only a non-http scheme and\nthe metadata address. One free call posted the stored provider key to any host\n-- and since the key resolver returns none rather than erroring when unset, the\negress fired even with no key configured, with the attacker's own response body\nfolded into the reply unfenced. That was an attacker-controlled injection\nchannel needing no job board at all.\n\nextract_resume was Read with an unvalidated path, and the read happens before\nthe extension check, so a unc path performed an smb connection and leaked ntlm.\nThis branch added a guard for that exact primitive two hundred lines away, on a\npath the app itself writes. It is also one of the commands no renderer calls --\ndead code the surface resurrected.\n\nBoth are now NotExposed, as is support_export_diagnostics, whose only proof was\nthe compile-time version constant -- vacuous by this file's own rule, the one\nthat demoted the token rotation row, while it truncates a caller-chosen file.\n\nThe provider settings rows moved to Irreversible. Reversible described the\nconfig row, which is undoable; it did not describe every later generation the\nUSER runs shipping resume text, contact details and the key to the new host.\n\nFencing covered description-shaped keys only, so a posting whose TITLE carried\nthe injection was never fenced, and array fields were skipped entirely. It now\ncovers title, company, location and requirements, fences string elements inside\narrays, and detects posting-shaped objects to catch the flattened extra map.\nVerified against the real store rather than fixtures.\n\nThe whole table was swept again on the input axis; no further destination\nprimitives were found, and the borderline rows now carry their reasoning.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(cli): close a persistent egress redirect and repair two broken ceremonies\n\nRound four. The previous sweep missed a row because its comment already\nreasoned convincingly about persistence, so it read as a completed audit and\nwas skipped. Re-deriving every row from its command SIGNATURE rather than its\ncomment found that row, and a fourth one nobody had named.\n\nai_set_embedding_config persists a caller-supplied base_url and every later\nembed sends the keychain key plus resume and job-ad text there, fired by\nordinary user activity long after the call. ai_seed_active_config was worse\nstill: its payload carries a provider map with per-provider base_urls AND the\nactive provider, so it persists and activates a redirect in a single call. Its\nrow-presence gate narrows that to a first-run window rather than closing it.\nBoth are now NotExposed.\n\nai_set_provider_settings joins them. Its proof was the active provider's name,\nbut the patch rewrites any provider's base_url and the active one does not\nchange -- so the confirmed value had no relationship to the target, the same\nshape that demoted the diagnostics export an hour earlier. A bound proof cannot\nbe expressed today, since the providers map cannot be reached by a static path.\nai_set_active_provider stays as it was: its proof is the field it overwrites.\n\nAdding title to the fence list had broken two ceremonies. The proof read\nbypassed fencing while every read a caller could actually make went through it,\nso following the hint compared a fenced value against a raw one and failed\nevery time -- and for a title containing a fence marker the raw value is not\neven recoverable. Both sides now read the same fenced value. The test that\nwould have caught it walks every irreversible row through the real fencing\npath; the first version of that test passed with the fix deleted, because it\nbuilt its own comparison instead of exercising production code.\n\nFencing title, company and location also meant a read-then-persist round trip\nwrote fence wrappers into the user's real store. The write path now strips\nthem, rather than dropping those fields from fencing and reopening the hole\nthey were added to close.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(scripts): check every leak on its own, not one survivor per key\n\nThe guard built a map from key to leak, so two sites sharing a key collapsed to\nwhichever was inserted last, and the check then re-derived \"the\" leak for that\nkey and compared ITS signature. When the declared site won that collapse -- the\nrealistic ordering when the undeclared call sits earlier in the file -- the\nundeclared leak's own signature was never looked at, and the guard reported OK\nwhile a genuine undeclared leak sat in the tree. The mirror bug hid staleness\nthe same way.\n\nThis is the second defect from this file's rework, and the same shape as the\nfirst: reasoning about a survivor as if it stood for all the records it\nreplaced. The earlier one let a message edited in place pass because the\nsignature was consulted only when a line had moved.\n\nThe map is now a full bucket and the single key-based predicate is split in\ntwo, one asking whether a given leak is satisfied by its own signature and one\nasking whether any leak in a key's bucket keeps an entry live. Nothing is\nre-derived from a key any more.\n\nConfirmed by putting the old code back: the new collision test reports zero\nproblems, which is the silent acceptance itself. Two properties that were only\ncovered indirectly -- a message changed in place, and two different declared\nsites in one file -- now have named tests rather than being inferred from the\nreal-repo integration case.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(ai): bill every embedding round trip, and resolve the provider once\n\nThe spend gate added earlier on this branch was right in shape and wrong in\narithmetic. It charged one daily unit per admitted call while the embedder\nsplits oversized input across many provider requests -- up to the chunk cap,\nand further again on a context-length retry -- so the daily ceiling was far\nlooser than it read. The input clamp bounds the input, never the number of\nrequests it becomes.\n\nThe counter is unchanged; only the moment of charging moved. A metering wrapper\ncharges immediately before each real attempt the embedder makes, retries\nincluded, so the count matches what was actually sent. Reserve-and-release was\nthe other option and was rejected: the limiter has no refund primitive, and\ninventing a partial-refund ledger to serve one caller is the wrong shape.\n\nSeparately, the charge and the dispatch each read the embedding config, so a\nconfig change landing between them billed one provider while the request went\nto another. The config is now read once and that single value feeds both.\n\nBulk re-index and match scoring are untouched: they call the embed helper\ndirectly, which still reads the store itself and passes no charge, so their\nbehaviour is byte for byte what it was.\n\nNote for whoever edits next: ai_provider/mod.rs is now 1399 lines against the\n1400 hard cap -- this change took it from 1390, so the next line added there\nfails the architecture test. It wants an extraction before it grows again.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix(cli): fence by struct fixture, and carry the confirm invariant in the type\n\nEight review findings across both reviewers.\n\nThe confirm path ended in an expect on an externally reachable ipc path, in a\ncrate that aborts on panic above crash reporting, guarded only by a separate\ncall agreeing with it. The gate now returns the decision and the proof source\nand the confirm value together, so reaching the confirmed case IS the evidence\nthe value was supplied and there is nothing left to unwrap.\n\nFencing missed six more prose fields, and this was the fourth round on that\nclass. Adding names buys one round, so the fix is the test: fixtures built from\nthe real structs assert every string field is fenced or explicitly safelisted,\nso a new prose field fails a test instead of leaking. PII and first-party\nfields are deliberately excluded and documented -- fencing exists to stop\nattacker text steering the agent, not to redact the user's own data, which is\na separate axis the adr settles.\n\nError details were never fenced, only success payloads, so a command whose\nerror embeds a remote server's message returned it verbatim. That channel is\nclosed. The two commands the finding named cannot actually produce it, since\nthey return a bare value rather than a result -- the example was wrong, the\nchannel was real.\n\nscrape_resolve_url claimed no persistence and upserts into the discovered\ncompany store on every resolve; it is Reversible now, with the comment\ncorrected. ai_pull_model claimed no data destroyed and additive, but nothing in\nthe app can delete a pulled model, so a multi-gigabyte write had no undo; it is\nIrreversible with a weak proof that at least forces a read first.\n\nFence stripping was applied at one writer; it now runs at the single dispatch\nchokepoint every invocation passes through, so a future writer inherits it\nrather than needing to remember. The circularity guard now resolves a proof's\nread command to its own row and rejects any irreversible target, covering the\ncase its doc always claimed.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-09-01T05:08:29+02:00",
          "tree_id": "a710d93c1c26fe7ea942a951b7ac11e2608604ab",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/38b990d0e9491845313946ad7de304e8a492d15c"
        },
        "date": 1788232843848,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2180413,
            "range": "± 70102",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2570679,
            "range": "± 41962",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 229238,
            "range": "± 5084",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f65103265f63cbe18143710d4448bd0c41d95007",
          "message": "fix: make the agent CLI reachable after an install or update (#1088)\n\n* fix: symlink the agent cli onto path for homebrew installs\n\nThe `ajh-tauri agent <verb>` CLI (ADR-037/038) ships inside every bundle but\nwas reachable only by callers that read the app's ~/.ajh-agent/agent.json\npointer file. A human typing `ajh-tauri agent` got \"command not found\" on a\nmachine where the app was installed.\n\nThe cask now symlinks the bundle's executable onto PATH. The binary name was\nverified against the shipped artifact rather than inferred: the macOS bundle\nlays out as `AI Job Hunter.app/Contents/MacOS/ajh-tauri`, i.e. named after the\nCargo [[bin]], not productName.\n\nLinux needs no equivalent change: the deb already installs `usr/bin/ajh-tauri`.\nWindows (nsis, installMode currentUser) is still unreachable and is handled\nseparately.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs: document the agent cli for users\n\nThe `ajh-tauri agent <verb>` surface (ADR-037/ADR-038) was invisible: it\nappeared nowhere in the README, nowhere under docs/ except CONTEXT.md, and\nnowhere on the landing site. The only landing mention is the `.claude/`\ndevelopment fleet, which a reader will conflate with it.\n\nAdds a README usage section and a knowledge-base entry, registered in the\nindex. The knowledge page states shape and points at `--help` and the owning\nsymbols rather than copying the verb table, the sentinel list or `max 50`,\nper AGENTS.md rule 17.\n\nBinary locations are stated per platform from shipped v0.145.0 artifacts, not\ninferred. Windows is documented as NOT on PATH, which is true today; the NSIS\nhook that changes it is a separate commit.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix: add the agent cli directory to the per-user path on windows\n\nCompanion to the Homebrew cask change: the `ajh-tauri agent <verb>` CLI\n(ADR-037/ADR-038) ships inside the Windows bundle but the per-user install\ndirectory is not on PATH, so a human could not invoke it. Linux already\ninstalls to /usr/bin, and macOS is covered by the cask's binary stanza.\n\nAdds an NSIS POSTINSTALL/POSTUNINSTALL hook pair. PATH is a value whose\ncorruption breaks the user's shell, so every branch is defensive:\n\n- Truncation guard. ReadRegStr SILENTLY truncates past NSIS_MAX_STRLEN\n  (1024 in the Unicode build Tauri bundles). A value read within one char\n  of the cap cannot be trusted to be complete, and writing it back would\n  permanently drop the remainder, so PATH is left untouched instead. A\n  second budget check refuses an append that would land near the cap.\n- Idempotent across updates. The installer re-runs on every update, so both\n  haystack and needle are ';'-padded before searching; a directory sharing\n  our prefix cannot match, and the entry is never appended twice.\n- Type preserving. REG_SZ vs REG_EXPAND_SZ is queried via Advapi32 and the\n  write-back matches; any other type is left untouched.\n- Uninstall splices out exactly our segment and the one delimiter it owns,\n  leaving no doubled, leading or trailing ';'.\n- Best effort throughout: every failure path DetailPrints and continues, so\n  a PATH problem can never abort an install or uninstall.\n\nVerified by compiling these macros with makensis and running them against a\nscratch registry key: absent/REG_SZ/REG_EXPAND_SZ values, sibling-prefix\ndirectories, re-runs, both truncation guards, removal from first/middle/last/\nonly position, and an unexpected REG_DWORD. A full release-bundle install was\nnot run.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs: note the windows path hook lands in the next release\n\nThe previous commit added the NSIS hook, so \"Windows is not on PATH\" is only\ntrue for v0.145.0 and earlier. Stated as a release boundary rather than a\nbare \"yes\", since no shipped installer carries the hook yet.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix: make the windows path hook compile and survive a failed probe\n\nSecurity review of 7b9e1755 blocked it on two defects and one that made the\nfeature a no-op on a realistic machine. All three are fixed here.\n\nThe uninstall hook did not compile. `${StrLoc}` expands to `Call StrLoc`, and\nNSIS forbids calling a non-`un.` function from an uninstall section. PR CI\nruns `cargo check` and never bundles, so this would first have appeared in\nthe release job. StrFunc is gone entirely, replaced by `Shlwapi::StrStrW`\nthrough `System::Call` — a plugin call, so the `un.` rule cannot apply.\n\nA failed probe took the destructive branch. `$1` was uninitialised, and a\n`System::Call` that fails to resolve writes the literal string \"error\" into\nits output register, which `${If} $1 != 0` treats as \"no PATH value exists\" —\nreplacing the whole per-user PATH with just $INSTDIR, then reporting success.\nOnly a literal ERROR_FILE_NOT_FOUND (2) now creates a value; every other\noutcome leaves PATH untouched.\n\nThe 1024-char ceiling made it a no-op on ordinary machines. NSIS string\nvariables cap at NSIS_MAX_STRLEN, and the old budget refused any PATH past\n~1016 chars — a developer machine with a 988-char PATH plus the install dir\nwas already over it, so the installer would silently do nothing while the\ndocs promised otherwise. The value's content now never enters an NSIS\nvariable: RegQueryValueExW/RegSetValueExW against GlobalAlloc'd memory, with\nStrStrW for the whole-segment search.\n\nCorrects the record from 7b9e1755: ReadRegStr does NOT silently truncate past\nthe cap. Measured, it sets the error flag and returns an empty string, so the\nreal protection was always the ${Errors} check. `${NSIS_MAX_STRLEN}` is also\na live compiler builtin, contrary to the previous comment.\n\nAdds a compile-time guard: the hook hardcodes HKCU, so it now refuses to\nbuild under any installMode other than currentUser rather than silently\nwriting an elevating admin's hive.\n\nVerified independently of the author: both macros compile clean in a real\nSection Install and a real Section Uninstall under makensis 3.11, and the\ninstallMode guard aborts the build when flipped to perMachine.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix: gate every buffer input in the windows path hook\n\nSecurity re-review of a026a81a found no blocking defect but three crash\npaths, all one root cause: a length or pointer reached RtlMoveMemory or\nlstrcpyW without a precondition. Fixed as a single gate rather than three\npatches, since patching each symptom as it appears is what produced two\nearlier bugs in this file.\n\n- A registry value with cbData = 0 is legal Win32, and made `$3 - 2` yield\n  -2, which RtlMoveMemory reads as 0xFFFFFFFE. The install macro had no\n  clamp on the value size at all; uninstall survived the same input only\n  because StrStrW failed to match first. Both now refuse a value too small\n  to be a valid PATH before any arithmetic runs.\n- GlobalAlloc's return was dereferenced unchecked at four sites. Each now\n  bails, and the two second allocations free the first buffer on the way.\n- `System::Call` writes the literal string \"error\" into its output register\n  on a resolution failure, so uninstall's `${If} $5 == 0` fell through into\n  pointer arithmetic with $5 parsing as 0. Both sides now compare\n  numerically, so the sentinel lands in the safe branch by construction\n  rather than by luck of polarity.\n\nAlso restores a guard the previous rewrite dropped silently: a PATH already\nending in ';' no longer gains a doubled separator.\n\nPATH was left untouched in every injected failure before this change too —\nthese were liveness defects, not data-loss ones. Verified: the 15-case\nregression matrix still passes including the >1024-char case, cbData = 0 no\nlonger crashes, and both macros still compile in a real Section Install and\nSection Uninstall. Forcing GlobalAlloc to fail at the real ~20-100 byte\nallocation sizes was not achievable, so that branch is verified by pattern\nrather than end to end.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-09-01T20:09:57+02:00",
          "tree_id": "df46d46872a2513c96fb9bc53f3b3e72ac4e6050",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f65103265f63cbe18143710d4448bd0c41d95007"
        },
        "date": 1788286885327,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2190180,
            "range": "± 15592",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2581973,
            "range": "± 16085",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 302241,
            "range": "± 7011",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f336aee29da2093700de054c75e2dbed99bef6c5",
          "message": "fix: move the nsis installMode guard inside the hook macros (#1089)\n\nThe guard added in #1088 aborted every Windows bundle, so the Release\nworkflow failed on main. Tauri's generated installer.nsi does\n`!include \"<hooks file>\"` about ten lines BEFORE `!define INSTALLMODE`, so at\nfile-scope include time the symbol is undefined. NSIS leaves an undefined\n${SYMBOL} as literal text, which is never equal to \"currentUser\", so the\n!error fired unconditionally:\n\n  !error: windows/hooks.nsh hardcodes HKCU for the per-user PATH ...\n  Error in script installer.nsi -- aborting creation process\n  failed to bundle project: `Failed to bundle app with makensis`\n\nBoth the local verification and the security review missed it because their\nharnesses defined INSTALLMODE BEFORE including the hook file -- the reverse\nof the real ordering. The guard was the one part of that change never\nexercised against the real generated script, and it is the part that broke.\n\nThe check now lives inside both macros, which expand at their insertion\npoints inside the install and uninstall sections, far below the define,\nwhere the value is real. The reason is recorded at the top of the file so\nnobody moves it back.\n\nVerified with a harness that includes the hook BEFORE defining INSTALLMODE,\nmatching the real template. The control matters more than the fix: the\npre-fix file reproduces the exact CI error under that harness, the fixed\nfile compiles, and flipping the harness to perMachine still aborts, so the\nguard kept its purpose rather than being defanged.\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-09-01T20:52:01+02:00",
          "tree_id": "349fe779e4aa93190174ec786fe51c504380e45e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f336aee29da2093700de054c75e2dbed99bef6c5"
        },
        "date": 1788289446029,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2195028,
            "range": "± 10895",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2594370,
            "range": "± 22819",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 308844,
            "range": "± 7980",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a27d89da76e2edf8921b3eac28448cda636ba173",
          "message": "feat: hybrid search over the live postings cache (#1090)\n\n* feat: hybrid search over the live postings cache\n\nThe README has claimed \"hybrid vector + keyword search\" in three places while\nno such thing existed: what shipped was hybrid *scoring*, a pairwise blend of\none resume against one posting. There was no index, no top-k, no search. This\nis the retrieval half.\n\nNew L1 `retrieval/` module, kept pure so it names neither EmbeddingVector nor\nCompleter -- fusion consumes ranks and the caller does its own embedding, with\nthe reranker declared as a port and implemented at L3.\n\n- Lexical arm: FTS5 over an in-memory connection built from the live cache.\n  Nothing is persisted, so this adds no retention or support-bundle surface.\n  FTS5 needed no dependency; it is already compiled into the shipped binary.\n  User input is sanitised into quoted phrases so FTS5 operator syntax cannot\n  error or misfire.\n- Dense arm: revives PostingsCache's embedding API, dead since it was written.\n  The f32-only surface is deliberate -- at N=2000/dim=768 a scan is 116.2ms of\n  which JSON parsing is 115.8ms, so the format was the bottleneck, not the\n  absence of an index.\n- Fusion: reciprocal rank fusion over ranks, not scores.\n- Rerank: top candidates through the existing structured-completion path, so\n  it inherits charge/ask/record and adds no egress host. Every candidate is\n  fenced under its own tag; a job ad is attacker-authored text entering a\n  ranking prompt.\n\nRanking is scoped to an eligible-id allowlist rather than the whole cache: the\nJobs page applies cluster-canonical, agency and work-type filters after the\nlist is built, so ranking everything would make a \"top 10\" render four rows,\nand the survivors would not be the ten best eligible postings.\n\nThe command snapshots a cache generation counter and refuses to return results\nif a replace-scrape cleared the corpus mid-search. Degradation is reported\nrather than hidden -- the reply carries which arms actually ran, so a lexical\nlist can never be presented as hybrid. semantic_scoring defaults to false, so\nthat is the common path, not an edge case.\n\nAdding a command forces the ADR-038 policy table; the new row is classified\nIrreversible because it can charge the per-provider daily ceiling. Note\npolicy.rs is now 1399 of R8's 1400-line cap -- pre-existing debt, but the next\ncommand addition must split it.\n\nRenderer work, the README correction and the ADR are not in this commit.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat: ranked search ui over the postings list\n\nWires the hybrid search command into the Jobs page. The interesting part is\nnot the search box, it is refusing to present a keyword list as a hybrid one.\n\nsemantic_scoring defaults to false, so on a default install the dense arm\nreports \"skipped\" and results are keyword-only. That is the common path, not\nan edge case, which is why the command reports which arms actually ran. The\nresults banner states what ranked the list, and when semantic ranking is off\nit offers a one-click path to enable it rather than a message the user cannot\nact on. Zero-hit, stale-corpus, cancelled and rerank-unavailable are all\ndistinct states -- in particular a query with no matches must never read like\n\"you have not scraped any jobs yet\", which is a different situation.\n\nOne input, two actions rather than two competing text boxes: the existing\nfilter stays an instant substring match on every keystroke, and Enter (or the\nSearch button) commits the same text as a ranked search. A committed search\nreplaces the substring view rather than layering on it -- pre-filtering the\neligible set by literal substring would defeat the point of semantic\nretrieval, which is surfacing matches that do not contain the query verbatim.\nWhether a search is active is derived from whether the committed query still\nequals the current text, so a stale ranked list can never sit under new text.\n\nRanking is scoped to the postings that will actually be shown: cluster-\ncanonical, agency and work-type filters decide the eligible ids that go to the\nbackend, and hits are re-intersected against the current eligible set on every\nrender. Ranking the whole cache and filtering afterwards would make a \"top 10\"\nrender four rows.\n\nA superseded search is cancelled before the next one fires, since a Tauri\ninvoke is not abortable from the renderer and would otherwise keep embedding\nand reranking for a query the user has already replaced.\n\nen and de both land in this commit at 2544 leaf keys with zero key-path\ndivergence; nothing in the repo enforces that, so a missing de key would be a\nsilent runtime regression rather than a build failure.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs: correct the hybrid-search claim in the readme\n\nThe README claimed \"hybrid vector + keyword search\" in three places while no\nsearch existed at all -- what shipped was hybrid scoring, a pairwise blend of\none resume against one posting, with no index and no top-k. An interviewer\ngrepping for the index would have found two stale comments describing a\nremoved feature and a dead embedding API with zero callers.\n\nSearch now exists, so the claim is nearly true -- but not the way it was\nworded. semantic_scoring defaults to false, so a default install gets BM25\nonly and the dense arm reports \"skipped\". Calling that \"hybrid\" would swap\none overclaim for another. The three sites now say keyword by default, hybrid\nwhen semantic ranking is enabled, and the tech-stack row names what actually\nruns rather than a capability label.\n\nAlso separates two things the old wording conflated: ranked search over\nscraped postings is new, and per-posting scoring against the resume is the\npre-existing feature.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* docs: record adr-039 and resync the pages hybrid search made stale\n\nAdds ADR-039 for hybrid postings search, recording each decision with the\nalternative it rejected: the in-memory cache as corpus (persisting postings was\nrejected for the migration, retention and support-bundle surface it would add),\nFTS5 needing no dependency, f32 vectors over the existing JSON-TEXT f64 (a scan\nwas 116.2ms of which 115.8ms was JSON parsing, so the format was the bottleneck\nand an ANN index was not needed), LLM listwise reranking over a cross-encoder\n(which would need an ML runtime in a three-OS build), ranking scoped to an\neligible-id allowlist, and reporting degradation rather than hiding it.\n\nThree pages went stale on this feature and a review caught all three:\n\n- anti-abuse-limits.md enumerates the limiter bucket inventory, which gained\n  hybrid_search_rerank. A first pass claimed no bucket was added; the constant\n  says otherwise.\n- matching-algorithm.md enumerated exactly two scoring modes, both of which\n  score a resume against a posting. Postings search ranks postings against a\n  user QUERY and shares no kernel with them, so it is listed as a third,\n  explicitly separate surface rather than left to be mistaken for one of them.\n- architecture-rules.md's L1 module list omitted retrieval; the const in\n  tests/architecture.rs is authoritative and the prose now matches it.\n\nThe ADR names the limiter bucket outright rather than relying on the inventory\npage, since a list that goes stale silently is the same failure class as the\nREADME overclaim this feature exists to fix.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* test: pin the hybrid-search wire contract and adversarial rerank output\n\nSix tests, each mutation-checked rather than assumed: the mutation is named,\nthe test that caught it is named, and the production file is byte-identical\nafterwards.\n\nThe most valuable three pin the wire strings the renderer's arm-honesty UI\ndepends on -- \"ran\"/\"skipped\"/\"unavailable\", \"ok\"/\"cancelled\"/\"staleCorpus\",\nand camelCase field casing. Nothing locked these before, so dropping a serde\nrename would have broken the renderer silently with no compiler error on\neither side of the IPC boundary. Verified by dropping the rename and watching\nthem fail.\n\nThe others cover an adversarial reranker returning an invented id, a duplicate\nand a dropped candidate in one response (not three isolated cases), the\nboundary between an absent allowlist and one that matches nothing, and that\nthe single choke point every early return funnels through can never emit hits.\n\nKNOWN GAP, recorded rather than papered over: the live orchestration is not\nintegration-tested. run_search/run_dense_arm/maybe_rerank take &AppHandle and\nread cache/store/limiter/preferences off it inline, and this crate has no mock\nAppHandle -- enabling Tauri's MockRuntime was rejected twice before in this\ncodebase for the same tradeoff (extension_bridge/test.rs:1225-1228 and the\ncommands/system/test.rs notes). So dense skipped-vs-unavailable-vs-ran as\nactually produced, rerank unavailable on a real provider error, staleCorpus\nand cancelled as actually detected, and the over-long allowlist rejection all\nremain uncovered. The fix is an AppHandle-free env trait mirroring RerankEnv,\nwhich is a production restructuring and deliberately out of scope here.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix: surface a failed semantic-scoring sync and keep the banner out of the scroller\n\nReview of the ranked-search UI found one blocking defect and three smaller\nones; all four are addressed here.\n\nThe one-click \"enable semantic ranking\" path wrote the preference through a\nfire-and-forget mutation with no error handler, and the QueryClient has no\nglobal mutation onError. A failed backend write therefore flipped the local\npreference to on, re-ran the search and showed results, leaving the user with\nno sign the persisted mirror never updated -- worse than silent, because the\nUI looked fully successful. The hook's own doc claimed to mirror the settings\npanel's call site, which does handle the error. It now does the same, reusing\nthe existing translation key, and a test fails if the handler is removed.\n\nThe results banner rendered inside the same overflow container as the\nvirtualised list without a scrollMargin, so scrolling a ranked list to the\nbottom left a gap the height of the banner. Rather than measure the banner\n(its height varies with which arm notices render), it now sits outside the\nscroller as a sibling; a test asserts it is never a descendant of the\nscrolling element.\n\nRetyping the last committed query resurrected the cached ranking without\nre-running against postings that may have arrived since. Deciding whether a\nbackground scrape should silently evict a ranked view the user is reading is\na product judgment, not a mechanical fix, so the text-only predicate is\nextracted, documented as intentional, and pinned by a test whose name states\nthe no-corpus-signal contract.\n\nThe zero-hit state reported the corpus size at search time, so a filter\ntoggle afterwards could claim \"no matches among 45\" when 30 were eligible;\nit now reports the current eligible count.\n\nSearch ids are minted with a `search-` prefix to match the Rust side's new\nvalidation, so a caller can never collide with a `job-{uuid}` in the shared\ncancel registry.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: state what hybrid search does not do, and fix five false claims in adr-039\n\nReview found five claims in ADR-039 false against the code it documents, and\na verification pass found the feature's headline claim structurally false in\nthe common case. Both are corrected here.\n\nThe dense arm's candidate pool is the lexical top-40 whenever keyword search\nfinds anything, so a posting BM25 missed is never embedded and cannot surface\nthrough dense or fused ranking. The dense arm and rerank RE-ORDER keyword\nhits; they RETRIEVE only when keyword search finds nothing, and then over the\nfirst 40 postings in cache order. The ADR said \"reranking is the mitigation\"\nfor FTS5's non-semantic nature -- it is not, because the rerank never sees a\nnon-lexical candidate. A \"What this does and does not do\" section now states\nthe policy and points at DENSE_CANDIDATE_MAX as the owning source, and the\nREADME's two claim sites are narrowed so they cannot be read as \"semantic\nsearch finds things keyword search misses\".\n\nThe five corrections: the rerank is described as gated on semantic_scoring\nalongside the dense arm (true as of the accompanying fix, enforced by a test);\nthe reply field is `arms`, not `degraded`; eligible ids are filtered at L3 and\nnever reach retrieval/; PostingsCache has no text hash, it is an id-keyed map\nwith explicit invalidation; and the f32 decision no longer cites a 116 ms\nJSON-parse measurement, which was of the SQLite posting_vectors store, not the\nin-memory cache. The honest reason is scale: the cache is bounded, so no ANN.\n\nA \"What is measured and what is not\" section records that ranking invariants\nare asserted in CI, retrieval quality is not measured because there is no\nlabelled relevance data, and the fusion constants are reasoned with cited\nsources rather than tuned. Extending this repo's deterministic-first candour\nto retrieval is the point.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: gate the rerank, make it cancellable, and never score across embedding spaces\n\nThree independent reviews of the hybrid-search backend converged on the same\ndefects. All are fixed here, each gate mutation-checked: the guard deleted,\nthe test watched to fail, the guard restored.\n\nThe rerank ran unconditionally. The dense arm read semantic_scoring; the\nrerank did not, so a default install with a cloud provider sent the query and\nup to twenty scraped postings to a paid provider on every Enter, while the\nmodule doc, ADR-039 and the README all said it was off by default. Three\nprose restatements of a missing gate are three copies of the same lie, not\nthree checks. should_rerank() is now the single gate, mirroring\nautopilot::rerank::should_semantic_rerank, and a test fails if it is removed.\n\nThe rerank could not be cancelled and inherited a 300s generation deadline.\nIt was a bare await, never raced against the cancellation token, so\njobs.cancel did nothing once it started -- contradicting the hook's own doc --\nand it held one of three concurrency slots for the duration. It now runs\nunder tokio::select! against the token and a new HYBRID_SEARCH_RERANK\ninteractive-tier timeout, justified in timeouts.rs against the existing\ntiers; each embed is raced the same way. Cancelled, timed out and errored all\ncollapse to Unavailable plus the fused order.\n\nThe #1087 billing race was back. The embedding config was snapshotted for\nthe budget but AppEmbedder re-read it at dispatch, so ai_set_embedding_config\nlanding between them charged one provider while billing another -- the exact\nshape commands/ai/mod.rs documents fixing. Both embeds now go through\nembed_with_config from one snapshot. Separately, the freshly-embedded branch\nnever checked the returned vector's space and the query vector was never\ncompared to a candidate's; two models at dim 768 produced a plausible cosine\nthat meant nothing. dense_pair() now refuses any pair whose spaces differ.\n\nSmaller: queryId is validated to 64 chars and a `search-` prefix so an agent-\nCLI caller cannot collide with a live job-{uuid} in the shared cancel\nregistry; the embedding write-back re-checks the cache generation under the\nsame lock so an in-flight search cannot resurrect vectors after\nprivacy_clear_data; ai_set_embedding_config now actually clears cached\nembeddings on a space change as its comment always claimed; PostingsCache::add\ninvalidates only when the text changed, so \"show more\" no longer wipes the\ncache for byte-identical postings; a Drop guard releases the cancel-registry\nslot on panic; the operator-safety test now asserts `-golang` finds the doc\ncontaining golang instead of discarding its results; and two constant docs\nstop claiming recall properties they do not measure.\n\nThe dense candidate pool is unchanged, but its doc now says plainly that the\ndense arm re-ranks lexical hits and only retrieves when lexical found nothing.\ndense.rs drops a 116 ms JSON-parse justification that was measured on a\ndifferent store.\n\nTwo script inventories re-anchored: the log-error-leak allowlist keys that the\npostings edit shifted, and the JobsPage event-subscription hash that the\nrenderer edit shifted. Neither note's content changed.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: point adr-039 at the file that actually owns the candidate cap\n\nTwo pointers in ADR-039 named `retrieval/mod.rs` as the home of\nDENSE_CANDIDATE_MAX. The constant and its doc comment live in\ncommands/hybrid_search.rs; retrieval/mod.rs does not mention it. A rule-17\npointer that points at the wrong file is worse than no pointer, because the\nreader stops looking.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: split the rerank timeout by provider class and pin the search-id prefix on the wire\n\nRe-review of the rerank fixes found one number that could make the feature\npermanently degraded for the offline-first audience, plus six consistency\ngaps. All are addressed; nothing here changes a gate or a charge path.\n\nThe 45s rerank timeout was a single flat tier. The rerank prompt is\nRERANK_TOP_K x RERANK_ITEM_CHAR_BUDGET, roughly 3,000 input tokens, and CPU-\nonly Ollama commonly evaluates prompts at 20-60 tok/s -- 50 to 150 seconds\nbefore the first output token. On that fully-supported configuration the\nrerank would have reported unavailable on every search, permanently, with no\nway for the user to tell a stalled provider from a slow one. It now follows\nthe file's existing local/cloud split for completions: 45s for cloud\nproviders, 180s for Ollama. The local bound is DERIVED from that token\narithmetic, not measured, and its doc says so and names the measurement that\nwould replace it. Relational tests pin both tiers against their neighbours,\nthe way every other constant in that file is pinned; dropping the local tier\nbelow its own derived floor fails a test that names the broken derivation.\n\nThe wire contract now states the search-id prefix the backend hard-rejects.\njobs/cancel.rs names that prefix as the safety net against colliding with a\nlive job id, yet the Zod schema said only min(1).max(64), so an agent-CLI\ncaller following the published contract would have been rejected with no\ndocumented reason. An existing test fixture was minting an unprefixed id and\nwould have failed its own new constraint; it is corrected and a rejection\ntest added. Generated docs are unaffected.\n\nSmaller: the id length check now counts chars like its sibling three lines\nup instead of bytes; a security-reasoning comment claimed resume_pipeline_run\nregisters run-{uuid} when it registers job-{uuid}, fixed in both places it\nwas stated; the embed_with_config doc no longer claims ai_embed is its only\ncaller; and clear_embeddings now bumps the cache generation so an in-flight\ndense arm holding an older config snapshot cannot re-seed stale-space vectors\nafter a settings-driven clear -- belt-and-braces, since every read was\nalready space-guarded, and stated as such.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: revert the clear_embeddings generation bump and anchor the timeout floor to its inputs\n\nThe previous commit made clear_embeddings bump the cache generation as a\nbelt-and-braces guard on the dense arm's write-back. That counter has a\nsecond reader: the search's stale-corpus return check. ai_reembed_all calls\nclear_embeddings unconditionally on every \"Re-index now\", so a re-index\nlanding mid-search discarded a fully-computed, still-correct ranking,\nreturned no hits, and told the user \"your scraped jobs changed\" -- which was\nfalse, since clear_embeddings leaves the job list intact. One field serving\ntwo consumers with opposite safe directions, for the third time on this\nbranch. The bump is reverted; the write guard covers clear_all only, by\ndesign, and a stale-space vector re-seeded after a settings-driven clear is\ndead weight, never scored, because every read is space-guarded. The existing\ngeneration test now asserts clear_embeddings does NOT bump, as a direct guard\nagainst reintroducing this.\n\nThe local rerank timeout's test compared against a re-typed 150 seconds. Its\ndocumented derivation reads RERANK_TOP_K and RERANK_ITEM_CHAR_BUDGET; doubling\nthe former makes the real floor 300s and the doc false, and the old test\nstayed green. The floor is now computed from those constants in the test, so\nit moves with its inputs -- verified by doubling RERANK_TOP_K and watching the\ntest fail with the recomputed floor, then reverting.\n\nTwo doc clauses: the local tier's margin over its derived floor is stated as\nthin, and named as having to absorb output tokens and a cold model load; and\nthe provider-class split is stated as Ollama-only -- a self-hosted\nOpenAiCompatible endpoint and the CLI agents sit on the cloud tier, which\nfails safe and is identical to the flat bound it replaced, with the follow-up\nnamed.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: fix a dangling link in the search contract and stop calling f32 ranking lossless\n\nTwo documentation findings from external review of #1090.\n\nThe contract's TSDoc linked to a `PostingsHybridSearchResult` type that does\nnot exist; the result type is `HybridSearchResult`. The link resolved to\nnothing in editors and in the generated API page, which is regenerated here.\n\nADR-039 described f32 vectors as \"lossless for cosine-distance ranking\". They\nare not: rounding perturbs cosine values in the last few bits and can reorder\nnear-tied postings. The honest claim is that f32 is sufficiently precise for\nthis bounded ranking, because no tie-tolerance guarantee is made or needed --\nthe rerank is the ordering authority when it runs, and the fused list is a\ncandidate pool rather than a scored ranking.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: retry after the semantic-scoring write lands, and keep degradation copy true per arm\n\nFour renderer findings from external review of #1090, each verified before\nfixing.\n\nThe one-click \"enable semantic ranking\" path flipped the local store, fired\nthe async preference write, and retried the search immediately. The retried\nsearch reads the preference from the Rust store, which the write had not yet\nreached, so it ran with the dense arm still skipped and the user who had just\nclicked the button got keyword results again. The retry now runs in the\nwrite's onSuccess; on failure the toast explains and nothing retries. A test\nwith a deferred promise proves no search is issued before the write resolves.\n\nThe mock client's hybrid-search stub resolved undefined, and the hook's\nsuccess handler reads outcome and hits from it, so a mock-backed search --\nthe client Playwright and dev mode use -- threw after a successful request.\nIt now returns a typed empty result with every arm skipped.\n\nThe degradation notices made claims about the combined order that were false\nin reachable combinations: with the dense arm unavailable and the rerank\nhaving run, \"showing keyword results\" and \"showing the fused keyword/semantic\norder\" rendered together. Each notice now describes only its own arm; the\n\"Ranked by\" line already states what actually ran. Copy-only change in both\nlocales, key counts unchanged at 2544 each.\n\nA test that called a pure predicate twice with identical arguments proved\nonly that it was deterministic, not that it takes no corpus input. It now\nasserts the arity directly, with a note on why a corpus-aware version would\nbelong in the page, not the predicate.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: report a failed lexical query as unavailable, and bound the dense arm's wall clock\n\nTwo backend findings from external review of #1090, both verified before\nfixing -- and the first one's stated trigger turned out to be wrong.\n\nThe review claimed a punctuation-only query such as \"-\" makes FTS5 reject the\nMATCH and the lexical arm reports a successful zero-hit search. Twenty-seven\nadversarial inputs, including that one, all succeeded with zero rows: quoting\nevery token genuinely defeats the operator grammar. What DOES reproduce is an\nembedded NUL byte, and it fails at a third swallow point the review did not\nname -- prepare and query_map both succeed, the error surfaces only when the\nrow iterator is stepped, and filter_map(Result::ok) was quietly discarding\nit. So the concern was real and the specifics were not. LexicalIndex::search\nnow returns a Result and propagates all three error paths; a per-row error\naborts the result, since it reflects the query expression failing rather than\none row's data. The degrade decision moves to L3, where a new run_lexical_arm\nmaps Err to ArmStatus::Unavailable, keeping the swallow-to-empty policy out of\nthe L1 module. Tests at both levels use the NUL-byte trigger.\n\nThe dense arm had no wall-clock bound: one query embed plus up to forty\ncandidate embeds, sequentially, each bounded only by the provider's 30s\nper-attempt timeout, with user cancel as the only exit. The loop now also\nbreaks on elapsed time -- bounding by elapsed rather than wrapping in a\ntimeout, because a timeout would drop the future and discard every pair\nalready embedded, whereas the loop can exit and rank what it collected, the\nsame partial-results posture the existing cancellation check already had.\nDENSE_ARM_TIMEOUT is derived (three worst-case embed round-trips plus a\nconnection margin), says so, and names the measurement that would replace it.\nIt is deliberately not split by local and cloud: the rerank split exists\nbecause generation is an order of magnitude slower on CPU, and embedding is a\nsingle forward pass with no such asymmetry -- the per-call ceiling it is built\nfrom is already identical for both. A relational test pins it against three\nembed round-trips and against COMPLETION; dropping it to 60s fails the test.\n\nNot covered: the elapsed-time break inside run_dense_arm itself has no test,\nfor the same reason the adjacent cancellation check never has -- it needs a\nmanaged-state harness this crate does not have.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n---------\n\nCo-authored-by: Claude Opus 5 <noreply@anthropic.com>",
          "timestamp": "2026-09-02T01:25:45+02:00",
          "tree_id": "ccb5de190e24a3583176245668f0524f3500e8e8",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/a27d89da76e2edf8921b3eac28448cda636ba173"
        },
        "date": 1788305850483,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2263958,
            "range": "± 39507",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2691112,
            "range": "± 40712",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 305715,
            "range": "± 7606",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c01e7eaf7a81cbc801498ee40ce1480f6e56924d",
          "message": "docs: make the ai rigour visible where a reviewer actually reads (#1091)\n\n* docs: make the ai rigour visible where a reviewer actually reads\n\nThe repo already contained a deterministic validator taxonomy with a measured\nfalse-positive budget, evidence grounding that drops any quote not literally\nin the source, a fabrication gate that refuses to save rather than save with\na flag, untrusted-text fencing, and an offline eval harness in CI. None of it\nwas visible on the surfaces a reviewer reads first: the README had zero\noccurrences of recall, precision, false-positive or eval harness, and\ndocs/knowledge covered the provider layer but not the generation pipeline.\n\nAdds a README section that lets a reader answer, without installing, how the\noutput is known to be true and how the checks are known to work -- the\nharness stated as properties (every planted defect recalled at its claimed\nseverity, zero Criticals on truthful documents, Warning false positives held\nto a named budget, CI-enforced) rather than as numbers, and a candour\nparagraph on what is not measured. The fencing bullet is written around the\nprinciple -- anything the user did not type is fenced, including the model's\nown earlier output -- with scraped job ads as the example, since that is\nwhere an attacker has a foothold.\n\nAdds docs/knowledge/generation-domain.md, the missing domain page, and\nregisters it. Every number that would drift is a pointer to its owning\nsymbol; a first draft pasted a stage count, a repair-round cap, a call count\nand a candidate cap, and each was replaced with the constant that owns it.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: say on the landing what the app does, not just how the repo is built\n\nThe landing's only AI-shaped page documents the .claude development agent\nfleet -- how this repo is built -- and a reader takes it for the product's\nAI. A note at the top of /agent-system now says which it is and points at\n/how-it-works for the product's own pipeline. It reuses the page's existing\naccent tokens and the same callout shape /how-it-works already has, rather\nthan a new style.\n\nThe /how-it-works interview cheat-sheet gains entries a reviewer would ask,\neach answered from the source with file paths and no numbers that drift: how\nthe model is stopped from inventing experience, how the checks are known to\nwork (the harness, stated as properties), what happens when it is not sure,\nhow prompt injection from scraped ads is handled, what is deliberately not\nmeasured, and whether the job search is actually semantic -- answered exactly\nas ADR-039 does. The landing is not localized, so these are plain strings.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* test: measure which synonym pairs the lexical arm misses, and that the pool does not bridge them\n\nA deterministic table over a frozen copy of the keyword kernel's synonym\npairs: for each, one posting containing only the alias, queried with the\ncanonical, against a real FTS5 index. Under rusqlite's bundled build, 22 of\n24 pairs are lexical misses; the two hits are react.js and vue.js, because\nunicode61 splits on the dot and a bare-word query already matches. The count\nand the exact hit set are asserted as literals, not derived from each other.\n\nA second row per pair adds a distractor containing the canonical term and\nasserts the alias posting is absent from the dense candidate pool, so the\nmiss is not bridged once keyword search finds anything -- the policy ADR-039\nstates.\n\nThis measures two things and its header says so: the lexical arm's\nno-synonym behaviour, and the pool policy. It does not measure retrieval\nquality and it does not measure the embedding model; a proposal to do that\nwas rejected as unsound and the header records why. The pairs are a frozen\ncopy with a drift guard against the live table rather than an import of it,\nbecause that table is scoring data pinned to the match-formula version and\nan eval-motivated edit must never silently change ATS scoring.\n\nOne honest limit: dense_candidate_pool is private to its module, so the\nsecond row mirrors its lexical-hits branch rather than calling it, with the\nmirror cited to the source line and the in-crate unit test that pins the\nbranch named. Mutation-checked by making one alias resolve in the sanitiser:\nthe count dropped to 21 and the assertion failed.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: keep the adr filename on one line in the cheat-sheet\n\nA template literal broke the ADR-039 filename across a line inside a\n`.path` span. `white-space: nowrap` stops wrapping but still collapses the\nembedded newline to a space, so the page rendered `adr-039-hybrid-\npostings-…` with a spurious space inside the identifier -- not the\ncopy-pasteable path it is meant to be. Review caught it from the CSS spec\nrather than a render; the fix is the filename on one line.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: point the grounding link at the right file, and scope the critical claim honestly\n\nTwo external-review findings on the README and one on the landing.\n\nThe README linked `evidence::ground` to documents/evidence/mod.rs, which is\nthe scoring kernel's evidence ranking and defines no such function. The\ngrounding step lives at pipeline/resume/stages/evidence.rs; the link now\npoints there. Also fixes a typo the typos check did not catch.\n\nThe cheat-sheet said every Critical the content validator raises is a\ncomparison against the résumé text. That is the shape of the factual\nCriticals, but not of language_mismatch, header_in_body or\ntemplate_placeholder, which are structural rules. The property the code\nactually guarantees -- and the one worth claiming -- is that every Critical\nis deterministic: a comparison or a structural rule the code runs with no\nmodel in the loop. It now says that.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: strip the last drift-prone literals from the generation-domain page\n\nThird pass on a page that is meant to be a thin pointer. External review\nfound what two internal passes had reported clean: the company-research\nfence cap as a number, module-doc references by line number, an eight-entry\nnumbered stage list with per-stage prose, a per-module validator enumeration,\nand the eval harness's assertions restated in full. Each is now the symbol\nthat owns it -- buildCompanyResearchBlock, the module doc by name,\nQUALITY_STAGES with the stages as a plain ordered list, CONTENT_ISSUE_CODES,\nand tests/eval.rs for its own assertions.\n\nAlso repoints the grounding section at pipeline/resume/stages/evidence.rs,\nwhere ground() actually lives -- documents/evidence/ is the scoring kernel's\nevidence ranking, a different thing -- and shortens the Eval heading so the\nin-page fragment link resolves.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* test: assert the pool policy against the real function, not a mirror\n\nExternal review graded the mirrored candidate-pool logic in the lexical-gap\ntable as Major, and the in-repo test audit reached the same conclusion with\nthe fix: the in-crate test module already has access to the private\ndense_candidate_pool, so the pool half of the measurement belongs there.\n\nThe mixed-corpus assertion -- an alias-only posting is absent from the pool\nonce a distractor contains the canonical term -- now lives in\ncommands/hybrid_search/test.rs and calls the real function with real rows.\nMutation-checked by making the pool return the whole eligible set: the new\ntest failed on the first pair, and so did the existing ordering test, which\nis how a load-bearing mutation should look. The mirror is deleted.\n\nThe integration test keeps only the lexical half and its header now says\nso: it measures which pairs BM25 misses on this corpus, still 22 of 24 with\nthe same two dot-split hits, and points at the in-crate test for the pool.\nPer-row results are collected before any assertion, so the diagnostic table\nprints even when a row errors -- the audit's one advisory.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-02T02:27:24+02:00",
          "tree_id": "1aee121471b198b0a096268dcfbf17f50bfdc360",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c01e7eaf7a81cbc801498ee40ce1480f6e56924d"
        },
        "date": 1788309544552,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2207096,
            "range": "± 19950",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2659041,
            "range": "± 27429",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 256917,
            "range": "± 6651",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "44fd17dde4ad2dbe8e5719db52b84fda02f51475",
          "message": "feat: mcp stdio server mode for the agent cli (#1092)\n\n* chore: route the agent-cli module directory to its own reviewer\n\nThe review routes sent only extension_bridge/agent_cli.rs to\nagent-cli-reviewer and the whole extension_bridge tree to\nextension-reviewer, so a new child module under agent_cli/ would be reviewed\nby the wrong critic and the agent-system drift check could not see it. The\ndirectory glob now sits beside the file glob in both the route list and the\nagent-cli scope. This also closes the gap that left policy.rs unrouted.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: stop pointing callers at a command table adr-038 does not contain\n\nTwo refusal strings sent a caller to ADR-038 \"for the full command table\".\nThat ADR contains no command table -- zero hits for any command name -- and\nthe MCP surface is about to put these strings in front of a model that will\nfollow them. They now point at `agent schema`, the MCP `commands` tool, and\npolicy.rs. String-only edits; no test compares the detail text.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: record adr-040 and document the mcp server mode for users and reviewers\n\nADR-040 records the MCP stdio server as a mode of the existing agent CLI,\nwith each decision's rejected alternative: the legacy 2025-11-25 wire on\npurpose, because Claude Code 2.1.258 and Codex 0.144.6 both open stdio\nservers with the initialize handshake that the current 2026-07-28 revision\nremoved, and legacy clients have no fall-forward; hand-rolled JSON-RPC over\nserde_json rather than an SDK crate; a mode of the binary rather than a\nTauri command, so the policy table and its exactness test are untouched;\ntools split along the Effect boundary with MCP annotations rather than one\ngeneric call tool; the confirm ceremony passed through verbatim, never\nresolved server-side; results rather than protocol errors for app refusals;\nand one writeln! site as code shape. It extends ADR-038's egress amendment\nto this surface -- tool results leave the machine via the client -- and\nrecords why the pairing-token row is reclassified NotExposed.\n\nThe knowledge base gains an MCP-mode section on the agent-cli page and the\nindex row the drift check requires. The README gains client config for\nClaude Code and Codex; a first draft had the Codex config at the wrong path\nin the wrong format with a nonsense env line, corrected to\n~/.codex/config.toml with an [mcp_servers.<name>] table. Three pointers\nthat sent callers to ADR-038 \"for the full command table\" -- which it does\nnot contain -- now point at the commands tool, agent schema, and policy.rs.\n\nThe agent-cli-standards skill gains an MCP-mode block amending its\none-document-per-invocation rule for this mode only.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* feat: add an mcp stdio server mode to the agent cli\n\n`ajh-tauri agent mcp [--allow-irreversible]` is a new mode of the agent\nCLI (agent_cli/mcp.rs), never a second binary or Tauri command. It speaks\nthe legacy 2025-11-25 JSON-RPC stdio lifecycle Claude Code and Codex\nactually open, answering server/discover with plain -32601 so a\nMCP_PROTOCOL_NEGOTIATION=auto client falls back to initialize (verified\nlive against both the raw wire and the installed Claude Code binary).\n\nEight tools along the Effect boundary: five curated (best-matches, job,\nprofile, automations) plus a local commands tool that enumerates the\npolicy table by effect with no bridge call, and three generic dispatch\ntiers (call-read/call-reversible/call-irreversible) instead of one\nmonolithic call tool, so a client can auto-approve reads while still\nprompting on deletes. call-irreversible is omitted from tools/list unless\nlaunched with --allow-irreversible and carries\n_meta.anthropic/requiresUserInteraction, since Codex never reads that\nhint and cannot be the only gate. call-* tools refuse locally (never\nforward) a namespace/command the bundled policy table doesn't know, and\nrefuse a known row dispatched through the wrong tool, naming the right\none. tools/call arguments become this CLI's own argv and run through the\nexisting parse_verb, so this layer owns no second validator. confirm\nreaches call-irreversible verbatim and is absent from the other two\ntools' schemas by construction.\n\nTwo security fixes bundled in because the MCP surface is what exposes\nthem: extension_bridge_status is reclassified NotExposed (it returned the\nplaintext pairing token verbatim, which a generic-tier or MCP caller must\nnever receive), and best-matches now fences title/company/location the\nsame way job.description already does, since MCP is the first surface\nthat reads those fields with no surrounding prompt.\n\nstdout carries only compact JSON-RPC lines through one writeln! site; a\nsource guard fails if a later edit adds println!/print!/to_string_pretty.\nMutation-checked live: removing either local refusal (unknown-command,\nwrong-tool) turns a dedicated test red immediately.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* chore: split the duplicated glob key into two review-route objects\n\na95f79d5 added the agent_cli directory glob by inserting a second \"glob\"\nline into the same JSON object as the agent_cli.rs glob. JSON parsers apply\nlast-key-wins, so the file glob was silently dropped and the most-touched\nfile in the MCP change had no reviewer route -- while the drift check passed,\nbecause the surviving glob was valid. Caught by the author building the\nmodule, not by any check. The two globs are now two objects.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: describe the mcp write tiers by effect class and correct adr-040\n\nThe security review found the privilege boundary's own description wrong: the README\nand the knowledge page called --allow-irreversible the \"mutations\" switch while the\nreversible write tier shipped ungated, and both README examples named commands the flag\ncannot enable (applications_track is Reversible; boards_login_with_browser is NotExposed).\nThe tiers are now described by Effect class, with the new read-only default and the\n--allow-reversible flag the rework adds.\n\nADR-040 carried three claims the code in the same branch contradicts: a single held\nWebSocket connection with exit on pairing_rejected (it is a fresh handshake per call and\nthe refusal is a result), best-matches fields \"unfenced\" (fenced in the same commit), and\na \"Verified behaviors\" section asserting Codex is written in Go (it embeds rmcp, Rust) and\npresenting unobserved client versions as observed. Section 6 now records why both write\ntiers get the same gate; the verified section separates scripted runs from source reading.\n\nThe knowledge page also loses the rule-17 literals (tool count, throttle numbers) for\npointers at tools() and BridgeState, and scopes its \"safe for transcripts\" claim to the\nerror envelope, since a successful reply's data is the command's real output.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: gate the mcp reversible tier and close three refusal leaks\n\nA scripted stdio run of the real binary (unit tests never drove tools/list\nor tools/call end to end) surfaced what the review rounds predicted from\nsource alone: launched with no flags, tools/list still listed call-reversible\nunder only destructiveHint:false, a hint Codex ignores; commands with\neffect=bogus answered {\"commands\":[]} isError:false exit 0 instead of a\nusage error; and a NotExposed row (extension_bridge_status, the plaintext\npairing token) was let through call-read locally and relied on the running\napp's own gate to refuse it, which a cross-version peer might not do.\n\n- Gate call-reversible behind --allow-reversible, mirroring the existing\n  --allow-irreversible gate; --allow-irreversible now implies it (three\n  strict-superset launch tiers, each pinned by a hand-written name list).\n- commands validates its effect filter against the same array it builds\n  the schema enum from, and marks a gated row \"unavailable\" instead of\n  naming a tool it cannot dispatch to.\n- One tool_for(Effect) replaces two independently-hand-written mappings\n  (commands_value and local_call_refusal), and local_call_refusal now\n  refuses a NotExposed row on every call-* tool by itself.\n- Added a 256 KiB MCP_RESULT_MAX_BYTES cap (refuses as result_too_large,\n  never truncates JSON) and dropped structuredContent everywhere (a live\n  `commands` frame measured 50,695 bytes with it, 26,821 without).\n- `agent mcp --help` now prints usage and exits 0 instead of exiting 2;\n  INSTRUCTIONS names app_not_located separately from app_not_running and\n  gains one sentence per enabled flag, built once at startup.\n\nEvery fix carries a test that fails when reverted; each was mutation-checked\nby hand (see the handoff) and restored before the next.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* refactor: share the not_exposed sentinel with the mcp local refusal\n\nThe rework's local NotExposed refusal in mcp.rs carried a second hand-typed copy of\nagent_call's private sentinel because that file was out of the author's scope. The\nclosed sentinel vocabulary is the contract every caller matches on, so a duplicate literal\nis a drift point: widen the constant to pub(super) exactly as ERR_UNKNOWN_COMMAND and\nERR_CONFIRMATION_REQUIRED already are, and import it.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: repair a duplicated clause in adr-040 and record the local-refusal trade\n\nThe earlier docs pass replaced \"an mcp verb\" inside a sentence that already explained the\nintercept, leaving two copies of the same clause. Section 10 now also records the trade the\nsecurity re-review named: refusing NotExposed rows before the bridge removes the app-side\nspan a probe used to leave, while adding no disclosure the commands tool did not already make.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: amend the mcp skill block and the knowledge page for the reworked gates\n\nThe binding standard still described a single --allow-irreversible gate and \"exactly one\nwriteln! site\" after the rework added the reversible gate, the read-only default and a\n--help write that precedes any frame; the invariant the source guard now enforces is one\nstdout() acquisition. The knowledge page claimed every error field is a closed-set sentinel,\nwhich one live call refutes (the curated tier's throttle refusal is prose), and called three\ncall-read targets \"tools\".\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: state the fence per surface in the mcp skill block and fix an adr line pointer\n\nThe skill block claimed third-party text is fenced \"unchanged from the CLI\", which the\npre-PR gate refuted for the job tool; a blanket claim in the file every future reviewer loads\nis what would have kept that hole hidden. The bullet now names the fence call on each\nsurface and makes a missing one a finding. ADR-040 cited a line range that pointed at the\nunconfirmed dispatch path instead of the confirmation logic; it names the functions now.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: gate the reversible tier by type, fence job like best-matches, cap every refusal\n\nThree more review rounds each found a shape a scripted run reproduced live: a 300 KB\nnamespace on an unknown-command refusal produced a 300 KB reply because the result\ncap only checked a dispatched command's own payload, never a local refusal that echoes\nthe caller's own oversized input; that refusal's detail told the model to retry through\n\"agent call ns:cmd\", a working bypass recipe addressed to the exact agent the cap exists\nto bound; {\"effect\":5} skipped the string-only validator and returned every row; and the\njob tool fenced only description while best-matches fenced title/company/location too,\nleaving one curated resource open on the same threat.\n\n- Move the MCP_RESULT_MAX_BYTES check into tool_result so it covers every payload the\n  tool wraps, not just a dispatched reply; reword the refusal for the human reading the\n  transcript, never naming a CLI invocation.\n- Validate `effect` by type, not just by value, so a non-string bogus filter is a usage\n  error instead of an unfiltered success.\n- Replace the raw allow_reversible/allow_irreversible bool pair with a Tier enum\n  (Read/Reversible/Irreversible) so `--allow-irreversible` alone implying the reversible\n  tier is enforced by the type, not a comment; deleting the implication now fails six\n  tests instead of zero.\n- Add fence_posting_display_fields and call it from resolve_job the same way\n  fence_best_match_fields already does per row, so `job` and `best-matches` fence\n  title/company/location identically.\n- Word the launch-tier instructions notice by tier, not by the literal flag typed\n  (--allow-irreversible alone was claiming the caller passed --allow-reversible too);\n  indent the mcp row in --help like every verb row; drop the trailing blank line\n  writeln! left after --help; and restore a dropped clause explaining why the\n  regenerate-token proof is vacuous even for `connected`, not just `port`/`token`.\n\nEvery fix here carries a test that fails when reverted; each was mutation-checked by\nhand (see the handoff) and restored before the next.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* refactor: forward instead of panic when the mcp tool lookup has no arm\n\nThe rework replaced an unreachable match arm with an expect. The invariant holds today\n(NotExposed returns earlier, so every remaining Effect maps to a tool), but this path runs\nunder panic = \"abort\" above crash reporting, where a panic is a silent server death. The\nlookup now forwards to the app on None, which refuses on its own.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: answer the coderabbit round on the mcp pages\n\nScope the method-not-found claim to request methods (a notification gets no frame), scope\nthe README requirement to bridge-backed calls (initialize, --help and the local commands\ntool run with the app stopped), rewrite the knowledge page's MCP section as pointers into\nthe owning symbols instead of copied versions, flags, sentinels and PII targets, and record\nsingle-flight dispatch in ADR-040 as a decision with its ceiling and its named follow-up.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* ui: restore contrast on the agent-system hero link\n\nThe how-it-works link added to the agent-system note in #1091 inherits the page's accent\nred on the note's tinted background and measures under the 4.5:1 minimum, so the landing\na11y gate fails on every run that reaches it (the reveal fade makes the measured blend vary,\nwhich is why #1091's own gate happened to pass). In-copy links inside the note now inherit\nthe note's text colour with an underline. Verified locally: build + check:a11y, 13 pages,\n0 violations. Unrelated to the MCP change, but it blocks this PR's required check.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* test: widen the mcp sentinel scanner and pin single-flight dispatch\n\nFrom the CodeRabbit round on the MCP mode. The sentinel drift scanner matched one exact\nspelling (\"error\": \" with a single space), so a compact literal or a rustfmt line split\nescaped it while the non-empty sanity check stayed green; it now walks the source and\ntolerates any whitespace around the key, the colon and the opening quote, mutation-checked\nwith a compact and an unregistered literal. A new test drives serve() with a tools/call, a\nping and a second tools/call and asserts replies come back in input order with the ping\nanswered only after the in-flight call, pinning the single-flight contract that ADR-040\nsection 12 now records; the module doc names the reader-task/serialized-writer follow-up.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-02T07:05:15+02:00",
          "tree_id": "c04ab1061b4b9e4368437c6cb258ae3534da9990",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/44fd17dde4ad2dbe8e5719db52b84fda02f51475"
        },
        "date": 1788326224117,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2284982,
            "range": "± 15482",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2747888,
            "range": "± 45601",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 310918,
            "range": "± 7932",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "84cbcc6d5edb79ae245ddc494e8fc7ad678d986f",
          "message": "chore: bump the desktop-rust group (#1097)\n\nBumps the desktop-rust group in /apps/desktop/src-tauri with 15 updates:\n\n| Package | From | To |\n| --- | --- | --- |\n| [tauri-plugin-opener](https://github.com/tauri-apps/plugins-workspace) | `2.5.4` | `2.5.5` |\n| [tauri-plugin-dialog](https://github.com/tauri-apps/plugins-workspace) | `2.7.2` | `2.7.3` |\n| [tauri-plugin-updater](https://github.com/tauri-apps/plugins-workspace) | `2.10.1` | `2.11.0` |\n| [tauri-plugin-clipboard-manager](https://github.com/tauri-apps/plugins-workspace) | `2.3.2` | `2.3.3` |\n| [tauri-plugin-shell](https://github.com/tauri-apps/plugins-workspace) | `2.3.5` | `2.3.6` |\n| [tauri-plugin-log](https://github.com/tauri-apps/plugins-workspace) | `2.9.0` | `2.9.1` |\n| [sentry](https://github.com/getsentry/sentry-rust) | `0.49.1` | `0.49.2` |\n| [tauri-plugin-single-instance](https://github.com/tauri-apps/plugins-workspace) | `2.4.3` | `2.4.4` |\n| [tauri-plugin-notification](https://github.com/tauri-apps/plugins-workspace) | `2.3.3` | `2.4.0` |\n| [tauri-plugin-deep-link](https://github.com/tauri-apps/plugins-workspace) | `2.4.9` | `2.4.10` |\n| [tauri-plugin-positioner](https://github.com/tauri-apps/plugins-workspace) | `2.3.3` | `2.3.4` |\n| [tauri-plugin-websocket](https://github.com/tauri-apps/plugins-workspace) | `2.4.2` | `2.4.3` |\n| [uuid](https://github.com/uuid-rs/uuid) | `1.25.0` | `1.26.0` |\n| [flate2](https://github.com/rust-lang/flate2-rs) | `1.1.9` | `1.1.10` |\n| [aes](https://github.com/RustCrypto/block-ciphers) | `0.9.2` | `0.9.3` |\n\n\nUpdates `tauri-plugin-opener` from 2.5.4 to 2.5.5\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/http-v2.5.4...http-v2.5.5)\n\nUpdates `tauri-plugin-dialog` from 2.7.2 to 2.7.3\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/dialog-v2.7.2...dialog-v2.7.3)\n\nUpdates `tauri-plugin-updater` from 2.10.1 to 2.11.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/updater-v2.10.1...updater-v2.11.0)\n\nUpdates `tauri-plugin-clipboard-manager` from 2.3.2 to 2.3.3\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/os-v2.3.2...nfc-v2.3.3)\n\nUpdates `tauri-plugin-shell` from 2.3.5 to 2.3.6\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/nfc-v2.3.5...nfc-v2.3.6)\n\nUpdates `tauri-plugin-log` from 2.9.0 to 2.9.1\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/log-v2.9.0...log-v2.9.1)\n\nUpdates `sentry` from 0.49.1 to 0.49.2\n- [Release notes](https://github.com/getsentry/sentry-rust/releases)\n- [Changelog](https://github.com/getsentry/sentry-rust/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/getsentry/sentry-rust/compare/0.49.1...0.49.2)\n\nUpdates `tauri-plugin-single-instance` from 2.4.3 to 2.4.4\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/fs-v2.4.3...fs-v2.4.4)\n\nUpdates `tauri-plugin-notification` from 2.3.3 to 2.4.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/nfc-v2.3.3...fs-v2.4.0)\n\nUpdates `tauri-plugin-deep-link` from 2.4.9 to 2.4.10\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/deep-link-v2.4.9...deep-link-v2.4.10)\n\nUpdates `tauri-plugin-positioner` from 2.3.3 to 2.3.4\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/nfc-v2.3.3...nfc-v2.3.4)\n\nUpdates `tauri-plugin-websocket` from 2.4.2 to 2.4.3\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/fs-v2.4.2...fs-v2.4.3)\n\nUpdates `uuid` from 1.25.0 to 1.26.0\n- [Release notes](https://github.com/uuid-rs/uuid/releases)\n- [Commits](https://github.com/uuid-rs/uuid/compare/1.25.0...v1.26.0)\n\nUpdates `flate2` from 1.1.9 to 1.1.10\n- [Release notes](https://github.com/rust-lang/flate2-rs/releases)\n- [Commits](https://github.com/rust-lang/flate2-rs/compare/1.1.9...1.1.10)\n\nUpdates `aes` from 0.9.2 to 0.9.3\n- [Commits](https://github.com/RustCrypto/block-ciphers/compare/aes-v0.9.2...aes-v0.9.3)\n\n---\nupdated-dependencies:\n- dependency-name: tauri-plugin-opener\n  dependency-version: 2.5.5\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-dialog\n  dependency-version: 2.7.3\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-updater\n  dependency-version: 2.11.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-clipboard-manager\n  dependency-version: 2.3.3\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-shell\n  dependency-version: 2.3.6\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-log\n  dependency-version: 2.9.1\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: sentry\n  dependency-version: 0.49.2\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-single-instance\n  dependency-version: 2.4.4\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-notification\n  dependency-version: 2.4.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-deep-link\n  dependency-version: 2.4.10\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-positioner\n  dependency-version: 2.3.4\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-websocket\n  dependency-version: 2.4.3\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: uuid\n  dependency-version: 1.26.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: flate2\n  dependency-version: 1.1.10\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: aes\n  dependency-version: 0.9.3\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-09-02T20:21:05+02:00",
          "tree_id": "0316ff2ac2fc008a3a0f79911ba17a6b5159b9db",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/84cbcc6d5edb79ae245ddc494e8fc7ad678d986f"
        },
        "date": 1788374712831,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2299437,
            "range": "± 25327",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2737169,
            "range": "± 35080",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 310167,
            "range": "± 9877",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0c14e8f15a38f4ced0a5f3bf5a9f5c96cfcb3702",
          "message": "feat: help chat, a11y lint over the design system, orphan copy cleanup and the mcp residuals (#1101)\n\n* feat: hybrid help search command with a text-hash vector cache\n\nhelp_search retrieves the best help entries for a question with the shipped\nretrieval stack (ephemeral FTS5 lexical arm, dense cosine arm, reciprocal-rank\nfusion). The renderer sends the active locale's entries per request, so no\ntranslation bundle is baked into the binary. The dense arm runs only under the\nsemantic-scoring opt-in and degrades honestly; one embedding-config snapshot\nfeeds both the daily-ceiling charge and the embed call; entry vectors are\ncached by text hash in a new help_vectors table that factory reset and an\nembedding-space change both clear. The policy row is Irreversible for the same\nreason the postings search's is. A lexical eval over the real corpus asserts\nten contested phrasings reach their entry in the top three.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* feat: answer mcp pings and local tools while a bridge call is in flight\n\nThe serve loop becomes one reader, one dispatcher and one writer over a\nchannel: bridge-backed tool calls still run single-flight in input order\nthrough one worker, while ping, initialize, tools/list and every local tool are\nanswered immediately from the writer thread. EOF drains queued calls; a write\nerror still ends the server with exit 0. The irreversible confirm path gains a\npure core so match, mismatch and unavailable proof are tested, including that\na mismatch never dispatches.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* feat: help chat on the support page over the shipped corpus\n\nAsk a question and get a grounded, streamed answer: the page's own entries are\nsent for retrieval, the top hits are rendered as trusted app copy, and the\nquestion, chat history and a capped glance of the user's data are fenced as\nuntrusted text with every fence tag neutralised. The keyword-only mode is\nsurfaced when semantic scoring is off. Also removes the thirteen orphaned\ndashboard translation leaves and adds the clear-location label the hoisted\nLocationInput control needs.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: keyboard-reachable ui primitives and the a11y lint over packages/ui\n\nThe advisory jsx-a11y job linted only the renderer. It now covers packages/ui\nwith two scoped, reasoned overrides, and the real defects it surfaced are\nfixed: a clear control nested inside another button, a clickable tag with no\nrole or key handling, an hrefless anchor, composite containers in the tab\norder, and mouse-only image panning.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: describe the dashboard the landing architecture map actually ships\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: record adr-043 and resync adr-040/041/042 for the help chat and mcp changes\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* refactor: move the agent-cli policy types out of the table module\n\npolicy.rs sat one line under the R8 cap after the help_search row. The\ntypes and the classification rules move to policy/types.rs, re-exported so\nevery import path is unchanged; the POLICY table itself is byte-identical.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: pin sentry log and metric pipelines off without the deprecated options\n\nMain's sentry patch bump deprecated enable_logs and enable_metrics; the\nfirst only governs integrations this build never installs and the second is\na no-op. Both calls go, and the privacy property moves to three static legs:\nthe feature gate in the manifest, an empty integrations list, and an egress\nscan for any capture call site.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: rank help questions by term overlap and bound the dense arm and its cache\n\nThe help arm inherited the postings search's implicit AND, so a full-sentence\nquestion matched nothing on a default install; it now ORs its terms so BM25\nranks by overlap, and the eval carries chat-shaped sentences that fail under\nAND. The dense arm stops on the dense-arm deadline and reports unavailable\nwhen it could not pair every entry; cache-miss embeds are capped per request\nand help_vectors joins the prune sweep, so an agent-tier caller cannot grow\nthe table without bound. Both spend-charging policy rows now carry the\nweak-proof flag the table's doctrine asks for.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: bound the mcp call queue and the drain after eof\n\nThe reader/dispatcher split had replaced the pipe's backpressure with\nunbounded channels. The bridge-call queue is now bounded and a call that\narrives while it is full is refused with a server_busy sentinel; after EOF the\nloop drains under one absolute deadline and exits even if calls remain.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: help chat states, prompt markers and primitive targets from review\n\nThe chat card's source chips ellipsize on a block label, the streamed answer\nno longer scroll-hijacks the page, one always-mounted live region announces\nthinking, errors and answers without re-reading the previous reply, muted text\nmeets the small-text contrast floor, a failed question keeps its text and\noffers retry, and the component now has a render test per state. The prompt\ndefuses forged section markers and sizes cloud models correctly; the recent\napplication list leaves the machine only for application questions and the\nglance is fetched inside send. Tag, MarkdownMessage, LocationInput and\nImagePreview take the reviewers' target-size, inline-flow and focus fixes.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: resync adr-040 and adr-043 for the review-round changes\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: bound the mcp reader queue and index the help vector sweep\n\nThe reader-to-writer event channel was still unbounded, so a client that\nstopped draining stdout could grow the process; it is now a bounded channel\nthe reader blocks on. The help_vectors migration gains the created_at index\nthe row-cap sweep is written for, the cache docs carry the concurrency\ncaveat, and the retrieval eval records its top-two rate against the smallest\nproduction limit.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: help chat single-flight, lightbox focus restore and primitive nits\n\nStop no longer re-enables Ask while retrieval is still running, a new run\naborts the controller it replaces, and both are pinned by tests along with\nthe shared-query-key claim of the glance fetch. The image lightbox restores\nfocus to its opener on close, the markdown anchor carries noreferrer, and the\ncomposite containers that gained a programmatic tab stop do not paint a ring\non a mousedown in their gaps.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: name the three help-vector bounds and the concurrency caveat in adr-043\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: guard markdown link schemes in the shape static analysis recognises\n\nCodeQL flagged the anchor href as DOM text reinterpreted as HTML: the user's\nown editor text flows into a markdown link and a regex on a trimmed copy is\nnot a barrier its taint model tracks. The same allowlist is now a startsWith\ncheck on the exact string that reaches the attribute.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: help dense-arm completeness after ranking and bounded mcp shutdown replies\n\nThe completeness check now compares the ranked count, so a degenerate vector\ncan no longer yield a partial ranking reported as hybrid; the per-request\nembed cap is asserted against the real corpus; the semantic-scoring gate has\na source guard; single-character CJK tokens survive the OR query. In the MCP\nloop a call the worker would start after EOF without budget to finish is\nanswered with a shutting_down sentinel instead of being abandoned, every\nearly exit sets the abandoned flag, and the drain-deadline test gains an\norder of magnitude of scheduling slack.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: markdown links safe on any click, help glance tolerant of one failed read\n\nThe scheme allowlist is a startsWith check on the string that reaches href,\nthe form static analysis recognises, and a middle-click is cancelled too.\nChat history keeps its newest turns when trimmed and indented forged markers\nare defused. Text typed while an answer streams survives; the glance reads\nsettle independently and an unavailable source is omitted rather than read\nas zero; the lightbox traps focus with the house hook and dialog shells keep\ntheir focus indicator; the API page's TSDoc is pointers only.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: record the rewritten mcp loop's real-binary smoke in adr-040\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-03T01:27:51+02:00",
          "tree_id": "2ade35a4059dc96a7cd540b300d115b37ba9218c",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/0c14e8f15a38f4ced0a5f3bf5a9f5c96cfcb3702"
        },
        "date": 1788393130588,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2377360,
            "range": "± 19475",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2888603,
            "range": "± 25843",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 259993,
            "range": "± 4617",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f5fa164d0bd9d5386a1b6c40ae1819dbfaa1be0e",
          "message": "feat: per-language help retrieval with cancellation, agent cli settings card, cross-os mcp smoke (#1102)\n\n* style: say why an outline-none class cannot opt out of the global focus ring\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* feat: help retrieval drops function words per language and can be cancelled\n\nThe lexical arm ORs a question's tokens, so every \"how\", \"the\", \"ich\" was\nan extra branch matching most of the corpus. The request now carries the\nlocale its entries are written in, and a help-only function-word table per\nlanguage is passed into the lexical index as a parameter, so the retrieval\nmodule stays language-blind. The job-ad stopword lists were measured and\nrejected: their whole gain came from one content word, and the German one\nmoved nothing. The English eval measures 18 of 18 in the narrow top-2 (the\nfloor rises from 17); a new German eval of 14 hand-written cases carries the\nover-filtering guard, where one wrong entry turns a hit into a miss because\nFTS5 does no decompounding.\n\nAn optional caller-minted queryId with its own prefix registers the request\nin the app-wide cancel registry before any async work, mirroring the\npostings search; each embed is raced against the token and the arm reports\nunavailable on cancel. No new command and no new policy row.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* feat: read command for the agent cli binary path, shared with the pointer file\n\nOne resolver in the platform layer answers where the binary lives as a user\nshould type it (the AppImage file rather than its transient mount), used by\nboth the launch-time pointer file and a new read-tier command the Settings\ncard queries. Full five-step IPC, policy row and the hand-written row count.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* feat: agent cli and mcp card in developer settings\n\nThe binary's path with a copy button, an access-tier picker, and copyable\nClaude Code and Codex registration snippets with the real path quoted for\nthe shell and for TOML. Nothing claims PATH membership. The help answer that\nsaid there was no Settings row for this is corrected in both locales.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* feat: help chat cancels its retrieval on stop, navigation and supersede\n\nMints a prefixed queryId per question and hands it to the shared cancel\ncommand, so the dense arm gives up sooner instead of embedding to completion\nafter the user has moved on. The busy-state release stays as it was.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* test: mcp stdio smoke against the real binary, on macos and windows too\n\nAn integration test spawns the shipped executable in MCP mode and replays\nthe two sessions ADR-040 recorded by hand, asserting one frame per request\nin order, an empty stderr and exit 0. It gates on Linux through the existing\ntests job; an advisory matrix job runs it on the two platforms nothing else\nexecutes on.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: help lexical arm reruns unfiltered when the drop list matched nothing\n\nThe token-list fallback could not see a question whose surviving tokens\nmatch no entry (\"Where is my stuff?\" keeps only \"stuff\"), so the arm\nreported a run with zero hits on the only arm a default install has. The\nfiltered query now falls back to the unfiltered expression once when it\nreturned no rows; the English eval gains that case and its floor rises to\n19. An omitted locale drops nothing instead of defaulting to English; the\nin-flight cancel is tested with an embedder that never returns; the\nGerman autopilot case is rephrased so it is a ranking, not a lookup; two\ndocs that described guards which do not exist are corrected.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: mcp smoke pins the child home so the sentinel is the one ci sees\n\nWith no pointer file the refusal is app_not_located, not app_not_running,\nso the test passed only on a machine where the app had launched. The\nchild now gets an empty temp home and the assertion is unconditional; the\nwatchdog prints frame ids, never bodies; the matrix step is marked\nadvisory like its siblings.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: publish the appimage path only when this process was launched from it\n\nThe AppImage runtime exports its variables to every descendant, so a .deb\ninstall started from another AppImage's terminal would have published a\nstranger's file as the command to type. The resolver now requires the\nimage to exist and the running binary to live under its mount, and the\npointer writer resolves the path itself so the choice is tested through\nthe function that publishes it.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: help chat keeps the superseding question cancellable, root lang follows the ui locale\n\nA superseded run resolving late cleared the id its replacement had just\nminted, leaving the second question uncancellable; only the run that owns\nthe id may clear it. The document root's lang attribute now follows the\nactive locale, so screen readers voice German text with a German voice.\nThe ask button keeps its label on one line in German.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: agent cli card wraps its commands, escapes the path and says what the tier does\n\nThe command blocks wrap instead of scrolling, so the privilege flag is\nnever clipped and keyboard users can read the whole line; the path is\nescaped for the three characters live inside shell double quotes; the\nirreversible tier no longer promises a prompt the app does not show; the\ntier picker says it only rewrites the commands and hides when there is no\npath; the pending, error and label states use the right primitives; both\ndeveloper cards share one header. The help answer about the CLI names the\ncard and says the command runs in a terminal.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: record cancellation, per-language drop lists and the appimage resolver in the adrs\n\nADR-043 gains an amendment reversing its own no-cancellation tradeoff and\ndescribing the drop lists with the measured reason the job-ad lists were\nrejected; its unverified list is closed by the live run. ADR-040 records the\nsmoke test, the corrected never-launched sentinel and the Settings card.\nADR-037 names the pointer's new resolver. The agent CLI page gains an\nAppImage row and a pointer to the command and the card; the README says\nwhere the registration commands now live.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* test: placeholder user in the snippet test, frame body kept out of the smoke panic\n\nA test comment carried a real username; the refusal assertion interpolated\nthe frame body its own comment said it must not; the resolver doc claimed a\nNone condition the code does not have.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: double a backslash before a shell metacharacter in the registration snippet\n\nBash consumes a backslash pair inside double quotes and leaves the\nmetacharacter after it active, so a path ending in a backslash followed by a\nbacktick opened a command substitution after the previous escaping. A\nbackslash that precedes a dollar sign, a backtick, a double quote or another\nbackslash is doubled first; every other backslash stays literal so an\nordinary Windows path survives for PowerShell and cmd. Tests run the escaped\nstring through a real bash where one is available.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: help chat keeps one locale for a whole run\n\nThe language was read twice per question, once for the retrieval request\nand once for the generation call after the await, so a switch during\nretrieval mixed locales. It is read once at the top now. The settings-title\nparity test asserts a non-empty title before the containment check.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: hide the binary path command from the agent tier\n\nA CLI or MCP client already holds the path it launched the binary from, so\nreaching this command through the agent tier added nothing it could use and\nput a home-directory path into a persisted transcript. The row is not\nexposed; the renderer still reaches the command over IPC, and a test drives\nthe real row through the real gate.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: point the agent cli page at the sources that own each platform fact\n\nThe per-platform table of install paths and PATH behaviour is replaced by\npointers to the installer hook, the cask, the bundle config and the AppImage\npredicate, and two release-version literals leave the README. ADR-040 no\nlonger describes the path command as agent-reachable.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-03T06:08:31+02:00",
          "tree_id": "14fb78edcbd6803bad702098430938ef0284d853",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f5fa164d0bd9d5386a1b6c40ae1819dbfaa1be0e"
        },
        "date": 1788409955100,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2266323,
            "range": "± 13027",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2661586,
            "range": "± 76440",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 298117,
            "range": "± 1689",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1034ca546f21556c211fad54e7370d18a5cca7b0",
          "message": "fix: extension answer tools capture script and reasoning-aware draft budget (#1103)\n\n* fix: keep the injected scripts' completion values out of the minifier\n\nFour injected scripts answer the background by completion value, and the\nper-entry build let the minifier fold capture.js's trailing object literal\ninto two bare calls, so \"Save my answers from this page\" failed on every\npage in every shipped build and the rewrite picker never filled. The pass\nis unminified now, its options are exported, and a test builds the real\nartifacts into a temp dir and asserts what each one evaluates to.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: answer drafts use the lowest reasoning effort, the full budget and one retry on an empty cut\n\nThe bridge's draft and rewrite compose ran with a 1000-token output budget\nand no reasoning effort, so a reasoning model's thinking exhausted the\nbudget before any answer text: \"make it 200 characters\" failed four of\nfour. The lowest effort the provider lists is sent when the model supports\none, the budget is the same cap the wire already enforces on visible text,\nand an empty length cut is retried once with the budget doubled, charging\nthe daily ceiling on both attempts through the same call.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* build: key the extension build cache on its config and pin the injected-entry partition\n\nturbo's build and test inputs now include *.config.*, so a vite.config.mts change can no longer serve a cached\ndist; the legacy esbuild key is dropped from the extension config, and the build-output test asserts which\nentries install globals versus which answer by completion value.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: retry the answer draft inside one registered round and send assist.done once\n\nThe retry re-registered the same reqId under a fresh generation, which leaked the first entry, let a cancel\nbetween the attempts start a billable job, and emitted assist.done after the failed first attempt. register is\nnow generation-scoped and rebinds the one entry, the done frame is sent once from the single exit, the retry is\nskipped when the client gave up, DRAFT_CAP bounds the visible text across both attempts, the budget is 2000\ntokens with the retry at DRAFT_CAP, and only a minimal or low effort tier is ever sent.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: return only the retry's own text from the answer draft and pin the anthropic thinking gate\n\nThe retry handed back the shared cross-attempt buffer, so text the failed first attempt had already forwarded\nwas prepended to the retry's answer; the driver now records the buffer length before each attempt and returns\nthe tail the successful attempt wrote, while the buffer still bounds both attempts under DRAFT_CAP. The\nAnthropic classic-thinking gate is a named predicate asserted against both compose budgets, with its boundary\npinned, and the retry-tail fixture is multi-byte so the byte-offset against char-clamp seam is exercised.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: record the compose budget rule and the unminified injected pass\n\nThe extension README now says which build pass is unminified and why, and the extension-domain page gains a\ncompose-budget note pointing at the owning constants and predicate.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* fix: give the answer-draft retry its own cap window and keep the provider modules under the line cap\n\nA retried draft was clamped against what the failed first attempt had already forwarded, so a long inline\nreasoning pass could leave the retry a stub returned as success; the cap accountant now rebases at each\nattempt while the buffer stays append-only, so the wire is bounded by two attempts of DRAFT_CAP with exactly\none retry. The budget assertion moves next to the Anthropic predicate it is sized against, the test-only\nre-export goes, the extension bridge stream's tests move to their own file, and three doc comments now say\nwhat the code does.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: the compose budget note says each attempt has its own cap window\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* docs: point the compose-budget section at its owning symbols instead of restating them\n\nThe section restated answer.assist's budget sizing, effort selection,\nretry predicate and cap-window behavior as prose bullets alongside the\nsource pointers, which is exactly the drift risk rule 17 exists to\navoid — one of the restated claims (a cancelled request between\nattempts is never paid for) is already only approximately true, since\nthe charge check and the charge itself are not atomic. Replaced with a\npointer to compose_with_length_retry and ComposeStream, whose own doc\ncomments carry the contract.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01PEB84mXWcrYHtHX1xyMkhW\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-04T15:26:18+02:00",
          "tree_id": "f44afc9a9e4a09f9bf5f83f5aaeaa9c4da4d42ec",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/1034ca546f21556c211fad54e7370d18a5cca7b0"
        },
        "date": 1788529869181,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2280344,
            "range": "± 81111",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2722401,
            "range": "± 58305",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 314275,
            "range": "± 17508",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "57be8988d8496a4ee79456a40b284c1565b4acbf",
          "message": "fix: give extension answer tools a side panel; fix the inline rewrite popover (#1108)\n\n* docs: adr-044 for the extension answer tools surface and three glossary terms\n\nRecords the side panel plus popup as two views of one per-tab, per-origin state, the user-gestured page access with\nno standing grant, the deferred in-page card, the draft-time character limit, and the manifest and parity obligations\nthe delivering PR owes; adds Application question, Saved answer and Draft to the glossary.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_012EQX4DcucKiSdacnJHHxgz\n\n* feat: carry a field's own length limit on the draft request and share the refusal sentinels\n\nThe answer.assist draft request gains an optional character limit, shaped only on the wire so a newer client\ncannot have a whole draft refused over a number, and clamped desktop-side to the cap every draft is held to\nanyway. The parser treats it as untrusted input and reads anything malformed as no limit. The two refusal\nsentinels a gated-off row has to tell apart move into the shared protocol constants, pinned to their Rust\noriginals by the hand-enumerated parity test, which also now pins the advertised ceiling to the cap it names.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix: make the inline rewrite reachable, honest about no-ops and limits, and keep the span's language\n\nFive defects measured by driving the running app. The popover rendered inside two clipping ancestors, so the\nresult and the accept control sat below the cut and could not be clicked; it now uses the portal branch that\nalready existed. A result identical to the selection was presented as success, so an unchanged reply is now\nnamed as one with accept disabled and regenerate live. The hardcoded sixty second abort was below the backend\ndeadline and did fire, so the bound is derived the way every other stream derives it, with a still-working\nline for a long pass. A numeric limit in the instruction is now counted in code and re-asked once with the\nmeasured overshoot, because the model missed it about half the time on characters and every time on words.\nAnd the language comes from the selection rather than the document, which was turning Dutch spans English.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* feat: give the extension answer tools a side panel that shares one state with the popup\n\nThe tools live in a persistent side panel and stay in the popup, as two views over one state per tab and\norigin held in session storage, so a draft survives the popup closing. Page access stays user-gestured: the\ntoolbar click opens the popup, whose control opens the panel, and a context-menu entry on selected text opens\nit directly; after a cross-origin navigation the rows stay and every write control says how to re-grant. The\npage's questions are the primary object, one composer each, with rewrite chips over the existing verb and a\nregenerate that redrafts, versions kept for the session only, and a live count against the field's own limit.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n\n* fix: give the inline rewrite retry its own timeout window\n\nThe one-time overshoot re-ask shared the first attempt's setTimeout\ninstead of getting a fresh one. A first attempt that burned most of\nits effort-scaled budget (measured up to ~152s of a ~330s ceiling)\nleft the retry almost no runway; if the shared timer fired mid-retry\nthe perfectly usable, honestly-labelled over-limit first draft was\ndiscarded for a generic \"Rewrite failed\" error. Re-arm the timeout\nwith resolveRewriteTimeoutMs(model) again right before the retry call.\n\nAlso, folded in from the same review round: a dedicated unit test for\nresolveRewriteTimeoutMs (previously only exercised through a mocked\nconstant); normalizeRewriteText no longer strips trailing \\p{S}\nsymbols alongside \\p{P} punctuation, since that could misclassify a\nresult that changed a trailing symbol (e.g. \"+\") as unchanged; a\ncomment explaining the popover's absolute-positioned wrapper div is\nload-bearing for the one render before the toolbar ref attaches, not\ndead code; and a stale doc comment on RewriteParams.language corrected\nto note its sole caller always supplies a resolved value today.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01PEB84mXWcrYHtHX1xyMkhW\n\n* fix: stop the answer tools from wiping focus and typed text on every stream tick\n\nrender() rebuilt the whole section on every storage.session push, and\nthe background mirrors EVERY streamed token into that storage — so any\nin-flight draft or rewrite, anywhere in the tab, tore down and rebuilt\nthe section several times a second. The always-visible \"Add question\"\ninput had no backing store at all and lost its typed value outright;\nevery input lost keyboard focus and caret position on each rebuild\neven where its value did survive (the per-row instruction box, backed\nby a Map). Fixed generically: a data-focus-key attribute plus\ncapture/restore of the focused element and its selection range around\nevery replaceChildren(), and a closure-backed value for the add-question\ninput mirroring the existing per-row Map.\n\nFour more defects from the same review round, same root cause class\n(a control looking idle or honest when it is not):\n\n- Rescan stayed enabled after a cross-origin navigation and silently\n  dropped a resolved {ok:false} response — no onResult callback was\n  wired up. Now disabled when pageChanged, and surfaces the error.\n- A chip-based rewrite that echoed the input verbatim was appended as a\n  fresh version with Accept enabled, the same \"looks like it worked\"\n  defect this PR's desktop half already fixes for RewritePopover. Now\n  compared (whitespace/trailing-punctuation normalized, via a local\n  isUnchangedRewrite mirroring the desktop's shape) against the row's\n  previous version; an unchanged result sets a neutral row notice\n  instead of growing the version list.\n- Chips/Accept/Restore/Regenerate were gated only by a view-local `busy`\n  flag, never by the shared stream — a surface that did not itself\n  start the in-flight request (or the same view reopened mid-stream)\n  showed every control enabled on a row that was actively streaming.\n  Now also gated on state.stream targeting that row.\n- The popup's disclosure summary and the panel body's own first line\n  repeated the identical \"N questions / M to go\" sentence; dropped the\n  popup's copy. ITERATION_HINT's \"and this posting\" claim is now\n  conditional on the row's own sourced.brief flag, so it can no longer\n  contradict the grounded-on line directly under it.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01PEB84mXWcrYHtHX1xyMkhW\n\n* docs: extension-domain + readme sync for the answer tools panel, adr-044 verification update\n\nextension-domain.md gets a new section covering the shared per-tab\nstate, the gesture model, the maxChars field and its deferred\nenforcement, the refusal sentinels, and the new permissions;\nREADME's feature list gains the Answer tools / side panel paragraph.\nADR-044's \"what is verified\" section is rewritten against what this\nbranch actually shipped and tested rather than what it planned to,\nincluding that parse_max_chars has no caller yet (deferred behind PR\n#1103) and that neither the panel nor the desktop popover fix could\nbe visually verified without running the app.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01PEB84mXWcrYHtHX1xyMkhW\n\n* fix: recover the folded capture-rows completion value and a lost-update lockout\n\nThis branch was cut from main before PR #1103 (open, separate) merged a\nfix for exactly this defect class in capture.ts: Vite's default\nminifier folds a single-expression arrow IIFE's trailing object\nliteral away, so capture-rows.js's built completion value was the bare\n`filled` array instead of `{questions, filled}`. Every scan threw\n\"Could not read the questions on this page\" — silently, since the\npopup's own scan call is .catch(() => undefined) — so the whole\nAnswer-tools feature was dead in both shipped builds. Ported #1103's\nfix (INJECTED_ENTRIES, injectedEntryConfig, minify:false, and its\nbuild-output.test.ts, which did not otherwise exist on this branch)\nin the same shape so a later main merge is a clean no-op, extended to\ncover capture-rows; also gave capture-rows.ts's IIFE a statement body\nmatching probe-fields.ts, as defense in depth independent of build\nconfig.\n\nSeparately: updateAnswerState's read-modify-write had no serialization\nagainst itself, so the terminal step's fire-and-forget stream mirror\nand its awaited row-settle call could interleave and drop one write.\nCombined with the streaming-disable gate from the prior fix round,\nthis could permanently lock every chip/Accept/Restore/Regenerate on\nthe row a draft just finished, in both surfaces, with no escape on a\nsingle-question form. Fixed with a per-tab FIFO queue so a read on one\ntab's state never starts until the prior call's write on that same tab\nhas landed.\n\nAlso folded in: data-focus-key on the buttons the focus-preservation\nfix missed (Rescan, the row-head toggle, Add-question submit, each\nchip), a stale doc comment claiming the extension's unchanged-rewrite\nnormalizer still diverges from the desktop's (a sibling commit on this\nbranch already closed that gap), and a dead popup element/lookup left\nover from dropping the duplicate summary line.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01PEB84mXWcrYHtHX1xyMkhW\n\n* fix: stop an attacker-authored job posting's language field from reaching a prompt unvalidated\n\nA review on this PR flagged unvalidated language interpolation as a\nMajor finding; tracing the actual data flow found it is a genuine\nprompt-injection vector rather than a cosmetic one. meta.targetLanguage\nis set from an LLM's own metadata-extraction read of a scraped job ad\n(metadata.ts's toLanguage, which only trims the value, unlike the\ncompanyName/jobTitle fields a few lines above it), so its value is\ninfluenced by the ad's own text. deriveRewriteLocale falls back to that\nstring whenever a rewrite's selected span is too short for language\ndetection to be sure, and the result reaches languageDisplayName, whose\nold `?? code` fallback echoed an unrecognized string straight into the\nrewrite system prompt, twice, unescaped.\n\nClosed at two points: deriveRewriteLocale now normalizes the fallback\nthrough toLanguageCode (meta.targetLanguage legitimately arrives as a\nname rather than a code on one extraction path) and gates the result\nwith a new isPlausibleLanguageCode, degrading anything that isn't\ngenuinely code-shaped to English; languageDisplayName gates its own\nfallback the same way, since RewriteParams.language is a public field\nother callers could someday populate directly. Neither gate is a bare\n$-anchored regex, since `/^[a-z]{2}$/.test('de\\n')` is true in\nJavaScript.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01PEB84mXWcrYHtHX1xyMkhW\n\n* fix: close a tab-id race in the assist run and thread the context-menu tab through\n\nThe comment above runAnswerAssist's supersede check claimed \"no await\nseparates this check from the reset below\", but assistTabId was still\nresolved by an await sitting between them. A run that suspends there\nwhile a newer overlapping run resets the shared buffer for its own row\ncan resume, overwrite that buffer back to its own (stale) row and\ntab id, and go on to fire its own billable request, exactly what the\nguard exists to prevent. Resolved the tab id before the check instead.\n\nThe context-menu entry passes the CORRECT tab to the panel it opens\nbut let the free-text row it adds resolve its own tab independently\nvia an active-tab query, so a focus change between the gesture and\nthat query could land the row in a different tab's state than the\npanel that just opened. Threaded the gesture's own tab id through.\n\nSix more: subscribeAnswerState's async initial read could clobber a\nstorage.onChanged event that arrived first, rendering stale rows; the\nside panel's tab-activation listener wasn't scoped to its own browser\nwindow, so a tab switch anywhere else could hijack its subscription;\nthe explicit neutral \"As is\" chip set a notice but never rendered it;\nthe popup misreported \"no side panel\" on Chrome when only the active\ntab id hadn't resolved yet; a duplicate local 4000-char cap literal\ncould drift from the shared constant it now reuses instead; and two\nweak tests (a race gate keyed to a call count instead of the read's\nown content, an assertion that never actually checked an in-range\nversion selection).\n\nDocs: corrected two claims against what the code actually does, the\nstate key is the tab id alone with origin embedded in the record, not\na (tab, origin) compound key, and pageChanged fires on every\nnavigation regardless of same-origin vs cross-origin, so write\ncontrols behave identically either way even though the underlying\nactiveTab grant does not.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01PEB84mXWcrYHtHX1xyMkhW\n\n* refactor: split parse_max_chars's tests out of answer_assist.rs for r8\n\nMerging main's real retry/cap-window machinery into this branch's own\nmaxChars addition pushed the module to 1482 lines, over the 1400-line\nhard cap. Moved the already-isolated parse_max_chars_tests module into\nits own file, the same way answer_assist_tests.rs already is. Pure\nrelocation, no assertion changed.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01PEB84mXWcrYHtHX1xyMkhW\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-04T16:51:43+02:00",
          "tree_id": "003df636ba1b3274bb4d39a73517a039fe651ea7",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/57be8988d8496a4ee79456a40b284c1565b4acbf"
        },
        "date": 1788534191281,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2278498,
            "range": "± 14501",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2692542,
            "range": "± 13688",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 302749,
            "range": "± 13992",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "84ef96d377a5d83ca632261fc8f5285744869b13",
          "message": "fix: posting score, trust and agent-cli addressing integrity (#1104-#1107) (#1109)\n\n* fix: recognize every adzuna market host and flag postings with no description\n\nCompanyDomainMismatch fired for nearly every Adzuna-sourced posting\nregardless of employer legitimacy, because the allowlist recognized\nonly api.adzuna.com while Adzuna's own redirect_url also points at\nper-market websites (www.adzuna.de and, less obviously, compound\nccTLDs like www.adzuna.co.uk covering 7 of 19 supported markets,\nincluding the UK). is_adzuna_market_host now recognizes both shapes,\nanchored so no lookalike host (adzuna.evil.com, a compound-shaped\nspoof) can borrow the exemption.\n\nSeparately, a posting with an empty description (LinkedIn's free-tier\nboard returns one for every listing) carried full High trust with no\nsignal anything was missing, because assess_trust never saw the\ndescription at all. It now takes a third parameter and a new\nDescriptionUnavailable flag caps trust below High when the text is\nblank, through the same score-penalty mechanism every other flag uses.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: stop autopilot from hiding a title-only match behind a full-confidence score\n\nbest-matches could show a URL and a score from two different rows in\nthe same cluster, because the row's display fields came from the\ncluster's canonical (richest-content) member while its score came from\na different, best-scored member. A new scoreUrl field names which\nposting a displayed score actually belongs to whenever the two\ndiverge, so a client is never left following a URL that doesn't own\nthe number next to it.\n\nSeparately, re-admitting a job a minMatchScore change had just\nfiltered out could overwrite a persisted, semantically-reranked\nCombined score with a cheaper same-run Keyword-only one, potentially\ndropping a genuinely good match out of best-matches entirely (the\nqualifying cut is higher for Keyword than Combined). Re-admission now\nrefuses to downgrade a persisted Combined score to a Keyword one,\nwithout blocking the reverse: a real same-run upgrade still applies.\n\nscore_provisional (the muted-score marker) also never fired for a\nposting with no usable description or requirements text unless it\nhappened to come from an aggregator snippet, letting a bare-title\nkeyword match round to 100% with full apparent confidence; it now\nalso fires whenever the scoring text was title-only, honestly\nincluding the LinkedIn and five other boards this affects.\n\nCorrecting a posting's description now refreshes its trust flags to\nmatch the new text, but deliberately leaves score_provisional alone —\nthe numeric score isn't recomputed by a manual correction, so nothing\nabout its confidence has actually changed.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: wire the new description-unavailable trust flag into the renderer\n\nTrustFlag gained a DescriptionUnavailable variant on the Rust side;\nthe wire type's flags union and the trust badge's exhaustive flag-key\nmap both need every variant mapped or a build breaks, by design, so a\nnew flag can never reach the renderer with no label the way one did\nbefore this repeated safeguard existed.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: let scrape_update_description correct a posting by url, not an internal id\n\nThe command required an id that addressed only the session-lifetime\nPostingsCache, but no Agent/MCP read command ever exposes that id —\nevery reader addresses a posting by url. Worse, even a correct\nid-based lookup could never reach a posting surfaced via job or\nbest-matches, since those read Autopilot.found_jobs, a completely\nseparate persisted store the old id-keyed cache write never touched.\n\nThe request now carries url, matched the same way job_resource\nalready resolves one — canonicalized through board-specific rewrites\n(so a LinkedIn search-view link and its canonical view-url land on the\nidentical identity) before normalizing, with an explicit http(s)\nscheme required so a stale caller's old id-shaped value is rejected\nup front rather than silently missing both stores. The write now\nreaches both PostingsCache and every matching found-job row across\nevery autopilot, and refuses an empty description outright — this\ncommand corrects a description, it must never be able to blank one.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* docs: regenerate api.md for this branch's contract changes\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* refactor: derive the update-description request type from its contract\n\nTwo call sites duplicated the same inline { url, description } object\ntype the ScrapeContract interface already declares. Deriving both from\nParameters<ScrapeContract['updateDescription']>[0] means a future\nfield change on the contract can no longer drift silently out of sync\nwith either caller.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: close two trust-module gaps a review found (blank-description check, adzuna spoofing)\n\nDescriptionUnavailable used a raw .trim().is_empty() check, weaker than\ndescription_is_blank (commands::autopilot's no_jd_text predicate) — a\ndescription that's just a bare URL or markdown noise read as \"usable\"\nscoring text and got full trust. Switch assess_trust to call the shared\npredicate so both checks agree on the same input.\n\nis_adzuna_market_host's single-label branch accepted adzuna.<any TLD>,\nletting a squatted lookalike (adzuna.xyz, adzuna.top, ...) suppress\nCompanyDomainMismatch for a spoofed posting. Restrict it to a curated\nADZUNA_SINGLE_LABEL_TLDS allowlist (11 ccTLD markets + the us -> com\nexception), mirroring how the compound-ccTLD branch already works.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Sonnet 5 <noreply@anthropic.com>",
          "timestamp": "2026-09-04T22:33:24+02:00",
          "tree_id": "d68d15b048bcfb34f39fc913030fa0cf71e5b35b",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/84ef96d377a5d83ca632261fc8f5285744869b13"
        },
        "date": 1788555487204,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2273493,
            "range": "± 63577",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2791233,
            "range": "± 91616",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 335420,
            "range": "± 14866",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8a289ef2b70c7e44b234c74ffd86fd5060891416",
          "message": "fix: make tray pause/resume-all autopilots actually work (#1112)\n\n* fix: make the tray pause-all item an actual pause/resume toggle\n\nClicking \"Pause all autopilots\" paused every autopilot but never flipped\nback to \"Resume all autopilots\" because the menu item handle was a local\nvariable, never stored past build() for a later click to relabel. pause_all\nwas also one-directional, with no resume counterpart.\n\nStore the item's handle in TrayState (mirroring new_jobs_item) and derive\nthe toggle direction from the store's real state: pause every Active\nautopilot if any is running, otherwise resume every Paused one back to\nActive. Archived autopilots are never touched by either direction. The\nlabel is recomputed from the store post-mutation (and at cold-start from\nthe persisted state) rather than tracked separately.\n\nThe mutation + relabel logic is pulled into an AppHandle-free\ntoggle_pause_all(&AutopilotStore) so it's covered by real tests despite\nthis crate having no tauri::test mock-app harness.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: replace the stale-label tray pause/resume toggle with two static actions\n\nReview of f1de242d found a real regression it introduced: the single stateful\ntoggle's label only got refreshed inside its own click handler, but autopilot\nstatus can also change via autopilot_pause/autopilot_resume (Settings) and\nprivacy_reset_app's autopilot wipe, neither of which touches the tray. A user\npausing both autopilots from Settings left the tray still reading \"Pause all\nautopilots\"; clicking it then derived the opposite action from live store\nstate and resumed everything, against explicit intent.\n\nReplace it with two separate, always-visible, statically-labeled menu items:\n\"Pause all autopilots\" (pauses every currently-Active autopilot, no-op\notherwise) and \"Resume all autopilots\" (resumes every currently-Paused one,\nno-op otherwise). Neither label ever changes, so neither can say the\nopposite of what it does — no relabeling, no cold-start label computation,\nno stored menu-item handle to keep in sync. resume_all is documented as\nliteral: it resumes every Paused autopilot, including ones paused\nindividually from Settings, not just ones a prior pause_all paused.\n\nRewrote the tray test suite for the new shape (9 tests, up from 6): explicit\nno-op coverage for pause_all/resume_all across all-Paused/all-Archived/empty\nstarting states, a single-Active positive case, and a pause-then-resume\nround trip proving a pre-existing individual pause also resumes.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Sonnet 5 <noreply@anthropic.com>",
          "timestamp": "2026-09-05T06:26:44+02:00",
          "tree_id": "9c4ab473f25975edebeb05098c908b97e2c8bc7e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/8a289ef2b70c7e44b234c74ffd86fd5060891416"
        },
        "date": 1788583886602,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2332834,
            "range": "± 65339",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2788942,
            "range": "± 50737",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 326718,
            "range": "± 11163",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e5f41630b1898a1a2f244607f29fabb4d245b8e1",
          "message": "feat: LinkedIn description enrichment + paginated found-jobs MCP resource (#1117)\n\n* feat: auto-enrich linkedin found-job descriptions after autopilot runs\n\nLinkedIn search results never carry a description (api_client always\nwrites an empty placeholder), so LinkedIn found-jobs scored title-only\nforever. A scheduled run now spawns a best-effort background pass\nright after record_run that resolves each still-blank LinkedIn posting\nvia the existing single-URL resolver, paced through LinkedIn's shared\nrate limiter, and writes any real description back through the same\nmechanism a manual correction uses. Capped at 15 fetches/run; a\nper-URL failure is logged and never fails the run itself.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: stop linkedin resurface from wiping an enriched description\n\nmerge_found_jobs treated any Some(description) as \"known\" and\noverwrote the existing row, but LinkedIn search results always carry\nSome(\"\") (never None) as their blank sentinel. A posting enriched by\nlinkedin_enrich on run N reverted to blank on run N+1 as soon as it\nresurfaced in a fresh LinkedIn scrape, which re-queued it for\nenrichment every run thereafter — defeating the feature and driving\nsustained repeat traffic against LinkedIn's guest endpoint. The merge\nguard now uses description_is_blank so only a genuinely non-blank\nincoming value can overwrite a row's description.\n\nAlso: wrap the scrape_update_description write-back in spawn_blocking\n(it does synchronous file I/O and was running inline on the async\nenrichment task), extract the per-URL resolve/description decision\ninto a pure, unit-tested classify_resolution helper, document why\nOk(None) and Err both retry identically rather than adding new\nper-posting state, and correct a stale ordering comment.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* feat: add paginated found-jobs agent/mcp resource (#1115)\n\nautopilot_get/autopilot_list return every autopilot's full, unbounded\nfound_jobs array, which exceeds the MCP bridge's 256 KiB result cap for\nevery real autopilot measured in the issue; autopilot_best_matches only\nranks the top 100 across ALL autopilots combined and takes no per-autopilot\nselector. Add a curated found-jobs resource/verb/MCP tool that pages\nthrough one autopilot's complete found_jobs list via a plain stored-order\noffset cursor, projecting a smaller FoundJobSlice (title/company/url/\nlocation/board/description-preview/salary/score/postedAt/foundAt/trust)\nthat excludes assistantNotes and cluster-annotation internals.\n\nPage size (default 50, max 100) is derived from a measured worst-realistic\nserialized row size, targeting half the MCP cap for real margin -\ndocumented and pinned by a byte-measuring test plus its own mutation check.\nSplit into extension_bridge/agent_read/found_jobs.rs to stay under the R8\nLOC cap. Purely additive: no changes to autopilot_get/list/best_matches,\npolicy.rs, or any renderer/IPC path.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: move linkedin enrichment orchestration to the shell layer\n\nautopilot_helpers/linkedin_enrich.rs (application layer) imported\ntauri::AppHandle and called crate::commands::scrape::scrape_update_description\ndirectly, violating the arch tests' R2 (no Tauri types below the shell\nlayer) and R7 (no upward layer imports) in tests/architecture.rs. Split\nthe module along the existing pure/orchestration boundary this crate\nalready uses elsewhere (rerank.rs, best_matches.rs):\nselect_linkedin_enrichment_targets, EnrichOutcome, and\nclassify_resolution stay pure at the application layer; the\nAppHandle-touching fetch/rate-limit/write-back loop moves to a new\nshell-layer submodule, commands/autopilot/linkedin_enrich.rs,\nmirroring how commands/autopilot.rs already composes rerank/best_matches.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: byte-budget found-jobs pages and fix numeric-cursor drop (#1115)\n\nTwo HIGH findings from review round 2:\n\n- A JSON NUMBER cursor (`{\"cursor\": 100}`) silently collapsed to offset 0\n  via `.and_then(Value::as_str)`, which returns None for a non-string value\n  too, not just an absent one -- restarting the traversal while nextCursor\n  kept reporting forward progress. Match on the Value variant explicitly in\n  both parse_found_jobs_cursor and mcp.rs's tool_argv (which had the\n  identical bug three lines below its own limit arm's correct pattern).\n\n- The row-count limit could not bound a page's byte size: title/company/\n  location are each fenced at JOB_CAP (8,000 chars), not a short realistic\n  string, so 100 ordinary (non-adversarial) rows could reach ~2.5 MB --\n  9.5x the MCP transport cap. Replace the row-count-only guard with\n  trim_to_byte_budget: build candidate rows, then drop from the end until\n  the serialized jobs array fits a 150,000-byte budget (half the 262,144-\n  byte MCP cap, leaving real margin), advancing nextCursor by rows actually\n  kept rather than rows requested. limit/max are now a compute ceiling, not\n  the safety mechanism; lowered to default 25 / max 50 and description\n  preview cap raised 500 -> 2,000 chars now that the byte budget (not a\n  starved per-field cap) is what keeps a page under the transport limit.\n\nAlso: corrected resolve_found_jobs's concurrency doc (a concurrent\nrecord_run prepends and never removes, so mid-traversal drift means\nduplicated rows and unreachable newest jobs, not a vague \"might race\");\nextended the untrusted-fields-notice test into a sweep over every curated\ntool instead of a one-off pair; added tool_argv coverage for the found-jobs\narm; tightened the preview-cap guard test to an exact byte count; fixed the\nthrottle doc's stale resource count.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* test: add found-jobs to the mcp smoke test's read-tier tool list\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: coalesce concurrent linkedin description enrichment by url\n\nCodeRabbit (PR #1117): enrich_linkedin_descriptions recomputed its\ntarget list from current store state on every autopilot_run and\nspawned a fresh detached task with no coordination, so a manual re-run\nracing a still-in-flight scheduled pass (or vice versa) could select\nand fetch the SAME still-blank LinkedIn URL twice concurrently —\nwasted rate-limiter budget and duplicate scrape_update_description\nwrites against a board this codebase already treats as\nsoft-block-sensitive.\n\nAdd a process-global in-flight set keyed by URL alone (a URL is the\nsame fetch target regardless of which autopilot record surfaced it,\nand select_linkedin_enrichment_targets already dedupes by URL): claim\nunclaimed URLs atomically before fetching, release each one right\nafter its own fetch+write-back finishes (success or failure, always),\nmirroring commands::autopilot::RUNS_IN_FLIGHT's shape. New unit tests\nassert a second overlapping pass excludes an in-flight URL and can\nreclaim it once released.\n\nAlso trim the automation-domain.md LinkedIn-enrichment bullet down to\na capability description + symbol pointers (rule 17) instead of\nrestating the selection rule, cap value, and write-back mechanism\ninline — another CodeRabbit finding on the same PR.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n* fix: fence autopilot name and budget the full found-jobs envelope\n\nCodeRabbit review on PR #1117 found two real gaps in the byte-budget guard:\n\n- autopilot.name was echoed into the response completely unbounded (no\n  fencing, no length cap) and NOT counted toward PAGE_BYTE_BUDGET at all --\n  a user can name an autopilot anything, so the byte-safety guarantee was\n  \"the jobs array fits,\" not \"the whole response fits.\"\n- trim_to_byte_budget only ever measured the jobs array in isolation, so a\n  large autopilotName riding alongside an already near-max jobs array could\n  push the total response over the MCP transport cap even though the array\n  itself measured fine.\n\nFix: fence autopilotName with the same fenced()/\"job_posting\" primitive and\ncap (200 chars) every other display field on this resource already uses.\ntrim_to_byte_budget now takes a base_cost parameter -- the real serialized\nsize of every OTHER envelope field (nextCursor/total/autopilotId/the now-\nfenced autopilotName), measured against an upper-bound nextCursor\nplaceholder so the estimate can only over-count, never under-count -- and\nsubtracts it from the row budget before accumulating row sizes. The O(n)\naccumulation shape is unchanged; only what counts against the budget changed.\n\nNew tests: an oversized autopilotName gets fenced/capped exactly like\ntitle/company/location; a larger base_cost measurably leaves less room for\nrows (proves the parameter isn't dead); and a maxed-out autopilotName\ncombined with worst-permitted job content still keeps the FULL envelope\n(not just the jobs array) under the MCP cap.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Sonnet 5 <noreply@anthropic.com>",
          "timestamp": "2026-09-07T08:44:14+02:00",
          "tree_id": "0123f35125bbbd569a653cffb6ef7b1564fc1c2e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/e5f41630b1898a1a2f244607f29fabb4d245b8e1"
        },
        "date": 1788765092974,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2433120,
            "range": "± 50747",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2872241,
            "range": "± 74892",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 333694,
            "range": "± 19315",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9440d7edd2ceb00c9813906a40afd52e6d791f98",
          "message": "feat: help chat bridges paraphrases, names the page on a miss and answers six more questions (#1145)\n\n* test: pin the create-an-autopilot phrasings in the help retrieval eval\n\nThe 2026-09-06 report said the help chat did not know how to create an\nautopilot; retrieval measured against the real corpus ranks the answering\nentry first for that phrasing family. Two English cases pin it and the\ntop-2 floor rises from 19 to 21 with the measurement.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(prompts): help chat bridges paraphrases, lists the app pages and names one on a miss\n\nThe grounding rules told the model three times to say it does not know\nand never that the user's verb may differ from an entry's title, so a\nretrieved answer could still be refused. Rule 2 now says a different verb\nfor the same task is a match; a miss names the sidebar page the feature\nlives on (from a trusted page list the renderer passes) and the help\nsearch box; the data glance can carry the user's autopilots when asked.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(help): six new help entries for the questions the corpus could not answer\n\nLinkedIn résumé import, the display language, updates and the running\nversion, applicant details for cover letters, GitHub projects in the\nbuilder, and the contact header. Every quoted control was checked against\nthe label the UI renders in both locales.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* test: pin one phrasing per new help entry and raise the eval floors\n\nEXPECTED_ENTRY_COUNT follows the corpus to 57; six user phrasings pin the\nnew entries' reachability and the top-2 floor rises with the measurement\nto 27.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(help): send the sidebar page list and the user's autopilots to the help chat\n\nThe sidebar's nav data moves to a pure module the help chat reads, so\nthe page list the model may name is the sidebar itself, translated by the\nsame t() as the corpus. The data glance carries name, status, run status\nand found count of the user's autopilots only when a retrieved entry is\nabout autopilots, mirroring the recent-application rule.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(prompts): list an unlabelled sidebar group without a dangling colon\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* test(prompts): build the unlabelled-group case from the module fixtures\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(help): the applicant-details entry names the browser extension as the form consumer\n\nAutopilot never submits anything; the salary and start-date fields reach\napplication forms through the extension's answer suggestions.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(prompts): name the page list in each only-clause, drop start from the synonyms, defuse labels\n\nReview fixes: rule 1 and the TASK block now list the APP PAGES block as\na permitted source and carry the page-naming exception; a page is offered\nas where to look if the app has the feature, never as proof it exists;\nstart no longer bridges a run request onto a creation entry; page labels\npass through the same defusing as fenced text and the ### anchor also\ncatches a marker behind a list bullet; a prompt without a page block no\nlonger mentions one.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(help): gate the glance name lists on the top hit, cap autopilots, correct three help claims\n\nThe recent-application and autopilot name lists now ship only when the\ntop-ranked entry is in that section: the answer is written from the top\nentry, and a secondary hit widened disclosure without improving it. The\nhook hands the prompt at most ten autopilots. The contact-header entry\nnow says changes save when leaving a field, the applicant-details entry\nthat the extension uses the salary expectation only, and the updates\nentry that the Store build still shows the changelog. The sidebar's\npinned footer is a named declaration instead of a null-label sentinel.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: amend adr-043 trust boundary, fix the new-page steps, add the help glossary terms\n\nThe trust class is stated as a condition (shipped translation strings\nare trusted, anything a user or a board can write is fenced) instead of\nan enumeration that went stale when the page list arrived; the new-page\nsteps point at the nav data module and the translations package; the\nglossary gains Help entry, Data glance and Page list.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(prompts): the question note names the page list exactly when it was rendered\n\nThe note under the user's question is the third place the prompt states\nits permitted sources; it still listed only the entries and the glance,\none line before the task. It now derives the same clause the TASK block\ndoes, so a prompt without a page block never mentions one. Two stale\nenumerations of what the glance carries name the autopilot names too.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* test: pin one german phrasing per new help entry and raise the german floor\n\nThe German bundle grew by the same six entries; DE_CASES now measures\neach and the top-2 floor rises with the measurement from 14 to 20.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: glossary entries point at their owning symbols; keep the new-page steps in one list\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(prompts): the system prompt names the page list only when one was rendered\n\nBoth help-chat builders now derive every APP PAGES clause from one\npredicate over the page sections, computed once by the caller, so a\nprompt without a page block never authorises naming a page from it.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-07T22:36:22+02:00",
          "tree_id": "aec3418a43ea0f4b4e1d02f89b55e30f7af6b807",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9440d7edd2ceb00c9813906a40afd52e6d791f98"
        },
        "date": 1788814666997,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1428426,
            "range": "± 44584",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1679404,
            "range": "± 101185",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 168808,
            "range": "± 5765",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "552362a96c903b2d185450a04ecb982dc6b26d68",
          "message": "fix: mcp contract, validation, fencing and the numbers a client reads (#1150)\n\n* docs: quote the mcp install path in the readme and point at the generator that already does\n\nThe Claude Code examples double-quote the exe path and the Codex example\nuses a TOML literal string, each with a one-clause warning that names the\nSettings card's snippet generator; the agent-cli page gains pointers for\nthe derived error-sentinel list and the generic input's parameter keys.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(agent-cli): fence an application answer's question by shape, leave a job's own result bare\n\nAn ApplicationAnswer's question is third-party form text and is now\nfenced when it sits beside an answer field, without re-fencing the app's\nown InterviewQuestion that shares the key name (#1139). A JobRecord's\nresult is exempt from name-keyed fencing so a generation read through\njobs_get returns the model's answer bare; job_complete documents that a\nkind carrying third-party text there must fence it itself (#1142).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(agent-cli): decode job url escapes, bind found-jobs cursors, expose the traversable total\n\nA percent-encoded variant of a cached URL now resolves: unreserved RFC\n3986 escapes are decoded on both sides of the identity compare, upstream\nof the scheme guard, without touching the persisted key (#1128). A\nfound-jobs cursor carries the autopilot it was issued for and a cursor\nfrom another autopilot or a bare offset is refused (#1130). automations\ngains foundJobsTotal beside the last-run totalFound, both documented\n(#1132). The page-size and best-matches limit constants become reachable\nfrom the MCP layer so tool descriptions can derive them (#1129).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: the found-jobs cursor is an opaque token bound to its autopilot\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: keep the found-jobs page-size and field-list pointers\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(mcp): enforce the advertised schemas, derive the numbers a client reads, name every sentinel\n\nThe argument key set is checked against each tool's own schema and an\nunknown key is a usage refusal instead of a silent drop (#1134); limit\nand confirm treat an explicit null as absent and a non-string confirm\nreaches the ceremony as its JSON text (#1137, #1140); both limit\ndescriptions derive from the constants the server enforces (#1129); the\ninstructions append the error sentinels the prose does not already name\nand say how a generic input is keyed (#1143, #1144); the cursor is\ndescribed as an opaque per-autopilot token and automations names both\ntotals. Every tool carries a human title, the tool order is pinned as\nprefix-stable across tiers, and call-irreversible says a long run can\noutlast the call. The instructions moved to mcp/instructions.rs under\nthe file-size cap.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: the instructions builder moved to mcp/instructions.rs\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(mcp): tolerate reserved keys, hand-list explained sentinels, guard job_complete producers\n\nReview fixes: the argument key-set gate ignores protocol-reserved\nunderscore keys; the sentinel table is filtered by a hand-written list\nof names the prose already explains, so connection_lost gets its row and\na future sentinel cannot vanish behind a substring; the found-jobs\ncursor refusal distinguishes a malformed cursor from one issued for\nanother autopilot; the job_complete producers are pinned by a literal\nlist in the architecture tests and the exemption warning states the\nprecise truth; the job lookup documents its deliberate leniency and the\ntool tells a caller to reuse the url the app returned.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* test(mcp): assert the derived limit phrases whole, tidy the key-set gate and two doc pointers\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(agent-cli): fence board-derived summary strings by shape, detect spaced job_complete calls\n\nA scrape summary's error, skipped and truncated fields and a board\nhealth entry's lastError are board-derived text; they are now fenced by\nshape on the agent read path, including inside a job record's result,\nwhile a generation's own text stays bare and the UI keeps reading them\nunfenced. The producer guard in the architecture tests tolerates\nwhitespace between job_complete and its parenthesis.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-08T00:56:32+02:00",
          "tree_id": "ea5ef0df08171cf3d72b2915cd7b773ba4c1631b",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/552362a96c903b2d185450a04ecb982dc6b26d68"
        },
        "date": 1788823211859,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2320717,
            "range": "± 47831",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2793644,
            "range": "± 28541",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 314471,
            "range": "± 6846",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8abc6a1022d67af90e3b138f018b3511d0ff042c",
          "message": "fix(prefs): merge and write under one lock, and clear the country with its location (#1153)\n\nThe store gains update(), which reads, merges and writes inside one\ncritical section so two partial updates cannot erase each other's\nfield; job_preferences_set runs its pure parse inside it. A body that\nclears location alone now clears countryCode with it, matching the\ncontract, while a body naming countryCode keeps what it sent.\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-08T03:02:39+02:00",
          "tree_id": "1d0edf1fe185f512bda072529de9e5562f624bb0",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/8abc6a1022d67af90e3b138f018b3511d0ff042c"
        },
        "date": 1788830461874,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2036718,
            "range": "± 50965",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2348343,
            "range": "± 43662",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 280309,
            "range": "± 3019",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "607c995c2121f5bb4a80cf6311fb2db9d290fd5b",
          "message": "fix(commands): label the health model probe and make the spend summary complete (#1178)\n\n* fix(system): label system_health ai block and add active provider\n\nsystem_health's ai block was the local Ollama daemon probe with no\nlabel saying so, and the renderer had no way to see the app's actually\nconfigured generation provider from this command at all. Add\nai.scope: \"localOllama\" and an additive activeProvider block sourced\nfrom the same AiConfigStore::active_config() lookup ai_active_config\nalready uses, so the two can never disagree.\n\nRefs #1159\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(ai): scope ai_spend_summary to a window and list every provider\n\nperProvider only showed providers active in the current window, so a\nprovider with historical spend but nothing today silently vanished from\nthe list. Add an optional days argument (default 1 = today, capped at\nSPEND_WINDOW_MAX_DAYS) that scopes today/perProvider; perProvider now\nlists every provider ever recorded in the ledger, with a zero row plus a\nshort reason (local, not priced, or no spend in window) for one with no\nactivity in the window. The resolved window is echoed back as window;\nthinkingByModel stays all-history, labelled thinkingByModelWindow.\n\nRefs #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix: honest spend-window total key and wire the days param through the client\n\nai_spend_summary now emits windowtotals alongside today (unchanged for\nback-compat) so a days > 1 caller reads a total that isn't misnamed as\n\"today\"; the tauri-client namespace forwards spendSummary's days\nargument instead of silently dropping it. Also stops the Settings >\nAI spend panel from rendering a zero-activity provider row as\n\"local — free\" and made its empty state reachable again.\n\nRefs #1159\nRefs #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(ai): make spend-summary window fields non-optional again\n\nwindow, windowTotals and thinkingByModelWindow were declared optional\nin the AiSpendSummary contract purely so createMockClient's stub kept\ncompiling, even though the backend emits all three unconditionally on\nboth branches of ai_spend_summary. That let the stub drift into a\nshape the backend never sends. Fill the mock stub in and drop the\nthree `?`, so the contract stays the wire spec and the mock can't\nsilently fall out of parity again.\n\nRefs #1159\nRefs #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(ai): make today an honest calendar-day aggregate in ai_spend_summary\n\ntoday was built from the days-scoped window total, so a days > 1 caller\nread an up-to-90-day sum under a key literally named today, and the\npayload had no field that actually carried today's spend. today now\ncomes from SpendStore::today_totals(); windowTotals keeps the days\nwindow. Also extracts spend_summary_value/resolve_window_days and\nhealth_value (system_health) into pure functions and adds tests that\nwould have caught the mislabelling and the missing scope/activeProvider\ncoverage.\n\nRefs #1159, #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix: repair ai-spend contract drift and untested call site\n\nThe round-1 #1161 fix corrected `today` back to calendar-day in Rust but\nleft the wire contract (ai.ts) and generated docs/API.md still describing\n`today` as window-scoped, telling every consumer the exact lie the issue\nwas filed to remove. Rewrite the TSDoc on `spendSummary`/`AiSpendSummary`\nto match the Rust semantics and regenerate docs/API.md.\n\nThe round-1 regression test drove `spend_summary_value` with hand-picked\nliterals and never touched the `ai_spend_summary` call site that chooses\nwhich store query feeds `today` vs `windowTotals` — a revert of that call\nsite would leave the whole suite green. Extract `spend_summary_from_store`\nso the call site is unit-testable against a real on-disk SpendStore, add a\ntest that backdates a row outside \"today\" but inside a 7-day window, and\ncorrect the old test's comment to state what it actually covers.\n\nRefs #1159, #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(ai): split spend helpers under the r8 loc cap, tighten a one-sided guard\n\ncommands/ai/mod.rs had grown past R8's 1400-LOC hard cap after the #1161\nfixes, failing the architecture test CI gates on. Split the pure/AppHandle-\nfree spend helpers (spend_summary_from_store, spend_summary_value,\nspend_totals_json, resolve_window_days, per_provider_with_zero_rows) into\ncommands/ai/spend.rs, same shape as commands::match_resume's constraints\nsplit; the #[tauri::command] itself stays in mod.rs so it's still reachable\nat commands::ai::ai_spend_summary. Their tests move with them into spend.rs's\nown #[cfg(test)] mod, and the call-site regression test gains a same-window\nrow so it can no longer pass with an always-zero today total.\n\nRefs #1159\nRefs #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(ai): fix the spend-window doc and require runtimehealth's ai/provider fields\n\nAiSpendWindow's own doc still said the window is what today covers, one type\nup from the today field doc it contradicted 13 lines below in the same file\n(#1161 failure mode 2) - reworded and regenerated docs/API.md.\n\nRuntimeHealth.ai.scope and RuntimeHealth.activeProvider (#1159) were declared\noptional purely so a stub kept compiling, even though system_health emits\nboth unconditionally on every path (mod.rs's json! macro has no branch that\nomits either key). Drop the two `?`, mirroring how 4151cb84 fixed the sibling\nAiSpendSummary fields, and fill useCanUseAI.test.tsx's readyHealth fixture in\nso it can no longer drift into a shape the backend never sends.\n\nRefs #1159\nRefs #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent): cover spend_summary_value/from_store json shape gaps\n\nspend_summary_value only had a today/windowTotals assertion; nothing\npinned that perProvider, thinkingByModel, window, and the\nthinkingByModelWindow discriminator are actually forwarded into the\nJSON payload the renderer reads by key. spend_summary_from_store's\nthinkingByModel mapping (provider/model/calls/thinkingTokens/\noutputTokens) moved out of an untestable AppHandle command body but\nwas never JSON-shape tested either. Both were mutation-checked\n(field/key temporarily removed, test failed, restored).\n\nRefs #1159, #1161\n\n* test(ai): cover spend per-provider vocabulary, fallback, and row order\n\nAdds call-site coverage for perProvider's since_ms=0 vocabulary\n(#1159 T1), which the existing spend_summary_from_store tests could\nnot see since they only exercised the pure merge with hand-built\nProviderTotals literals. Extracts the store-unavailable branch into\nspend::zero_summary, sharing one window_json builder with the live\npath instead of hand-rebuilding it in mod.rs (#1159 T2), and unit\ntests the extracted helper directly.\n\nFixes perProvider row order: the merge now follows windowed cost\n(by_provider_since's own ordering, scoped to the requested window)\ninstead of all-time cost, so a provider that dominated last month no\nlonger outranks the window's actual top spender in settings (#1159\nT4). Types the renderer's mocked useSpendSummary against the real\nAiSpendSummary contract and builds every fixture from one complete\nbase object, so a future required field is caught by tsc instead of\nsilently passing an incomplete payload through the mock (#1159 T3).\n\nRefs #1159\nRefs #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(mock-client): echo the requested spend window and record the generated-api-doc exclusion\n\nThe mock `ai.spendSummary` discarded its `days` argument and always answered\n`window.days: 1`, so a multi-day caller could not be exercised against it. Also\nrecords in review-config that docs/API.md is generator output, not hand-copied drift.\n\nRefs #1159, #1161\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-09T00:53:51+02:00",
          "tree_id": "9ecf40a0f91cd8656c9b5482ec22b8e47dc94d1e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/607c995c2121f5bb4a80cf6311fb2db9d290fd5b"
        },
        "date": 1788909460040,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2189838,
            "range": "± 12893",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2569672,
            "range": "± 14156",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 306113,
            "range": "± 20031",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7777b9d487cd9e8d90a9d4bf9d6f8d5bd91488c2",
          "message": "fix(agent): honest policy rows, job identity by board id, and a usable found-jobs read (#1182)\n\n* fix: correct four agent-cli policy rows and drop a dead applied count\n\nupdater_check becomes Read (issue #1165): it only probes the release\nfeed and its in-memory UpdaterState is a re-entrancy cache never\nobservable through another command, so a read-only agent session can\nfinally answer \"is an update available\" without guessing from the\nchangelog.\n\nnotifications_mark_read/notifications_mark_all_read become\nIrreversible with notifications_list as their ListMatch/Count proof\n(issue #1164): no \"mark unread\" path exists anywhere on this surface,\nso flipping the read bit is permanent, same as notifications_remove\nwhose proof shape they now reuse.\n\ncommands::help::help_search becomes NotExposed (issue #1169): its\ncorpus is the caller's own request field, not anything Rust can read\n(the real corpus lives only in the renderer's translation bundles per\nADR-043), so no dispatch here ever has a real corpus to search.\n\nThe MCP INSTRUCTIONS text and the profile tool description now point\na caller at documents:documents_list / documents:documents_get_text /\nmatch_resume:resume_extract_text before it judges job fit, so it reads\nthe user's own resume instead of guessing from contact fields alone\n(issue #1170). A new test asserts every ns:cmd pair named in prose is\na real POLICY row.\n\ntotalApplied is dropped from the agent automations projection (issue\n#1171): the field is dead on the source Autopilot struct too, so\nexposing it promised an applied count that never existed.\n\nRefs #1165, #1164, #1169, #1170, #1171\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): stop instructions pointing residue reads at resume_extract_text\n\n`resume_extract_text` extracts text from caller-supplied bytes, it reads\nnothing from the user's own document store — pointing a calling model at it\nas a résumé read let it launder its own bytes as the user's résumé, the\nexact laundering failure #1170's résumé-read guidance was meant to close.\nDrop the `(or match_resume:resume_extract_text)` clause and keep the two\ncorrect pointers (documents:documents_list / documents:documents_get_text).\n\nThe guard test that validates INSTRUCTIONS' ns:cmd citations only checked\nthat the cited POLICY row exists, not that it is Effect::Read — so a row\nlater reclassified away from Read would still pass while every caller\nfollowing the prose earns a wrong_tool refusal. Both tests (the ns:cmd\nscanner and the profile-tool description check) now assert the matched\nrow's effect too; verified against a reintroduced Reversible mutation.\n\nRefs #1165, #1164, #1169, #1170, #1171\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* docs(agent-cli): correct the updater_check policy row's false comment\n\nThe comment justifying updater::updater_check as Effect::Read claimed its\nUpdaterState write is \"never observable through any other command\" — false:\nupdater_download reads pending_update/pending_version to pick the transfer\ntarget, so this write selects the install artifact for the rest of the\nflow. The comment also omitted that the command emits updater:status to\nthe renderer, changing what the user sees, while call-read's own tool text\npromises \"no state change\". Neither omission changes the classification\n(the write still isn't persisted to disk and is idempotent across retries,\nconsistent with existing Read rows that populate an in-memory cache or do\nnetwork I/O elsewhere in this table) so the row stays Effect::Read; only\nthe comment was wrong and is now corrected.\n\nRefs #1171\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): close six round-2 policy and prose gaps\n\n- profile tool description test now checks documents_get_text too,\n  and derives its ns:cmd pairs from the description string itself\n  instead of a hand-picked pair a rename could silently drift past\n- notifications_mark_all_read's proof comment now calls the count a\n  superset of the blast radius, not the exact count that flips\n- INSTRUCTIONS drops the documents_get_text mention, whose id param\n  (id) never matches documents_list's row id (_id); documents_list's\n  rows already carry the full text\n- call-read's tool description now claims \"no persisted state\n  change\" instead of an unqualified \"no state change\", which\n  updater_check (Read, but writes UpdaterState + emits an event)\n  falsified\n- updater_install's confirm proof now reads updater_check's pending\n  version instead of system_get_version's vacuous running-version\n  echo, closing the untracked deferral from the prior round\n- commands/help.rs's four module-doc spans now say help_search is\n  reachable in principle but currently NotExposed (issue #1169),\n  instead of asserting reachability that does not exist\n\nEach fix is paired with a test verified by mutation (reverting the\nfix locally reproduces the failure the finding described).\n\nRefs #1164, #1165, #1169, #1170, #1171\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): close five round-3 policy and prose gaps\n\nDrop documents:documents_get_text from the profile tool description (its\nid param never matches documents_list's _id rows) so both surfaces agree\nwith INSTRUCTIONS; add documents_list to PAGINATED_LIST_COMMANDS so a\ncaller who hits result_too_large on that now-sole résumé read has an\nactual narrowing path (limit/cursor) instead of dead advice.\n\nPin commands::help::help_search's NotExposed status against POLICY\nitself, not just its own prose, and make the doc-reachability guard\nread that row too, so a reclassification fails both checks.\n\nCorrect ADR-043 section 1, which still claimed help_search is reachable\nfrom the agent CLI/extension bridge after the code that reachability\nclaim justified was removed.\n\nSet call-read's readOnlyHint to false: it dispatches every Read row\nincluding updater:updater_check, which writes UpdaterState and emits a\nUI event, so a client auto-approving on that hint would silently let an\nagent probe for updates and drive the update banner unprompted.\n\nRefs #1165, #1164, #1169, #1170, #1171\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix: match a job by posting identity, not the byte-exact stored url\n\nThe agent `job` resource compared the caller's url against the stored url\nas a byte-exact normalized string, so a regional LinkedIn host, the\nnumeric-only /jobs/view/<id> form, the currentJobId=<id> query form, an\nhttp-scheme paste, or a scheme-less paste all missed a job the app\nactually held.\n\nAdd scrape_url::job_identity (apps/desktop/src-tauri/src/scraping/scrape_url/identity.rs,\nsplit out of scrape_url/mod.rs to stay under the R8 LOC cap): a canonical\n(board, id) identity for LinkedIn and Indeed, folding regional/apex hosts\nand the direct/slugged/query id forms onto one id, tolerant of a missing\nor http scheme. resolve_job now compares identity first and falls back to\nthe existing normalized-string compare for boards with no stable id space;\nstorage is unchanged. A job-not-found refusal now carries a `detail`\nnaming best-matches/found-jobs as where the stored url can be read.\n\nRefs #1166\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): stop mcp prose claiming documents_list carries full text\n\nThe generic-tier response pipeline fences every documents_list `text`\nfield through prompt_fence with JOB_CAP (8,000 chars), silently\ntruncating longer resumes/cover letters with no truncation marker on\nthe wire. INSTRUCTIONS and the `profile` tool description both told a\ncalling model the opposite — that rows \"already carry the full text\" —\nso an assistant reasoning over a truncated prefix could present its\nanswer as grounded in the complete document. Both strings now say the\ntext is fenced and capped instead of claiming it is full.\n\nAdd a regression test that fences text well past the cap and asserts\nit comes back shorter, and asserts neither prose surface claims \"full\"\ntext; reverting the prose fix alone (verified via a temporary mutation)\nfails it.\n\nRefs #1170\n\n* fix(agent-cli): revert updater_check to reversible and close round-5 gaps\n\nupdater_check writes UpdaterState and emits updater:status, so\nreclassifying it Effect::Read (issue #1165) forced call-read's\nreadOnlyHint false for every one of the table's other 63 Read rows,\nsince the hint is a per-tool promise, not per-row - reinstating the\nexact monolithic-tool failure ADR-040 rejected an alternative for. It\nis reverted to Reversible; a new updater::updater_status row reports\nthe already-maintained pending state (silent_check populates it) with\nno network call and no event, and updater_install's confirm proof now\nreads that row instead.\n\nnotifications_mark_read/notifications_mark_all_read (issue #1164) had\nonly an aggregate-count guard and a comment-only mention respectively;\na direct pin now asserts both rows' Effect and ProofSource shape.\n\nINSTRUCTIONS and the profile tool description (issue #1170) now name\ndocuments:documents_get_text alongside documents_list, spelling out\nthat its `id` param maps to documents_list's `_id` value, instead of\ndropping the command over a prose mismatch. Its reply was a bare\nstring that fence_named_fields_recursive's name-keyed walk could not\nreach; dispatch now fences that one bare-string reply too\n(reshape::SCALAR_FENCE_COMMANDS). The round-4 honesty guard test now\nasserts the positive \"fenced and capped\" claim on both surfaces, not\njust the absence of two exact banned substrings.\n\nADR-043 corrected an overclaimed reachability guarantee: agent.call's\nCLI-origin gate is a spoofable label, not a boundary, matching\nextension_bridge::mod.rs's own caveat on the equivalent AGENT_QUERY\ngate.\n\nRefs #1165, #1164, #1169, #1170, #1171\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): stop mcp prose claiming a document read is uncapped\n\nBoth documents_get_text mentions (INSTRUCTIONS and the profile tool's\ndescription) claimed it returns a document's \"FULL, uncapped text\" while\nits reply is fenced and capped at the same prompt_fence::JOB_CAP limit\ndocuments_list rows already carry — there is currently no path to more\nthan the fence cap of any document's text through this server. Reword\nboth surfaces to say documents_get_text returns the SAME text by id,\nfenced and capped at the SAME limit, and never claim more.\n\nTighten the honesty guard (documents_text_prose_never_claims_full_past_\nthe_fence_cap): the round-5 version only checked two hand-picked banned\nsubstrings (\"carry the full\", \"full text\"), neither of which matched the\nactual defect phrase (\"FULL, uncapped text\"). It now walks a window\naround every documents:<cmd> token in each string and rejects \"uncapped\"\nor a standalone \"full\" nearby, and requires a cap disclosure near that\nSAME token rather than anywhere in the paragraph.\n\nRefs #1170\nRefs #1164\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): distinguish checked/current from unknown on updater_status\n\nupdater_status's {\"available\": false} used to be identical whether the\napp had never checked for an update, its last check failed, or the\nMicrosoft Store build never checks at all — a read-only caller could not\ntell \"genuinely current\" apart from \"we simply don't know\". Add\nUpdaterState.checked, set once updater_check/silent_check actually\ncompletes (found an update OR confirmed none, never on a network error),\nand have status_reply consult store_managed first so a Store build\nreports its own managedBy marker instead of a bare false.\n\nRefs #1165\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): fence a confirm proof through the same path a caller reads\n\nextract_from_fenced_response called fence_scraped_fields directly, one\ncall short of what reshape_reply actually does to every reply a caller\nsees (fence_scraped_fields PLUS fence_scalar_reply for a bare-string\nreply like documents_get_text's). No real ProofSource::read_command is\non SCALAR_FENCE_COMMANDS today, so this was latent, but a future row\nlanding on both would have reintroduced the \"permanently unsatisfiable\nconfirm\" bug security review round 4 already fixed once. Factor the\ncomposition into reshape::fence_reply and call it from both reshape_reply\nand the proof path, so they cannot diverge again.\n\nRefs #1171\nRefs #1169\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix: close round-7 agent-cli findings on updater, mcp prose and documents\n\n- pin the four production writes of UpdaterState.checked with a source-text\n  regression test (updater/test.rs) — every prior test only read the field,\n  so deleting all four writes stayed green\n- bound the mcp documents-prose cap-disclosure check to a forward-only span\n  ending at the NEXT documents:<cmd> token, so one command's disclosure can\n  no longer satisfy a neighbouring command's own requirement\n- drop the dead Autopilot.totalApplied counter from autopilot_list/\n  autopilot_get's agent-cli reply only (new DROP_FIELDS reshape step); the\n  automations resource already dropped it, this closes the remaining raw\n  dispatch path (issue #1171's residual)\n- state in INSTRUCTIONS/profile that an empty documents_get_text fence means\n  \"no such document\", not \"this document has no text\"\n- reserve a TRUNCATED marker inside the fence cap on documents_list rows and\n  documents_get_text so a caller can tell a prefix from a complete document\n  without knowing the cap number\n\nRefs #1165, #1164, #1169, #1170, #1171\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix: mirror reshape's pre-fence steps in the confirm-proof path\n\nreshape_reply grew drop_dead_fields/mark_truncated_document_text as\nsteps before fencing, but extract_from_fenced_response only mirrored\nthe fencing step - a future proof reading documents_list's text or\nautopilot_get/autopilot_list's totalApplied would have compared a\nraw value against a caller-visible marked/dropped one, making the\nconfirm ceremony permanently unsatisfiable. Factor both steps into\nreshape_pre_fence and call it from both reshape_reply and\nextract_from_fenced_response, then pin the full composition with a\nsynthetic documents_list/text ListMatch fixture.\n\nAlso stop instructions/schemas prose from claiming an empty\ndocuments_get_text fence means only \"no such document\" -\nstore.get(&id).map(|doc| doc.text).unwrap_or_default() returns the\nidentical empty string when a real document's own extracted text is\nempty, so the two causes are not distinguishable from that reply\nalone. Reword both spans to say so and point the caller at\ndocuments:documents_list to cross-check.\n\nRefs #1166\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix: correct the third stale copy of the documents_get_text prose\n\nThe doc comment above documents_get_text's unwrap_or_default() still\nclaimed an empty reply means the id did not resolve, contradicting\nthe prose already fixed in agent_cli::mcp's INSTRUCTIONS and profile\ntool description, and it sat directly above the line that disproves\nit. Reword it to match, and add a source-text guard test that pins\nthis third copy independently of the other two.\n\nRefs #1166\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent-cli): close the confirm-proof drift class, not just its steps\n\nRound 6 and round 8 each caught the confirm-proof fencing path one step\nbehind reshape_reply's real composition. Both fixes hand-rolled the two\nsteps extract_from_fenced_response happened to know about at the time,\nso a future step touching a live proof leaf field would still silently\nmake that row's confirm ceremony permanently unsatisfiable in production\nwith every existing test green.\n\nExtend the 35-row loop in every_irreversible_proof_agrees_with_what_a_\ncaller_reads_through_fencing to also compare the proof path's result\nagainst extract() over the FULL reshape::reshape_reply composition\n(paging/base64 included), not a third hand-rolled copy of the same two\nsteps. Verified: a simulated future reshape_reply step that rewrites\nautopilot_get's name field (the live proof leaf for autopilot_remove and\nautopilot_run) fails only this new assertion, with the two prior asserts\nand both round-6/round-8 regression tests still green.\n\nRefs #1166\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): compact found-jobs rows, server-side filters, cross-autopilot query\n\n`found-jobs` rows are now compact by default (title/company/location/score/\nurl/foundAt/applied/isAgency/autopilotId/autopilotName), with `description`\nopt-in via `includeDescription`, and five server-side filters (`minScore`,\n`country`, `remote`, `applied`, `query`) applied before paging so `total`\nalways reflects the filtered count. `applied` reuses\n`commands::autopilot::applied_job_urls` (now `pub(crate)`) and `remote`\nreuses `scraping::engine::location_filter::REMOTE_MARKERS` (now\n`pub(crate)`) -- the same predicates best-matches/job already use, never a\nsecond hand-typed matcher.\n\n`autopilotId` is optional: omitted, the traversal spans every autopilot in\nstore order, deduped by `canonical_job_key` across lists, answering \"is this\nrole already in my list?\" in one call (`found-jobs {query: \"...\"}`). The\ncursor's issuer half is a fixed sentinel for a spanning traversal so it can\nnever be replayed against a scoped one or vice versa.\n\n`best-matches` gains the same cursor/query paging found-jobs already had\n(issue #1146 P11), reusing `extension_bridge::paging::clamp_limit` in place\nof a hand-rolled clamp that let `limit: 0` through as zero instead of\nfalling back to the default -- a page whose `nextCursor` never advances\nwould have hung any paging loop built on it. The `job` tool description now\nstates it matches by posting url only and points a title/company lookup at\nfound-jobs' own `query` filter.\n\nRefs #1146\nRefs #1167\nRefs #1168\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): filter-before-dedup, selector/filter validation and cursor scoping on found-jobs\n\nResolves five round-1 review findings on the found-jobs/best-matches\nserver-side filters and spanning traversal (#1167, #1168):\n\n- Dedup across autopilots now runs AFTER passes_filters, so the first\n  PASSING copy of a posting wins instead of the first copy in store\n  order — minScore is scored per-autopilot, so filtering-after-dedup\n  silently under-reported total whenever an earlier autopilot's copy\n  failed the filter.\n- A present-but-blank or flag-shaped autopilotId now errors instead of\n  silently widening the scope to every autopilot, both on the bridge\n  resource (parse_autopilot_id_arg) and on the MCP tool_argv translation\n  layer, which additionally guarded against a flag-shaped id being\n  forwarded as a bare CLI positional and misread as a real flag.\n- FoundJobsFilters::from_payload is now fallible: a filter key that is\n  present but not readable as its declared type refuses the call rather\n  than silently dropping the filter and returning an unfiltered page.\n  --min-score also rejects a non-finite value (1e400/inf/nan) at CLI\n  parse, before it can collapse to JSON null and vanish.\n- found-jobs' and best-matches' paging cursors now fold their active\n  filter arguments (a fingerprint) into the issuer half, so a cursor\n  replayed under different filters/query hits the existing wrong-scope\n  refusal instead of silently paging a different filtered list at a\n  stale offset.\n- found-jobs' compact row shape restores scoreProvisional (the \"don't\n  trust this score\" flag) since minScore is the one filter that reads\n  it; the compact-page byte-size test now compares against the same\n  page with description opted in, so a description regression fails it.\n\nRefs #1167, #1168\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): harden best-matches query and document its scope/caveats\n\nRound-2 review findings on found-jobs/best-matches:\n\n- best-matches' own `query` filter read `.and_then(Value::as_str)`, the\n  exact silent-drop combinator found-jobs' filters were hardened away\n  from — a wrong-typed or blank query collapsed to \"absent\" and returned\n  the unfiltered ranked list with a `total` read as filtered. It now\n  reuses found_jobs::trimmed_lowercase_filter, which also now refuses a\n  present-but-blank query/country instead of treating it as unset (the\n  shell \"--query $UNSET_VAR\" repro).\n- found-jobs' cursor-binding statement (\"valid only for the same\n  autopilotId scope\") is now paired with \"AND the same filter arguments\"\n  on the schema resource and the CLI's own VERB_TABLE row, matching the\n  MCP tool schema and the refusal text.\n- The MCP includeDescription argument is now type-checked before argv is\n  built, mirroring the existing autopilotId guard, so a non-bool value\n  refuses instead of silently vanishing through\n  `.and_then(Value::as_bool)`.\n- best-matches' query is now documented (schema resource, VERB_TABLE,\n  MCP tool schema) as scoped to the already-capped ranked candidate\n  list, not the full stored corpus — found-jobs' own query is the one\n  that spans everything.\n- resolve_found_jobs' offset-stability doc now names `applied` (a fresh,\n  live re-derivation on every call) as an input that can REMOVE a row\n  mid-traversal and shift later indices down, unlike the pre-#1167\n  duplicates-never-skips guarantee for a `record_run` merge.\n\nRefs #1167, #1168\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): refuse the applied filter without a store, cap best-matches at one page\n\nResolves round-3 confirmed review findings on the found-jobs/best-matches\nresources (#1167, #1168):\n\n- found-jobs: refuse the `applied` filter (not the whole call) when\n  ApplicationStore failed to open, instead of silently reading the\n  store's absence as \"the user has applied to nothing\" (applied: true\n  wrongly returning total: 0, applied: false wrongly returning postings\n  already applied to).\n- best-matches: raise MAX_BEST_MATCHES_LIMIT to match the underlying\n  command's own BEST_MATCHES_CAP (100) so a single max-limit page always\n  reaches the whole reachable set, instead of needing 2-5 calls that each\n  re-run the real clustering pass behind a 30s-refill, burst-1 bucket.\n- Document the cursor<->query binding and the capped meaning of `total`\n  on best-matches' agent-schema/--help descriptions (found-jobs already\n  had both); extend the found-jobs cursor-binding drift test to actually\n  fail on the pre-fix wording and to cover best-matches too.\n- Extract best_matches_resource's argument parsing into a pure,\n  AppHandle-free fn so its delegation to the hardened query parser is\n  itself pinned, not just the shared helper underneath it.\n- Refuse an empty found-jobs positional (the `agent found-jobs \"$AP_ID\"`\n  unset-variable shape) with the same blank-selector message the\n  downstream layers already give, instead of \"unknown argument\".\n- Add a cursor test that drives two different filter sets by hand (every\n  existing cursor test derives its expected issuer from the function\n  under test, so none could catch a dropped filter fingerprint) and a\n  round-trip guard tying VERB_TABLE's advertised flags to what\n  parse_found_jobs/parse_best_matches actually accept, both directions.\n- docs/knowledge/agent-cli.md: fold the found-jobs bullet's cursor claim\n  into a scope+filter description and add best-matches as a second\n  paginating resource.\n\nRefs #1167, #1168\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent): cover best-matches query filtering by title or company\n\nresolve_best_matches's `query` retain() had no test asserting it actually\nnarrows the row set or matches on company as well as title -- only cursor\nissuance/refusal and argument parsing were exercised. Mutation-checked by\ntemporarily disabling the retain block: the new test fails (total 3 vs\nexpected 1) and passes again once restored.\n\nRefs #1168\nRefs #1146\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent-cli): close five round-3 test-gap findings on agent reads\n\nParameterize found_jobs_cursor_issued_under_one_filter_set_is_rejected_under_another\nover all five folded filter parts (minScore/country/remote/applied/query), not just\nminScore, so dropping any one from found_jobs_cursor_issuer's fingerprint still fails.\n\nPin silent_check's Err(_) arm as a required no-op in updater's all_four_checked_true\nwrites test, so swapping it with the Ok(None) arm (claiming checked after a failed\nprobe) fails instead of passing on position-independent substring counts.\n\nExtend the found-jobs forbidden-key sweep to also assert on the fixtures' distinctive\nVALUES (SECRET RESUME TEXT, gpt-secret, internal.example.local, etc), not just key\nnames, so a projection leak under a renamed key can't slip through.\n\nReplace the vacuous if-let (no else) around the bogus-flag half of the agent-cli flag\nround-trip test with expect_err, so the parser silently accepting an undocumented\nflag now fails the test instead of passing.\n\nAdd a real-fixture cross-check (agent_call::proof's new\nextract_scalar_reads_updater_installs_real_pending_version_off_status_reply) that\nfeeds updater::status_reply's real output through the real updater_install POLICY\nrow via proof::extract, so the two independent literal-pinned tests (policy path and\nstatus_reply shape) can no longer drift apart silently. Widens status_reply to\npub(crate) for this test-only reuse; no behavior change.\n\nEvery fix mutation-tested by hand: reverting it back to the pre-fix shape reliably\nfails the corresponding new/adjusted assertion, then reverted clean.\n\nRefs #1164, #1165, #1166, #1167, #1168, #1169, #1170, #1171\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): fence ai_research_answer's bare-string reply too\n\nSCALAR_FENCE_COMMANDS shipped with a completeness audit that named\nonly documents_get_text, so ai_research_answer's provider web-search\nnotes - the most injection-prone reply on the surface, and its own\ndoc's promise that \"the prompt layer fences them as untrusted\" - reached\na caller completely unfenced. Its policy row is Irreversible and\ndispatches through the same dispatch_direct -> reshape_reply ->\nfence_reply path documents_get_text does, so it was reachable at the\nsame tier that holds call-irreversible dispatch.\n\nAdds ai_research_answer to the list and corrects the doc comment to\nname every bare-string command actually reviewed (documents_get_text,\nai_research_answer, contact_profile_header_line, system_get_version,\nsystem_get_protocol_version) and why each is or is not included, so a\nfuture prose audit can't silently narrow back to one entry.\n\ncontact_profile_header_line stays off the list even though it also\nreturns a bare string: it is rendered from the user's own profile\nfields, not third-party content, the same reasoning already applied to\nsystem_get_version/system_get_protocol_version - the fencing mechanism\nguards against third-party prompt injection, not the user's own data.\n\nFiled and fixed here as issue #1181 (security review round 9, SEC-1).\nA related but separate finding from the same round - contact_profile_get\nbeing reachable at the default Read tier with no photo projection - is\ndeliberately NOT touched in this PR; filed as issue #1180 per its own\nsuggested fix to avoid scope creep on this change.\n\nRefs #1181, #1180\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(egress): declare identity.rs's scheme-prepend as a dynamic site\n\nThe #1166 job-identity fix (5c45c10d) added a `https://{url}` literal\nto satisfy reqwest::Url::parse on a possibly scheme-less caller-pasted\nurl; EGRESS-3 flags any undeclared dynamic-host literal and the\npre-push hook's egress test was failing on this branch as a result.\nThe parsed Url is only read for host/path comparison, never fetched.\n\nRefs #1166\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): trim best-matches to a byte budget, derive job.applied\n\nbest-matches builds its page from an already-capped ranked list but had\nno byte-size guard, unlike the sibling found-jobs resource added in the\nsame PR for the exact same reason (title/company/location can each\nindependently reach the JOB_CAP fence). A max-limit page of legitimate\nCJK-length postings could reach several MB, tripping the MCP result cap\nor the bridge's own frame cap. resolve_best_matches now fences each row\nthen runs the page through the shared trim_to_byte_budget primitive,\nmirroring found_jobs::trim_page_to_budget.\n\nThe job resource echoed the stored FoundJob.applied bit, which is\nalways false on disk (only enrich_applied/project_found_job_row fill it\nin at read time). resolve_job now derives applied from\ncommands::autopilot::applied_job_urls, the same set found-jobs and\nbest-matches already use, so a caller reading job after found-jobs no\nlonger risks a duplicate application off a stale false.\n\nAlso: fold the fence enumeration in docs/knowledge/agent-cli.md into a\npointer at the owning fence fns instead of a hand-kept list (it had\nalready drifted, missing fence_scalar_reply/fence_autopilot_name), and\nrename policy_table_has_exactly_167_rows to a count-free\npolicy_table_row_count_is_pinned so its own name (cited by two module\ndoc pointers as the row-count authority) can't restate a stale number\nagain.\n\nRefs #1165, #1164, #1169, #1170\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): trust the board's own remote flag, not just location text\n\nfound-jobs' server-side remote filter only scanned REMOTE_MARKERS against\nlocation text, so an all-remote board (WeWorkRemotely/RemoteOK/Remotive/\nJobicy) whose posting carries location:None or a marker-free jurisdiction\nstring (\"USA Only\") was classified NOT remote — --remote true silently\ndropped the whole board and --remote false returned it as on-site.\n\nPersist the board's own extra[\"remote\"] flag onto FoundJob.board_remote at\nfind-time and OR it into the filter, mirroring location_verdict's own\nboard_remote short-circuit exactly instead of reusing only half of it.\n\nRefs #1167\n\n* fix(agent-cli): stop the truncation marker's doc claiming document-only scope\n\nfence_scalar_reply applies reserve_truncation_marker to every command on\nSCALAR_FENCE_COMMANDS, which already included ai_research_answer — but the\nmarker's own doc comment and wire text still claimed the two document call\nsites (\"documents_get_text\"/\"documents_list\" rows) were the only place it\napplied, and the wire text said \"the whole document\" for a research answer\nthat isn't one.\n\nReword both to describe the actual SCALAR_FENCE_COMMANDS-wide scope instead\nof gating the marker to documents_get_text alone, which would have removed\nthe truncation signal ai_research_answer legitimately needs.\n\nRefs #1169\n\n* docs(agent-cli): best-matches now shares found-jobs' paging primitives too\n\nThe generic-tier paging bullet named only the curated found-jobs resource\nas sharing the limit-clamp/byte-budget primitives and owning its own\nissuer-scoped cursor grammar; best-matches picked up both (clamp_best_matches_limit,\ntrim_best_matches_page_to_budget, best_matches_cursor_issuer/parse_best_matches_cursor)\nwithout this bullet being updated to match.\n\nRefs #1167, #1168\n\n* docs(agent-cli): stop pinning adr-038's command count to a stale literal\n\nThe ADR restated a command count of 164 in its title and twice in the body;\nthis branch alone moved it to 168 (updater::updater_status), and it was\nalready stale at 167 before that. Point at\npolicy::tests::policy_table_row_count_is_pinned instead of a literal, the\nsame authority policy.rs's own module doc already cites.\n\nRefs #1165\n\n* docs(agent-cli): note found-jobs as a resource, amend adr-040's stale tool count\n\nCONTEXT.md's Resource glossary entry listed the five original curated\nverbs and omitted found-jobs, which shipped as a sixth curated resource\n(issue #1115) before this branch. ADR-040's title/§2/§4 state \"five\ncurated\"/\"eight tools total\", now stale after found-jobs and this\nbranch's best-matches/found-jobs filter work (issues #1167/#1168); added\na dated Amendment section pointing at mcp::schemas::tools() as the\nauthoritative, current list instead of restating a count that will drift\nagain with the next tool.\n\nRefs #1167 #1168\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(scraping): derive an all-remote board's remoteness at read time\n\nfound-jobs' `remote` filter trusted only the stored `board_remote` bit,\nwhich every FoundJob written before that field existed defaults to\nfalse, so an all-remote board (wwr/remoteok/remotive/jobicy) row\npersisted before the fix silently dropped from `--remote true`.\n\nAdds `Scraper::is_all_remote` (default false, overridden by the four\nall-remote boards) plus a registry-level `is_all_remote_board(id)`\nhelper, so found-jobs re-derives remoteness from the stored `board` id\ninstead of only the possibly-stale stored flag. The `remote` filter is\nnow three-valued: an empty-location row with no board/registry/marker\nsignal is UNDECIDED and matches neither `remote: true` nor\n`remote: false`, instead of defaulting to a confident (and wrong)\n\"not remote\". Updates the MCP tool schema's `remote` description to\nmatch.\n\nRefs #1167 (PR #1182 review threads T1, T2)\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(agent-cli): derive found-jobs remote/applied and fix paging\n\nCombines three overlapping fixes touching the same functions in\nfound_jobs.rs/agent_read.rs:\n\n- The undecided-remote handling from the previous commit is applied to\n  found-jobs' `passes_filters`/`remote_determination` (T1/T2).\n- `applied` on both `job` and `found-jobs` was derived by byte-comparing\n  normalize_job_url(found.url) against applied_job_urls(), so a stored\n  posting under a different LinkedIn host/path spelling than the\n  recorded application reported applied=false. Adds one shared\n  `job_is_applied` helper (identity-first via `job_identity`, falling\n  back to the normalized-string compare) used by both `resolve_job` and\n  `found_jobs::candidate_jobs` (T4).\n- When the applications store is unavailable, both resources used to\n  ship a confident `applied: false` on every row/reply instead of\n  omitting the key — the unsafe direction for an autonomous caller.\n  Both now omit `applied` and add `appliedUnavailable: true` (T3).\n- `project_found_job_row` is made infallible: its allowlist round trip\n  cannot fail for any real FoundJob value, so the `filter_map` that fed\n  it used to drop an unreachable \"failure\" while still counting it in\n  `total`, which could in principle reissue the same paging cursor\n  forever. Removing the `Option` removes the class instead of\n  recovering from it (T5).\n\nRefs #1166, #1167 (PR #1182 review threads T1-T5)\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs(agent-cli): document updater_status scope, point at the resource table\n\nupdater_status is registered in generate_handler! with no ipc/contracts,\ntauri-client, or services hook half (AGENTS.md rule 14) — notes on the\nfn's own doc that this is by design: it is agent-tier only, reachable\nfrom the webview like any command but with no renderer caller for a\nbare-status read.\n\nAlso replaces docs/CONTEXT.md's hand-enumerated Resource list with a\npointer at extension_bridge::agent_read::RESOURCES, the table --help\nand schema already derive from, so the glossary entry cannot drift the\nnext time a resource is added or removed.\n\nRefs #1167 (PR #1182 review threads T6, T7)\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(agent-cli): close round-4 applied-flag gaps in job/found-jobs\n\nChecks both the raw and unreserved-decoded url spelling before the\nidentity fallback so a percent-escaped stored url matching the exact\nrecorded application spelling still reads as applied on a board with\nno stable id space, hoists the applied-url identity index out of\nfound-jobs' per-row loop into one build per call, documents\nappliedUnavailable on the job/found-jobs tool descriptions and the\nschema resource text (not only in Rust doc comments), and lets a\nlocked/corrupt applications DB read as store-unavailable instead of a\nconfident applied:false by distinguishing a query failure from a\ngenuinely empty applied set. Replaces the one panicking .expect() on\nthis surface with a debug_assert! plus a minimal-row fallback so a\nfuture FoundJob/FoundJobSlice type drift degrades one row instead of\naborting the whole release build.\n\nRefs #1166, #1167\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-09T02:12:46+02:00",
          "tree_id": "2f3a13c3408566629a16b6837f621aa43188e788",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7777b9d487cd9e8d90a9d4bf9d6f8d5bd91488c2"
        },
        "date": 1788913548489,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2338711,
            "range": "± 40678",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2774095,
            "range": "± 24360",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 250443,
            "range": "± 7728",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2be376971d60a0859d73af47d6e499d51ab80fe2",
          "message": "fix(agent): generated command catalogue, argument validation and one refusal contract (#1183)\n\n* feat: generate the agent-cli command catalogue from tauri-client + contracts\n\nAdds pnpm gen:agent-catalogue (packages/shared/scripts/gen-agent-catalogue.ts),\nparsing apps/desktop/src/tauri-client/namespaces/**/*.ts invoke() call sites for\neach command's declared top-level argument keys and required-ness, and the\nmatching packages/shared/src/ipc/contracts/*.ts TSDoc for a one-line\ndescription, reusing gen-api-docs.mjs's own TS-AST helpers (now exported,\nincluding its correctly-resolved `ts` module instance, since this generator's\nown nearest typescript devDependency is the incompatible v7 line). A wrapper\nkey typed as a generated Zod request schema or a plain contract interface also\ngets its own nested field names resolved, so a nested unknown key is\ncatchable too. A construct the generator cannot parse with confidence is\nlisted in UNCATALOGUED rather than guessed at.\n\nEmits apps/desktop/src-tauri/src/extension_bridge/agent_cli/catalogue.rs (the\naggregator) plus sharded catalogue/shard_*.rs data files, split by rendered\nline budget to stay under this crate's own R8 hard LOC cap — the struct-literal\ntable cannot be reorganized to shrink, and rustfmt's own default\nstruct_lit_width forces one field per line regardless. A LazyLock<Vec<_>>\nassembles the shards at first access, transparent to every reader\n(Deref<Target = [CatalogueEntry]>).\n\nWires gen:agent-catalogue[:check] into the root/packages/shared package.json\nscripts and into the CI lint/format job that already runs gen:ipc:check,\ninstalling rustfmt there (that job otherwise has no Rust toolchain) since the\ngenerator shells out to the real binary rather than hand-approximating its\nformatting.\n\nRefs #1163, #1158, #1160\n\n* fix: validate call-* input keys against the catalogue before the confirm ceremony\n\nAdds agent_call::validate::check_input, run as dispatch's own first check,\nbefore gate: an unknown top-level key, an unknown nested field on a wrapper\nwhose type resolved, or a missing required top-level key now refuses with a\nnew invalid_input sentinel naming the offending/missing key and the command's\ndeclared keys, rather than serde silently dropping it and reporting\nsuccess:true (#1158) or an approved irreversible confirm dying afterward on\nthe target command's own deserializer (#1160 — applications_delete's\nkeepDocuments is the concrete case). This layer's own limit/cursor paging\nkeys are exempted on a row reshape::PAGINATED_LIST_COMMANDS marks. A command\nabsent from the generated catalogue keeps today's behaviour unchanged.\n\nThe commands MCP tool now reads the same catalogue: each row carries\ndescription and args (name/required/fields, or null when uncatalogued), and\nan Irreversible row also carries proofField (agent_call::proof::proof_field,\nthe dotted path a confirm ceremony will require) so \"what would deleting this\nrequire\" is answerable without dispatching anything. Adds an optional\nnamespace filter next to the existing effect one, validated the same way\nagainst a typo'd value. unknown_command's detail (both the app-side\nagent_call::Refusal and the MCP server's own local pre-check) now names the\nreal namespace when the bare command name matches exactly one POLICY row\n(agent_call::namespace_suggestion) — an exact match only, never a fuzzy one.\n\nRefs #1163, #1158, #1160\n\n* test: cover the catalogue-backed validation and commands tool additions\n\nagent_call::validate::tests pulls real rows straight out of the generated\nCATALOGUE (applications_delete, applications_save_from_posting,\njob_preferences_set, applications_list) rather than a hand-typed fixture:\nunknown top-level/nested key refused, missing required key refused (and\nproved to be caught before gate would ask for a --confirm, the #1160\nordering fix), a wrapper whose nested type didn't resolve skips the nested\ncheck, paging keys exempt only on a PAGINATED_LIST_COMMANDS row, and an\nuncatalogued command stays unvalidated.\n\nagent_call::tests covers namespace_suggestion/unknown_command_detail\ndirectly, and policy::tests adds a catalogue-coverage test: every\nRead/Reversible/Irreversible POLICY row must be catalogued, listed in the\ngenerator's own UNCATALOGUED, or on a small hand-written allowlist\n(boards_list, privacy_clear_data — the two POLICY rows with zero renderer\ninvoke() call sites at all), so a new command cannot silently skip\nvalidation; paired with a test that the allowlist itself doesn't rot.\n\nagent_cli::mcp::tests covers commands' new description/args/proofField rows\n(including the issue's own applications_delete -> \"application.title\"\nexample, and a Count-sourced row carrying no proofField), the namespace\nfilter composing with effect and rejecting a typo'd value the same way\neffect already does, and unknown_command naming the right namespace on both\nthe local MCP pre-check and the real dispatch path.\n\nRefs #1163, #1158, #1160\n\n* fix: pin rustfmt, fence caller keys and fix catalogue codegen gaps\n\nResolves confirmed review findings on the agent-cli catalogue generator and\ndispatch-time validation (issues #1163, #1158, #1160):\n\n- gen-agent-catalogue.ts's rustfmt shell-out now passes cwd into\n  apps/desktop/src-tauri so rustup resolves the SAME pinned toolchain\n  `cargo fmt --check` gates on, instead of the machine's default `stable`;\n  the CI step now installs that exact pinned toolchain too, rather than\n  `rustup component add rustfmt` against the runner's own default.\n- A caller-supplied JSON key echoed into an `invalid_input` refusal detail\n  is now fenced and capped the same way `Refusal::InvokeError` already is,\n  closing the one remaining unfenced/uncapped echo path.\n- A `T | undefined` parameter is now recognised as optional (alongside `?`\n  and a default), so the renderer's own clear-a-value payload is no longer\n  refused as missing a required key.\n- The catalogue's description cuts on `.` only (never `:`), with a\n  fallback to the full first paragraph when the cut is too short or leaves\n  a backtick/paren unbalanced — fixes content-free (\"Factory reset:\") and\n  malformed descriptions.\n- `applications.remove` gained TSDoc naming the `keepDocuments` cascade;\n  `catalogue::UNCATALOGUED` and the no-description row count are now\n  pinned against hand-written guards so a generator regression can't\n  silently widen either class.\n- A wrapper arg whose type this generator recognises but cannot resolve\n  (including a rest-destructured request object) now reads `fields: null`\n  on the wire, distinct from a plain scalar arg, and that class is pinned\n  against a hand-written list the same way UNCATALOGUED is.\n- Duplicate `invoke()` call sites across namespaces (e.g. boards/linkedin)\n  now fail codegen if their TSDoc disagrees, instead of letting\n  unsorted `readdirSync` order decide which description wins; both\n  directory listings are now sorted for determinism regardless.\n- docs/knowledge/agent-cli.md now names `invalid_input` as the primary\n  pre-dispatch signal for a catalogued command, `invoke_error` only as the\n  uncatalogued fallback.\n\nRefs #1163\nRefs #1158\nRefs #1160\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): close round-2 catalogue and ordering gaps\n\nContinues the round-1 fixes on issues #1163/#1158/#1160 with the findings\nfrom a second self-review pass:\n\n- NotExposed now refuses with its own cause before catalogue validation\n  runs (agent_call::dispatch_plan::plan), so a bad key on a not-exposed\n  command never surfaces the lesser invalid_input cause first; the pure\n  ordering decision moved out of agent_call.rs into its own module so a\n  test can drive it directly instead of only trusting dispatch's source.\n- A required wrapper key sent as an empty object ({} for req/prefs/etc.)\n  is refused on a Reversible/Irreversible row before it reaches an\n  all-Option request struct as a silent no-op success:true.\n- The generic MCP tools (call-read/call-reversible/call-irreversible) now\n  run the same catalogue check locally before forwarding, so a mis-keyed\n  body is caught even if the paired app process is stale.\n- commands rows gain proofKind (field/count/response_value), naming what\n  an absent proofField actually means instead of collapsing two causes\n  into one silence.\n- gen-agent-catalogue.ts: an unknown/inline-literal/indexed-access wrapper\n  type is now recognised as an unresolved wrapper (fields: null on the\n  wire) instead of falling through to a plain scalar; the description\n  cutter falls back to pulling in more sentences instead of publishing a\n  content-free fragment or an entire implementation-detail paragraph.\n  gen-api-docs.mjs's main() is exported and driven from a tiny\n  gen-api-docs.cli.mjs entry point rather than an import.meta.url\n  comparison that could silently no-op.\n- Backfilled TSDoc on the remaining undescribed IPC contract members\n  (ai.pullModel, aiGenerations.remove/removeBulk, autopilot.remove,\n  documents.remove, notifications.remove/clearAll, referrals.remove,\n  scrape.clearPostings, system.openExternal) and regenerated the\n  catalogue + docs/API.md.\n\nTests: dispatch_plan's plan() ordering driven directly, empty-wrapper\nrefusal, local MCP catalogue validation, proofKind per ProofSource shape,\nand hand-written pins for the resolved/unresolved wrapper-arg sets so a\ngenerator regression can't silently widen either class.\n\nRefs #1163\nRefs #1158\nRefs #1160\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): mirror the empty-wrapper gate and fix two contract-shape bugs\n\nResolves three CONFIRMED review findings on the call-* / MCP surface:\n\n- A1-r1-AC-1 / A1-r1-SEC-1 (MEDIUM): the MCP server's local pre-dispatch\n  gate ran check_input but never its sibling check_no_empty_required_wrapper,\n  so an empty required wrapper (issue #1158 member 3's `{\"req\":{}}` shape)\n  still forwarded to a possibly-stale peer app instead of refusing locally.\n  invalid_input_detail now takes the row's Effect and runs both checks.\n\n- A1-r1-AC-2 (MEDIUM): a zero-arg command's invalid_input detail ended in a\n  dangling, content-free \"declared keys: \" — declared_keys now names what\n  the caller CAN do (\"this command declares no arguments\") instead.\n\n- A1-r1-AC-3 (MEDIUM): gen-agent-catalogue.ts published a plain scalar\n  wrapper-key type (PipelineStage, a string-union alias) as an unresolved\n  object wrapper (\"fields\": null) whenever its declaration fell outside the\n  generator's two known sources. A narrow third lookup (\n  collectScalarTypeAliasNames) proves a named type's declaration is\n  provably a scalar (a keyword, a literal union, or a `(typeof X)[number]`\n  const-array alias) before flipping it to a plain scalar arg — genuinely\n  unresolved object wrappers (ResumePipelineRunRequest,\n  PerformanceBackendConfig) are unaffected.\n\nA1-r1-SEC-2 (the confirm proof not binding to applications_delete's\nkeepDocuments flag) is deliberately deferred — see the ponytail note on\ndispatch_irreversible_confirmed for the reasoning and the upgrade path.\n\nRefs #1163, #1158, #1160\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): close round-2 findings on the empty-wrapper gate and proof surface\n\nResolves three CONFIRMED round-2 review findings (issues #1163, #1158, #1160):\n\n- check_no_empty_required_wrapper filtered out any wrapper whose fields the\n  generator could not resolve (Some(&[])), so a required wrapper sent as {}\n  skipped the emptiness check entirely on those rows — autopilot_update's\n  {\"autopilotId\":\"ap-1\",\"req\":{}} reproduced #1158's exact symptom (dispatches,\n  bumps updatedAt, merges nothing). Now keyed on arg.fields.is_some() alone,\n  wording the refusal detail without a field list when the shape is\n  unresolved. New test pins autopilot_update's real catalogue row.\n\n- The MCP commands tool description stated unconditionally that an\n  Irreversible row carries proofField, but ~9 of ~34 such rows (Count/\n  MatchCount/empty-path Scalar-Lookup) never do, and proofKind — added\n  precisely to explain the absence — was never mentioned. Reworded to name\n  proofKind on every row and qualify proofField as present only when\n  proofKind is \"field\". New test asserts both keys are named in the\n  description.\n\n- A1-r1-SEC-2 (the confirm proof authorising applications_delete's\n  keepDocuments:false cascade as readily as :true) was deferred, but the\n  deferral existed only in a Rust doc comment, not on issue #1160 or an\n  ADR-038 amendment — so nothing in the tracker recorded the unmet\n  expectation. Posted the deferral + upgrade path as a comment on #1160 and\n  cross-referenced it from the doc comment.\n\nAlso closes a latent AC finding: gen-agent-catalogue.ts's\ncollectContractInterfaceFields keyed a flat Map by bare interface name with\nlast-wins semantics, so two same-named exported interfaces in different\nipc/contracts/*.ts files would silently make the nested-key contract\ndispatch enforces depend on readdirSync order — the same hazard the sibling\nmulti-call-site guard already fail()s on. Now fails on a genuine field-list\ndivergence; silent when the two declarations agree. main() is now guarded\nto run only on direct invocation (matching check-agent-system.mjs's\npattern) so the new duplicate-name test can import the module's pure\nhelper without regenerating catalogue.rs as a side effect.\n\nRefs #1163, #1158, #1160\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): qualify the invalid_input promise for unresolved wrappers\n\nINSTRUCTIONS and the commands tool description promised unconditionally\nthat an unrecognised or missing key refuses with invalid_input, and\nqualified that only for args: null rows. For the ~7 mutating rows whose\nwrapper the catalogue generator could not resolve (fields: null, e.g.\nautopilot_update's req), a wholly-unrecognised NESTED key is neither\nvalidated nor refused -- validate.rs already skips the nested walk for\nthose rows, but the prose never disclosed it. Both surfaces now say so,\npinned by a new test and mutation-verified via a live probe against the\nbuilt binary with the app closed.\n\nAlso closes the sibling test gap where proofField/proofKind came from two\nindependent matches over ProofSource with only 4 hand-picked rows checked\nagainst each other -- a divergence on any other row (verified against\nresume_pipeline_run by a planted mutation) would have made the commands\ndescription's \"only when proofKind is field\" promise a lie with every\nexisting test green. A loop over every irreversible row now pins the\ninvariant directly.\n\nRefs #1163, #1158\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(agent-cli): split the catalogue generator's cli entry point out\n\ngen:agent-catalogue/gen:agent-catalogue:check ran gen-agent-catalogue.ts\ndirectly, guarded by an `if (resolve(process.argv[1]) ===\nfileURLToPath(import.meta.url))` self-invocation check. If that\ncomparison ever stopped matching (a symlinked/shimmed invocation, or a\nWindows drive-letter/short-path mismatch) the check would exit 0 having\ndone nothing, passing CI against a stale catalogue -- exactly the class\nof bug gen-api-docs.cli.mjs was already split out to close, and the two\ngenerators disagreed on how to solve the identical problem.\n\nmain is now exported with no self-invocation, and a separate\ngen-agent-catalogue.cli.ts wrapper (mirroring gen-api-docs.cli.mjs) is\nthe only thing package.json's two scripts run. Verified live: pnpm\ngen:agent-catalogue:check passes against the current tree and fails\nagainst a planted drift, through the new wrapper.\n\nRefs #1160\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(agent-cli): one throttle envelope, query frame cap, and tier_not_enabled\n\n- One `rate_limited` refusal shape on both agent.query and agent.call: the\n  sentinel replaces the prose that used to sit in `error` on the read tier,\n  `detail` carries the prose, and both tiers now attach `retryAfterMs`\n  (computed fresh off `AgentQueryThrottle`'s own refill rate, never a\n  duplicated constant) plus the refused request's own identity (`resource`\n  and, where the resource takes one, `url`/`autopilotId` on the read tier).\n\n- agent.query gets the bounded-refusal and frame-cap controls agent.call\n  already had: refusals clamp the `resource`/`reqId` they echo the same way\n  `agent_call::clamp_ident` does (now `pub(super)` and reused directly), and\n  every reply this tier builds is re-measured against the bridge's frame cap,\n  substituting a `result_too_large` refusal for an oversized success payload\n  instead of dropping it silently.\n\n- `local_call_refusal` now takes the active `Tier`: a `call-*` mismatch where\n  the right tool is registered on this launch stays `wrong_tool`; where it\n  is NOT registered, it refuses `tier_not_enabled`, naming the launch flag to\n  relaunch with instead of pointing at a tool the client cannot see. The MCP\n  `instructions` text is updated to match (`wrong_tool` only promises \"retry\n  on the other tool\" when that tool is actually registered).\n\n- agent_call.rs was at the R8 hard LOC cap before this change; the untrusted\n  scraped-text fencing tables and walk (no dependency on `Refusal`/dispatch)\n  move to their own `agent_call/fence.rs`, the same split this file already\n  made for proof/reshape/validate/dispatch_plan.\n\nRefs #1155, #1151, #1154\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): pin the query tier's frame-cap/throttle wiring with tests\n\nRound-2 review (#1155, #1151) found the frame cap and retryAfterMs were\ncorrect in isolation but nothing pinned the CALL SITES that use them, so\na future edit could quietly reopen the connection_lost/instant-retry\nfailure modes with the whole suite green.\n\n- origin_refused_reply now has a direct cap-sized-resource test (it takes\n  no AppHandle); handle_agent_query's async call site is pinned instead\n  by a source-text scan of its own body (this crate has no tauri::test\n  mock-app harness, matching the established precedent in\n  extension_bridge/test.rs and tests/architecture.rs's job_complete scan).\n- mod.rs's two throttled-dispatch arms get the same source-text pin, plus\n  a BridgeState-level test anchoring agent_retry_after_ms to the bucket's\n  own refill rate (30s for best-matches, 1s cheap) instead of `> 0`, which\n  a hardcoded constant or a discarded 0 could both satisfy.\n- the \"unknown agent resource\" refusal now clamps the resource name\n  embedded IN the message, not just the envelope, so a cap-sized unknown\n  resource reports its real cause instead of collapsing into\n  result_too_large.\n- MCP instructions.rs now tells the model rate_limited carries\n  retryAfterMs; the agent-cli.md privacy bullet no longer claims the\n  curated tier's non-throttle refusals are sentinels (they stay prose).\n\nRefs #1155\nRefs #1151\nRefs #1154\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): fix a refusal field's presence, share the tier gate, de-flake a clock test\n\nThe generic call tier's refusal envelope carried retryAfterMs:null on every\nrefusal, not just rate_limited, disagreeing with the curated query tier which\nomits the key entirely when there is no wait. A client keying on presence\nwould instant-retry a non-throttle refusal. Now the key is only inserted for\nRefusal::RateLimited.\n\nlocal_call_refusal's tier gate was a hand-typed second copy of\ncommands_value's gate, with a comment claiming reuse that wasn't real.\nExtracted both to one tier_exposes(tier, effect) in mcp/schemas.rs and added\na test exercising the gate across every POLICY row at Tier::Read and\nTier::Reversible (the exhaustive row test only ran at Tier::Irreversible,\nwhere the gate is always open by construction).\n\nbridge_state_agent_retry_after_ms_reads_the_same_bucket_try_acquire_agent_drew_from\nasserted an exact wall-clock-derived value with under 1ms of headroom against\nreal scheduler preemption. Now asserts a range.\n\nRefs #1155, #1154, #1151\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent-cli): hand-write the tier-gate test's expectation\n\nthe_tier_gate_agrees_between_commands_and_local_call_refusal_at_every_tier\nderived its expected gate_open from tier_exposes, the very function both\ncall sites under test route through, so it could never disagree with a\nbroken shared gate. Spell the tier->effect gate out by hand instead, so a\nregression in tier_exposes has an independent value to diverge from.\n\nMutation check: replacing tier_exposes's body with `true` now fails this\ntest (previously stayed green, only the three pre-existing literal tests\ncaught it).\n\nRefs #1154\nRefs #1151\nRefs #1155\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): fence by origin and give spend proofs a grace window\n\nFences the job_posting tag onto only third-party scraped text: autopilotName,\nnotification title/body deviation aside (see below), the changelog and the\nTauri invoke-error detail leave the fence (the last two capped, not wrapped).\nLong user-authored document text (documents_list.text, documents_get_text,\nresume_extract_text) moves to a distinct user_document tag, chosen by shape\n(DocumentRecord/resume_extract_text anchors), never by field name alone, so\na scraped JobPosting.title/company/location or an imported profile's text\nstill defaults to job_posting. notifications.title/body stay fenced by\ndefault: several producers (status_update, import_flow, reminder_scheduler,\nresume_pipeline::notify) interpolate a scraped job title/company into that\ncopy, so this is a genuinely mixed field per the issue's own escape valve.\n\nAdds a per-process, TTL-bounded snapshot of the proof value disclosed by a\nconfirmation_required refusal (agent_call/proof.rs), so a caller that reads\nai_spend_summary and pastes today.inputTokens back is no longer refused just\nbecause background AI spend moved the counter in between. A value matching\nan expired snapshot gets a distinct confirmation_mismatch detail telling the\ncaller to re-read, rather than the generic \"you guessed wrong\" text.\n\nRefs #1157\nRefs #1162\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): bind the confirm grace window to its one target and defuse error text\n\nRound-3 security review confirmed 11 findings against #1162's confirm grace\nwindow and #1157's fence-by-origin change:\n\n- the grace window is now eligible ONLY for ai_spend_summary-backed proofs\n  (the one Scalar source with no per-target caller input at all), never keyed\n  by command name alone, closing the cross-target authorization bypass\n- a matching snapshot is consumed on accept, so one disclosed proof buys\n  exactly one irreversible dispatch, not unlimited ones for the whole TTL\n- a direct read of ai_spend_summary now refreshes the snapshot, closing the\n  double-drift gap where the counter moves both before and after disclosure\n- InvokeError's unfenced detail is now run through\n  neutralize_transcript_boundaries, restoring the boundary defence #1157\n  dropped along with the label\n- fence.rs's DocumentRecord title exemption is ANDed with !job_posting_shaped\n  so a forged extra map can never borrow it, and DocumentRecord title/name\n  are neutralized+capped rather than fully raw\n- cap_autopilot_name neutralizes forged transcript boundaries again\n- proof::resolve mirrors reshape_reply's second fencing step\n  (fence_user_document_bare_text), and the MCP instructions no longer claim\n  title/company/location/description are always third-party by name\n\nRefs #1157, #1162\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): stop truncating documents, tighten fencing, and consume proofs on match\n\n- documents_get_text no longer silently caps a user's own document at\n  RESUME_CAP chars; enforce_frame_cap's own result_too_large refusal\n  bounds the reply instead of content being cut with nothing on the wire.\n- fence_named_fields_recursive's user_document relabel now ANDs with\n  !job_posting_shaped, closing the same forged-anchor hole AC-3 already\n  closed for the DocumentRecord exemption (a JobPosting's flattened\n  extra carrying a confidence key could relabel board-authored text).\n- proof::accepted_at now consumes a snapshot on its exact-match fast\n  path too, not only the snapshot-match branch, so one disclosure can\n  no longer buy two dispatches when the live counter returns to the\n  disclosed value between them.\n- notifications::AppNotification.title/body now fence under a new,\n  distinct app_notification tag instead of job_posting, matching\n  #1157's own remedy for a genuinely mixed-provenance field.\n- Two tests sharing the fixed ai_spend_summary snapshot key now\n  serialize behind proof::GRACE_WINDOW_KEY_TEST_LOCK instead of racing\n  the process-global snapshot map under concurrent scheduling.\n- The per-target grace-window guard test now seeds its probe under the\n  key a regressed, read-command-keyed implementation would actually\n  consult, so it fails if that eligibility gate is ever widened.\n- A new POLICY-scanning test pins that every ai_spend_summary-backed\n  Irreversible row proves on the same path the shared snapshot tracks.\n\nRefs #1157, #1162\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): strip app_notification wrapper inbound and pin its proof tag\n\nRound-3 AC-7 taught the outbound fence walk to tag a notification's\ntitle/body app_notification instead of job_posting, but the inbound\nunfence mirror still stripped only job_posting for those two field\nnames -- a caller echoing a fenced notification title back into a\nwrite persisted the literal wrapper into the user's own store. Both\nstrips now run (a no-op on the other tag's wrapper, same as the\nexisting text field's origin split).\n\nThe proof-vs-fencing regression test built its ListMatch fixture\nwithout the createdAt/read anchor a real notifications_list row\nalways carries, so it never actually exercised the app_notification\nbranch; production always saw a non-notification-shaped fixture and\nthe test's hardcoded job_posting expectation happened to match. The\nfixture now carries that anchor for a notifications_list-backed row,\nand the expected tag is derived per-row so the fenced branch asserts\nthe tag production actually emits.\n\nAdded agent_call::reshape::EMITTED_FENCE_TAGS, the surface's own\nhand-audited list of every tag it can pass to prompt_fence::fenced,\nand a test pinning that mcp instructions documents each one -- the\ncrate-wide EXPECTED_FENCE_TAGS registry covers tags this surface never\nemits and can't stand in as its coverage source.\n\nMoved proof.rs's inline test module to proof/tests.rs (same split\nvalidate.rs already uses) -- the fixture/assertion additions pushed it\npast R8's hard LOC cap.\n\nRefs #1157, #1162\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent-cli): pin job_posting winning notification_shaped on a forged anchor\n\nfence_named_fields_recursive's notification_shaped disjunct is ANDed with\n!job_posting_shaped in production (A3-r2-AC-7), the same discipline\ndocument_record_shaped and user_document_shaped already get a dedicated\nadversarial test for -- but nothing pinned the notification case. Adds a\ntest forging createdAt+read onto a real JobPosting-shaped object and\nasserting title/body still fence under job_posting, never relabelled\napp_notification. Mutation-checked: removing the !job_posting_shaped\nguard turns this red (confirmed locally, then reverted).\n\nRefs #1157, #1162\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(shared): pin the --check flag the agent catalogue ci gate depends on\n\nThe drift-gate guard test asserted both package scripts mention the\nwrapper filename but never that gen:agent-catalogue:check passes\n--check -- the one flag that makes the generator refuse (exit 1) a\nstale catalogue instead of silently rewriting it. Dropping --check\nwould leave ci exiting 0 against drift, and nothing caught it.\n\nRefs #1163\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-call): stop double-fencing a listed field name nested in extra\n\nfence_named_fields_recursive's extra-field catch-all already fenced an\nArray/Object-valued JobPosting.extra key leaf-by-leaf via\nfence_all_string_leaves; the trailing name-keyed recursion then walked\nthe same subtree again and re-fenced any inner key that also happens\nto be on FENCE_FIELD_NAMES (e.g. salaryDetail.description), leaving a\ndouble <job_posting> wrapper that unfence_named_fields_recursive's\nsingle strip cannot fully undo. Track which extra keys were already\nfenced and skip them in the trailing walk.\n\nAlso adds a source-scan test asserting fence.rs/reshape.rs's own\nliteral fenced(...)/strip_fence_wrapper(...) tag arguments match\nEMITTED_FENCE_TAGS exactly, so a new tag can no longer go undocumented\nin mcp::INSTRUCTIONS without reddening a test first.\n\nRefs #1157\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent-cli): pin catalogue arg names and required flags to the handler\n\nNothing before this compared the generated CATALOGUE -- assembled from\nthe TS tauri-client, a different source of truth -- against the Rust\ntauri::command handler signature validate::check_input actually gates\ndispatch on. Existing tests pin table membership and wrapper class\nonly, never an argument's name or required flag, so drift there would\nsilently convert a valid agent call into invalid_input or drop a\nrequired-key refusal.\n\nByte-scans every tauri::command handler in src for its own\nnon-injected parameters (name converted snake_case -> camelCase,\nrequired derived from Option<..>) and asserts the set matches each\ncatalogued command's declared args exactly.\n\nRefs #1163, #1158, #1160\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(extension-bridge): widen the retry-after-ms wiring pin to the call site\n\nThe existing source-text pin only asserted that the call expression\nstate.agent_retry_after_ms(..) appears somewhere in mod.rs, never that\nits result reaches throttled_reply's third argument. A future edit\nthat binds the computed value to an unused local while hardcoding the\nreply call to a literal (the same \"Mutation A\" this test exists to\ncatch) would keep the old needle green. Widen each needle to the whole\nlet-binding-through-call-site statement pair.\n\nRefs #1155\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-call): fence invoke-error's underlying value under a distinct tag\n\nRefusal::InvokeError's detail carried its underlying value fully unlabelled,\ndropping the \"treat as data\" boundary from text that can embed remote,\nattacker-influenced prose (a scrape/HTTP/provider error body) on a surface\nwhose caller holds destructive tools. Wrap only that value under a new\ncommand_error tag, registered with the same cross-tag forgery defence every\nother fence tag gets, and document it in the MCP instructions. The\nexplanatory prose around the value stays unfenced.\n\nRefs #1157\n\n* docs(agent-cli): document the confirm grace window and origin-based fencing\n\nRound-3 security fixes (issue #1162's proof grace window scoped to\nai_spend_summary, issue #1157's shape-based fence tags replacing a\nfield-name-only job_posting wrap, plus the invoke-error command_error\ntag) landed without a docs pass. Add a dated amendment to ADR-038\ncovering the grace window and the fence-tag split, and update\nagent-cli.md's confirmation-ceremony and personal-data bullets to\npoint at the owning constants/tags instead of restating them.\n\nRefs #1163, #1158, #1160, #1155, #1151, #1154, #1157, #1162\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): fence a forged changelog anchor and cap the fence, not the input\n\nF1 (HIGH): changelog_entry_shaped was the only sibling shape flag not ANDed\nwith !job_posting_shaped, so a board-authored JobPosting.extra forging\npublishedAt+prerelease skipped the body fence entirely (extra's catch-all\nexcludes body, and the trailing recursion ignores string leaves).\n\nF5 (LOW): fenced_key tagged a caller's own offending key as job_posting\n(third-party board authorship), when it is app-generated error text; use the\ncommand_error tag introduced for exactly this content class.\n\nF6 (LOW): fence_user_document_bare_text passed usize::MAX as the fence cap,\nso neutralize_transcript_boundaries scanned the whole documents_get_text\nreply before enforce_frame_cap could reject an oversized one. Cap at\nMAX_FRAME_BYTES instead — anything that fits the frame stays untruncated.\n\nO2: the fencing-never-changes-a-proof cross-check now builds a wire-realistic\nDocumentRecord fixture (via a_document_record) for documents_list-backed\nrows too, matching the notifications_list treatment already in place.\n\nO3: the grace-window regression guard now matches on the whole ProofSource,\nnot only Scalar, and panics on any other variant naming ai_spend_summary\ninstead of silently skipping it.\n\nRefs #1183\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): key catalogue description equality on namespace, not command\n\nF2 (MEDIUM): the duplicate-call-site guard keyed BOTH the description- and\nargs-equality checks on the bare command name, which forced two namespaces\nsharing a dispatched command (boards.disconnect / linkedin.disconnect ->\nboards_logout) to publish byte-identical TSDoc or fail codegen; linkedin.ts's\nown wording had been genericized to boards.ts's to satisfy this, degrading\ndocs/API.md's LinkedIn-specific documentation.\n\nSplit the merge logic into mergeCatalogueEntry: args-equality stays keyed on\nthe bare command (the shape check_input actually enforces really is\nsingle-valued), while description-equality is now keyed on (namespace,\ncommand) — first-namespace-wins still decides which description is\npublished when two legitimately differ, cosmetic only since nothing\ndownstream validates against it. Restored linkedin.ts's original TSDoc\nwording and regenerated the catalogue + docs/API.md (byte-identical shard\ncontent; boards.ts still wins the published description by sort order).\n\nO1: shard packing verifies the REAL post-rustfmt entries-only line count\nagainst SHARD_LINE_BUDGET (re-deriving it from the formatted file rather\nthan a fragile empty-shard baseline, since rustfmt collapses an empty\narray literal to one line) and fails loudly on drift instead of silently\nshipping an oversized shard; corrected the struct_lit_width doc comment,\nwhich overclaimed uniform governance across CatalogueEntry and the nested\nCatalogueArg literals that actually stay single-line when short.\n\nRefs #1183\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(scripts): pin gen-api-docs.mjs's direct-invocation entry point\n\nF3 (MEDIUM): this PR removed gen-api-docs.mjs's self-invocation guard and\nsplit the CLI into gen-api-docs.cli.mjs, but shipped no test for it, unlike\nthe identical fix already pinned for gen-agent-catalogue.ts. Mirrors that\nfile's own \"CI drift-gate entry point\" test pair: no argv/import.meta.url\nself-invocation comparison remains, and both gen:api scripts route through\ngen-api-docs.cli.mjs.\n\nRefs #1183\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent-cli): pin the one destructive allowlisted-uncatalogued row\n\nF4 (MEDIUM): privacy_clear_data is Effect::Irreversible yet sits on\nALLOWLISTED_UNCATALOGUED, the one dispatchable row agent_call::validate's\ngenerated-catalogue key checking cannot reach at all — the allowlist's own\ndoc framed every entry purely as \"zero renderer references\", without\ncalling out that this one is also destructive.\n\nAdd a hand-written ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE and assert both\ndirections: every ALLOWLISTED_UNCATALOGUED row whose POLICY Effect is\nIrreversible appears in it, and vice versa, so adding a future destructive\nuncatalogued command must be a deliberate, reviewed edit to this list.\n\nRefs #1183\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* test(agent-cli): pin the f6 frame cap and scope command_error's actionable claim\n\nreshape.rs's fence_user_document_bare_text passed MAX_FRAME_BYTES as fenced()'s\ncap with no test that would fail if it regressed to usize::MAX (mutation-checked:\nthe existing under-cap fixture passes identically either way); adds an over-cap\nfixture that pins the cap value at the call site.\n\nAlso scopes INSTRUCTIONS' command_error \"read the key/argument names as\nactionable\" claim away from a key name that only echoes the caller's own\ncall input (validate.rs's unknown-key refusal), since that text is never a\ntrustworthy fix suggestion the way an app-declared name is.\n\nRefs #1157, #1163\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n* fix(agent-cli): reconcile the merged policy/spend-count tests and the r8 cap\n\nupdater_status has no renderer invoke() call site (its only consumer is\nthis agent surface's own updater_install confirm proof), so it needs the\nsame ALLOWLISTED_UNCATALOGUED entry boards_list/privacy_clear_data\nalready have; give notifications_mark_read/markAllRead the TSDoc the\ncatalogue coverage test expects for a described Irreversible row.\n\nai_spend_summary's grace-window proof count drops from 10 to 9: issue #1169\nmoved help_search off Irreversible, taking its charge_provider_daily proof\nread with it. Regenerate the catalogue/API docs for the new TSDoc.\n\nSplits mcp.rs's reply-shaping (tool_result/oversized_result/busy_result/\nshutting_down_result) into mcp/results.rs, the same R8 LOC-cap move\ninstructions.rs/schemas.rs already made -- merging PR #1182 pushed mcp.rs\n11 lines over the hard cap.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01JnPnL9VTrHnv4syNEC4wVn\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-09T03:46:53+02:00",
          "tree_id": "5707cac1838dd83f4e7aeb40372b0750ba34fa76",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/2be376971d60a0859d73af47d6e499d51ab80fe2"
        },
        "date": 1788919839044,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2207462,
            "range": "± 12243",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2623838,
            "range": "± 30617",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 312810,
            "range": "± 3859",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "22b21751377c9f20b9ec59c2a19d1a7078794fd6",
          "message": "feat(release): automate microsoft store submission via msstore cli (#1193)\n\n* feat(release): automate microsoft store submission via msstore cli\n\nAdds a publish-msstore job that uploads and submits the built MSIX via\nthe msstore CLI, authenticating with an Entra app registration (Manager\nrole) linked to Partner Center. Closes the manual-submission gap noted\nin ADR-049.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RPd8KiANfeysL8tL7MTKsx\n\n* docs(readme): add microsoft store badge to installation section\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RPd8KiANfeysL8tL7MTKsx\n\n* feat(release): add snap store + flathub distribution alongside msix\n\nExtends the packaged-build concept (ADR-049) from MSIX-only to also\ncover Snap and Flatpak on Linux: self-updater skip, native-messaging\nregistration skip (a disclosed limitation — no clean solution exists\nfor either sandbox), and flavour-aware \"why can't I update\" copy\nend-to-end through the wire contract, renderer, and both locales.\n\nAdds publish-snap (single-store re-runnable, edge channel only) and\nupdate-flathub (gated on FLATHUB_DEPLOY_KEY until the pending Flathub\nsubmission is reviewed) release jobs, plus the Snap/Flatpak packaging\nmanifests. Flatpak's vendored cargo/node sources were generated and\nvalidated against the real lockfiles; a known open risk (pnpm's\noffline install still touching the registry during frozen-lockfile\nverification, a real upstream gap) is disclosed inline pending\nverification against Flathub's actual buildbot.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RPd8KiANfeysL8tL7MTKsx\n\n* fix(release): address coderabbit findings on msix/snap/flathub pr\n\n- release.yml: install msstore-cli from its actual tarball release\n  (not a nonexistent dotnet tool), checkout the resolved release tag\n  instead of mutable main before packaging snap/flathub, pin the\n  flatpak generator scripts to a reviewed commit sha with checksum\n  verification, and sync the flathub appstream release entry\n- snapcraft.yaml: wire the desktop file into the autostart key so\n  snapd honors tauri_plugin_autostart's xdg entry\n- metainfo.xml: add the flathub-required <releases> element\n- sync-snapcraft.cjs: accept pre-release versions in the version regex\n- flatpak.rs/snap.rs: drop the two environment-dependent tests that\n  assert ambient host state and would fail inside a real sandbox\n  (reviewed by rust-backend-architect, approved)\n- docs: fix thin-pointer violations, wrong job/script attribution,\n  and swapped snap/flatpak flatpak-spawn attribution in adr-049\n  (project-steward)\n\n---------\n\nCo-authored-by: Claude Sonnet 5 <noreply@anthropic.com>",
          "timestamp": "2026-09-10T08:41:13+02:00",
          "tree_id": "e7d54c4f54ddfd362287654adfe5bf4f131163b9",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/22b21751377c9f20b9ec59c2a19d1a7078794fd6"
        },
        "date": 1789023927784,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2207111,
            "range": "± 34281",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2627426,
            "range": "± 25011",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 307807,
            "range": "± 3646",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "728c1ce8d9a8589b73ef2ac2d3887505e034595c",
          "message": "fix(release): correct flathub app id and two linter-blocking manifest bugs (#1194)\n\nflathub's linter derives a github url straight from any io.github.* app\nid (underscores to dashes, nothing else) and requires it to resolve,\nwith zero exceptions granted. io.github.saeedkolivand.AIJobHunter\ndemangled to github.com/saeedkolivand/aijobhunter, which 404s — the\nreal repo is ai-job-hunter-app. renamed to\nio.github.saeedkolivand.Ai_Job_Hunter_App, which demangles correctly.\n\nfound via the first real flathub submission's test-build failure log\n(flathub/flathub#10161), alongside two more real bugs fixed in the\nsame pass: a redundant --talk-name=org.freedesktop.portal.Background\nin finish-args (flathub rejects any explicit talk-name to\norg.freedesktop.portal.*, since portal access is granted by default —\nno exception exists for this class), and the icon source path\n(../../icons/icon.png doesn't resolve once update-flathub copies files\nflat into the external flathub repo — added a committed sibling copy\nand wired it into both the manifest and the ci copy list).",
          "timestamp": "2026-09-10T09:11:32+02:00",
          "tree_id": "804cfb9a672817384d01651ae01c3e5e87f8f35d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/728c1ce8d9a8589b73ef2ac2d3887505e034595c"
        },
        "date": 1789025470128,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1466857,
            "range": "± 81996",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1850184,
            "range": "± 82826",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 207782,
            "range": "± 31048",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "saeedkolivand1997@gmail.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "saeedkolivand1997@gmail.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "distinct": true,
          "id": "89d84df1ca998a1a69f6136df0b706f2f78f2cf3",
          "message": "fix(desktop): pin autostart app_name to kebab-case slug\n\nThe Snap Store's manifest schema rejects a .desktop filename containing\na space, but tauri_plugin_autostart defaulted to productName verbatim\n(\"AI Job Hunter\"), which the Linux autostart XDG file is named after.\nPin app_name to \"ai-job-hunter\" via the plugin's Builder and update the\nsnap manifest's autostart key to match.\n\nCo-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01QGpSqLip51VERGwR6LLqus",
          "timestamp": "2026-09-10T13:50:42+02:00",
          "tree_id": "72f203e039d694543854bc18f3fdbcc4f9fe7991",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/89d84df1ca998a1a69f6136df0b706f2f78f2cf3"
        },
        "date": 1789042498263,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2187879,
            "range": "± 8941",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2570493,
            "range": "± 11042",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 303116,
            "range": "± 9804",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dd5b0d9081c41905a70ab558d1668afc8f7219a5",
          "message": "feat(extension): read-only agent tier + live settings switches (ADR-050) (#1203)\n\n* feat(extension): read-only agent tier + live settings switches (adr-050)\n\nThe paired extension gets its own CallerClass on the bridge: agent.query/agent.call dispatch\nonly Effect::Read rows, only with the Assisted-autofill opt-in on, behind a per-pairing throttle\nand a dedicated 256 KiB reply cap (refuse, never truncate); every other row answers a fixed\nsentinel and never enters the CLI confirm ceremony. The CLI path is unchanged. New settings.get /\nsettings.set verbs let the extension's Settings page flip its own consent switches through the\nsame setters the desktop uses, with a Notification Center entry on every actual change. The\noptions page toggles go live (optimistic flip, rollback, in-flight guard) and the side panel trust\nline reads the followed job's title and company through the curated job resource.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_019MiSvmrSngrTR8dYb9kEPF\n\n* style(extension): format options page markup\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_019MiSvmrSngrTR8dYb9kEPF\n\n* test(ui): move the storybook browser port pin to test.api for vitest 5\n\nvitest 5 deprecated browser.api, so the 6317 pin was ignored and the default 63315 fell inside a\nwindows hyper-v port exclusion block, failing every pre-push run with zero failing tests.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_019MiSvmrSngrTR8dYb9kEPF\n\n* test(bridge): assert the settings.set notification through a pure helper\n\nthe two source-text-scanning tests carried a lone closing brace inside a string literal, which the\negress guard's naive brace counter read as the end of the test module; notification_for() makes\nthe once-per-actual-change rule directly testable instead.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_019MiSvmrSngrTR8dYb9kEPF\n\n* build(deps): bump rustls to the rustsec-2026-0285 fix version\n\ncargo deny rejects the tls 1.3 handshake advisory on every branch until the lock moves; a plain\ncargo update stops at 0.23.43, so the fix version was pinned with --precise, pulling rustls-webpki\nand aws-lc-sys along (all msrv 1.71, no toolchain change).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_019MiSvmrSngrTR8dYb9kEPF\n\n* fix(extension): review round 1 — bounded req ids, serialized opt-in writes, flat wire contracts\n\nBridge: reqId is capped once at the protocol boundary (MAX_REQ_ID_BYTES) with a fixed refusal that\nnever echoes the raw id; the three consent setters hold one write lock across swap + persist and\nreport whether the value changed, so settings.set's compare and write are one critical section.\nShared/extension: agent request schemas model the flat wire shapes, refusal detail/retryAfterMs\nsurvive parsing and normalization, settings.get is strict, success guards require data, dispose()\nsettles in-flight requests, the explicit resource wins over params, the options page surfaces\nsettings failures, and a stale trust-line response is pinned by a test. Docs trimmed to pointers.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_019MiSvmrSngrTR8dYb9kEPF\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-14T23:45:38+02:00",
          "tree_id": "f514df53a4803149168fb399b8cb4d3778031863",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/dd5b0d9081c41905a70ab558d1668afc8f7219a5"
        },
        "date": 1789423785462,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2279048,
            "range": "± 19348",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2661332,
            "range": "± 17193",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 295894,
            "range": "± 11061",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c5940511bbdef8704b64c30739f53311cfdc09fe",
          "message": "feat(extension): documents into ATS — résumé attach, cover-letter paste, deep links, copy fallback (#1204)\n\n* feat(extension): documents into ats — résumé attach, cover-letter paste, deep links, copy fallback\n\nThe paired extension gets a dedicated document.export/document.result verb (Autofill opt-in, its own\nper-pairing throttle, frame-cap refusal) that resolves this job's generation or a base résumé on the\ndesktop and calls the existing export chain with the generation language, letter market and stored\ncontact; a curated documents resource feeds the picker with ids and flags only. ajh://generate and\najh://open join the deep-link allowlist and the menu navigation path. FrameDecision moves out of\nmod.rs for R8 headroom, and a frame-budget test exports a dense résumé through every template.\n\nTEMPLATE_LABELS and LETTER_LAYOUT_LABELS live next to the id unions with a registry parity test; the\nmenu navigation hook lands the two deep links; the shared ./ipc export pointed at a file that never\nexisted and now resolves at runtime.\n\nThe side panel gains a Documents tab (picker, template/layout/format, attach, paste, copy, generate or\nopen in the app). A new attach-file injected entry assigns the file through DataTransfer, re-reads the\ninput and fails closed; the first-time per-site confirmation carries attach copy; a copy-field fallback\nshows profile fields when a fill filled nothing; a real-Chromium test pins the DataTransfer claim.\nADR-0009 is amended to correct the file-upload disclosure, with README, privacy copy, knowledge\npointers and the glossary updated.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_011ocEc45NGuZTYhxdJheNTL\n\n* fix(extension): review round 1 — attach bytes survive injection, exports stop at revocation\n\nThe résumé bytes were handed to the injected runner as a typed array through executeScript args,\nwhich the browser serializes as JSON, so the page received a plain object, built a zero-byte file\nand refused its own verification every time. The base64 string now crosses that boundary untouched\nand is decoded inside the runner, with a round-trip regression test and a real-browser decode check.\nAttach and cover-letter paste both capture their target before the export and abort if the tab,\norigin or followed generation changed while it was in flight, and a late profile reply can no longer\nrepaint the copy-field fallback after a tab switch.\n\nOn the desktop side an export whose pairing is revoked mid-flight is now discarded instead of being\nhanded to the revoked client, keyed off the rotation epoch that moves inside the same lock hold as\nthe revoke broadcast. The locale path and read helpers moved to the configuration layer that owns\nfilesystem paths, a bare scheme no longer parses as a job deep link, and a failed applications fetch\nfalls back to the existing no-match destinations instead of leaving the deep link nowhere.\n\nDocs lose the copied protocol details in favour of pointers, and the contract doc comment that\nduplicated the renderer resolver was trimmed at its source and the page regenerated. Tests gained\nthe assertions they were missing: canonical URLs, the DOCX reply format, and the confirmation result.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_011ocEc45NGuZTYhxdJheNTL\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-15T18:18:14+02:00",
          "tree_id": "dcd1d8599673411e73182a5d466dc2b8e886a4f7",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c5940511bbdef8704b64c30739f53311cfdc09fe"
        },
        "date": 1789490549049,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2210488,
            "range": "± 48574",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2622995,
            "range": "± 38463",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 308342,
            "range": "± 23495",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dbb36fe20af0d7fac23e4dd0725c6f5360a497d0",
          "message": "feat(extension): Check-fit on the page — fit badge, results stamps, salary facts (#1205)\n\n* feat(extension): check-fit on the page — fit badge, results stamps, salary facts\n\nA new batched applied lookup answers a whole results page in one round trip, reusing the single-URL\nresolver's normalization, capped and refusing rather than truncating over the cap, throttled per\npairing, and returning only the status each stamp needs. Salary facts ride the existing match reply\nas optional fields: the desktop extracts a pay range from the posting text it just scored and pairs\nit with the saved expectation, both verbatim, absent unless a real range was found, and never a\ncomparison or a verdict.\n\nOn the page, a fit badge shows the score with its source qualifier and a saved or applied chip,\nexpanding on click to the missing keywords, the salary facts and a single link into the panel, and a\nresults page can be stamped with what is already saved or applied. Both render inside a closed shadow\nroot so the page cannot read the score, keywords or salary back out, both are off by default behind\nthe appearance toggles PR0 left hidden, and neither offers any form action. A palette module carries\nthe design tokens into the injected scripts, with a parity test against the stylesheet.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_011ocEc45NGuZTYhxdJheNTL\n\n* fix(extension): review round — honest salary ranges, no auto-track from a results page\n\nThe pay-range pattern let an ungrouped second bound stop after three digits, so a posting offering\n100,000 to 120,000 was shown as a range ending at 120 — a fabricated figure in a surface whose whole\npremise is quoting the posting verbatim. The number pattern now splits into a grouped branch that\nrequires a separator and a bounded raw-digit branch, and a candidate whose match ends against another\ndigit is rejected as truncated rather than returned. The rule that a saved salary expectation never\ntravels without a posting range is now a pure function with its own tests.\n\nStamping a results page armed the auto-track submit watcher on that page, where a later submit-like\ninteraction could mark a saved application as applied from a search page the user never applied on.\nStamping is read-only, so it no longer arms anything. The badge also resolved the active tab three\nseparate times and rendered after the round trip, which could paint one page's score onto another;\nthe tab and URL are now captured once and re-verified before the badge is drawn.\n\nThe expand control exposes its state to assistive technology, the shared request schema enforces the\nsame cap the desktop does, two tests that could not fail were repaired, and the comments and docs no\nlonger claim the closed shadow root hides that a card matched, only what the marker says.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_011ocEc45NGuZTYhxdJheNTL\n\n* refactor(extension): move salary parsing to the domain layer and pin the batch cap\n\nThe salary parser was pure text extraction living in the bridge, which the layer rules ask to stay\nthin, so it moves to the domain extraction module that already owns pulling structured values out of\ntext. The regex, its percentage and truncation rejections, the clamp and all its tests move verbatim;\nthe bridge keeps the call and the wire mapping. Nothing there duplicated an existing parser, and the\nbridge module drops back under its size cap as a side effect.\n\nThe shared cap constant claimed in its own doc comment to be pinned by a parity test against the\ndesktop constant, and no such test existed. It does now, modelled on the one guarding the answer\nassist cap, so changing either side alone fails the build.\n\nThe badge's tab check ran before two later awaits, so a navigation in that window could still land a\nstale badge on a different posting. The expected URL now travels into the injected call and the page\ncompares it against its own location as the last step before rendering, strictly, because relaxing it\nwould reopen the single-page-app case the guard exists for.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_011ocEc45NGuZTYhxdJheNTL\n\n---------\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-16T04:50:27+02:00",
          "tree_id": "7ce0e868b12b9bbd86d958a306b0d658bf6cfc23",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/dbb36fe20af0d7fac23e4dd0725c6f5360a497d0"
        },
        "date": 1789528462201,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2192430,
            "range": "± 6537",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2579317,
            "range": "± 8769",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 299027,
            "range": "± 8711",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c8722079dbf926289b24e48dc733b3a9b093ec82",
          "message": "fix(bridge): match a tracked job by posting identity when applied.check misses (#1242)\n\napplied.check resolved a url by exact normalized-key equality, and\nnormalize_job_url strips only a leading www. from the host. A posting saved\nunder de.linkedin.com/jobs/view/<id> therefore never matched the same posting\nopened as www.linkedin.com/jobs/view/<id>/, and a slugged /jobs/view/<slug>-<id>\nnever matched a bare numeric one, so the popup card, the side-panel stage strip\nand the results-page stamps all showed a genuinely tracked job as untracked.\n\nThe exact lookup stays the fast path. On a miss, the url now folds onto its\n(board, id) posting identity through scrape_url::job_identity and the store\nlooks for the most recent application whose own url folds onto the same\nidentity. This is the matcher agent_read::job_is_applied already used to close\nthe same gap for the agent read surface, which applied_check never picked up.\nBoth halves decode unreserved escapes before extracting, matching that\nfunction's symmetric leniency. A board job_identity does not cover behaves\nexactly as before.\n\nThe store side scans a light id/job_url/created_at projection and loads only the\nwinning row, rather than materialising every application with its description\nand answers to answer the question — the batch verb reuses this resolver per\nurl, so a results-page stamp of mostly untracked cards takes the miss path\nalmost every time.\n\nCloses #1214\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-22T23:28:11+02:00",
          "tree_id": "5a1e25f3fd075fae669afc30b92f8085f5ff3b89",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c8722079dbf926289b24e48dc733b3a9b093ec82"
        },
        "date": 1790113519520,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2003004,
            "range": "± 25851",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2366421,
            "range": "± 55796",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 210013,
            "range": "± 8117",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c92785b43daae7293043e38db952ff50efb36e20",
          "message": "fix(extension): tell a dead api key apart from a retriable draft failure (#1246)\n\n* fix(extension): escalate bridge backoff after a failed handshake\n\nThe desktop accepts the transport before it judges our HMAC proof, so a\nstale token produces connect-then-silent-close on every attempt. Resetting\n`backoffIndex` at the top of `attach()` meant that loop re-armed at the\nladder's floor forever — one probe every 500ms instead of settling at the\ndocumented 10s cap.\n\nReset the ladder only where the connection is genuinely usable: the\nno-token attach that reaches `connected`, and a completed mutual handshake.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\n* fix(extension): tell a dead api key apart from a retriable draft failure\n\n`to_draft_failed` collapsed every downstream compose error into one\nsentinel ending \"Please retry.\" — including a provider 401/403, which\n`friendly_api_error` maps to `AppError::Config` and which retrying can never\nfix. The daily provider budget is charged before each attempt, so every retry\nthe message invited also burned quota on a doomed call.\n\nBranch on the typed error and return a second fixed sentinel pointing at\nSettings → AI. Still a fixed string, so the provider's raw error text\ncontinues never to reach the wire.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\n---------\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-23T04:38:11Z",
          "tree_id": "5fc91c0d7277cfe496b6dc8dcfa8735350785e54",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c92785b43daae7293043e38db952ff50efb36e20"
        },
        "date": 1790139774459,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2296775,
            "range": "± 45194",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2805849,
            "range": "± 94540",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 326957,
            "range": "± 5601",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9aa17ab1b86bba99865965b5d6240740d26416b5",
          "message": "fix(extension): clear a stale fit badge, read trailing-currency salaries, drop junk chips (#1247)\n\nThree defects found in the same \"Check fit\" surface.\n\nThe on-page badge never noticed an SPA job-to-job navigation, so it sat on\nscreen showing the previous posting's verdict. A pushState fires no event in\nany world and the isolated world cannot observe the page world's own calls,\nso the rendered badge now polls its captured url and clears itself the moment\nthe page moves. Teardown runs on dismiss and on re-render.\n\nSalary extraction required the currency marker before the first number, so\nthe German convention of a trailing symbol never matched. The pattern gains a\nsecond, trailing-currency branch; a magnitude floor keeps that new branch from\nclaiming bare number ranges, and is scoped to it alone so the pre-existing\nleading-currency path keeps matching sub-10k monthly ranges verbatim.\n\nMissing-keyword chips could carry scraped page chrome — url path crumbs and\nchart axis date labels — as if they were skills. The keyword kernel now drops\nboth shapes, after synonym canonicalization so no real keyword is lost.\n\n\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-23T05:34:53Z",
          "tree_id": "61c302ef5fdfa5e17042c464ef504443cc14fffc",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9aa17ab1b86bba99865965b5d6240740d26416b5"
        },
        "date": 1790142729467,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1381460,
            "range": "± 39816",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1615859,
            "range": "± 80510",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 155374,
            "range": "± 3365",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9bd91fefa972b854cd187e0db19b1b1fd16055ec",
          "message": "fix(extension): keep accepted answers writable, honour instructions, stop cancelled spend (#1251)\n\n* fix(extension): keep accepted answers writable, honour instructions, stop cancelled spend\n\nAccept into field worked exactly once per row. A successful write into an\nempty field never flipped the field's kind, so the next accept still took the\nfill path, whose locator only searches empty candidates — it could not find\nthe field it had just filled and claimed the page had changed. The kind now\nflips on a confirmed write, and a rescan migrates drafts onto the row that\nowns the field instead of orphaning them.\n\nThe free-text instruction was sent and then dropped: chips ignored the typed\nbox, and the draft compose path never read the field at all. Both now reach\nthe model. The instruction is user-typed and untrusted, so it is byte-capped\nand fenced as its own block, with the tag registered so a crafted question\ncannot forge a sibling.\n\nCancelling a Prep draft reverted the UI but still paid for the request. Only\nthe company-brief grounding checked for a cancel that raced ahead of\nregistration; the salary lookup and the web-search notes did not, which is\nexactly the path the report hit. All three now share one named guard.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\n* test(extension): register the new fence tag with the drift guard\n\n`every_registered_fence_tag_is_load_bearing` keeps an independent copy of the\nfence vocabulary so a tag added to the registry cannot go silently untested.\nAdding `candidate_instruction` without updating that copy failed the guard,\nexactly as designed. With it listed, the shared loop now probes the new tag\nwith all three forgery shapes too.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\n* refactor(extension): split answer_assist's payload parsing into its own module\n\nThe instruction work pushed answer_assist.rs to 1471 LOC, past R8's hard cap\nof 1400. The pure, AppHandle-free half lifts out cleanly: clamping, every\npayload parser, the mode enum, and rewrite-field validation are total\nfunctions of the incoming wire value, touching no network, store or registry.\n\n1471 down to 1282. No behaviour change; the architecture test passes on its\nown terms rather than through an allowlist entry.\n\nCo-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\n---------\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-23T10:42:35Z",
          "tree_id": "368caef11a8083b9596744e466fa555d23730687",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9bd91fefa972b854cd187e0db19b1b1fd16055ec"
        },
        "date": 1790161566891,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2227905,
            "range": "± 21541",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2675077,
            "range": "± 58470",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 251787,
            "range": "± 1534",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4d3244b3c1770e8855b50651893548a4da914aa5",
          "message": "fix(extension): stop import claiming success it did not earn (#1252)\n\nTwo ways a capture produced a confident-looking record that described the\nwrong thing.\n\nA posting embedded in a cross-origin iframe is unreachable from the top-level\ncapture, so the parser fell back to the wrapper careers page's own heading and\nreported a clean success. The only hint was a low percent-fit. Import is now\nflagged partial when it extracted no description AND the captured document\nembeds a third-party frame — two signals together, so the analytics and\nconsent frames on ordinary pages never trip it, and a page that parsed a real\ndescription never does either.\n\nScan-mode LinkedIn kept the whole title bar, branding and all, and stored the\nhostname as the employer. The generic fallback now drops one trailing site\nsuffix, and only when that segment matches the og:site_name or the host's own\nlabel, so a title that merely contains a separator survives. When nothing else\nnames the employer it reads the company-logo alt text before giving up.\n\nSplits the generic-extraction half into its own module: the additions crossed\nR8's hard LOC cap, and nothing in that half makes a request or knows a board's\nshape.\n\n\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-23T11:08:58Z",
          "tree_id": "0b2db5d07e352efaa9782691e6cc77c843015d6c",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4d3244b3c1770e8855b50651893548a4da914aa5"
        },
        "date": 1790162414323,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1522503,
            "range": "± 67620",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1920789,
            "range": "± 57963",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 192248,
            "range": "± 12836",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fce24632a740741f5a9d90722d082b1a6f17652c",
          "message": "feat(settings): give the browser extension its own nav section (#1253)\n\nPairing, permissions and the per-feature opt-ins had outgrown a sub-section of\nAccounts, so they move to a top-level Settings entry of their own. Accounts\nkeeps connected boards and email watch.\n\nThe anchor id stays as it was: the tray deep link and any link already handed\nout address that panel by it, and renaming would break them. The tray's target\nsection moves with the panel, and every message that told the user where to go\nnow names the new path, on both sides of the Rust/TS sentinel parity.\n\nTwo drift hazards showed up while doing it and are closed rather than worked\naround. The section union was written out twice, so adding a member to one copy\ntypechecked everywhere except the single line assigning between them; the\nsettings copy is now an alias of the store's. And the anchor drift guard mocked\nthe moved component by its old path, which silently un-mocked it — the new\nsection gets its own guard, plus a count assertion, because `it.each` over an\nempty list registers no tests and would have passed vacuously.\n\n\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-23T11:54:16Z",
          "tree_id": "64fe036e7ea343f79dcdf7fb3ed24b660edfbffa",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/fce24632a740741f5a9d90722d082b1a6f17652c"
        },
        "date": 1790165952024,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2352642,
            "range": "± 21012",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2822031,
            "range": "± 44097",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 252414,
            "range": "± 6847",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5bd90be1510f1c236850728fb495747c41afac43",
          "message": "fix(extension): recognise the embedded board, not just any third-party frame (#1256)\n\nEnd-to-end testing against a real careers page showed the #1238 fix never\nfired. It required that no description was extracted AND that the page\nembedded a cross-origin frame. A careers page has plenty of prose, so a\ndescription WAS extracted - just not the job's - and the import still reported\na confident success naming the company's own heading at a single-digit fit.\n\nThe unit test passed because its fixture was written to match that assumption\nrather than the page.\n\nKey the signal on recognising the BOARD instead, through the existing ATS\nregistry, and drop the description condition. Recognising the board is what\nseparates a wrapper careers page from the analytics, consent and video frames\nthat are cross-origin on nearly every page, so the extra condition was not\nearning its place - it was suppressing the finding.\n\n\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-23T14:43:59Z",
          "tree_id": "96ec10a8d7071b839155ea636bb2eba0bcb60eb1",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/5bd90be1510f1c236850728fb495747c41afac43"
        },
        "date": 1790175905543,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1776430,
            "range": "± 34021",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2106609,
            "range": "± 48133",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 246639,
            "range": "± 7519",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d59cd036d0a601ca68889b9a1b4ef63b35842d09",
          "message": "fix(extension): abandon a cancelled grounding step instead of paying for it (#1257)\n\nEnd-to-end testing showed Cancel still billed. A cancel acknowledged 260ms\nafter the draft began was followed by a web search AND a completion request,\nthe second of which STARTED after the cancel and ran to completion.\n\nThe pre-flight peek only fires when the cancel beats the step's start. Each\ngrounding helper makes several provider calls internally, so a cancel landing\nmid-step passed the peek and paid for everything after it.\n\nAwait the grounding future against the cancel marker and drop it when one\nlands - dropping an in-flight request future is what actually cancels the HTTP\ncall. Contained here rather than threaded through salary_research and the ai\ncommands, which are shared by callers that have nothing to do with the\nextension's cancel.\n\n\nClaude-Session: https://claude.ai/code/session_01MoNkcapcQph3CR4mx2nzGc\n\nCo-authored-by: Claude Opus 5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-23T18:03:49+02:00",
          "tree_id": "14c60ba66bba878c8dc9e4302833d1a7b04ae284",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/d59cd036d0a601ca68889b9a1b4ef63b35842d09"
        },
        "date": 1790180141937,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2188606,
            "range": "± 19430",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2552271,
            "range": "± 33456",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 304015,
            "range": "± 11005",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c42211c7c4d50c2f5970e4ea8a58fbec6353cc39",
          "message": "perf(pipeline): make match_evidence deterministic, with no provider call (#1270)\n\nmatch_evidence asked a model to rank résumé lines per requirement, then Rust\noverwrote the status, clamped the strength and dropped non-verbatim quotes.\nThe call took 13-145 s and re-asked on 23 of 33 logged runs. The stage now\npicks each requirement's best-supporting bullet with the keywords kernel\n(ties to the earlier line, word-bounded fallback for untokenized short terms)\nand is free like validate: listed in PIPELINE_STAGES_FREE, so it can no\nlonger take a per-stage model override, and the JSON-stage call count in\nthe run deadline drops from 6 to 4.\n\nCloses #1269\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-24T13:51:16+02:00",
          "tree_id": "2e6fbd8945e917dbe7209c1fc920a9bcdd45ec6d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c42211c7c4d50c2f5970e4ea8a58fbec6353cc39"
        },
        "date": 1790252103193,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2188765,
            "range": "± 65566",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2539451,
            "range": "± 20636",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 301390,
            "range": "± 2325",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f4a5f35ae087832b0a6d18be6774f649aad2f048",
          "message": "perf(ai): isolate cli-agent calls from user config, schema-constrain claude code (#1273)\n\n* perf(ai): isolate cli-agent calls from user config, schema-constrain claude code\n\nEvery CLI-agent call loaded the user's own agent setup (settings, hooks,\nplugins, MCP servers) into the app's prompts, which leaked the user's\nconfiguration into generation and slowed startup.\n\n- claude code: --strict-mcp-config --setting-sources= --disable-slash-commands\n  (not --bare, which forces API-key auth); --effort from a fixed allowlist\n  with an effort picker; structured calls pass --json-schema and read the\n  validated structured_output, falling back to prompt-only above an argv cap\n- codex: --ignore-user-config --ignore-rules --ephemeral, feature disables\n  via -c features.<x>=false (--disable hard-errors on unknown names) and\n  project_doc_max_bytes=0; an old codex rejecting a flag gets an update hint\n- gemini cli: -e none plus a sentinel mcp allow-list; antigravity has no\n  isolation flags and is unchanged\n- the two one-shot runners share one spawn/parse core\n- cleanup after #1270: drop the unused verbatim module, fix stale stage\n  names in validate/budget docs, the misnamed lock test in ai-timeouts, and\n  docs still describing model-ranked evidence\n\nKnown ceiling: the global ~/.codex/AGENTS.md and ~/.gemini/GEMINI.md still\nload; only a temporary agent home with linked credentials would stop that.\n\nCloses #1271\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* refactor(ai): move cli-agent tests into their own file under the module loc cap\n\nThe isolation work pushed cli_agent/mod.rs to 1509 lines, over the R8 hard\ncap of 1400. The test module moves to cli_agent/tests.rs unchanged.\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-24T16:47:45+02:00",
          "tree_id": "a4fc989e1f830ccf36a614a49db4b78ea09e6e1e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f4a5f35ae087832b0a6d18be6774f649aad2f048"
        },
        "date": 1790262469393,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2010385,
            "range": "± 22983",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2387620,
            "range": "± 43953",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 211522,
            "range": "± 892",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dc3556bb941f5957848004f14a4fe4f033a974e3",
          "message": "feat(ai): opencode, cursor and qwen code as cli-agent providers (#1275)\n\n* feat(ai): add opencode, cursor and qwen code cli-agent backends\n\nTools are denied through a config file written into a private per-agent\nworkspace under the app data dir, refusing symlinks and junctions on every\npath component. opencode's deny-all agent uses the v2 `agents` key: with the\nv1 `agent` key the definition is ignored and the shell tool runs, verified\nlive. Cursor and Qwen are kept here for reference only and are removed in the\nnext commit until their tool denial is verified against the real CLIs.\n\nRefs #1272\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* fix(ai): keep opencode zen free models working while refusing every tool call\n\nAny deny rule removes tools from what the model is offered, and OpenCode\nZen's free tier then refuses the request, so the deny-all agent made Zen free\nmodels unusable. A top-level ask-everything rule keeps the tool set intact\nwhile headless `opencode run` declines every call. Verified live on 2.0.16: a\nnormal prompt answers, and injected shell-write and file-read requests are\ndeclined. --auto would approve ask rules, so a test pins that argv never\ncarries an auto-approve flag.\n\nThe workspace dir is now wiped and recreated before every spawn, so a stale\nfile the writer doesn't own (an opencode agent definition, a plugin) can't\noverride the config. Also: stream the real opencode error event, keep the\nlatest text per part id in one-shot output, run the workspace symlink checks\non Windows via junctions, and fix the unix test's nonexistent symlink_dir.\n\nRefs #1272\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* fix(ai): give each cli-agent spawn its own workspace and make qwen tool denial fail closed\n\n- Every spawn now gets a fresh <provider>/<run id> dir, removed when the child\n  exits. The shared dir let one generation delete another's config before the\n  CLI read it, and for opencode a missing config means the shell tool runs.\n- Qwen's tool denial moves to its SYSTEM settings via\n  QWEN_CODE_SYSTEM_SETTINGS_PATH (overrides user and project settings and\n  still applies in an untrusted folder, where project settings are ignored),\n  with permissions.deny plus tools.exclude over every built-in and bridge tool.\n- Cursor and Qwen text parsing repeated the first text block; both now use one\n  shared text_blocks helper.\n- No one-click install (or npm warning) for an agent without an install\n  command; Qwen gets a documented fallback model.\n- Workspace errors name the file, never the full path.\n\nRefs #1272\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-24T20:30:20+02:00",
          "tree_id": "cac82173a69b24e7a1e3923d3f6857d64a958aff",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/dc3556bb941f5957848004f14a4fe4f033a974e3"
        },
        "date": 1790276105758,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2397447,
            "range": "± 85612",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2905826,
            "range": "± 162884",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 316552,
            "range": "± 6836",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ae118911f35631beec7e21409a21b6a90846479d",
          "message": "fix(data): write json stores atomically and never overwrite a corrupt autopilots file (#1276)\n\n* fix(data): write json stores atomically and never overwrite a corrupt autopilots file\n\nautopilots.json was saved with a bare fs::write, so a kill between the\ntruncate and the last byte left it truncated or zero-filled, and loading a\nfile that failed to parse as a whole silently yielded an empty list that the\nnext save wrote over, destroying the only recoverable copy.\n\n- platform::fs::write_atomic: sibling temp file, sync_all, rename; the temp is\n  removed on any failure and the original is never written in place. Used by\n  autopilots, postings (#775, gains the fsync), notifications, credential\n  meta, board-login cookies, crash-reporting settings, the extension opt-ins\n  and token, and the locale file.\n- Autopilot load moves a corrupt file to the first free\n  autopilots.json.corrupt[.N] slot (older backups are never overwritten, so a\n  second incident can't lock the store) and blocks saves only when no slot is\n  free. A file that is merely unreadable (sharing violation, permissions) is\n  not corruption: it stays in place and saves are blocked for the session.\n\nCloses #1274\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* fix(data): keep paths out of the autopilots load error logs\n\nThe read-error log interpolated a raw io::Error, which can carry the absolute\npath; it and the corrupt-backup log now go through sanitize_reason. The\nallowlisted per-record parse warning moved to autopilot/corrupt.rs, so its\nentry in the log-leak guard moves with it.\n\nRefs #1274\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* fix(data): unique temp file per atomic write and retry an unreadable autopilots file\n\n- write_atomic now uses a temp name unique to each call. With one shared\n  <name>.tmp, overlapping saves of the same file (the autopilot scheduler and\n  an IPC update) collided: on Unix one writer truncated a temp that had already\n  become the live file, and on Windows 60-73 of 160 concurrent writes failed\n  (measured; zero with unique names). A new threaded test asserts every write\n  succeeds and the file is one complete payload.\n- An unreadable autopilots.json is retried briefly before giving up, and a\n  blocked outcome is never cached, so the file is read again once it is\n  readable. A save blocked by it is not cached either, so the change visibly\n  doesn't stick instead of vanishing on restart.\n- Tests: the non-UTF-8 branch; recovery once the file is readable; a blocked\n  save not served from memory. Two corrupt-file tests had raw NUL bytes in\n  the source, making git treat test.rs as binary; they are \\0 escapes now.\n\nRefs #1274\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* test(data): inject write failures instead of blocking a fixed temp name\n\nwrite_atomic's temp name is unique per call now, so a test can no longer\nmake a write fail by putting a directory at <name>.tmp. The save-answers\nopt-in persist-failure test did exactly that and started passing the write.\nA cfg(test), thread-local fail_next_write_on_this_thread() switch makes the\nnext write fail before touching the disk, and the test uses it.\n\nRefs #1274\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* refactor(data): move the autopilot save-guard helpers next to the load logic\n\nautopilot/mod.rs reached 1401 lines, one over the R8 module cap. The\nblock_save accessors belong with the load outcome that sets them.\n\nRefs #1274\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-24T21:58:44+02:00",
          "tree_id": "febd9a94fe8a41c1512a12e0c8f08d4d7b538a92",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ae118911f35631beec7e21409a21b6a90846479d"
        },
        "date": 1790281001891,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1503490,
            "range": "± 109407",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1839149,
            "range": "± 69591",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 198291,
            "range": "± 17499",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "14be1f08d555d6d47d3ca53103fee98ce7b83e92",
          "message": "feat(updater): back up user data before installing an update (#1279)\n\n* feat(updater): back up user data before installing an update\n\nAtomic writes (#1276) make a kill mid-save safe, but not the new version\nitself damaging data through a bad migration or a bug. Right before\nupdate.install, the full backup bundle (the same one Export writes, so the\nexisting Restore reads it) is saved compactly to\n<data dir>/backups/pre-update-<version>-<date>.json, and older pre-update\nbackups are pruned to the newest three. Files the user put in backups/ are\nnever touched. The bundle is built off the async runtime.\n\nA failed backup is logged and the install goes ahead: holding back an update\nalso holds back its security fixes, and the app restarts right after, so a\nstatus message would never be seen.\n\nCloses #1278\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* fix(updater): keep pre-update backups in their own folder\n\nPruning matched pre-update-*.json directly in backups/, so a user file named\nlike pre-update-notes.json could be deleted. Backups now go to\nbackups/pre-update/, which only the updater writes, so pruning can never reach\na file the user put in backups/.\n\nRefs #1278\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-24T23:05:36+02:00",
          "tree_id": "b517ad979c9ae2a8645bae7c4dfdea03faa99888",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/14be1f08d555d6d47d3ca53103fee98ce7b83e92"
        },
        "date": 1790285228479,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2006689,
            "range": "± 46706",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2344840,
            "range": "± 14752",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 210098,
            "range": "± 3742",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "65dfb7a3c75b9be76fc5724cb948fdc7f67e266e",
          "message": "perf(autopilot): store found jobs in sqlite instead of rewriting them into autopilots.json (#1281)\n\n* perf(autopilot): store found jobs in sqlite instead of rewriting them into autopilots.json\n\nFound jobs were over 99% of autopilots.json (34 MB of about 6,700 job\ndescriptions on one machine), rewritten on every autopilot change after a\nfull re-read. They now live in autopilot_found_jobs.db, one row per job keyed\nby (autopilot id, position), and autopilots.json holds only the autopilots.\n\n- Autopilot::found_jobs keeps its in-memory and IPC shape: no reader changes.\n- Saves are diffed against a per-row hash kept in memory, so a status change\n  writes no found-job rows.\n- A sync never deletes rows of an autopilot it wasn't given: a corrupt\n  autopilots.json that loads empty can't wipe the found jobs. Rows go only via\n  delete, restore and factory reset.\n- Legacy files migrate on load: rows are committed before the JSON is\n  rewritten without them, so a crash reruns it; the original is kept once as\n  autopilots.json.pre-sqlite. If the database can't open or a write fails,\n  found jobs stay in the JSON as before.\n- write_to_disk moves to autopilot/persist.rs; the new store is registered\n  under R3.\n\nRefs #1277\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* fix(autopilot): never let a failed found-jobs read wipe the stored rows\n\nIf reading autopilot_found_jobs.db failed, the autopilots hydrated with empty\nfound jobs and the next save synced those, trimming every stored row: one\nread error became a permanent wipe. A failed read now marks the table\nunreadable for the session, so found jobs ride in autopilots.json instead and\nthe untouched rows hydrate on the next clean load.\n\nRestore now deletes the old rows and writes the new ones in one transaction,\nso a crash can't leave neither. Tests for the failed read, a failed row write\n(JSON keeps the jobs and the next load migrates them) and a failed restore\n(fault injected after the delete, before the commit). The persistence doc no\nlonger lists the found-jobs table as its own DataStore.\n\nRefs #1277\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-24T23:43:30+02:00",
          "tree_id": "5d796b2f3c06b6df6acb84ff786b1195840937fd",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/65dfb7a3c75b9be76fc5724cb948fdc7f67e266e"
        },
        "date": 1790286827896,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1422900,
            "range": "± 96673",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1685377,
            "range": "± 69181",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 180796,
            "range": "± 12816",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "76986bf37a46734f7214dcb4f0c33b56ee83be0b",
          "message": "feat(ai): prompt audit — native anthropic structured output, de-dated prompts and agent config (#1283)\n\n* chore(agents): fix dated and contradictory rules in the claude config\n\n- critics: tie-break rule now matches token-efficiency (was \"down\" in one section, \"up\" in another)\n- commands: subagent tool is Agent, not Task; move the dormant webgl /gate and /review-webgl commands to .claude/dormant\n- frontend skills/agents: @ajh/ui exports Dropdown (no SelectDropdown); file placement points at AGENTS.md rule 9\n- tauri/coding standards: IPC flow matches AGENTS.md rule 14 (query-client/ dir + pnpm gen:api)\n- testing-rules: repo is on vitest 5; drop session-relative history asides\n- project-steward, docs-standards, CLAUDE.md: drop migration-relative archaeology\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* chore(agents): refresh eu ai act and owasp top 10 status in skills\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* feat(ai): native structured output on anthropic and de-dated prompts\n\n- anthropic: complete_structured sends output_config.format (json_schema, GA) with per-tier effort\n- anthropic: falls back to prompt discipline off the supported models or without a closable schema\n- anthropic: web_search_20260209 on opus 5/4.8/4.7/4.6 and sonnet 5/4.6\n- anthropic: wire-format parsers moved verbatim to anthropic_wire.rs (R8 line cap)\n- prompts: examples no longer invent metrics or technologies\n- prompts: drop duplicate scoring rubric, made-up claims, grader vocabulary, plan scripts, CRITICAL shouting\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* fix(ai): keep unsupported schema constraints off anthropic structured output\n\n- reject minimum/maximum/exclusive*/multipleOf/pattern/maxItems, minItems > 1 and undocumented formats\n- such schemas fall back to prompt discipline instead of 400ing the generation\n- resume examples now state no more than their source bullets\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-25T05:29:22+02:00",
          "tree_id": "ef5182cad422cf073be775ecb92007ccda72ed3a",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/76986bf37a46734f7214dcb4f0c33b56ee83be0b"
        },
        "date": 1790308441742,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2224107,
            "range": "± 41713",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2673626,
            "range": "± 38655",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 317677,
            "range": "± 5904",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "40b66e90c9dba20eedb0dd90f0757c0e7aff6e4b",
          "message": "chore(arch): cap rust files at 300 lines and require sibling test files (#1284)\n\nR8 now caps every .rs file under src (tests included) at 300 lines. Files\nalready over the cap are ratcheted in tests/r8_baseline.txt: they may\nshrink, never grow, and stale entries fail. R8b forbids inline\n#[cfg(test)] mod bodies; tests go in a sibling file wired with #[path].\nExisting inline bodies are ratcheted in tests/r8b_inline_tests_baseline.txt.\nR8_BLESS=1 regenerates both after a split. Rule 18 added to AGENTS.md.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-25T19:35:59+02:00",
          "tree_id": "1851bda07f7e9d536c62f53a186c91156f542478",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/40b66e90c9dba20eedb0dd90f0757c0e7aff6e4b"
        },
        "date": 1790359196631,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2214775,
            "range": "± 65170",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2675145,
            "range": "± 45048",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 315587,
            "range": "± 15887",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "177c12e4c039ea42820c204384c483611355dce3",
          "message": "refactor(agent-cli): split agent_cli into modules under 300 lines (#1285)\n\n* chore(hooks): scope review-gate regex fallback like the ast-grep rules\n\nThe fallback checked every .rs file and matched inside comments, so it\nflagged integration tests under tests/ and doc comments that merely\nmention a rule. It now only scans src-tauri/src and blanks comment lines\nfirst (line numbers preserved), matching the ast-grep rules it stands in for.\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* refactor(agent-cli): split agent_cli into modules under 300 lines\n\nSplits extension_bridge/agent_cli.rs and agent_cli/** (policy, mcp, http,\nstdio and their test suites) by responsibility so every file is under R8's\n300-line cap. Behaviour-preserving moves: same 296 tests, same 168 POLICY\nrows in the same order, source-scan tests read the concatenation of the\nfiles split out of mcp.rs/policy.rs so they still cover the same code.\n\nTests use Rust's standard out-of-line layout (foo.rs -> foo/tests.rs, large\nsuites as foo/tests/<topic>.rs). R8b now also flags #[path]-wired test\nmodules (the existing ones are baselined), and files under a tests/\ndirectory count as test code for the other arch rules.\n\nThe catalogue generator packs shards to a smaller budget and only imports\nCatalogueArg when a shard uses it.\n\nRefs #1280\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-25T23:39:12+02:00",
          "tree_id": "186a7271166fe5ab5d6424913abe3120a9c68b17",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/177c12e4c039ea42820c204384c483611355dce3"
        },
        "date": 1790373827884,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2178828,
            "range": "± 54484",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2578509,
            "range": "± 30085",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 304509,
            "range": "± 7208",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "56972bb1ad7075be90107123a61ce49bb7687963",
          "message": "refactor(agent-call): split agent_call and agent_read into modules under 300 lines (#1286)\n\n* refactor(agent-call): split agent_call and agent_read into modules under 300 lines\n\nMoves only; reduction pass follows. R8 gains a reviewed exception list (up to a 400-line ceiling); egress and arch guards treat files under a tests/ directory as test code.\n\nRefs #1280\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* refactor(agent-cli): name catalogue shards by command prefix\n\ncatalogue/shard_N.rs files were packed by line budget, so their names said\nnothing about their contents. The generator now emits one file per\ncommand-name prefix (ai.rs, applications.rs, ...), spilling into\n<prefix>_2.rs only if one prefix outgrows the budget. Entry order is unchanged.\n\nRefs #1280\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n* refactor(agent-call): shrink agent_call and agent_read after the split\n\nDedupes the envelope cost estimate shared by best_matches and found_jobs,\nand condenses doc comments that narrated review rounds blow-by-blow down to\nthe current rule and its reason. No other code change.\n\nRefs #1280\n\nCo-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-26T07:59:29+02:00",
          "tree_id": "578f15e1cd50cb0353e924a2c5a39706653d01c5",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/56972bb1ad7075be90107123a61ce49bb7687963"
        },
        "date": 1790403800652,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2185607,
            "range": "± 102191",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2588325,
            "range": "± 25982",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 303977,
            "range": "± 2812",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ec6ea39cf94549a5e796ba36e327c0e94a86a718",
          "message": "refactor(extension-bridge): split and shrink the answer/assist family (#1287)\n\nSplits answer_assist, assist_registry, answers_save and answers_suggest by\nresponsibility and moves every test into the standard <stem>/tests.rs layout\n(no inline bodies, no #[path]). All files are under 300 lines.\n\nThe scope is 102 lines smaller than before the split: the three identical\nclamp_bytes copies become one helper on str::floor_char_boundary, and doc\ncomments that narrated review history are condensed to the rule and its\nreason. The clamp_bytes test gains a multi-byte case, since the ASCII-only\nversion passed even with a cut that split a character.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-09-26T13:14:10+02:00",
          "tree_id": "c23283e6fdc3e1e2d3c5d9cb9afe46c80c8263a3",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ec6ea39cf94549a5e796ba36e327c0e94a86a718"
        },
        "date": 1790422445261,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1752515,
            "range": "± 27666",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2100351,
            "range": "± 19621",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 243862,
            "range": "± 4122",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ad0edc4f55450e50b181124a89e332d9b5a87745",
          "message": "refactor(extension-bridge): split the bridge core into modules under 300 lines (#1288)\n\n* refactor(extension-bridge): split the bridge core into modules under 300 lines\n\nSplits mod.rs, match_live, stream, register, auth, settings, document_export,\napplied_check_batch, import_flow, native_host, handshake and frame by\nresponsibility, and redistributes the crate-level test.rs, import_tests.rs and\nstream_tests.rs suites into each unit's own <stem>/tests.rs. Every file under\nextension_bridge/ is now under 300 lines with no inline or #[path] test modules.\n\nSecurity-relevant code (origin allowlist, HMAC handshake, token persistence,\nframe and reqId caps, throttles, rotation and revoke) moved byte-identical;\nthree functions were extracted as pure code motion (accept_ws,\nnotify_import_result, register_windows/register_unix). Near-identical opt-in\nand auto-flag tests became table-driven, keeping every case and message.\n\nRefs #1280\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* chore(extension-bridge): move log-leak allowlist entry with the accept loop\n\nThe TcpListener::accept error log moved from mod.rs to server.rs in the split; its allowlist entry follows it unchanged.\n\nRefs #1280\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-09-28T17:22:39+02:00",
          "tree_id": "6913692615ec0a9087d92045c3086c9bde3fdbfc",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ad0edc4f55450e50b181124a89e332d9b5a87745"
        },
        "date": 1790609714983,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2083728,
            "range": "± 41538",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2525318,
            "range": "± 50556",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 222428,
            "range": "± 4982",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b0b0c85c073e9db41a58f08dcfbf4e2bc627a92f",
          "message": "chore(arch): count only code lines against the R8 file-size cap (#1291)",
          "timestamp": "2026-09-28T17:39:46+02:00",
          "tree_id": "54cd7fe0a5cb1f78b0ffcdb39d9238224dca7c37",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b0b0c85c073e9db41a58f08dcfbf4e2bc627a92f"
        },
        "date": 1790611450636,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2191288,
            "range": "± 24455",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2617473,
            "range": "± 24878",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 251586,
            "range": "± 6943",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Saeed Kolivand",
            "username": "saeedkolivand",
            "email": "51081940+saeedkolivand@users.noreply.github.com"
          },
          "committer": {
            "name": "GitHub",
            "username": "web-flow",
            "email": "noreply@github.com"
          },
          "id": "555beb359433e6d323aae4c1d0eff18f9131802f",
          "message": "refactor(scraping): split the scraping core into modules under 300 code lines (#1295)\n\nSplits engine, scrape_url, http, cluster, trust, board_health, board_login,\nats_ref and the linkedin api client by responsibility. scrape_boards becomes\na short orchestrator over one file per phase (resolve boards, filters, skip\nchecks, fan-out sinks); the per-board URL parsers get a file each. Tests\nmove to <stem>/tests.rs hubs with topic files named for what they test.\n\nProduction code is 617 lines smaller; the Greenhouse/Lever URL tests become\ntwo table-driven tests that keep every case, and the skip-check tests share\none set of panicking scrapers. Allowlist entries (R3, egress dynamic sites,\nlog leaks) move with their statements; no new exemptions.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-09-28T23:31:07Z",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/555beb359433e6d323aae4c1d0eff18f9131802f"
        },
        "date": 1790639877297,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2205777,
            "range": "± 95691",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2729961,
            "range": "± 78598",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 320614,
            "range": "± 7927",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3ddc41fb9190bbc47245fa30fafe1a78576d91ae",
          "message": "refactor(scraping): split the job-board scrapers into modules under 300 code lines (#1296)\n\nThe aggregator splits into budget, fallback, merge, adzuna/adzuna_fetch,\njsearch, jooble and apify modules; its 3.3k-line test file becomes topic\nfiles. Every touched board's test.rs becomes a <board>/tests.rs hub.\n\nBoard tests used to copy an 18-field BoardSearchInput and a ScrapeContext\nliteral into every test; they now share default_search_input/default_ctx\nfrom boards/test_support.rs, keeping each test's non-default values as\nexplicit overrides. The boards directory is 417 code lines smaller.\nLog-leak and egress allowlist entries move with their statements.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-09-30T08:06:52+02:00",
          "tree_id": "c2f53fbf044460fb51992b43f2386d786baf416c",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/3ddc41fb9190bbc47245fa30fafe1a78576d91ae"
        },
        "date": 1790749710901,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1772400,
            "range": "± 37163",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2197339,
            "range": "± 39067",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 255449,
            "range": "± 5016",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "42b1bb9013c0d284304e2bda4e6af2c54c58986f",
          "message": "refactor(ai-provider): split the provider adapters into modules under 300 code lines (#1297)\n\nSplits anthropic, openai, gemini and ollama into thinking, capabilities,\nbody, wire, transport, chat, tools and web_search submodules, with tests\nin <adapter>/tests.rs hubs. The #[path]-loaded siblings (anthropic_wire,\nopenai_body, gemini_body, ollama_local_chat) become ordinary submodules of\ntheir adapter, so no module is loaded by path any more.\n\nThe \"check status, read capped body, end trace, friendly error\" block that\neleven request paths repeated is now checked_response(), and the two\npaginating adapters share the incomplete-catalogue error. User-facing\nmessages and trace calls are unchanged.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-09-30T17:09:43+02:00",
          "tree_id": "6f7d11feb418dc469982c7b4abbfd65831b27885",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/42b1bb9013c0d284304e2bda4e6af2c54c58986f"
        },
        "date": 1790782212368,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1765650,
            "range": "± 22909",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2084207,
            "range": "± 18198",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 241005,
            "range": "± 7504",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "cd7eb5d206cbd2669eec632d6f23aee5529ee83e",
          "message": "refactor(ai-provider): split the shared provider layer into modules under 300 code lines (#1304)\n\nSplits mod.rs (sampling, chat, usage, catalogue, embeddings, error_map),\nstructured (per-provider translators), stream (piece, text, finish),\nresearch (salary, answer) and search (exa), and moves every inline or\n#[path]-wired test module into <stem>/tests.rs. Nothing in ai_provider\noutside cli_agent is loaded by #[path] any more.\n\nFunction bodies are unchanged; the growth is imports in the new files. The\nretry and timeouts suites each fit in one tests.rs, so they stay whole.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-09-30T18:46:52+02:00",
          "tree_id": "07c51b6c5b2821415f4e4c554918db3d145fe6aa",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/cd7eb5d206cbd2669eec632d6f23aee5529ee83e"
        },
        "date": 1790788563800,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2214351,
            "range": "± 69779",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2591281,
            "range": "± 29842",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 307987,
            "range": "± 4618",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a4463fbda07627b6188bd707ab50e85288e973e2",
          "message": "refactor(ai-provider): split the cli agent layer into modules under 300 code lines (#1306)\n\ncli_agent/mod.rs splits into backend (the trait and invocation types),\ndetect, spawn, errors, client, stream and complete. Every backend's inline\ntests move to <backend>/tests.rs; codex's suite splits into parsing and\ninvocation. The spawn path (stdin-only prompt delivery, the cmd.exe wrapper,\nworkspace isolation, argv flags) moves unchanged.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-09-30T20:06:45+02:00",
          "tree_id": "a2ea7fc0f21c7b6471ca281cd420f9689be94032",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/a4463fbda07627b6188bd707ab50e85288e973e2"
        },
        "date": 1790793035559,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2204490,
            "range": "± 90283",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2583321,
            "range": "± 31211",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 303359,
            "range": "± 3525",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Saeed Kolivand",
            "username": "saeedkolivand",
            "email": "51081940+saeedkolivand@users.noreply.github.com"
          },
          "committer": {
            "name": "GitHub",
            "username": "web-flow",
            "email": "noreply@github.com"
          },
          "id": "455ba14de0ff7b409adc333ee57c6d6c50ca89fe",
          "message": "refactor(pipeline): split the pipeline core into modules under 300 code lines (#1307)\n\npipeline/mod.rs splits into completer (construction, accounting, research),\ncompletion (provider calls and the complete_json seam) and stage; runs/\nsplits into model, store, maintenance and import_export. json.rs keeps its\nproduction code and moves its tests out. Tests move to tests.rs hubs.\n\ncomplete and chat_with_tools now call the existing record_spend helper\ninstead of repeating its body. The dump-run-metrics drift guard reads the\npipeline_runs schema from its new file.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-09-30T19:37:35Z",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/455ba14de0ff7b409adc333ee57c6d6c50ca89fe"
        },
        "date": 1790797900315,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1765337,
            "range": "± 19017",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2104821,
            "range": "± 15034",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 241553,
            "range": "± 1722",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "cf816f592825385569200bc6ff6b4dbbf2fc5467",
          "message": "refactor(pipeline): split the resume pipeline into modules under 300 code lines (#1308)\n\npipeline/resume splits prompts into one file per stage, moves the run\ndeadline into deadline.rs, and gives humanize (predicates, attempt) and\nrepair (judge) their own submodules. commands/resume_pipeline splits into\nrun, persist, read and regenerate. Both huge test files become tests.rs\nhubs with topic files and shared support modules; projects and evidence\nmove their inline tests out.\n\nTauri commands are registered from their new modules, because\n#[tauri::command] generates items next to the function that a re-export\ndoes not carry. The agent CLI policy rows, the proof tests and the\ndump-run-metrics drift guard follow the new paths. Prompt text is unchanged.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-01T00:08:17+02:00",
          "tree_id": "92ad7f8ba521e0137b8dabdd7e92ea0ed8fa3fae",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/cf816f592825385569200bc6ff6b4dbbf2fc5467"
        },
        "date": 1790807312268,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1998087,
            "range": "± 24225",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2389337,
            "range": "± 44848",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 213349,
            "range": "± 1680",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4a5c0307a58eb29999dcfd0a49c2991623fa1e10",
          "message": "refactor(export): split the typst engine into modules under 300 code lines (#1309)\n\nThe 5.4k-line typst_engine test file becomes a tests.rs hub with shared\nfixtures (resume, letter, pdf introspection, svg geometry) and one topic\nfile per template or feature. letter.rs moves its data model into\nletter/model.rs; letter, letterhead, render and photo move their inline\ntests out.\n\nletter's style mapping now uses render's existing rgb_to_hex and\nfont_family_to_typst instead of its own identical copies. The ten\nper-template stray-Typst guards and the Swiss Minimal/Academic render\nchecks become table-driven tests that keep every case. Rendered output\nis unchanged.\n\nRefs #1280\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-01T05:51:36+02:00",
          "tree_id": "ad9896c78199dcd885c439397e971851a23b8824",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4a5c0307a58eb29999dcfd0a49c2991623fa1e10"
        },
        "date": 1790827526286,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2173716,
            "range": "± 25486",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2581718,
            "range": "± 36253",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 308930,
            "range": "± 9206",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ccfc91f423e3495e899c17315fa72544ae40932f",
          "message": "refactor(export): split docx, parser, templates, pdf and model_docx under 300 code lines (#1310)\n\n* chore: continue on claude cloud\n\n* refactor(export): finish splitting docx, parser, templates, pdf and model_docx\n\nCompletes batch 5b of #1280 on top of the partial split: every file under\nexport/ is now under 300 code lines with tests out of line. Removes an unused\ntest import, replaces an 11-argument letter-header helper with a HeaderLook\nstruct (body unchanged), and blesses the R8/R8b baselines (removals only).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\n---------\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-04T11:30:45+02:00",
          "tree_id": "d39a8d8f80d95ededd165b8ddc6ece1166cf9c62",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ccfc91f423e3495e899c17315fa72544ae40932f"
        },
        "date": 1791107493819,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1761726,
            "range": "± 52803",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2172758,
            "range": "± 79724",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 253679,
            "range": "± 3792",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "54dee32722b5d93968d590040ae5d83afe4621c0",
          "message": "refactor(validate): split validate/ into modules under 300 code lines (#1311)\n\nBatch 6a of #1280. Splits validate/mod.rs (readback, pdf_links, header_links),\ncontent/mod.rs, factual.rs and credentials/tenure.rs by responsibility, and the\n6,360-line content/test.rs into one topic file per checker with shared fixtures.\nThe two header-link mismatch/missing blocks share one helper each; finding\ncodes, severities, messages and check order are unchanged. Seven table-driven\ntest merges keep every case and message (273 -> 262 tests). Net code lines in\nvalidate/ go from 10,974 to 10,434.\n\n\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-04T13:02:30+02:00",
          "tree_id": "9911e095f4a95bcf22b3d5f55a0df1087b111a1d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/54dee32722b5d93968d590040ae5d83afe4621c0"
        },
        "date": 1791112339284,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1391453,
            "range": "± 48573",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1609969,
            "range": "± 42923",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 156217,
            "range": "± 2847",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7da82191fbbe21ba7785b143784fd27e2dcb939c",
          "message": "refactor(email-watch): split email_watch/ and extraction/ under 300 code lines (#1312)\n\nBatch 6b of #1280. Moves every inline test module in email_watch/ and\nextraction/ out of line, splits the large suites by topic with shared\nfixtures, and moves the IMAP connect helpers into imap_client/connect.rs.\nThe authenticity, matching and auto-write production code is byte-identical.\nextraction/ shares one link-reference helper between pdf and rtf, drops a\ndead branch in docx and simplifies a punctuation check. 113 near-identical\ntests become table rows that keep every case and message. Net code lines in\nboth directories go from 6,873 to 6,063.\n\n\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-04T14:33:02+02:00",
          "tree_id": "89b9123f377abb5d53e37d972ef58f08d66bdb56",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7da82191fbbe21ba7785b143784fd27e2dcb939c"
        },
        "date": 1791118639173,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2195100,
            "range": "± 17936",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2610008,
            "range": "± 29684",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 322128,
            "range": "± 16832",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7a7386f85443f7d14aeaa993e9094a7a8b5f3bd9",
          "message": "refactor(documents): split documents/ into modules under 300 code lines (#1313)\n\nBatch 7a of #1280. Splits the document store (migrations, vectors, caches,\nasync ops, backup), the evidence extractor and the keyword extractor by\nresponsibility, and the three large test files into topic files with shared\nfixtures. SQL, migrations, keyword and stopword lists, weights and matching\nrules move verbatim; two private helpers replace identical copies of a row\nmapper and a single-statement write. Six table-driven merges keep every case\n(182 -> 172 tests). Net code lines in documents/ go from 7,595 to 7,177.\n\n\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-04T15:49:11+02:00",
          "tree_id": "0ec72a46ee881f75a681713884cd8328788f679c",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7a7386f85443f7d14aeaa993e9094a7a8b5f3bd9"
        },
        "date": 1791123181096,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2184251,
            "range": "± 21415",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2577230,
            "range": "± 15703",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 308595,
            "range": "± 1929",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "de5ce7cdebbb4857f63a2bb2bfc050d649492b64",
          "message": "refactor(applications): split applications/ and ai_generations/ under 300 code lines (#1314)\n\nSplit both SQLite stores by responsibility (rows, reads, writes, migrations,\nbackup, answers, orphan link, quality report) and move every test suite out\nof line into tests.rs hubs plus topic files, with shared fixtures in\ntests/support.rs. SQL, migrations, status rules and error messages move\nverbatim; the one whole-record INSERT that insert and import each carried a\ncopy of is now a single rows::insert_row.\n\nFixture dedupe: one legacy_db(user_version) builder replaces five hand-copied\npre-migration schemas, and near-identical tests become table-driven with every\ncase and message kept. Net code lines: 7696 -> 6983. R3_ALLOW entries added\nfor the files that received moved rusqlite statements of the same store;\nr8 and r8b baselines shrink (removals only).\n\nRefs #1280\n\n\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-04T17:21:40+02:00",
          "tree_id": "dd04084b8ed83bf36f82c8e8774ae53e71cd4227",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/de5ce7cdebbb4857f63a2bb2bfc050d649492b64"
        },
        "date": 1791128574874,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1781041,
            "range": "± 38579",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2101203,
            "range": "± 38347",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 247517,
            "range": "± 4667",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "89af8b5c6e1e682c16b67ed89598430b5deef136",
          "message": "refactor(autopilot): split the autopilot family under 300 code lines (#1315)",
          "timestamp": "2026-10-04T18:37:07+02:00",
          "tree_id": "e807c4e189cb5aa680d678d12d61195c1a05f39f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/89af8b5c6e1e682c16b67ed89598430b5deef136"
        },
        "date": 1791133078022,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2005695,
            "range": "± 44219",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2334014,
            "range": "± 76253",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 210902,
            "range": "± 3435",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Saeed Kolivand",
            "username": "saeedkolivand",
            "email": "51081940+saeedkolivand@users.noreply.github.com"
          },
          "committer": {
            "name": "GitHub",
            "username": "web-flow",
            "email": "noreply@github.com"
          },
          "id": "2eb3b06ff5fad3983913ac427e7b69f7e12c9fc1",
          "message": "refactor(commands): split match_resume, ai, applications, scrape and lib.rs under 300 code lines (#1316)\n\nlib.rs keeps the builder and plugin chain; the setup hook, its managed\nstate, the deep-link plumbing, the panic hook and the generate_handler!\nlist move to a new L3 `shell` module. Store registration, plugin, manage\nand spawn order are unchanged, and the command list is byte-identical, so\nthe agent-CLI POLICY set-equality test now reads shell/handler.rs.\n\ncommands/{ai,scrape,match_resume} split by responsibility with every\n#[tauri::command] kept at its module path (glob re-exports);\ncommands/applications gains fail/finish/ok_reply reply helpers and a shared\ncreation_target, and scrape shares acquire_scrape_slot/harvest_ats_slugs.\nAll inline tests move out of line; 28 input-only test variants become 8\ntable-driven tests with every case and message kept. Net code lines for\nthe five areas: 6186 -> 6130.\n\nJOB_COMPLETE_PRODUCERS follows ai_pull_model, run_embed_job and the two\nscrape producers; include_str! source scans follow the moved code; r8 and\nr8b baselines shrink (removals only). Review routing now covers\ncommands/match_resume/**.\n\nRefs #1280\n\n\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-04T17:23:07Z",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/2eb3b06ff5fad3983913ac427e7b69f7e12c9fc1"
        },
        "date": 1791135403764,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2197542,
            "range": "± 59509",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2561913,
            "range": "± 33151",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 307276,
            "range": "± 14668",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b85791312fce94ec15fd78f2ee4351761862cc09",
          "message": "refactor(stores): split the user data stores under 300 code lines (#1317)\n\nSplit contact_profile (header, classify, conflicts, store), spend (rates),\nai_config (types, validation), discovered (queries), postings (interactions)\nand jobs (persist) by responsibility; every SQL statement, migration, serde\nshape, cap, validation rule and error message moves verbatim, and public\npaths stay put via re-exports. All inline and singular test.rs suites move\nto the standard tests.rs layout, including the #[path]-wired LinkedIn SSRF\ntest. Shared per-store fixtures replace copied struct literals, and 70\ninput-only test variants become table-driven tests with every case and\nmessage kept (320 -> 268 tests). Net code lines: 9102 -> 8693.\n\nR3_ALLOW follows the moved rusqlite code (contact_profile/store.rs,\ndiscovered/queries.rs, jobs/persist.rs); the two postings log-leak keys\nfollow their unchanged statements; r8 and r8b baselines shrink (removals\nonly).\n\nRefs #1280\n\n\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-04T20:37:10+02:00",
          "tree_id": "0ab26a2d838058e00359673ad610b70affb40ddf",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b85791312fce94ec15fd78f2ee4351761862cc09"
        },
        "date": 1791140474667,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2269504,
            "range": "± 24783",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2669512,
            "range": "± 24013",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 303193,
            "range": "± 3429",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3ff408e52b6d1147a9618beb09bb1f73b36859e9",
          "message": "chore: bump the ci-actions group with 2 updates (#1303)\n\nBumps the ci-actions group with 2 updates: [taiki-e/install-action](https://github.com/taiki-e/install-action) and [astral-sh/setup-uv](https://github.com/astral-sh/setup-uv).\n\n\nUpdates `taiki-e/install-action` from 2.87.17 to 2.87.21\n- [Release notes](https://github.com/taiki-e/install-action/releases)\n- [Changelog](https://github.com/taiki-e/install-action/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/taiki-e/install-action/compare/94c31af3204a9f15ab40b35ad084410b905bbc73...4cef1412cce204788f482e778a0b9187f9626a29)\n\nUpdates `astral-sh/setup-uv` from 10.1.0 to 10.2.0\n- [Release notes](https://github.com/astral-sh/setup-uv/releases)\n- [Commits](https://github.com/astral-sh/setup-uv/compare/bec219d24cd3e171d82865faccec33120bb574f4...c18668ad3cf93ea998bef934396af7bb5c839dc7)\n\n---\nupdated-dependencies:\n- dependency-name: taiki-e/install-action\n  dependency-version: 2.87.21\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: ci-actions\n- dependency-name: astral-sh/setup-uv\n  dependency-version: 10.2.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: ci-actions\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-10-04T22:29:03+02:00",
          "tree_id": "684a5548d48224816436ce8f4852f7ab27f8be89",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/3ff408e52b6d1147a9618beb09bb1f73b36859e9"
        },
        "date": 1791146396007,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1732504,
            "range": "± 103911",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2102744,
            "range": "± 124978",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 191064,
            "range": "± 4252",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a4ae94a0511af7e7969e1538539205d3cdb4aa82",
          "message": "chore: bump the desktop-rust group (#1300)\n\nBumps the desktop-rust group in /apps/desktop/src-tauri with 22 updates:\n\n| Package | From | To |\n| --- | --- | --- |\n| [tauri](https://github.com/tauri-apps/tauri) | `2.11.6` | `2.12.0` |\n| [tauri-plugin-opener](https://github.com/tauri-apps/plugins-workspace) | `2.5.5` | `2.6.0` |\n| [tauri-plugin-dialog](https://github.com/tauri-apps/plugins-workspace) | `2.7.3` | `2.8.0` |\n| [tauri-plugin-updater](https://github.com/tauri-apps/plugins-workspace) | `2.12.0` | `2.13.0` |\n| [tauri-plugin-clipboard-manager](https://github.com/tauri-apps/plugins-workspace) | `2.3.3` | `2.4.0` |\n| [tauri-plugin-shell](https://github.com/tauri-apps/plugins-workspace) | `2.3.6` | `2.4.0` |\n| [tauri-plugin-log](https://github.com/tauri-apps/plugins-workspace) | `2.9.2` | `2.10.0` |\n| [tauri-plugin-window-state](https://github.com/tauri-apps/plugins-workspace) | `2.4.1` | `2.5.0` |\n| [tauri-plugin-single-instance](https://github.com/tauri-apps/plugins-workspace) | `2.4.5` | `2.5.0` |\n| [tauri-plugin-notification](https://github.com/tauri-apps/plugins-workspace) | `2.4.0` | `2.5.0` |\n| [tauri-plugin-deep-link](https://github.com/tauri-apps/plugins-workspace) | `2.4.10` | `2.5.0` |\n| [tauri-plugin-autostart](https://github.com/tauri-apps/plugins-workspace) | `2.5.1` | `2.6.0` |\n| [tauri-plugin-os](https://github.com/tauri-apps/plugins-workspace) | `2.3.2` | `2.4.0` |\n| [tauri-plugin-process](https://github.com/tauri-apps/plugins-workspace) | `2.3.1` | `2.4.0` |\n| [tauri-plugin-positioner](https://github.com/tauri-apps/plugins-workspace) | `2.3.4` | `2.4.0` |\n| [tauri-plugin-global-shortcut](https://github.com/tauri-apps/plugins-workspace) | `2.3.2` | `2.4.0` |\n| [tauri-plugin-store](https://github.com/tauri-apps/plugins-workspace) | `2.4.5` | `2.5.0` |\n| [tauri-plugin-websocket](https://github.com/tauri-apps/plugins-workspace) | `2.4.3` | `2.5.0` |\n| [encoding_rs](https://github.com/hsivonen/encoding_rs) | `0.8.41` | `0.8.42` |\n| [thiserror](https://github.com/dtolnay/thiserror) | `2.0.20` | `2.0.21` |\n| [notify-rust](https://github.com/hoodie/notify-rust) | `4.18.0` | `4.18.1` |\n| [tauri-build](https://github.com/tauri-apps/tauri) | `2.6.3` | `2.7.0` |\n\n\nUpdates `tauri` from 2.11.6 to 2.12.0\n- [Release notes](https://github.com/tauri-apps/tauri/releases)\n- [Commits](https://github.com/tauri-apps/tauri/compare/tauri-v2.11.6...tauri-v2.12.0)\n\nUpdates `tauri-plugin-opener` from 2.5.5 to 2.6.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/http-v2.5.5...fs-v2.6.0)\n\nUpdates `tauri-plugin-dialog` from 2.7.3 to 2.8.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/dialog-v2.7.3...log-v2.8.0)\n\nUpdates `tauri-plugin-updater` from 2.12.0 to 2.13.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/updater-v2.12.0...updater-v2.13.0)\n\nUpdates `tauri-plugin-clipboard-manager` from 2.3.3 to 2.4.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/nfc-v2.3.3...os-v2.4.0)\n\nUpdates `tauri-plugin-shell` from 2.3.6 to 2.4.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/nfc-v2.3.6...os-v2.4.0)\n\nUpdates `tauri-plugin-log` from 2.9.2 to 2.10.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/log-v2.9.2...log-v2.10.0)\n\nUpdates `tauri-plugin-window-state` from 2.4.1 to 2.5.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/fs-v2.4.1...fs-v2.5.0)\n\nUpdates `tauri-plugin-single-instance` from 2.4.5 to 2.5.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/fs-v2.4.5...fs-v2.5.0)\n\nUpdates `tauri-plugin-notification` from 2.4.0 to 2.5.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/os-v2.4.0...fs-v2.5.0)\n\nUpdates `tauri-plugin-deep-link` from 2.4.10 to 2.5.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/deep-link-v2.4.10...fs-v2.5.0)\n\nUpdates `tauri-plugin-autostart` from 2.5.1 to 2.6.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/fs-v2.5.1...fs-v2.6.0)\n\nUpdates `tauri-plugin-os` from 2.3.2 to 2.4.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/os-v2.3.2...os-v2.4.0)\n\nUpdates `tauri-plugin-process` from 2.3.1 to 2.4.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/os-v2.3.1...os-v2.4.0)\n\nUpdates `tauri-plugin-positioner` from 2.3.4 to 2.4.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/nfc-v2.3.4...os-v2.4.0)\n\nUpdates `tauri-plugin-global-shortcut` from 2.3.2 to 2.4.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/os-v2.3.2...os-v2.4.0)\n\nUpdates `tauri-plugin-store` from 2.4.5 to 2.5.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/fs-v2.4.5...fs-v2.5.0)\n\nUpdates `tauri-plugin-websocket` from 2.4.3 to 2.5.0\n- [Release notes](https://github.com/tauri-apps/plugins-workspace/releases)\n- [Commits](https://github.com/tauri-apps/plugins-workspace/compare/fs-v2.4.3...fs-v2.5.0)\n\nUpdates `encoding_rs` from 0.8.41 to 0.8.42\n- [Commits](https://github.com/hsivonen/encoding_rs/compare/v0.8.41...v0.8.42)\n\nUpdates `thiserror` from 2.0.20 to 2.0.21\n- [Release notes](https://github.com/dtolnay/thiserror/releases)\n- [Commits](https://github.com/dtolnay/thiserror/compare/2.0.20...2.0.21)\n\nUpdates `notify-rust` from 4.18.0 to 4.18.1\n- [Release notes](https://github.com/hoodie/notify-rust/releases)\n- [Changelog](https://github.com/hoodie/notify-rust/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/hoodie/notify-rust/compare/v4.18.0...v4.18.1)\n\nUpdates `tauri-build` from 2.6.3 to 2.7.0\n- [Release notes](https://github.com/tauri-apps/tauri/releases)\n- [Commits](https://github.com/tauri-apps/tauri/compare/tauri-build-v2.6.3...tauri-build-v2.7.0)\n\n---\nupdated-dependencies:\n- dependency-name: tauri\n  dependency-version: 2.12.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-opener\n  dependency-version: 2.6.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-dialog\n  dependency-version: 2.8.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-updater\n  dependency-version: 2.13.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-clipboard-manager\n  dependency-version: 2.4.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-shell\n  dependency-version: 2.4.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-log\n  dependency-version: 2.10.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-window-state\n  dependency-version: 2.5.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-single-instance\n  dependency-version: 2.5.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-notification\n  dependency-version: 2.5.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-deep-link\n  dependency-version: 2.5.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-autostart\n  dependency-version: 2.6.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-os\n  dependency-version: 2.4.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-process\n  dependency-version: 2.4.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-positioner\n  dependency-version: 2.4.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-global-shortcut\n  dependency-version: 2.4.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-store\n  dependency-version: 2.5.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: tauri-plugin-websocket\n  dependency-version: 2.5.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n- dependency-name: encoding_rs\n  dependency-version: 0.8.42\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: thiserror\n  dependency-version: 2.0.21\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: notify-rust\n  dependency-version: 4.18.1\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: desktop-rust\n- dependency-name: tauri-build\n  dependency-version: 2.7.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: desktop-rust\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-10-04T21:32:20Z",
          "tree_id": "f72fcab8e20e43c820b252e002d0d50915d25453",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/a4ae94a0511af7e7969e1538539205d3cdb4aa82"
        },
        "date": 1791151050141,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2369978,
            "range": "± 91111",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2900767,
            "range": "± 165117",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 358021,
            "range": "± 9923",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1102f91dcd2024e7741feb8a13c5c3015bf1a476",
          "message": "fix: unbreak the build and ship an appimage any user can start (#1324)\n\n* build(deps): revert tauri-plugin-sentry to 0.6 to unbreak the build\n\nThis reverts commit 369b1be (#1301). tauri-plugin-sentry 0.7 removed the\n`tauri_plugin_sentry::minidump` re-export that `src/lib.rs` calls to start\nthe crash-reporter process, so `cargo check` fails on every desktop target\n(error E0433: could not find `minidump` in `tauri_plugin_sentry`). 0.7 moves\nthe supervisor into sentry's `MinidumpIntegration` on `ClientOptions`, which\nchanges when the crash reporter forks relative to consent and redaction; that\nmigration needs its own reviewed change.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\n* ci: repoint the windows atomic-rename test filter after the split\n\nThe #1280 split moved `save_replaces_the_file_atomically_and_leaves_no_temp_file`\nto `postings::interactions::tests::persistence`, so the Windows job's\n`postings::test::…` filter matched zero tests and passed without running the\nrename-over-existing check it exists for.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\n* fix(release): ship an appimage any user can start\n\ntauri-bundler 2.10.1 saves its downloaded AppRun helper as 0770 and copies it\ninto the AppImage as AppRun.wrapped (tauri-apps/tauri#16155). Any user other\nthan the image's owner then gets \"Permission denied\": the AppImage catalog's\nfirejail test fails on v0.156.0 with exactly that error.\n\nThe bundler reuses a cached helper, so the release job now seeds a 0755 copy\n(pinned to the sha256 of the AppRun.wrapped shipped in v0.156.0) before\n`tauri build`, and a Linux-only step fails the job if the built AppImage's\nAppRun.wrapped is not readable and executable by other users.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\n---------\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-05T17:10:17+02:00",
          "tree_id": "dc0991a1a281ec7e0ee9176f13439dd3d9367f43",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/1102f91dcd2024e7741feb8a13c5c3015bf1a476"
        },
        "date": 1791213836812,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2192734,
            "range": "± 24682",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2635053,
            "range": "± 34377",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 319003,
            "range": "± 8216",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "03f6a47ea2b0de0a4cadca873ef8518a235a7895",
          "message": "build(deps): move to tauri-plugin-sentry 0.7 and sentry's minidump integration (#1327)\n\ntauri-plugin-sentry 0.7 drops its sentry-rust-minidump re-export; the crash\nsupervisor is now sentry's own MinidumpIntegration, registered in\ncrash_reporting::client_options() (gated on the targets sentry builds it for)\ninstead of tauri_plugin_sentry::minidump::init in lib.rs. The reporter process\nis still forked inside sentry::init, before the Tauri builder, and its client\nis cloned from the same options, so before_send and the GuardedTransportFactory\nwire gate still apply to the minidump event; no DSN or no consent still means\nno client and no reporter.\n\n- inherit_args(false): the old reporter was spawned with no argv; the new\n  default would forward the app's argv (which can carry an ajh:// deep link).\n- sentry's `minidump` feature is now declared explicitly in Cargo.toml.\n- client_options_pin_every_privacy_switch now asserts the only integration\n  registered is the minidump supervisor; new tests pin inherit_args(false)\n  and forbid an on_process hook or inherit_args(true) by source scan.\n- observability::redact_token now looks through a Debug wrapper such as\n  Path(\"/var/folders/...\") for its absolute-path checks: 0.7's reporter sends\n  an error event naming its socket path that way when it fails to start.\n\nBehaviour notes: the reporter no longer starts its own release-health session\n(it never returns from sentry::init), so session counts may drop slightly; on\nunix its argv[0] is now \"Crash Reporter (Sentry Rust SDK)\".\n\n\nClaude-Session: https://claude.ai/code/session_01EW5udttVAwPVeHLEyNV8bD\n\nCo-authored-by: Claude <noreply@anthropic.com>",
          "timestamp": "2026-10-05T18:59:57+02:00",
          "tree_id": "3f2cae8e40332905f1d1cd5576fa677dedc15fe0",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/03f6a47ea2b0de0a4cadca873ef8518a235a7895"
        },
        "date": 1791221081894,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2317250,
            "range": "± 26699",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2712865,
            "range": "± 24232",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 305975,
            "range": "± 10090",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "062c6c19ce63724d32b3510feec7a4f0827b9096",
          "message": "fix(ai): redact provider errors before they reach the model list and key test in settings (#1347)\n\n* fix(ai): redact provider errors before they reach the model list and key test in settings\n\nfriendly_api_error appends the upstream body on 400/404/422 and default statuses, and the\nmodel-list and key-test commands returned it raw, so a gateway that echoed the key or a\nkey-bearing URL showed it in settings. Both commands now run the error through the existing\nstream-error redactor and the 200-char reason cap.\n\nCloses #1341\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): strip the stored key and base-url secrets verbatim from model-list and key-test errors\n\nShape-based redaction misses bare keys such as AIza or gsk_ and the x-api-key header forms, so\nboth commands now strip the stored key (raw and trimmed), the base-URL userinfo password and\nquery values verbatim before the shape pass, through one finish_provider_result helper.\n\nRefs #1341\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T17:20:47+02:00",
          "tree_id": "2505583e18d33a549daca2772f265d5a9b33229e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/062c6c19ce63724d32b3510feec7a4f0827b9096"
        },
        "date": 1791301424103,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1810003,
            "range": "± 31849",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2167165,
            "range": "± 26567",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 256011,
            "range": "± 6124",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2bd3c21182a96712d673beb5d74600a595eab7ea",
          "message": "fix(ai): redact upstream provider error text where it is built, and on the regenerate and embed edges (#1349)\n\n* fix(ai): redact upstream provider error text where it is built, and on the regenerate and embed edges\n\nfriendly_api_error now bounds the upstream detail to 8 KiB and shape-redacts it at the source,\nwithout the 200-char cap so the embedding context-length retry still reads its wording. The\nregenerate-section and ai_embed commands strip the stored key and base-url secrets verbatim\nlike the model-list fix, and raw upstream bodies in logs and Ollama errors are bounded and\nredacted.\n\nCloses #1346\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): cover the ollama search and pull error bodies, use the embedding base url, resolve the regenerate provider once\n\nWraps the two missed raw-body sites, strips the embedding config's own base url in ai_embed,\nbinds the regenerate completer once so the strip uses the provider that actually ran, and pins\nthe no-cap-at-source invariant with a context-length message past 200 chars.\n\nRefs #1346\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T19:42:08+02:00",
          "tree_id": "389792237b847c531151f6bc17c0cfb71726ff79",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/2bd3c21182a96712d673beb5d74600a595eab7ea"
        },
        "date": 1791309273046,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1805995,
            "range": "± 37683",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2244876,
            "range": "± 29776",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 251921,
            "range": "± 3350",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0139d5c7461622cc3a2dece66bec4a443fc6047d",
          "message": "fix(ai): strip provider keys verbatim at the completer and embed seams, and redact header-style key echoes (#1350)\n\n* fix(ai): strip provider keys verbatim at the completer and embed seams, and redact header-style key echoes\n\nEvery Completer call and embed_adaptive's result now strip the stored key and base-url secrets\nverbatim on the error path, keeping the error variant and full text. The shared redactor learns\nheader and JSON echoes (x-goog-api-key, x-api-key, *-key/*-token headers, any Authorization\nscheme, bare long Bearer tokens) without touching the context-length retry's keywords.\n\nCloses #1348\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): stop the seam test leaking pooled connections, redact compact json key echoes, scan every pipeline provider call\n\nThe seam test built its errors through real requests on the shared pooled client, which left\nkeep-alive connections that made the ollama timeout tests flaky; it now builds them with\nfriendly_api_error directly. The redactor catches compact JSON and glued header forms. The\nwiring guard now requires every self.provider call under src/pipeline to sit inside\nstrip_secrets unless it is a listed non-io method.\n\nRefs #1348\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T21:30:35+02:00",
          "tree_id": "bca2a3cef89e5ff1b8b4b61bd18514cb72f5d848",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/0139d5c7461622cc3a2dece66bec4a443fc6047d"
        },
        "date": 1791316516410,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2380560,
            "range": "± 19788",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2852169,
            "range": "± 42213",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 258319,
            "range": "± 9980",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "387a75ca5bcfd522771bfcd1f3bfbbd4e2fff119",
          "message": "perf(ai): run cover-letter company research alongside strategy and draft (#1371)\n\n* perf(ai): start cover-letter company research right after analyze\n\nResearch used to run serially inside the letter stage. It now starts as soon as\nanalyze publishes the role, overlaps strategy and draft, and the letter stage\nawaits only what is left. Dropped with the pipeline on error or cancel.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): fail early research soft when the role is never published\n\ntake_brief now drops the role sender too, so an unpublished role yields no\nbrief instead of hanging the letter stage. A source guard pins the analyze and\nletter-stage wiring.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T02:29:48+02:00",
          "tree_id": "6558fe3fc223e184624cc02cf4c6dcc962748fed",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/387a75ca5bcfd522771bfcd1f3bfbbd4e2fff119"
        },
        "date": 1791333733536,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1771059,
            "range": "± 38452",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2069773,
            "range": "± 11887",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 240020,
            "range": "± 2563",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "99e825b92ee627b3afbda181e2ef79e7fc3176fc",
          "message": "perf(ai): stream every completion and time out on silence, not wall clock (#1372)\n\n* perf(ai): stream every completion and replace wall-clock timeouts with an idle timeout\n\nCompletions, including structured ones, now stream on Ollama, OpenAI-style,\nAnthropic and Gemini and are re-assembled with the existing frame parsers,\nschema fields kept. A call fails only after its old deadline passes with no\ndata, with an absolute ceiling of four times that; the renderer outlasts it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): fail collected streams on error frames, missing terminals, empty answers and oversize\n\nEach parser now surfaces in-band error frames, Anthropic and Gemini map their\nstop reasons, and collect fails on EOF without a terminal piece, on an empty\nanswer or refusal, on a length stop for JSON calls, and past the 8 MB body cap.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): count refusal text toward the collected-stream size cap\n\nRefusal deltas accumulate outside the answer, so a refusal-only stream could\ngrow past the 8 MB cap. The cap now covers both.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T03:44:48+02:00",
          "tree_id": "64d0762c5c3b3eca32eefbe6ef12475342002ac4",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/99e825b92ee627b3afbda181e2ef79e7fc3176fc"
        },
        "date": 1791338927694,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2209428,
            "range": "± 76160",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2611122,
            "range": "± 98552",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 303984,
            "range": "± 11032",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0f54d0145277eecf00a971641f4af04afab10132",
          "message": "perf(ai): key the analyze cache on the job alone and trim unused stage output fields (#1373)\n\nAnalyze never reads the résumé, so its cache key no longer chains it; the\nrésumé is folded in at match_evidence, where it is first read. redFlags and\nthe program-seeded company-plan fields are no longer requested from the model.\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T06:43:18+02:00",
          "tree_id": "bed3e0db65c9bd3b4ece796a38478a558801b3e4",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/0f54d0145277eecf00a971641f4af04afab10132"
        },
        "date": 1791348919310,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2001127,
            "range": "± 12095",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2333657,
            "range": "± 89090",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 210122,
            "range": "± 4761",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d9c422c4174a95bbb44e023a0acea972f52bfa40",
          "message": "chore(ai): persist content-free provider routing and timing per pipeline stage (#1374)\n\n* chore(ai): persist content-free provider routing and timing per pipeline stage\n\nEach stage's finish or error event artifact now carries the provider, model,\neffective effort, token counts and wall time of every call, plus Ollama's\nload, prompt-eval and eval timings. No prompt, output or job text is stored.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): trace failed calls by error class and never inherit a stale stream usage\n\nFailed and timed-out calls are now noted with only their error variant, the\nobserved-usage slot is cleared before each stream, and a stage keeps at most\ntwelve call records with a dropped count.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T07:14:55+02:00",
          "tree_id": "2f726f243583dfbbcec2c53a80f47498270977e3",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/d9c422c4174a95bbb44e023a0acea972f52bfa40"
        },
        "date": 1791351561852,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2203913,
            "range": "± 11062",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2558253,
            "range": "± 66845",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 312159,
            "range": "± 3280",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "819fa1efd3dc04ff4190a38cf9e48b3df00130e0",
          "message": "perf(ai): humanize with line patches and run the résumé and letter arms together (#1375)\n\n* perf(ai): humanize with line patches and run the résumé and letter arms together\n\nThe model now returns replacements for the flagged lines only, applied in Rust\nbehind the existing never-worse guards, and the résumé and letter humanize run\nconcurrently. Patches that change digits, span lines or hit unflagged lines\nare dropped.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): keep the whole-document humanize rewrite for documents with only document-wide flags\n\nRhythm, rule-of-three and generic-letter flags have no line to patch, so a\ndocument flagged only by them gets the previous whole-document rewrite again.\nThe ledger records which mode ran.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): charge the humanize rewrite against the daily cap and locate flags with the validator's matcher\n\nThe whole-document rewrite now charges the provider's daily ceiling like every\nother call, guarded by a source check. Flagged lines are found with the\nvalidator's own word-boundary, whitespace and apostrophe rules, the size cap\napplies only to rewrites, and bold text and headings keep their markers.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n* fix(ai): offer only whole flagged lines within the humanize excerpt budget\n\nThe excerpt is built to a budget below its fence cap, and only the lines it\nshows whole are offered and patched, so a truncated line can never replace a\nfull one. An echoed bullet is stripped once, leaving a bold opener intact.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T08:04:59+02:00",
          "tree_id": "8e12a49bcf3e5b6eb49d8dde4c1f0deda005e14e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/819fa1efd3dc04ff4190a38cf9e48b3df00130e0"
        },
        "date": 1791353990161,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1773360,
            "range": "± 24653",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2146092,
            "range": "± 36169",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 250012,
            "range": "± 3705",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "79c6ae90ec40fc2b865e591d58b4856894855827",
          "message": "fix(ai): retry a degraded job analysis or strategy once and never cache it (#1384)\n\nAt the cheapest effort tier a local qwen3 analysis came back with no\nrequirements and its reasoning dumped into a string field, and the analyze\ncache then served it for a week. Both JSON stages now check a quality floor,\nretry once at the provider default when it trips, keep the richer answer, and\nskip the cache write for a floor miss. A cached row below the floor is read as\na miss, so already-poisoned rows stop being served. A source guard pins the\nwiring in both stages.\n\nAlso: the four complete paths streamed since #1372 now log streaming=true, and\nstructured Ollama calls log a content-free request-shape line (think value,\nformat kind, option keys) to pin down the unreproduced root cause.\n\nCloses #1382\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T06:05:01+02:00",
          "tree_id": "ad66ef77029c5436d908ff26ce39683e70afde6f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/79c6ae90ec40fc2b865e591d58b4856894855827"
        },
        "date": 1791433440349,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1558352,
            "range": "± 154348",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1779373,
            "range": "± 112369",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 196404,
            "range": "± 8640",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5f241803726e64880d30da8ae93df083b797e0aa",
          "message": "perf(ai): write the cover letter beside the résumé draft when the provider can serve both (#1389)\n\nThe letter reads nothing the draft produces, so it now starts with the draft and\nthe cover_letter stage reports the parked result. Runs stay sequential when both\nstages route to the same one-slot server: local Ollama, or an OpenAI-compatible\nserver on a loopback, private or LAN-named host. The letter streams under its own\nchild id (<jobId>#letter) that is cancelled with the run and is never persisted,\nlisted or announced as a job.\n\nRefs #1355\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T08:31:51+02:00",
          "tree_id": "86393171eb39c38bcc8461e2bbf745f41956c949",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/5f241803726e64880d30da8ae93df083b797e0aa"
        },
        "date": 1791442335058,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1488981,
            "range": "± 16802",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1742285,
            "range": "± 147558",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 171038,
            "range": "± 5491",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "563d44611212ad2e38e2d52746bfe865ef575241",
          "message": "fix(extension): refuse non-job pages on import, surface stalled answer drafts, and sync the side panel with saved status (#1404)\n\n- generic-HTML imports need a job signal: a named board, an ATS URL, an embedded ATS board,\n  a JSON-LD JobPosting, a job word (en/de/fr/es/it/nl/pt/pl) in the title or path, or a long\n  description with at least two job words; imported titles have their whitespace collapsed\n- a zero-chunk answer draft now shows \"No response — try again.\" instead of vanishing; a user\n  cancel still shows nothing\n- the side panel shares the popup's re-import label and applied check; switching jobs always\n  clears \"I already applied\", and a same-job refresh keeps a hand tick\n\nCloses #1399\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T14:51:04+02:00",
          "tree_id": "2de4fa4dad711aca0e671a91e8acdfc04f9aa5d1",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/563d44611212ad2e38e2d52746bfe865ef575241"
        },
        "date": 1791465372330,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1796851,
            "range": "± 32957",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2243500,
            "range": "± 45818",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 260759,
            "range": "± 8337",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "683d92020dc9c500de0ae97ec89d122c7f8ecbc8",
          "message": "fix: cap local structured calls, abandon stages on cancel, name provider errors (#1406)\n\nStructured pipeline stages on local providers (Ollama, LAN OpenAI-compatible)\nnow cap output at 8192 tokens and surface the cutoff as its own output_limit\nstop reason instead of a generic failure. Stages that opt in (strategy,\nhumanize) are abandoned the moment the run is cancelled. A mid-stream network\nbreak is retried once and re-charged; rate-limit/busy errors and the\ndeterministic cutoff are never retried. Provider, network and refusal failures\nmap to provider_error, refusal text is never logged, and a timeout at the\nlowest effort hints at a faster model instead.\n\nCloses #1391\nCloses #1393\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T19:28:13+02:00",
          "tree_id": "55a357aa28b62ba8505c2e1159a63a69eae58a39",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/683d92020dc9c500de0ae97ec89d122c7f8ecbc8"
        },
        "date": 1791481252745,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2211814,
            "range": "± 78591",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2620473,
            "range": "± 22648",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 309963,
            "range": "± 3273",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dcb931effca447ed33b06dc99f3f1097be3aa069",
          "message": "fix: prefer the full posting body over thin json-ld and tidy export names (#1407)\n\nImport: a JSON-LD description that is only a thin summary now yields to a much\nlonger body of the same posting (title in the body's heading, or the summary's\nopening in a readability extraction), never on hinted or listing pages.\nGreenhouse imports use the board's company name instead of the slug.\n\nExport: the Rust and renderer filename sanitizers follow one rule set, pinned by\na shared vector table, with a Candidate fallback. DOCX core properties carry the\ncandidate as author and real timestamps instead of \"unknown\" and 1970.\n\nCloses #1400\nCloses #1398\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T19:51:11+02:00",
          "tree_id": "7fd2a499670bc11f9629a31a3f0736e2c347aaac",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/dcb931effca447ed33b06dc99f3f1097be3aa069"
        },
        "date": 1791483221005,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1773836,
            "range": "± 26674",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2148467,
            "range": "± 50456",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 259841,
            "range": "± 2579",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7a73d053807a9c64482c6878a7a3a4718be8b33c",
          "message": "fix: name the company and keep only the ad body on greenhouse page imports (#1414)\n\nA direct job-boards.greenhouse.io page import went through the DOM path, which\nstored the raw board slug as company and kept the header logo link, location\nand Apply line at the top of the description. Read the board's own header logo\nalt (falling back to the title-cased slug) and take only the job description\nblock. Scoped to Greenhouse URLs; the shared generic parser is unchanged.\n\nCloses #1409\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T21:02:29+02:00",
          "tree_id": "40670814d0c1cabb139bfcafbcaf2ecd9fbfd333",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7a73d053807a9c64482c6878a7a3a4718be8b33c"
        },
        "date": 1791486977902,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2242352,
            "range": "± 24120",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2643058,
            "range": "± 45427",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 313256,
            "range": "± 15874",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9d66ed8ba3a582d53ba8bfb804f33eaadfcc60f3",
          "message": "fix(extension): refuse article and company pages on import and refresh the panel on status changes (#1415)\n\nThe prose-only job check accepted any long page that mentioned two job words, so\nencyclopedia articles and company about pages imported as applications. With no\nATS, JobPosting or title/path signal, the description now also needs a job-word\ndensity, a length within posting range, and a requirements-type cue (multilingual\nprefixes plus common English section phrases). The refusal now tells the user\nto open the job's own page.\n\nThe side panel now refreshes when the popup marks a job applied or imports it,\ninstead of only after a tab switch.\n\nCloses #1408\nCloses #1410\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T21:21:22+02:00",
          "tree_id": "8128e51529c0110e83dc728d73c82636fcd093d6",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9d66ed8ba3a582d53ba8bfb804f33eaadfcc60f3"
        },
        "date": 1791488092110,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2269829,
            "range": "± 40680",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2739882,
            "range": "± 20073",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 323740,
            "range": "± 6892",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8891631274c5f8ac712e4740aaa845f2c1498be2",
          "message": "fix(scraping): drop the greenhouse header on extension imports and decode escaped json-ld text (#1417)\n\nThe extension marks the whole Greenhouse <main> as the job root, so the hinted\nbranch skipped the Greenhouse description override and the stored description\nbegan with the logo link, Back to jobs, location and Apply. Apply the override on\nGreenhouse URLs whether or not the page is hinted.\n\nLever's JSON-LD carries HTML-escaped strings, and its title overrode the decoded\npage title, so \"Artist &amp; Label\" was stored. Decode entities once in JSON-LD\ntitles, company names and locations, keeping tag-shaped text intact.\n\nCloses #1409\nCloses #1416\n\n\nClaude-Session: https://claude.ai/code/session_01RwZFadYd3YUadtn2ik5TmT\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T21:52:32+02:00",
          "tree_id": "381988a1e355b093cba46c9072947f052085a299",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/8891631274c5f8ac712e4740aaa845f2c1498be2"
        },
        "date": 1791490685240,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2189307,
            "range": "± 17832",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2623601,
            "range": "± 34925",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 315224,
            "range": "± 29532",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "125562577+NimaNabi@users.noreply.github.com",
            "name": "Nima Mirzanabikhani",
            "username": "NimaNabi"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1e0d11f47023d9dd2075b5ecb797938e7cecc493",
          "message": "fix(backup): keep application status history across backup and restore (#1419)\n\n* fix(backup): keep application status history across backup and restore\n\nThe export bundle carried applications but not their status_events, and\nrestoring replaced the whole history with one synthetic \"imported\" event\nper application. That dropped provisional email-derived writes and the\nemail_reject rows email tracking relies on (ADR 0013 records the gap),\nand it is the first step of the self-hosted sync plan on #1402, which\nneeds status history as separate records.\n\n- new `applicationStatusEvents` bundle section (ApplicationStatusEvents,\n  applications/status_events_backup.rs): one record per event with\n  source and confirmed, no device-local rowid\n- restored after \"applications\", replacing the seeded events; events for\n  an unknown application are skipped; a bundle without the section\n  restores exactly as before\n- tests for the round trip, the old-bundle path, orphan events and a\n  malformed section; ADR 0013 and the stale comments updated\n\nRefs #1402\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* style(backup): apply rustfmt to the status-history section\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* test(arch): allow the status-history backup section under R3\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* test(egress): declare the docx namespace uris and the linkedin url builder\n\n#1407 and #1388 added literals the egress inventory flags but that never\nreach the network: the OOXML, Dublin Core and XML Schema namespace URIs\nin a generated DOCX's core properties, and the scheme classify.rs puts\non a LinkedIn slug from an imported CV. Declared with notes saying so,\nwhich brings `cargo test --test egress` back to green on main.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* fix(backup): keep the status-history import on the routing form the pin scans\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* fix(backup): never write an empty status history on a failed read\n\nAddresses the review on #1419:\n- a status_events row that fails to map now fails the read instead of\n  being skipped\n- build_bundle omits applicationStatusEvents when the read fails (logged\n  by error code), so a restore keeps the seeded events instead of\n  replacing the history with nothing; the trait export returns null,\n  which the restore's shape pre-pass rejects\n- tests: an order pin that status history is restored after\n  applications, and a round trip of a provisional email-sourced event\n- ADR 0013 points at ApplicationStore::import instead of restating the\n  seeded values\n\nRefs #1402\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-14T23:03:45+02:00",
          "tree_id": "e142e7952891be9a1e797611295179926faee250",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/1e0d11f47023d9dd2075b5ecb797938e7cecc493"
        },
        "date": 1792013250616,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2216555,
            "range": "± 27992",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2680723,
            "range": "± 23074",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 255940,
            "range": "± 4095",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6022e52f3b43ef9089491abad5733938bbf4793e",
          "message": "chore: bump zip from 8.6.0 to 9.0.2 in /apps/desktop/src-tauri (#1424)\n\nBumps [zip](https://github.com/zip-rs/zip2) from 8.6.0 to 9.0.2.\n- [Release notes](https://github.com/zip-rs/zip2/releases)\n- [Changelog](https://github.com/zip-rs/zip2/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/zip-rs/zip2/compare/v8.6.0...9.0.2)\n\n---\nupdated-dependencies:\n- dependency-name: zip\n  dependency-version: 9.0.2\n  dependency-type: direct:production\n  update-type: version-update:semver-major\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-10-15T22:04:16+02:00",
          "tree_id": "44e8878e6f354980bf7ac292e231ba9c83af3a92",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/6022e52f3b43ef9089491abad5733938bbf4793e"
        },
        "date": 1792096060948,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1764600,
            "range": "± 46901",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2092014,
            "range": "± 25224",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 247891,
            "range": "± 6774",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "125562577+NimaNabi@users.noreply.github.com",
            "name": "Nima Mirzanabikhani",
            "username": "NimaNabi"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "829e4f2067916563f09c332c4f1bcd9c2a89392c",
          "message": "chore(deps): bump rustls to 0.23.46 for rustsec-2026-0346..0349 (#1429)\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T23:14:47+02:00",
          "tree_id": "e7b0544285e774eb943b1274164e7e3a24606a4d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/829e4f2067916563f09c332c4f1bcd9c2a89392c"
        },
        "date": 1792100276505,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2287780,
            "range": "± 49297",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2670991,
            "range": "± 20701",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 304673,
            "range": "± 3097",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "125562577+NimaNabi@users.noreply.github.com",
            "name": "Nima Mirzanabikhani",
            "username": "NimaNabi"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c719c0d53e4bc2ef6ea211492951a1fa06cf2b46",
          "message": "feat(sync): http transport, background runs and sync commands (PR 2b) (#1430)\n\n* feat(sync): http transport, background runs and sync commands (PR 2b)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* fix(sync): register the sync_state reset label in setup order\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* fix(sync): address review on configure validation, agent exposure, token loss, autopilot lock, url check\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-17T14:08:14+02:00",
          "tree_id": "eb327fa684c9206b649406e29a9d01a49358996b",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c719c0d53e4bc2ef6ea211492951a1fa06cf2b46"
        },
        "date": 1792240029682,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1475260,
            "range": "± 45638",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1822569,
            "range": "± 110876",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 191593,
            "range": "± 9675",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "125562577+NimaNabi@users.noreply.github.com",
            "name": "Nima Mirzanabikhani",
            "username": "NimaNabi"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "709612b00c56a6f9d19840905b14e45adce48b71",
          "message": "feat(settings): self-hosted sync card under privacy (PR 3) (#1431)\n\n* feat(sync): http transport, background runs and sync commands (PR 2b)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* fix(sync): register the sync_state reset label in setup order\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* feat(settings): self-hosted sync card under privacy (PR 3)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* fix(sync): address review on configure validation, agent exposure, token loss, autopilot lock, url check\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* fix(sync): address review on status shape, url case, error text, card states and disclosures\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-17T15:18:54+02:00",
          "tree_id": "846bce4270e4630408fca7209d49bb18058204df",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/709612b00c56a6f9d19840905b14e45adce48b71"
        },
        "date": 1792243915763,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2286621,
            "range": "± 20552",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2722875,
            "range": "± 20143",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 243394,
            "range": "± 3695",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "125562577+NimaNabi@users.noreply.github.com",
            "name": "Nima Mirzanabikhani",
            "username": "NimaNabi"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "cbd80960197a01d2b9a13c0ed32bfd8e5eee7680",
          "message": "chore(deps): clear the zip <9 advisories (rustsec-2026-0367..0369) (#1432)\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-17T19:35:49+02:00",
          "tree_id": "49f81ecb378a114b5dc32d4efc3bf8d2fb7fe8af",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/cbd80960197a01d2b9a13c0ed32bfd8e5eee7680"
        },
        "date": 1792259650440,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1383094,
            "range": "± 41440",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1599109,
            "range": "± 54125",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 160404,
            "range": "± 3503",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "29712567+johnisanerd@users.noreply.github.com",
            "name": "John Cole",
            "username": "johnisanerd"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d03407ffe4fea90137d56b5a08ec96aacb77486d",
          "message": "feat(aggregator): send a keyword body for an optional LinkedIn jobs actor (#1462)\n\n* feat(aggregator): send a keyword body for an optional LinkedIn jobs actor\n\nThe default actor still receives search URLs. The optional actor id sends keyword, location, timeRange, and maxJobs, and job rows keep the existing job fields.\n\n* fix(aggregator): parse date-only postedDate values\n\nThe optional actor documents postedDate as YYYY-MM-DD in its dataset schema. parse_apify_posted_at now reads a date-only value as midnight UTC, after RFC 3339 and before the numeric epoch fallback. A mapping test covers it.",
          "timestamp": "2026-10-24T22:46:36+02:00",
          "tree_id": "554c696c4a4390e5f7ed10cfb925db925e072b3e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/d03407ffe4fea90137d56b5a08ec96aacb77486d"
        },
        "date": 1792876042517,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1770658,
            "range": "± 26734",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2082779,
            "range": "± 51785",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 245873,
            "range": "± 6149",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4c686f8e65448b50e4bac68c3e5a6a1b7ef71bb9",
          "message": "fix(deps): unbreak the linux build and clear the october security advisories (#1466)\n\n* fix(deps): unbreak the linux build and clear the october security advisories\n\ntauri-plugin-deep-link 2.6.2 (via #1465) builds ini::ParseOption with the two\nfields rust-ini 0.21.1 has; the locked 0.21.3 added two more, so every Linux CI\njob failed with E0063. Hold rust-ini at 0.21.1 until a deep-link release ships\ntauri-apps/plugins-workspace#3711.\n\nRust advisories: semver-compatible updates of quinn-proto, rustls-webpki,\nwebpki-roots, webpki-root-certs, ctutils and the zstd family; plist 1.10 and\nwayland-scanner 0.31.11 drop quick-xml 0.39. The weekly sweep now runs\ncargo deny check advisories, so it reads the same triaged ignore list as the PR\ngate instead of re-failing on it.\n\nnpm advisories: major-scoped pnpm overrides for handlebars, fast-uri,\nbrace-expansion, undici, browserslist, source-map-js, js-yaml and zod.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* fix(landing): put visible text in accessible names for axe-core 4.14\n\naxe-core 4.14 (via #1463) enforces label-content-name-mismatch (WCAG 2.5.3)\nunder the a11y gate's tags, failing 177 nodes on /, /architecture-map and\n/tech-radar. Each accessible name now contains the element's visible text,\nfixed at the component that emits it; architecture-map nodes also spell their\nbadge counts out in words. Also corrects the deny.toml quick-xml note: RustSec\nlists the fix as >=0.41.0, so 0.39.4 was never patched.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-27T01:42:53+01:00",
          "tree_id": "61bf1413d138b67eb2e1bea5941479ec02fb2adc",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4c686f8e65448b50e4bac68c3e5a6a1b7ef71bb9"
        },
        "date": 1793063253513,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2233095,
            "range": "± 63087",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2706665,
            "range": "± 68843",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 323749,
            "range": "± 14583",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "32b066b807bd72affc73516f5ac3eb7819e62992",
          "message": "fix: skip the crash supervisor inside the snap, and tidy github actions (#1469)\n\n* fix(crash-reporting): skip the minidump supervisor inside the snap\n\nThe snap's strict confinement only allows abstract Unix sockets under the\nsnap's own name, and minidumper-child always binds `temp-socket-<uuid>`, so the\nsupervisor failed with EACCES and reported that on every launch (Sentry\nDESKTOP-2) while capturing nothing. Gate on platform::snap::is_packaged(),\nwhich also requires the running exe to live under $SNAP, so a .deb or dev build\nrun from a snapped terminal keeps its crash capture.\n\nCloses #1467\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* ci: drop harden-runner, share setup composites, cache apt downloads\n\n- remove step-security/harden-runner from every workflow\n- release.yml uses the setup-node-pnpm and setup-rust composites like every\n  other workflow, so setup renders as nested groups; setup-rust gains an\n  optional cache-key input\n- setup-rust caches the downloaded .debs (keyed on runner image and package\n  list) and bounds apt with timeouts: two Build Verification runs were\n  cancelled after the Azure mirror served 65 MB at ~48 kB/s\n- pin the composite's setup-node (was an unpinned @v6), align pnpm/action-setup\n  and rust-cache, and make every pin comment the exact tag its SHA resolves to\n- persist-credentials: false on checkouts whose jobs never use git credentials;\n  repo-charts gets a top-level permissions: {}\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* docs: mark the harden-runner parts of adr-0002 and adr-0003 superseded\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-27T02:18:26+01:00",
          "tree_id": "61f88b5adb6dfb05dbae2a0f10b554d9aafb9767",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/32b066b807bd72affc73516f5ac3eb7819e62992"
        },
        "date": 1793064958195,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1439874,
            "range": "± 50168",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1634621,
            "range": "± 49368",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 166622,
            "range": "± 4091",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e23820e1772ffcfa51ec902615eee9f108abe8f3",
          "message": "chore: bump scraper from 0.27.0 to 0.29.0 in /apps/desktop/src-tauri (#1474)\n\nBumps [scraper](https://github.com/rust-scraper/scraper) from 0.27.0 to 0.29.0.\n- [Release notes](https://github.com/rust-scraper/scraper/releases)\n- [Commits](https://github.com/rust-scraper/scraper/compare/v0.27.0...v0.29.0)\n\n---\nupdated-dependencies:\n- dependency-name: scraper\n  dependency-version: 0.29.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-10-29T13:21:34+01:00",
          "tree_id": "aca8cc344fe8728b4fcbd92b370657d1438180ba",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/e23820e1772ffcfa51ec902615eee9f108abe8f3"
        },
        "date": 1793278397165,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2382700,
            "range": "± 26738",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2869251,
            "range": "± 47522",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 269715,
            "range": "± 7360",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}