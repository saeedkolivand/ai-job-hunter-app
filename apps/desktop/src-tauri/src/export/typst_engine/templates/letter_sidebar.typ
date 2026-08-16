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
// The rail also collapses to the same plain/symmetric treatment (`show-rail`,
// below) when the letterhead has no content at all to show — a suppressed
// name (see `is_letterhead_name`) with no contact and no title. Otherwise the
// pale panel would paint an empty tinted box with nothing in it.
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

// A letterhead can be entirely empty — the parser suppresses `letterhead.name`
// outright when the fallback "first line of the letter" isn't a name (a date
// or salutation opening; see `is_letterhead_name` in `typst_engine/letter.rs`).
// If a letter with no name ALSO has no contact and no title, the rail would
// otherwise paint a pale panel with nothing in it: an empty tinted box
// asserting itself for no reason. Same idea as `letter_monogram.typ`'s
// `show-device` gate (content-based, not just `ats`), applied to the rail as a
// whole rather than one glyph pair. A real generated letter always has SOME
// letterhead content (a `ContactProfile` is attached in practice — ADR 0021),
// so this is a defensive floor, not the common case.
#let has-letterhead-content = (
  data.letterhead.name != ""
    or ("contact" in data.letterhead and data.letterhead.contact.len() > 0)
    or ("signature_title" in data and data.signature_title != none)
)
#let show-rail = not ats and has-letterhead-content

// ── Rail geometry ─────────────────────────────────────────────────────────────
// Fixed millimetre constants, not measured content: the rail is `place`d, so
// nothing about the body depends on how tall the letterhead turns out.

#let rail-w      = 52mm   // tinted panel width
#let rail-pad    = 7mm    // inset of the rail text from the page edge
#let rail-gutter = 10mm   // clear space between the rail and the body column
#let margin-y    = 25.4mm
#let margin-r    = 22mm

// The usable text column inside the rail. Named because both the layout and the
// shrink-to-fit below have to agree on it exactly.
#let rail-text-w = rail-w - 2 * rail-pad   // 38mm

// Shrink-to-fit targets 97% of the column, not 100%. A standalone `measure()`
// under-reports the advance the same token gets once it is laid out in a
// paragraph — different shaping context, and the trailing tracking is not
// counted the same way. Measured at ~1.8% on the worst case in the test matrix
// (Cadence + "Wojciechowski"), which fitted by measurement and still put its
// last glyph 1.8pt past the column. 3% covers that with room; the geometry test
// is what holds it honest if the gap ever widens.
#let fit-limit = rail-text-w * 0.97

// Absolute floor, NOT a fraction of the base size, and ONE value for both the
// name and the contact.
//
// A proportional floor ("never below 60% of base") reads as the safer choice
// and is the opposite: a template with a 24pt name floors at 14.4pt while
// "Wojciechowski" needs 13.3pt to fit 38mm, so the clamp re-created the
// overflow it was meant to bound — on exactly the templates whose large names
// make overflow likely. It also made the floor depend on which template was
// picked rather than on the paper.
//
// 6pt, measured not guessed: the widest case in the test matrix (a 31-character
// e-mail in a 38mm rail) fits between 6pt and 7pt, and a 7pt floor put it back
// in the gutter. Shrinking is the right trade here rather than breaking the
// token: an e-mail broken with a zero-width space would read correctly on paper
// and extract corrupted, which is the soft-hyphen defect again.
#let fit-floor = 6pt

// Left margin: wide enough for the rail plus its gutter when the rail is
// actually shown, the ordinary symmetric margin otherwise (ATS mode, or a
// design-mode letter with no letterhead content to put in it).
#let margin-l = if show-rail { rail-w + rail-gutter } else { 25.4mm }

// ── Page & typography ─────────────────────────────────────────────────────────

#set page(
  width:  pg-w,
  height: pg-h,
  margin: (left: margin-l, right: if show-rail { margin-r } else { 25.4mm }, top: margin-y, bottom: margin-y),
  background: if show-rail {
    place(top + left, rect(width: rail-w, height: 100%, fill: c-rail, stroke: none))
  } else { none },
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
// entirely. Justification stays (wider word gaps, no split words) — including
// for German: letter bodies are one full-measure column, so justified rivers
// are a non-issue at this width. The other four layouts (letter.typ,
// letter_refined.typ, letter_banded.typ, letter_navy.typ) carry the identical
// flag + comment.
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

// [`render-runs`], but each contact entry starts its own line instead of one
// long " | "-joined row — the owner-requested "same vertical treatment as
// letter_refined" for the rail's own contact block, which otherwise relies on
// ragged auto-wrap that can break mid-entry in the narrow 38mm column.
// Splits on the literal " | " separator `ContactProfile::header_markdown`
// bakes into the joined contact string; links/bold runs never carry it.
#let render-runs-stacked(runs) = {
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
    } else if not r.bold and not r.italic and r.text.contains(" | ") {
      let parts = r.text.split(" | ")
      for (j, part) in parts.enumerate() {
        if j > 0 { linebreak() }
        if part != "" { part }
      }
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

// Strip a leading "<label>[:]" prefix from the subject.
//
// `data.subject` is published VERBATIM by `parse_cover_letter`, label and all,
// so a DE subject arrives as "Betreff: Bewerbung …". Rendering the caption on
// top of that printed the label twice — "BETREFF" over "Betreff: Bewerbung …".
// Same rule as letter_refined.typ / letter_navy.typ, and the same rule the DOCX
// renderer applies in `strip_market_label`; the three have to agree or one
// export contradicts the other. Labels are ASCII, so slicing by byte length
// removes exactly the prefix.
#let strip-subject-label(s, label) = {
  let t = s.trim()
  if label != "" and lower(t).starts-with(lower(label)) {
    let rest = t.slice(label.len()).trim()
    if rest.starts-with(":") { rest = rest.slice(1).trim() }
    rest
  } else { t }
}

// ── Shrink-to-fit ─────────────────────────────────────────────────────────────
//
// The rail is a FIXED 38mm column and `place` neither reflows nor clips, so
// anything too wide simply runs out of the rail, across the 10mm gutter and into
// the body column — measured at up to 44pt past the block for a long surname.
// Wrapping does not save it: text only breaks at spaces, and the overflow is
// always a single unbreakable TOKEN — "Papadopoulos", or a 27-character e-mail
// address. (`hyphenate: false`, which this layout needs for ATS extraction,
// removes the one other break opportunity.)
//
// So measure the widest token at the intended size and scale the size down by
// the ratio it overflows by. Glyph width is very nearly proportional to font
// size, so that solve lands within a fraction of a point; the loop then walks
// off the residue from tracking and hinting, which are not proportional. Floored
// so a pathological input degrades to small-but-legible instead of vanishing.
//
// Applies to the RAIL ONLY. Under ATS mode the letterhead is in the full-width
// single column, where ordinary wrapping works, so it renders at its base sizes
// and this never runs.

#let fit-size(tokens, style-fn, limit, base, floor) = {
  let widest(size) = {
    let w = 0pt
    for tok in tokens {
      let m = measure(style-fn(size, tok)).width
      if m > w { w = m }
    }
    w
  }
  let natural = widest(base)
  if natural <= limit or natural == 0pt {
    base
  } else {
    let size = calc.max(floor, base * (limit / natural))
    while size > floor and widest(size) > limit {
      size = size - 0.25pt
    }
    calc.max(size, floor)
  }
}

// Whitespace-separated tokens of a string, empties dropped. `s` may be `none`:
// `array.join` returns `none` (not `""`) for an EMPTY array, which
// `contact-tokens` below hits whenever the letterhead has no contact runs at
// all (a suppressed-name letterhead — see `is_letterhead_name` — with no
// attached `ContactProfile` and nothing scraped from the letter text). `none`
// has no `.split` method, so this used to panic the whole render instead of
// degrading to an empty rail.
#let tokens-of(s) = if s == none { () } else { s.split(regex("\\s+")).filter(t => t != "") }

// Plain text of the contact runs, for measurement only (the rendered version
// keeps its links and per-run styling via `render-runs`).
#let contact-tokens = if "contact" in data.letterhead {
  tokens-of(data.letterhead.contact.map(r => r.text).join(""))
} else { () }

#let title-tokens = if "signature_title" in data and data.signature_title != none {
  tokens-of(data.signature_title)
} else { () }

#let name-styler = (sz, s) => text(
  size: sz,
  weight: "bold",
  font: (font-name, "Carlito", "Inter"),
  tracking: 0.04em,
  s,
)
#let meta-styler = (sz, s) => text(size: sz, font: (font-body, "Carlito", "Inter"), s)

// The letterhead itself — identical content in both modes, only the type sizes
// and the container change. Ragged-right: a 38 mm rail cannot justify without
// opening rivers, and the ATS stack reads better ragged too.
//
// `meta-size` covers the role line AND the contact line: they share the rail's
// width, so a long e-mail has to shrink the pair, not just itself, or the two
// lines end up at visibly different sizes.
#let letterhead(name-size, meta-size) = [
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
    block(above: 6pt, text(size: meta-size + 0.5pt, fill: c-date, data.signature_title))
  }

  #if "contact" in data.letterhead and data.letterhead.contact.len() > 0 {
    block(above: sp-name-below, text(size: meta-size, fill: c-body, render-runs-stacked(data.letterhead.contact)))
  }
]

// ── Letterhead placement ──────────────────────────────────────────────────────

#if not show-rail {
  // Plain stacked letterhead in the single column (ATS mode, or design mode
  // with no letterhead content to put in the rail), then a hairline to
  // separate it from the correspondence — a rule carries no text, so it costs
  // an ATS parser nothing. Full width, so no fitting: ordinary wrapping works
  // here. When the letterhead is genuinely empty this just stacks nothing
  // above the rule, which is the honest degraded shape.
  block(below: 10pt, letterhead(name-pt, body-pt - 1pt))
  block(below: 14pt, line(length: 100%, stroke: 0.6pt + c-rule))
} else {
  // Into the rail: back out of the left margin by (rail-w + rail-gutter) and in
  // again by rail-pad. Arithmetic on the constants above — no measurement, so
  // the position is identical on every render.
  //
  // `context` is what lets `fit-size` call `measure`; it wraps only the rail
  // content, so nothing else in the document is deferred.
  place(
    top + left,
    dx: -(rail-w + rail-gutter - rail-pad),
    block(width: rail-text-w, context {
      let base-name = name-pt - 4pt
      let base-meta = body-pt - 1pt
      let fitted-name = fit-size(
        tokens-of(data.letterhead.name),
        name-styler,
        fit-limit,
        base-name,
        fit-floor,
      )
      // Role and contact share one size — measured together against the same
      // 38mm so the two lines cannot end up visibly mismatched.
      let fitted-meta = fit-size(
        title-tokens + contact-tokens,
        meta-styler,
        fit-limit,
        base-meta,
        fit-floor,
      )
      letterhead(fitted-name, fitted-meta)
    }),
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

// `subj-used` stays: whether a subject renders AT ALL is the market's call
// (US omits it), and that gating is separate from the duplicate-label fix
// below, which is about how it renders once the market has asked for one.
#if subj-used and "subject" in data and data.subject != none {
  let subj-body = strip-subject-label(data.subject, subj-label)
  let subj-body-lower = lower(subj-body.trim())
  // Defensive second check: the label is normally gone by now, but a subject
  // may carry its own marker the market label does not match — a literal "Re:".
  let has-own-label = (
    (subj-label != "" and subj-body-lower.starts-with(lower(subj-label)))
      or subj-body-lower.starts-with("re:")
  )
  block(above: 8pt, below: 8pt, {
    if subj-label != "" and not has-own-label {
      text(size: body-pt - 1.5pt, weight: "bold", fill: c-accent, tracking: 0.1em, smallcaps(subj-label))
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

// ── Sign-off + signature ────────────────────────────────────────────────────
// Grouped as one non-breakable unit — see `sp-signature-lead`/
// `sp-signature-gap` in _scale.typ: a page break must never land between the
// sign-off and the name it belongs to.
#block(breakable: false, above: sp-signature-lead, {
  if "signoff" in data and data.signoff != none {
    text(fill: c-body, data.signoff)
    v(sp-signature-gap)
  }
  text(
    weight: "bold",
    fill: c-name,
    font: (font-name, "Carlito", "Inter"),
    data.signature_name,
  )
})
