// Cover-letter layout: BANDED (Belinda-Davidson angled accent band).
//
// Same data contract as letter.typ — `data.style` / `data.opts` / LetterModel.
// The palette + fonts inherit from the chosen résumé template (data.style); the
// band tint is derived from that template's accent, so the letterhead always
// matches the résumé family. Market conventions still own the WHAT/WHERE
// semantics (DE DIN date-top-right, DE/UK subject line).
//
// Arrangement vs. Classic:
//   • An angled pale accent band across the top of PAGE 1 ONLY (decorative,
//     drawn behind the text via page(background:) + a page-counter guard so it
//     never repeats on later pages — the same technique the résumé templates use
//     for their page-1 header bands).
//   • Small-caps name (in the template's name font, serif fallback) over the
//     band; right-aligned stacked contact; bold recipient + date; a short
//     accent rule footer after the signature.
//
// House spacing constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-accent  = rgb(if "c_accent"  in st { st.c_accent  } else { "#2563EB" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#222222" })
#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#111111" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#555555" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#aaaaaa" })

// Pale tint of the accent for the decorative band (very light so dark name text
// stays legible over it).
#let c-band = c-accent.lighten(85%)

#let font-name = if "font_name" in st { st.font_name } else { "Carlito" }
#let font-body = if "font_body" in st { st.font_body } else { "Carlito" }

#let name-pt = if "name_pt" in st { st.name_pt * 1pt } else { 20pt }
#let body-pt = if "body_pt" in st { st.body_pt * 1pt } else { 10.5pt }

// ── Opts resolution ───────────────────────────────────────────────────────────

#let pg-w  = if "page_width_mm"  in data.opts { data.opts.page_width_mm  * 1mm } else { 210mm }
#let pg-h  = if "page_height_mm" in data.opts { data.opts.page_height_mm * 1mm } else { 297mm }
#let lang  = if "lang"           in data.opts { data.opts.lang            } else { "en" }
#let date-pos      = if "date_position"    in data.opts { data.opts.date_position    } else { "below-header" }
#let subj-used     = if "subject_line_used"  in data.opts { data.opts.subject_line_used  } else { false }
#let subj-label    = if "subject_line_label" in data.opts { data.opts.subject_line_label } else { "" }

// ── Band geometry ─────────────────────────────────────────────────────────────
// Full-bleed angled band: level along the top edge, sloping down to the left.
//
// band-right-h must clear the header (name + right-aligned contact stack),
// since that content sits at the page's right edge where the band is
// shortest. Measured empirically (Typst `here().position()` debug probe,
// SwissMinimal, both A4 and US-Letter, 2-item and 5-item contact profiles):
// the header consistently bottoms out at ~38.9mm from the page top (contact
// stays on one line at realistic widths). 44mm leaves a ~5mm margin.
#let band-right-h = 44mm
#let band-left-h  = 54mm

// ── Page & typography ─────────────────────────────────────────────────────────
// Extra top margin so the header sits inside the band; the band is drawn once,
// on page 1 only, behind all content.

#set page(
  width:  pg-w,
  height: pg-h,
  margin: (left: 25.4mm, right: 25.4mm, top: 22mm, bottom: 25.4mm),
  background: context {
    if counter(page).get().first() == 1 {
      place(top + left,
        polygon(
          fill: c-band,
          (0mm, 0mm),
          (pg-w, 0mm),
          (pg-w, band-right-h),
          (0mm, band-left-h),
        )
      )
    }
  },
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
    text(size: body-pt, weight: "bold", fill: c-date, date-str)
  )
}

#let emit-recipient-block() = {
  if "recipient_lines" in data and data.recipient_lines.len() > 0 {
    block(above: 12pt, below: 12pt, {
      for line in data.recipient_lines {
        text(weight: "bold", fill: c-body, line)
        linebreak()
      }
    })
  }
}

// ── Header: small-caps name (template's name font, serif fallback) over the band
// DE DIN convention: name (left) + date (right) on one row. Otherwise name only.

#let name-block = smallcaps(text(
  size: name-pt + 2pt,
  weight: "bold",
  fill: c-name,
  font: (font-name, "Source Serif 4", "Carlito"),
  tracking: 0.04em,
  data.letterhead.name,
))

#if date-pos == "top-right" {
  grid(
    columns: (1fr, auto),
    gutter: 8pt,
    align(left + horizon, name-block),
    align(right + horizon,
      if "date" in data and data.date != none {
        text(size: body-pt, weight: "bold", fill: c-date, data.date)
      } else { "" }
    ),
  )
} else {
  name-block
}

// Right-aligned stacked contact line (below the name, still over the band).
#if "contact" in data.letterhead and data.letterhead.contact.len() > 0 {
  block(above: 5pt,
    align(right,
      text(size: body-pt - 0.5pt, fill: c-body, render-runs(data.letterhead.contact))
    )
  )
}

// Space to clear the band before the correspondence block begins.
#v(12pt)

// ── Date (below-header position, non-DIN markets) ─────────────────────────────

#if date-pos == "below-header" and "date" in data and data.date != none {
  emit-date-block(data.date)
}

// ── Recipient block (bold) ────────────────────────────────────────────────────

#emit-recipient-block()

// ── Subject line (DIN / formal markets — honours subject_line_used) ───────────

#if subj-used and "subject" in data and data.subject != none {
  block(above: 8pt, below: 8pt,
    text(weight: "bold", fill: c-accent, data.subject)
  )
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

// ── Sign-off + signature ──────────────────────────────────────────────────────

#if "signoff" in data and data.signoff != none {
  block(above: 20pt, below: 4pt,
    text(fill: c-body, data.signoff)
  )
}

#v(20pt)

#smallcaps(text(
  weight: "bold",
  fill: c-name,
  font: (font-name, "Source Serif 4", "Carlito"),
  tracking: 0.03em,
  data.signature_name,
))

#if "signature_title" in data and data.signature_title != none {
  block(above: 2pt,
    text(size: body-pt - 0.5pt, fill: c-body, data.signature_title)
  )
}

// Short accent rule footer.
#block(above: 14pt,
  line(length: 28%, stroke: 1.2pt + c-accent)
)
