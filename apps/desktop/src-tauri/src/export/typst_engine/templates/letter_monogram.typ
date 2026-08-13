// Cover-letter layout: MONOGRAM (initials device + name lockup header block).
//
// Same data contract as letter.typ / letter_refined.typ / letter_banded.typ —
// `data.style`, `data.opts`, LetterModel. This layout owns the COMPOSITION only;
// the palette and fonts inherit from the chosen résumé template (`data.style`),
// and market conventions still own the WHAT/WHERE semantics (DE DIN
// date-top-right vs US below-header) — where they conflict, the convention wins.
//
// IMPORTANT: a SHARED layout, not "Awesome's letter". Résumé template and letter
// layout are orthogonal axes here. Styled to pair with the bold-header résumé
// families (Awesome, Jake, Throughline), but offered to every template and
// inheriting whichever palette is active.
//
// Arrangement:
//   • A header BLOCK: a square monogram device (up to two initials, from
//     `data.letterhead.initials`, derived in Rust by `monogram_initials`) beside
//     a lockup of name / role / contact.
//   • A full-width accent rule under the block.
//   • The whole correspondence (date, recipient, subject, salutation, body,
//     sign-off) below it in one column.
//
// ── ATS mode (data.opts.ats) ──────────────────────────────────────────────────
// The device is the one genuinely decorative element, and it is worse than a
// tint: its initials are real text, so extraction yields "JS Jane Smith …" —
// two characters of noise ahead of the candidate's actual name. Under ATS mode
// the device is dropped entirely and only the lockup remains, left-aligned at
// the margin. Nothing else changes: no words are added or removed either way,
// and the rule stays (a rule carries no text). Gated on `data.opts.ats`, never
// on the layout id.
//
// The device is a PALE accent square with accent-coloured initials rather than
// a saturated square with reversed-out white text: the accent comes from
// whichever résumé template is active, so a light accent would put white on
// near-white. Pale-fill + dark-ink is legible for every palette, and it is the
// same 85 % lightening `letter_banded.typ` and `docx::band_tint_hex` already
// use — which is what lets the DOCX approximation match it exactly.
//
// House spacing constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-accent  = rgb(if "c_accent"  in st { st.c_accent  } else { "#2563EB" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#222222" })
#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#111111" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#555555" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#aaaaaa" })

#let c-device = c-accent.lighten(85%)

#let font-name = if "font_name" in st { st.font_name } else { "Carlito" }
#let font-body = if "font_body" in st { st.font_body } else { "Carlito" }

#let name-pt = if "name_pt" in st { st.name_pt * 1pt } else { 20pt }
#let body-pt = if "body_pt" in st { st.body_pt * 1pt } else { 10.5pt }

// ── Opts resolution ───────────────────────────────────────────────────────────

#let pg-w  = if "page_width_mm"  in data.opts { data.opts.page_width_mm  * 1mm } else { 210mm }
#let pg-h  = if "page_height_mm" in data.opts { data.opts.page_height_mm * 1mm } else { 297mm }
#let lang  = if "lang"           in data.opts { data.opts.lang            } else { "en" }
#let date-pos   = if "date_position"      in data.opts { data.opts.date_position      } else { "below-header" }
#let subj-used  = if "subject_line_used"  in data.opts { data.opts.subject_line_used  } else { false }
#let subj-label = if "subject_line_label" in data.opts { data.opts.subject_line_label } else { "" }
#let ats        = if "ats"                in data.opts { data.opts.ats                } else { false }

// ── Device geometry ───────────────────────────────────────────────────────────
// Fixed size, so a three-word name can never grow the square: `initials` is
// capped at two characters in Rust for exactly this reason.

#let device-size = 44pt
#let device-gap  = 12pt

// ── Page & typography ─────────────────────────────────────────────────────────

#set page(width: pg-w, height: pg-h, margin: (x: 25.4mm, y: 25.4mm))

#set text(
  font: (font-body, "Carlito", "Inter", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: lang,
)

#set par(leading: lead, spacing: sp-letter-para, justify: true)

// ── Rich-text renderer (identical to letter.typ / letter_banded.typ) ──────────

#let render-runs(runs) = {
  for r in runs {
    let t = if r.bold and r.italic {
      text(weight: "bold", style: "italic", r.text)
    } else if r.bold {
      text(weight: "bold", r.text)
    } else if r.italic {
      text(style: "italic", r.text)
    } else {
      r.text
    }
    if "link" in r and r.link != none {
      link(r.link, text(fill: c-accent, t))
    } else {
      t
    }
  }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#let emit-date-block(date-str) = {
  block(above: 14pt, below: 14pt, text(size: body-pt, fill: c-date, date-str))
}

#let emit-recipient-block() = {
  if "recipient_lines" in data and data.recipient_lines.len() > 0 {
    block(above: 12pt, below: 12pt, {
      for line in data.recipient_lines {
        text(fill: c-body, line)
        linebreak()
      }
    })
  }
}

// Name / role / contact lockup — identical in both modes.
#let lockup = [
  #set par(justify: false, leading: lead)
  #text(
    size: name-pt + 1pt,
    weight: "bold",
    fill: c-name,
    font: (font-name, "Carlito", "Inter"),
    // 0.04em — inside the 0.03–0.10em band every template here uses; wider
    // tracking makes the PDF extract the name letter-by-letter.
    tracking: 0.04em,
    data.letterhead.name,
  )

  #if "signature_title" in data and data.signature_title != none {
    block(above: 5pt, text(size: body-pt, fill: c-date, data.signature_title))
  }

  #if "contact" in data.letterhead and data.letterhead.contact.len() > 0 {
    block(above: 6pt, text(size: body-pt - 0.5pt, fill: c-body, render-runs(data.letterhead.contact)))
  }
]

// The monogram square. Skipped entirely when there are no initials to show (a
// letterhead-less letter parses to an empty name), so an empty box never
// appears next to a nameless lockup.
#let device = box(
  width: device-size,
  height: device-size,
  fill: c-device,
  radius: 3pt,
  stroke: 0.8pt + c-accent,
  align(center + horizon,
    text(
      size: device-size * 0.42,
      weight: "bold",
      fill: c-accent,
      font: (font-name, "Carlito", "Inter"),
      tracking: 0.02em,
      data.letterhead.initials,
    ),
  ),
)

// ── Header block ──────────────────────────────────────────────────────────────

#let show-device = not ats and "initials" in data.letterhead and data.letterhead.initials != ""

#if show-device {
  // `align: horizon` on the grid itself: the device and the lockup are
  // different heights (a two-line lockup is shorter than the square, a
  // four-line one taller), and centring them against each other keeps the
  // device optically attached to the name for either.
  grid(
    columns: (device-size, 1fr),
    column-gutter: device-gap,
    align: horizon,
    device,
    lockup,
  )
} else {
  lockup
}

// Rule under the header block. Kept in ATS mode: a rule is a vector line with
// no text content, so it costs a parser nothing.
#block(above: 12pt, below: 14pt,
  line(length: 100%, stroke: (if ats { 0.6pt + c-rule } else { 1pt + c-accent })),
)

// ── Date ──────────────────────────────────────────────────────────────────────

#if date-pos == "top-right" and "date" in data and data.date != none {
  block(below: 6pt, align(right, text(size: body-pt, fill: c-date, data.date)))
}

#if date-pos == "below-header" and "date" in data and data.date != none {
  emit-date-block(data.date)
}

// ── Recipient ─────────────────────────────────────────────────────────────────

#emit-recipient-block()

// ── Subject line (honours the market's subject_line_used) ─────────────────────

#if subj-used and "subject" in data and data.subject != none {
  block(above: 8pt, below: 8pt, {
    if subj-label != "" {
      text(size: body-pt - 1.5pt, weight: "bold", fill: c-name, tracking: 0.1em, smallcaps(subj-label))
      linebreak()
    }
    text(weight: "bold", fill: c-body, data.subject)
  })
}

// ── Date above-salutation position ────────────────────────────────────────────

#if date-pos == "above-salutation" and "date" in data and data.date != none {
  emit-date-block(data.date)
}

// ── Salutation ────────────────────────────────────────────────────────────────

#if "salutation" in data and data.salutation != none {
  block(above: 12pt, below: 8pt, text(fill: c-body, data.salutation))
}

// ── Body ──────────────────────────────────────────────────────────────────────

#if "body" in data {
  for para in data.body {
    block(above: 0pt, below: sp-letter-para, breakable: true, render-runs(para))
  }
}

// ── Sign-off + signature ──────────────────────────────────────────────────────

#if "signoff" in data and data.signoff != none {
  block(above: 20pt, below: 4pt, text(fill: c-body, data.signoff))
}

#v(28pt)

#text(
  weight: "bold",
  fill: c-name,
  font: (font-name, "Carlito", "Inter"),
  data.signature_name,
)
