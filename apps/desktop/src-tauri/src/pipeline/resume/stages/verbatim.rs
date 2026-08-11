//! The verbatim-quote filter — the mechanical half of "the model may quote the
//! résumé, never write it".
//!
//! A `sourceQuote` the model returns is only evidence if it is actually IN the
//! source résumé. This is the check, and its outcome is DROP, never repair: a
//! quote that is nearly right is a paraphrase, and a paraphrase presented as
//! the candidate's own words is precisely the fabrication the whole pipeline is
//! built to prevent. Re-asking for it would spend money to be lied to more
//! carefully.

/// Two normalizations are allowed, and only two.
///
/// * **Whitespace runs collapse to one space** (and the ends are trimmed).
///   Résumé text arrives from PDF extraction full of double spaces, non-breaking
///   spaces and hard wraps; a model that re-emits the same words with one space
///   instead of two has copied the line, and dropping it would delete real
///   evidence over a rendering artifact.
/// * **Case is ignored.** Models routinely normalize the capitalization of a
///   heading-ish line. The WORDS are what has to be the candidate's.
///
/// Nothing else is: no punctuation stripping, no stemming, no fuzzy distance. A
/// changed number, a dropped qualifier, an added adjective — every one of those
/// changes what the candidate claimed, and every one of them fails here.
fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Minimum length (in normalized characters) a quote must have to count.
///
/// Below this a "quote" is a word, and a word is trivially a substring of any
/// résumé — "Go", "led", "AWS" would all pass the substring test while
/// evidencing nothing. The filter's job is to prove the model copied a LINE.
const MIN_QUOTE_CHARS: usize = 12;

/// Whether `quote` is a verbatim span of `source`, under the two normalizations
/// above.
///
/// An empty quote is `false` — but callers treat "no quote" and "a dropped
/// quote" identically anyway (both leave `source_quote` empty), so this is
/// about the SHAPE of the answer, not a distinction the artifact carries.
pub fn is_verbatim(source: &str, quote: &str) -> bool {
    let quote = normalize(quote);
    if quote.chars().count() < MIN_QUOTE_CHARS {
        return false;
    }
    normalize(source).contains(&quote)
}

/// Blank a quote that is not verbatim, returning whether it survived.
///
/// Blanking rather than dropping the whole item: the REQUIREMENT is still real
/// and its status is still computed from the source, so the honest artifact is
/// "this requirement, no supporting quote" — which is also what
/// `stages::strategy` needs in order to not emphasize it.
pub fn keep_or_blank(source: &str, quote: &mut String) -> bool {
    if is_verbatim(source, quote) {
        return true;
    }
    quote.clear();
    false
}
