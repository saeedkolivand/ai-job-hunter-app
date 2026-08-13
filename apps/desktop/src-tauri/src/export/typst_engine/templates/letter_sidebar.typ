// Cover-letter layout: SIDEBAR (tinted contact rail beside the letter body).
//
// Same data contract as letter.typ / letter_refined.typ / letter_banded.typ —
// `data.style`, `data.opts`, LetterModel. This layout owns the COMPOSITION only;
// the palette and fonts inherit from the chosen résumé template (`data.style`),
// and market conventions own the WHAT/WHERE semantics that this layout can
// honour — which date position is used (DE DIN top-right vs US below-header)
// and whether a subject line renders. Where those conflict with the
// arrangement, the convention wins.
//
// LIMITATION — NOT window-envelope compatible. DIN 5008 Form B puts the
// Anschriftenfeld 20 mm from the left edge; this layout's recipient block starts
// at 62 mm, behind the rail's own margin, so a DE letter printed for a
// window envelope will not show the address through the window. This is not a
// "narrow the rail for DE" fix: a rail that cleared 20 mm would be under 20 mm
// wide, which cannot hold a contact line at 9.5 pt — DIN compliance and a
// contact rail want the same strip of paper. Sidebar is therefore a
// screen/attachment layout; DE users who need the window envelope should pick
// Classic or Navy, whose recipient blocks sit at the ordinary margin. Recorded
// here rather than silently implied, because the paragraph above used to read
// as if every DE convention was honoured.
//
// IMPORTANT: a SHARED layout, not "Atelier's letter". Résumé template and letter
// layout are orthogonal axes here — the user picks a layout independently of the
// template, so this arrangement is offered to every template and inherits
// whichever palette is active. It is styled to pair with the sidebar résumé
// families (Atelier, Aria, Saffron, Deedy); a Regent user selecting it gets
// Regent's burgundy rail.
//
// Arrangement:
//   • The LEFT MARGIN is widened to `rail-w + rail-gutter` and filled with a
//     pale accent rail (every page, so the wide margin never reads as an
//     accident on page 2).
//   • The letterhead — name, role, contact — is `place`d into that rail, level
//     with the first body line. `place` does not participate in layout, so the
//     rail can never push the body around or change where it breaks.
//   • Everything else (date, recipient, subject, salutation, body, sign-off)
//     runs in ONE column beside the rail, in the same order as every other
//     layout.
//
// ── ATS mode (data.opts.ats) ──────────────────────────────────────────────────
// The rail is decorative: it is a tint plus a repositioning of text that exists
// either way. Under ATS mode the rail rectangle is not drawn, the margins go
// back to symmetric, and the letterhead is emitted as a plain stacked block at
// the top of the single column — same words, same order, no tint, no side
// panel. Gated on `data.opts.ats`, never on the layout id.
//
// Reading order note: even in design mode the letterhead is emitted FIRST in the
// content stream (it is placed from the top of the flow, before the date), so
// text extraction reads letterhead → date → recipient → body → sign-off. The
// rail is a margin position, not a second column of prose.
//
// House spacing constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-accent  = rgb(if "c_accent"  in st { st.c_accent  } else { "#2563EB" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#222222" })
#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#111111" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#555555" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#aaaaaa" })

// Pale tint of the accent for the rail — the same 85 % lightening
// `letter_banded.typ` uses for its band and `docx::band_tint_hex` mirrors, so
// dark ink stays legible on top and the DOCX approximation matches.
#let c-rail = c-accent.lighten(85%)

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

// ── Rail geometry ─────────────────────────────────────────────────────────────
// Fixed millimetre constants, not measured content: the rail is `place`d, so
// nothing about the body depends on how tall the letterhead turns out.

#let rail-w      = 52mm   // tinted panel width
#let rail-pad    = 7mm    // inset of the rail text from the page edge
#let rail-gutter = 10mm   // clear space between the rail and the body column
#let margin-y    = 25.4mm
#let margin-r    = 22mm

// Left margin: wide enough for the rail plus its gutter in design mode, the
// ordinary symmetric margin under ATS mode.
#let margin-l = if ats { 25.4mm } else { rail-w + rail-gutter }

// ── Page & typography ─────────────────────────────────────────────────────────

#set page(
  width:  pg-w,
  height: pg-h,
  margin: (left: margin-l, right: if ats { 25.4mm } else { margin-r }, top: margin-y, bottom: margin-y),
  background: if ats { none } else {
    place(top + left, rect(width: rail-w, height: 100%, fill: c-rail, stroke: none))
  },
)

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
// entirely. Justification stays (wider word gaps, no split words).
//
// The four SHIPPED layouts (letter.typ, letter_refined.typ, letter_banded.typ,
// letter_navy.typ) all still hyphenate — that is pre-existing and is being
// swept in its own PR, together with the justify-vs-rivers call for DE.
#set text(hyphenate: false)
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

// The letterhead itself — identical content in both modes, only the type size
// and the container change. Ragged-right: a 38 mm rail cannot justify without
// opening rivers, and the ATS stack reads better ragged too.
#let letterhead(name-size) = [
  #set par(justify: false, leading: lead)
  #text(
    size: name-size,
    weight: "bold",
    fill: c-name,
    font: (font-name, "Carlito", "Inter"),
    // 0.04em — inside the 0.03–0.10em band every template here uses. Wider
    // tracking makes PDF text extract letter-by-letter, which an ATS reads as
    // gibberish (see the note in cologne_navy.typ).
    tracking: 0.04em,
    data.letterhead.name,
  )

  #if "signature_title" in data and data.signature_title != none {
    block(above: 6pt, text(size: body-pt - 0.5pt, fill: c-date, data.signature_title))
  }

  #if "contact" in data.letterhead and data.letterhead.contact.len() > 0 {
    block(above: 9pt, text(size: body-pt - 1pt, fill: c-body, render-runs(data.letterhead.contact)))
  }
]

// ── Letterhead placement ──────────────────────────────────────────────────────

#if ats {
  // Plain stacked letterhead in the single column, then a hairline to separate
  // it from the correspondence — a rule carries no text, so it costs an ATS
  // parser nothing.
  block(below: 10pt, letterhead(name-pt))
  block(below: 14pt, line(length: 100%, stroke: 0.6pt + c-rule))
} else {
  // Into the rail: back out of the left margin by (rail-w + rail-gutter) and in
  // again by rail-pad. Arithmetic on the constants above — no measurement, so
  // the position is identical on every render.
  place(
    top + left,
    dx: -(rail-w + rail-gutter - rail-pad),
    block(width: rail-w - 2 * rail-pad, letterhead(name-pt - 4pt)),
  )
}

// ── Date (below-header, non-DIN markets) ──────────────────────────────────────

#if date-pos == "top-right" and "date" in data and data.date != none {
  block(below: 8pt, align(right, text(size: body-pt, fill: c-date, data.date)))
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
      text(size: body-pt - 1.5pt, weight: "bold", fill: c-accent, tracking: 0.1em, smallcaps(subj-label))
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
