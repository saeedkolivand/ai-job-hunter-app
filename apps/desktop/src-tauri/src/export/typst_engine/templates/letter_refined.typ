// Cover-letter layout: REFINED (Olivia-Wilson minimalist).
//
// Same data contract as letter.typ — `data.style` / `data.opts` / LetterModel.
// This layout owns the COMPOSITION only; the palette + fonts still inherit from
// the chosen résumé template (data.style), and market conventions still own the
// WHAT/WHERE semantics (DE DIN date-top-right vs US below-header) — where they
// conflict the convention wins for formal correctness.
//
// Arrangement vs. Classic:
//   • Large sans name + role top-left, RIGHT-aligned contact block on the same
//     header row, then a full-width horizontal rule.
//   • A JOB REFERENCE line rendered from `data.subject` whenever a subject is
//     present — even when the market's `subject_line_used` is false (this layout
//     always foregrounds the reference), styled small-caps/bold label + text.
//   • Extra vertical space above the signature for a handwritten signature.
//
// House spacing constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-accent  = rgb(if "c_accent"  in st { st.c_accent  } else { "#2563EB" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#222222" })
#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#111111" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#555555" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#aaaaaa" })

#let font-name = if "font_name" in st { st.font_name } else { "Carlito" }
#let font-body = if "font_body" in st { st.font_body } else { "Carlito" }

#let name-pt = if "name_pt" in st { st.name_pt * 1pt } else { 20pt }
#let body-pt = if "body_pt" in st { st.body_pt * 1pt } else { 10.5pt }

// ── Opts resolution ───────────────────────────────────────────────────────────

#let pg-w  = if "page_width_mm"  in data.opts { data.opts.page_width_mm  * 1mm } else { 210mm }
#let pg-h  = if "page_height_mm" in data.opts { data.opts.page_height_mm * 1mm } else { 297mm }
#let lang  = if "lang"           in data.opts { data.opts.lang            } else { "en" }
#let date-pos      = if "date_position"    in data.opts { data.opts.date_position    } else { "below-header" }
#let subj-label    = if "subject_line_label" in data.opts { data.opts.subject_line_label } else { "" }

// ── Page & typography ─────────────────────────────────────────────────────────

#set page(
  width:  pg-w,
  height: pg-h,
  margin: (x: 25.4mm, y: 25.4mm),
)

#set text(
  font: (font-body, "Carlito", "Inter", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: lang,
)

#set par(leading: lead, spacing: sp-letter-para, justify: true)

// ── Rich-text renderer (identical to letter.typ / single_column.typ) ──────────

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
  block(above: 14pt, below: 14pt,
    text(size: body-pt, fill: c-date, date-str)
  )
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

// Strip a leading "<label>[:]" prefix from the subject so the small-caps label
// isn't duplicated (a DE subject already carries "Betreff: …"). Labels are ASCII
// so slicing by the label's byte length removes exactly the prefix.
#let strip-subject-label(s, label) = {
  let t = s.trim()
  if label != "" and lower(t).starts-with(lower(label)) {
    let rest = t.slice(label.len()).trim()
    if rest.starts-with(":") { rest = rest.slice(1).trim() }
    rest
  } else { t }
}

// ── Header: name + role (left), right-aligned contact (right) ─────────────────

#grid(
  columns: (1fr, auto),
  gutter: 18pt,
  {
    // Large sans name.
    text(
      size: name-pt + 4pt,
      weight: "bold",
      fill: c-name,
      font: (font-name, "Carlito", "Inter"),
      data.letterhead.name,
    )
    // Optional professional role under the name (letter-spaced caps accent).
    if "signature_title" in data and data.signature_title != none {
      block(above: 3pt,
        text(size: body-pt, fill: c-accent, tracking: 0.08em,
          upper(data.signature_title))
      )
    }
  },
  align(right + horizon,
    if "contact" in data.letterhead and data.letterhead.contact.len() > 0 {
      text(size: body-pt - 0.5pt, fill: c-body, render-runs(data.letterhead.contact))
    } else { "" }
  ),
)

// DIN top-right date: honour the market convention by placing the date
// right-aligned at the top (just under the header row, before the rule).
#if date-pos == "top-right" and "date" in data and data.date != none {
  block(above: 8pt,
    align(right, text(size: body-pt, fill: c-date, data.date))
  )
}

// Full-width horizontal rule under the header.
#block(above: 10pt, below: 12pt,
  line(length: 100%, stroke: 0.6pt + c-rule)
)

// ── Date (below-header position, non-DIN markets) ─────────────────────────────

#if date-pos == "below-header" and "date" in data and data.date != none {
  emit-date-block(data.date)
}

// ── Recipient block ───────────────────────────────────────────────────────────
// recipient_position is an alignment hint ("left"); the recipient always reads
// left-aligned above the salutation here. Rendered whenever present.

#emit-recipient-block()

// ── JOB REFERENCE line (always shown when a subject is present) ───────────────
//
// The small-caps caption is skipped when the (already label-stripped) subject
// body still opens with its own reference marker — either the configured
// market label (defensive: normally already removed by strip-subject-label
// above, e.g. DE "Betreff") or a literal "Re:" prefix a market/label-less
// subject (e.g. US) may carry as-is. Without this guard a US letter would show
// a redundant "SUBJECT / Re: …" pair; DE is unaffected because its label is
// already stripped from subj-body, so this never matches there.

#if "subject" in data and data.subject != none {
  let subj-body = strip-subject-label(data.subject, subj-label)
  let subj-body-lower = lower(subj-body.trim())
  let has-own-label = (subj-label != "" and subj-body-lower.starts-with(lower(subj-label)))
    or subj-body-lower.starts-with("re:")
  block(above: 4pt, below: 12pt, {
    if not has-own-label {
      text(
        size: body-pt - 1.5pt,
        weight: "bold",
        fill: c-accent,
        tracking: 0.1em,
        smallcaps(if subj-label != "" { subj-label } else { "Subject" }),
      )
      linebreak()
    }
    text(weight: "bold", fill: c-body, subj-body)
  })
}

// ── Date above-salutation position ────────────────────────────────────────────

#if date-pos == "above-salutation" and "date" in data and data.date != none {
  emit-date-block(data.date)
}

// ── Salutation ────────────────────────────────────────────────────────────────

#if "salutation" in data and data.salutation != none {
  block(above: 12pt, below: 8pt,
    text(fill: c-body, data.salutation)
  )
}

// ── Body paragraphs ───────────────────────────────────────────────────────────

#if "body" in data {
  for para in data.body {
    block(above: 0pt, below: sp-letter-para, breakable: true,
      render-runs(para)
    )
  }
}

// ── Sign-off + generous signature area ────────────────────────────────────────

#if "signoff" in data and data.signoff != none {
  block(above: 22pt, below: 4pt,
    text(fill: c-body, data.signoff)
  )
}

// Extra vertical space for a handwritten signature (larger than Classic).
#v(34pt)

#text(
  weight: "bold",
  fill: c-name,
  font: (font-name, "Carlito", "Inter"),
  data.signature_name,
)

#if "signature_title" in data and data.signature_title != none {
  block(above: 2pt,
    text(size: body-pt - 0.5pt, fill: c-body, data.signature_title)
  )
}
