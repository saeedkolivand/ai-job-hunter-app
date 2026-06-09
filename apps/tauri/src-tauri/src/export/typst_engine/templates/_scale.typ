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
#let sp-subtitle-gap  = 1pt      // space above subtitle row
#let sp-subtitle-below = 2pt     // space below subtitle row
#let sp-header-contact = 14pt    // space below contact line before first section
#let sp-name-below    = 9pt      // space below candidate name block (before contact)

// Cover-letter paragraph spacing (#26) — letters read better with a clearer gap
// between paragraphs than résumé summary lines, so the letter uses its own
// constant instead of reusing sp-para. Used only by letter.typ.
#let sp-letter-para   = 1.4em    // space between cover-letter body paragraphs

// Sidebar-specific constants (used by Atelier and any future two-column template)
#let sp-sb-section-above = 17pt  // space above a sidebar section heading
#let sp-sb-rule-below    = 3pt   // space between sidebar heading text and rule
#let sp-sb-after-rule    = 6pt   // space after sidebar rule before first block
#let sp-sb-item          = 6pt   // space between discrete sidebar items
#let sb-lead             = 0.75em // sidebar leading — looser than main column
