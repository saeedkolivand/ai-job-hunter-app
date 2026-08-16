// Cover-letter layout: NAVY (Cologne Navy's companion).
//
// Same data contract as letter.typ / letter_refined.typ — `data.style`,
// `data.opts`, LetterModel. This layout owns the COMPOSITION only; the palette
// and fonts still inherit from the chosen résumé template (`data.style`), and
// market conventions still own the WHAT/WHERE semantics (DE DIN date-top-right
// vs US below-header) — where they conflict the convention wins.
//
// IMPORTANT: this is a SHARED layout, not "Cologne Navy's letter". Résumé
// template and letter layout are orthogonal axes in this app — the user picks a
// layout independently of the template, so this arrangement is offered to every
// template and inherits whichever palette is active. It is styled to pair with
// Cologne Navy, but a Regent user selecting it gets Regent's burgundy.
//
// Arrangement vs. Refined:
//   • CENTRED letterhead — name in tracked caps, then the contact line beneath,
//     mirroring the résumé's centred header rather than Refined's left/right
//     split.
//   • A rule directly under the letterhead, matching the résumé's ruled section
//     headings (same 0.9pt weight).
//   • Section-style small-caps subject caption, as Refined, but centred header
//     means date/recipient sit flush left below the rule.
//
// Tracking is 0.06em on the name — deliberately NOT the 0.14em the Cologne Navy
// design brief asked for. Wider tracking makes the PDF text extract
// letter-by-letter ("À LVA R O"), which an ATS reads as gibberish. Same reason,
// same value, as `cologne_navy.typ`; see the note there.
//
// House spacing constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-accent  = rgb(if "c_accent"  in st { st.c_accent  } else { "#1F5C99" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#1A1A1A" })
#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#1F3864" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#4A4A4A" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#1F3864" })

#let font-name = if "font_name" in st { st.font_name } else { "Carlito" }
#let font-body = if "font_body" in st { st.font_body } else { "Carlito" }

#let name-pt = if "name_pt" in st { st.name_pt * 1pt } else { 20pt }
#let body-pt = if "body_pt" in st { st.body_pt * 1pt } else { 10.5pt }

// ── Opts resolution ───────────────────────────────────────────────────────────

#let pg-w  = if "page_width_mm"  in data.opts { data.opts.page_width_mm  * 1mm } else { 210mm }
#let pg-h  = if "page_height_mm" in data.opts { data.opts.page_height_mm * 1mm } else { 297mm }
#let lang  = if "lang"           in data.opts { data.opts.lang            } else { "en" }
#let date-pos   = if "date_position"      in data.opts { data.opts.date_position      } else { "below-header" }
#let subj-label = if "subject_line_label" in data.opts { data.opts.subject_line_label } else { "" }

// ── Page & typography ─────────────────────────────────────────────────────────

#set page(width: pg-w, height: pg-h, margin: (x: 25.4mm, y: 25.4mm))

#set text(
  font: (font-body, "Carlito", "Inter", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: lang,
)

// `hyphenate: false` is load-bearing, not a style choice. Typst turns
// hyphenation on with `justify`, and a hyphenated line break puts a real break
// in the PDF text layer: "microservices architecture" extracts as
// "architec­ture", so an ATS tokenising on whitespace loses the keyword
// entirely.
//
// Justify stays ON for every market, German included: letter bodies are a
// single full-measure column (not the résumé's narrower multi-block layout),
// so justified rivers are a non-issue at this width — the owner's explicit
// call, not a deferred TODO.
#set text(hyphenate: false)
#set par(leading: lead, spacing: sp-letter-para, justify: true)

// ── Rich-text renderer (identical to letter.typ / letter_refined.typ) ─────────

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

// Strip a leading "<label>[:]" prefix so the small-caps caption isn't
// duplicated (a DE subject already carries "Betreff: …").
#let strip-subject-label(s, label) = {
  let t = s.trim()
  if label != "" and lower(t).starts-with(lower(label)) {
    let rest = t.slice(label.len()).trim()
    if rest.starts-with(":") { rest = rest.slice(1).trim() }
    rest
  } else { t }
}

// ── Centred letterhead ────────────────────────────────────────────────────────

#align(center)[
  #text(
    size: name-pt + 2pt,
    weight: "bold",
    fill: c-name,
    font: (font-name, "Carlito", "Inter"),
    tracking: 0.06em,
    upper(data.letterhead.name),
  )

  #if "signature_title" in data and data.signature_title != none {
    block(above: 4pt, text(size: body-pt, fill: c-date, data.signature_title))
  }

  #if "contact" in data.letterhead and data.letterhead.contact.len() > 0 {
    block(above: sp-name-below,
      text(size: body-pt - 0.5pt, fill: c-body, render-runs(data.letterhead.contact)),
    )
  }
]

// DIN top-right date: honour the market convention even under a centred header.
#if date-pos == "top-right" and "date" in data and data.date != none {
  block(above: 8pt, align(right, text(size: body-pt, fill: c-date, data.date)))
}

// Rule under the letterhead — same 0.9pt navy weight as the résumé's section
// rules, which is what ties the two documents together visually.
#block(above: 10pt, below: 12pt, line(length: 100%, stroke: 0.9pt + c-rule))

// ── Date (below-header, non-DIN markets) ──────────────────────────────────────

#if date-pos == "below-header" and "date" in data and data.date != none {
  emit-date-block(data.date)
}

// ── Recipient ─────────────────────────────────────────────────────────────────

#emit-recipient-block()

// ── Subject / job reference ───────────────────────────────────────────────────
//
// The small-caps caption is skipped when the (already label-stripped) subject
// still opens with its own marker — the configured market label, or a literal
// "Re:" a label-less market may carry — so a US letter never shows a redundant
// "SUBJECT / Re: …" pair.

#if "subject" in data and data.subject != none {
  let subj-body = strip-subject-label(data.subject, subj-label)
  let subj-body-lower = lower(subj-body.trim())
  let label-match = subj-label != "" and subj-body-lower.starts-with(lower(subj-label))
  let re-match = subj-body-lower.starts-with("re:")
  let has-own-label = label-match or re-match
  block(above: 4pt, below: 12pt, {
    if not has-own-label {
      text(
        size: body-pt - 1.5pt,
        weight: "bold",
        fill: c-name,
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
  block(above: 22pt, below: 4pt, text(fill: c-body, data.signoff))
}

#v(30pt)

#text(
  weight: "bold",
  fill: c-name,
  font: (font-name, "Carlito", "Inter"),
  data.letterhead.name,
)
