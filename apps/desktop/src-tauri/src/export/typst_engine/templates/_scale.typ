// Shared house spacing scale — single source of truth for all templates.
//
// All templates #include this file (prepended by engine.rs before the template
// source) so that vertical rhythm is identical across Classic, Atelier,
// Single-Column, and any future template.  Values are LOCKED — change here
// only, never inside individual template files.
//
// Main-column constants
// Rhythm principle (#25): more air BETWEEN groups (sections, entries), slightly
// tighter WITHIN a group (bullets) — stronger visual hierarchy / scannability.
#let lead             = 0.78em   // paragraph line-spacing (leading)
#let sp-para          = 1.0em    // space between paragraphs / summary lines
#let sp-section-above = 20pt     // space above a section heading
#let sp-rule-below    = 4pt      // space between heading text and the rule
#let sp-after-rule    = 8pt      // space after the rule before first block
#let sp-entry         = 15pt     // space below each entry block
#let sp-bullet-above  = 7pt      // space above the bullet list inside an entry
#let sp-bullet-gap    = 5pt      // space BETWEEN bullet items within an entry
#let sp-subtitle-gap  = 6pt      // space above subtitle row — 1pt, then 3pt,
                                  // both still read as crammed against the
                                  // title in owner review (academic, aria,
                                  // atelier, awesome, cadence, "almost all of
                                  // them") across Experience/Projects/
                                  // Education. 6pt is the step that reads as a
                                  // deliberate paragraph-like gap rather than
                                  // a rounding artifact, while staying well
                                  // inside the tight entry-block rhythm (under
                                  // half of sp-entry's 15pt). Judged by eye
                                  // against rendered output, not a formula.
#let sp-subtitle-below = 2pt     // space below subtitle row
#let sp-header-contact = 14pt    // space below contact line before first section
#let sp-name-below    = 9pt      // space below candidate name block (before contact)
#let sp-header-title-below = 6pt // space below the role/title line before the contact line

// Cover-letter paragraph spacing (#26) — letters read better with a clearer gap
// between paragraphs than résumé summary lines, so the letter uses its own
// constant instead of reusing sp-para. Used only by letter.typ.
#let sp-letter-para   = 1.4em    // space between cover-letter body paragraphs

// Cover-letter signature block. All six letter layouts used to hardcode this
// gap independently (20pt/22pt block-above for the sign-off, then
// 20/28/30/34pt of `v()` before the printed name) — six different numbers
// for the same visual unit, the same "nobody used the shared token" pattern
// sp-name-below existed to fix. Owner review still read it as cramped/
// disjointed across banded, classic and refined even at the largest of those
// per-template values, so the fix is grouping (sign-off + name + role as one
// non-breakable block — see the `.typ` files) with ONE shared gap, not a
// bigger per-template literal.
#let sp-signature-lead = 20pt    // space between the last body paragraph and the sign-off line
#let sp-signature-gap  = 26pt    // room for a handwritten signature, between the sign-off and the printed name

// Sidebar-specific constants (used by Atelier and any future two-column template)
#let sp-sb-section-above = 17pt  // space above a sidebar section heading
#let sp-sb-rule-below    = 3pt   // space between sidebar heading text and rule
#let sp-sb-after-rule    = 6pt   // space after sidebar rule before first block
#let sp-sb-item          = 6pt   // space between discrete sidebar items
#let sb-lead             = 0.75em // sidebar leading — looser than main column
