//! One text per curated language, and the genuine wrong-language outputs the check must
//! still catch.

use super::{support::*, *};

/// One text per language [`detected_language`](crate::documents::keywords::detected_language)
/// covers — the full 19-language set `documents::keywords::locale_tag_of`
/// curates — long enough to clear [`MIN_CHARS_FOR_LANGUAGE_CHECK`] and
/// confidently detected. Built by repeating a short sentence per language
/// rather than composing longer original prose this file's author cannot
/// verify is grammatical in all nineteen languages: repetition is redundant
/// content, not wrong content, and `whatlang`'s n-gram model reads it the
/// same way it would read a longer original. The SAME sentences
/// `documents::keywords::test::detected_language_identifies_*` pins at the
/// primitive level, so a regression in either place is visible in both.
///
/// Six of the seven Latin-curated languages repeat 7×, not 3× — measured: at
/// a single instance, the worst pairwise-evidence case across all 42
/// cross-language pairs is Spanish against a French target (Spanish and
/// French share most of their short function words, so only "tiene" survives
/// pairwise pruning), at 1 hit. 7 repeats gives that pair 7 hits, two clear
/// of [`MIN_DISTINCTIVE_HITS`]'s floor. The SAME reasoning
/// [`MIN_CHARS_FOR_LANGUAGE_CHECK`] already applies to length: a document too
/// short is a poor candidate for a confident read, and a real wrong-language
/// document is never this artificially terse.
///
/// **Spanish itself stays at 3×, not 7×, and that is deliberate, not an
/// oversight.** Uniformly inflating every language to clear the WORST pair
/// left this the only test exercising non-English targets, and it exercised
/// them exclusively at 441 characters — never in the 120–315 character band
/// where [`MIN_DISTINCTIVE_HITS`] actually decides anything, which is
/// exactly the coverage gap that let the close-relative-target silent band
/// (see `a_short_paragraph_against_a_close_relative_target_is_an_accepted_miss`)
/// go unexercised here. At 3× (189 characters, squarely in that band),
/// Spanish's evidence against French is 3 — still under the floor, still
/// silent, the SAME accepted miss that test pins directly — while every
/// OTHER target (`en` 18, `de` 18, `it` 15, `pt` 9, `nl` 6) clears the floor
/// at this length; 3× was chosen, not assumed, by sweeping reps 1–7 against
/// all six other curated targets and picking the shortest length where only
/// the ONE known accepted-miss pair stays quiet. The cross-product test
/// below excludes exactly that one cell, not the whole language.
fn per_language_samples() -> Vec<(&'static str, String)> {
    let sentences: &[(&str, &str, usize)] = &[
        (
            "en",
            "The candidate has eight years of backend experience with payment systems.",
            7,
        ),
        (
            "de",
            "Die Kandidatin hat acht Jahre Erfahrung im Backend-Bereich mit Zahlungssystemen.",
            7,
        ),
        (
            "fr",
            "La candidate a huit ans d'expérience dans les systèmes de paiement back-end.",
            7,
        ),
        (
            "es",
            "La candidata tiene ocho años de experiencia en sistemas de pago de backend.",
            3,
        ),
        (
            "it",
            "La candidata ha otto anni di esperienza nei sistemi di pagamento backend.",
            7,
        ),
        (
            "pt",
            "A candidata tem oito anos de experiência em sistemas de pagamento de backend.",
            7,
        ),
        (
            "nl",
            "De kandidaat heeft acht jaar ervaring met backend-betalingssystemen.",
            7,
        ),
        ("zh", "我是一名后端工程师，在支付系统和容器平台方面工作了八年。", 5),
        (
            "ja",
            "私はバックエンドエンジニアで、決済システムとコンテナプラットフォームの構築を8年間担当してきました。",
            3,
        ),
        (
            "ko",
            "저는 8년 동안 결제 시스템과 컨테이너 플랫폼을 구축해 온 백엔드 엔지니어입니다.",
            5,
        ),
        (
            "vi",
            "Tôi là kỹ sư backend với tám năm kinh nghiệm trong các hệ thống thanh toán và nền tảng container.",
            3,
        ),
        (
            "th",
            "ฉันเป็นวิศวกรแบ็กเอนด์ที่มีประสบการณ์แปดปีในระบบชำระเงินและแพลตฟอร์มคอนเทนเนอร์",
            3,
        ),
        (
            "ar",
            "أنا مهندس أنظمة خلفية لدي ثماني سنوات من الخبرة في أنظمة الدفع ومنصات الحاويات.",
            3,
        ),
        (
            "he",
            "אני מהנדס backend עם שמונה שנות ניסיון במערכות תשלומים ופלטפורמות מכולות.",
            3,
        ),
        (
            "hi",
            "मैं एक बैकएंड इंजीनियर हूं जिसके पास भुगतान प्रणालियों और कंटेनर प्लेटफार्मों में आठ साल का अनुभव है।",
            3,
        ),
        (
            "bn",
            "আমি একজন ব্যাকএন্ড ইঞ্জিনিয়ার যার পেমেন্ট সিস্টেম এবং কন্টেইনার প্ল্যাটফর্মে আট বছরের অভিজ্ঞতা রয়েছে।",
            3,
        ),
        (
            "tr",
            "Ödeme sistemleri ve konteyner platformlarında sekiz yıllık deneyime sahip bir backend mühendisiyim.",
            3,
        ),
        (
            "uk",
            "Я бекенд-інженер з восьмирічним досвідом роботи з платіжними системами та контейнерними платформами.",
            3,
        ),
        (
            "ru",
            "Я бэкенд-инженер с восьмилетним опытом работы с платёжными системами.",
            3,
        ),
    ];
    sentences
        .iter()
        .map(|(code, sentence, reps)| {
            (
                *code,
                std::iter::repeat_n(*sentence, *reps)
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        })
        .collect()
}

/// The mechanical guard for "consistent across every language `detected_language`
/// covers": for each language, its OWN text must never fire against its own
/// target, and its text must fire against every OTHER language's target — the
/// exact per-language sweep this fix's owner asked for explicitly.
///
/// Turkish and Vietnamese are excluded from the "fires against every other
/// target" half (not from the "silent for itself" half, which holds trivially
/// for them too): they are Latin-script and so now correctly NEED
/// corroboration (`needs_distinctive_evidence`), but this crate curates no
/// `tr`/`vi` function-word vocabulary, so — same as every other
/// uncurated-language miss this module documents — a genuine `tr`/`vi`
/// mismatch goes quiet rather than firing on zero evidence. See
/// `turkish_text_needs_evidence_too_and_goes_quiet_without_a_curated_list` for
/// the fix this replaces (gating the requirement on script rather than on
/// curated-vocabulary membership, which used to let tr/vi skip corroboration
/// entirely and fire on nothing).
///
/// ONE more cell is excluded for the same reason, at the length this sample
/// set actually uses rather than in the abstract: Spanish's sample is 189
/// characters (3×, see [`per_language_samples`]'s doc), and at that length
/// its evidence against a French target is 3 — under the floor, the exact
/// close-relative-target accepted miss
/// `a_short_paragraph_against_a_close_relative_target_is_an_accepted_miss`
/// pins directly. Excluded here, not silenced by lengthening the sample to
/// clear it, so this loop stays honest about what actually holds at a
/// realistic length instead of only at the 441-character length that used to
/// hide this cell entirely.
///
/// Mutation check: hardcode `is_language_mismatch` to always return `false`
/// and every `_fires` assertion in this test goes red; hardcode it to always
/// return `true` and every `_stays_silent` assertion goes red — the table
/// cannot be satisfied by a guard that answers one way regardless of input.
#[test]
fn every_curated_language_is_silent_for_itself_and_fires_for_every_other() {
    let samples = per_language_samples();
    for (lang, text) in &samples {
        assert!(
            !is_language_mismatch(text, lang),
            "a document confidently written in {lang} must not mismatch its own target"
        );
    }
    for (lang, _) in &samples {
        for (other_lang, other_text) in &samples {
            if lang == other_lang || other_lang == &"tr" || other_lang == &"vi" {
                continue;
            }
            if other_lang == &"es" && lang == &"fr" {
                // Accepted miss at this sample's realistic 189-character length —
                // see the doc comment above.
                continue;
            }
            assert!(
                is_language_mismatch(other_text, lang),
                "a document confidently written in {other_lang} must mismatch target {lang}"
            );
        }
    }
}

/// The THIRD-language case, answered directly rather than left implicit:
/// target `"de"`, an untranslated English source, a German job ad (so the
/// target IS corroborated) — but the model returns neither German nor
/// English, it returns genuine ITALIAN prose.
///
/// **The rule catches it.** `distinctive_evidence_confirms` never needs
/// `found`'s evidence to be evaluated against a specific OTHER curated
/// language chosen in advance — it pairwise-compares `found` (whatever
/// `detected_language` actually named) against `target` in the SAME text, and
/// real Italian prose is exactly as function-word-dense as real English or
/// German prose: it carries its own distinctive evidence ("ha", "di", "che",
/// "sono", …), comfortably exceeding German's (zero, since the text is not
/// German at all), independent of whatever the SOURCE or job ad happened to
/// be written in.
#[test]
fn a_genuine_third_language_output_still_fires_against_the_real_target() {
    let italian_output = "Ho lavorato come ingegnere backend senior presso una azienda di \
        pagamenti, dove ho ridotto la latenza del servizio di cassa da 480 millisecondi a \
        90 millisecondi grazie a una cache Redis posizionata davanti al servizio di \
        contabilità. Ho eseguito i carichi di lavoro Docker su un cluster Kubernetes che \
        risponde a dodicimila richieste ogni secondo, e ho riscritto lo scheduler dei \
        tentativi in Rust, riducendo i pagamenti falliti del trentacinque per cento.";
    assert!(
        significant_chars(italian_output) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: fixture must clear the char floor"
    );
    assert!(
        matches!(
            crate::documents::keywords::detected_language(italian_output),
            Some("it")
        ),
        "premise: this must genuinely and confidently read as Italian, or the test \
         proves nothing about the third-language case specifically"
    );
    assert!(
        document_language_mismatch(italian_output, EN_SOURCE, DE_JOB_AD, "de"),
        "answer: a genuine third-language output DOES still fire — real Italian prose \
         carries plenty of its own distinctive function-word evidence, which comfortably \
         exceeds German's (zero, since the text is not German), regardless of what the \
         source or job ad were written in"
    );
}

/// The measured regression a review caught: genuine Spanish/Portuguese text
/// against a non-Spanish/Portuguese target used to go SILENT under the first
/// pass's global function-word pool, in BOTH the flowing-prose register and
/// the bullets-plus-date-column register a real résumé's EXPERIENCE section
/// actually uses. Both registers, both languages, pinned end-to-end through
/// `validate_content` so a real `ContentIssue` at `Severity::Critical` is what
/// is asserted — not just the internal predicate.
///
/// Mutation check (performed, not hypothetical): reverted
/// `pairwise_evidence_count` to prune a word the moment it appeared in ANY
/// two of the seven curated lists (the original global-pool design) — RAN,
/// went red on all four cases below (evidence dropped to 0-1, under
/// `MIN_DISTINCTIVE_HITS`), reverted.
#[test]
fn spanish_and_portuguese_regression_is_fixed_in_both_registers() {
    let es_prose = "Ingeniero de software con ocho años de experiencia en sistemas de pago y \
        plataformas de contenedores. He liderado la migración de nuestros servicios hacia \
        Kubernetes y he reducido la latencia del servicio de pagos en un cuarenta por ciento. \
        Trabajo con Rust, Python y PostgreSQL en un entorno de alta disponibilidad.";
    let es_column = "EXPERIENCIA\n\n\
        Ingeniero Backend Senior | Acme Payments | 2021 - Presente\n\
        - Reduje la latencia del checkout de 480ms a 90ms con una cache Redis\n\
        - Desplegué contenedores Docker en un clúster de Kubernetes\n\
        - Reescribí el planificador de reintentos en Rust\n\n\
        Desarrollador Backend | Globex Logistics | 2018 - 2021\n\
        - Construí una interfaz de facturación en Python y PostgreSQL para 40 almacenes\n\
        - Migré la flota a AWS con Terraform";
    let pt_prose = "Engenheiro de software com oito anos de experiência em sistemas de \
        pagamento e plataformas de contêineres. Liderei a migração dos nossos serviços para \
        Kubernetes e reduzi a latência do serviço de pagamentos em quarenta por cento. \
        Trabalho com Rust, Python e PostgreSQL em um ambiente de alta disponibilidade.";
    let pt_column = "EXPERIÊNCIA\n\n\
        Engenheiro Backend Sênior | Acme Payments | 2021 - Presente\n\
        - Reduzi a latência do checkout de 480ms para 90ms com um cache Redis\n\
        - Implantei contêineres Docker em um cluster Kubernetes\n\
        - Reescrevi o agendador de tentativas em Rust\n\n\
        Desenvolvedor Backend | Globex Logistics | 2018 - 2021\n\
        - Construí uma interface de faturamento em Python e PostgreSQL para 40 armazéns\n\
        - Migrei a frota para AWS com Terraform";

    for (name, text, lang) in [
        ("es_prose", es_prose, "es"),
        ("es_column", es_column, "es"),
        ("pt_prose", pt_prose, "pt"),
        ("pt_column", pt_column, "pt"),
    ] {
        assert!(
            significant_chars(text) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
            "premise[{name}]: fixture must clear the char floor"
        );
        assert_eq!(
            crate::documents::keywords::detected_language(text),
            Some(lang),
            "premise[{name}]: must confidently read as {lang}, or this proves nothing \
             about the regression"
        );
        assert!(
            document_language_mismatch(text, EN_SOURCE, EN_JOB_AD, "en"),
            "[{name}] a genuinely {lang} document against an English target must still \
             raise the mismatch — this is the measured true-positive regression"
        );
        let report = report_against(text, EN_SOURCE);
        let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
        assert_eq!(
            hits[0].severity,
            Severity::Critical,
            "[{name}] the document-level mismatch must be a real Critical, not merely a \
             true predicate"
        );
        assert!(!report.ok, "[{name}] a Critical must block the report");
    }
}

/// The title-case-sandwich exclusion silenced more than German: ANY document
/// rendered in Title Case (a real résumé-theme style, not a synthetic
/// construction) put every function word between two capitalised
/// neighbours, in every curated language. Pinned here for French — German
/// already has nominal-register coverage above, and this proves the fix is
/// not German-specific.
#[test]
fn title_case_rendered_french_document_is_critical() {
    let fr_title_case = "La Candidate A Huit Ans D'Expérience Dans Les Systèmes De Paiement \
        Back-End Et A Dirigé La Migration De Nos Services Vers Kubernetes En Réduisant La \
        Latence Du Service De Paiement De Quarante Pour Cent Cette Année Dans L'Entreprise.";
    assert!(
        significant_chars(fr_title_case) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: fixture must clear the char floor"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(fr_title_case),
        Some("fr"),
        "premise: must confidently read as French"
    );
    assert!(
        document_language_mismatch(fr_title_case, EN_SOURCE, EN_JOB_AD, "en"),
        "a Title-Case-rendered French document must still fire — every function word \
         sitting between two capitalised neighbours is a rendering choice, not evidence \
         the text is a proper noun"
    );
    let report = report_against(fr_title_case, EN_SOURCE);
    let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert!(!report.ok);
}
