//! Minimal Typst [`World`] implementation for the resume/cover-letter engine.
//!
//! [`ResumeWorld`] is purely in-memory:
//! - A single [`Source`] (the Typst markup passed in at construction time).
//! - An optional in-memory `data.json` served via `file()` so templates can use
//!   `#let data = json("data.json")` without disk access.
//! - Fonts loaded from the bundled TTFs already compiled into the binary
//!   via `include_bytes!`.
//! - `today` returns a fixed deterministic date so PDF output is reproducible
//!   across machines and time.
//! - `file` returns [`FileError::NotFound`] for everything that is not the
//!   registered in-memory `data.json` — no disk access is ever performed.
//!
//! Offline hard-wall: only `/main.typ`, `/data.json`, and (when provided)
//! `/photo.png` are ever served.  Everything else remains NotFound to prevent
//! unintended network/disk probes.

use std::sync::LazyLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

// ── Fixed deterministic date ──────────────────────────────────────────────────
// All exports use this fixed date so two identical documents produce
// bit-identical PDFs regardless of when they are rendered.  The date is
// intentionally arbitrary and is NEVER surfaced to the user in rendered output
// (Typst's `today()` is called by internal library code only, not by our
// templates).  Keeping it constant is the entire point: reproducibility.
const FIXED_YEAR: i32 = 2030;
const FIXED_MONTH: u8 = 1;
const FIXED_DAY: u8 = 1;

// ── Virtual paths ─────────────────────────────────────────────────────────────
const MAIN_PATH: &str = "/main.typ";
const DATA_PATH: &str = "/data.json";
/// Virtual photo path served to templates.  Templates reference this as
/// `image("photo.png", …)`.  Only served when the caller supplies photo bytes.
pub(super) const PHOTO_PATH: &str = "/photo.png";

/// Collected font data parsed once at first use from the bundled TTF bytes.
///
/// Includes all four Carlito faces (regular/bold/italic/bold-italic),
/// both Inter weights (regular/bold), Source Serif 4 (regular/bold/italic),
/// and Manrope (regular/bold) — 11 faces total, one for each `FontFamily`
/// variant used by the active templates. Fonts are compiled into the binary
/// via `include_bytes!` and live for `'static`, making `Font` values cheap
/// to clone across render calls.
struct LoadedFonts {
    fonts: Vec<Font>,
    book: LazyHash<FontBook>,
}

/// Process-wide singleton: fonts are parsed exactly once and reused by every
/// [`ResumeWorld`]. `Font` is `Clone` + cheaply reference-counted internally;
/// `FontBook` is wrapped in `LazyHash` as the `World` trait requires.
static LOADED_FONTS: LazyLock<LoadedFonts> = LazyLock::new(|| {
    // Full font set — Carlito (4 faces) + Inter (2) + Source Serif 4 (3) +
    // Manrope (2) = 11 faces. Only families referenced by `FontFamily` variants
    // used in active templates are included; removed templates' fonts are not
    // bundled here.
    let raw: &[&[u8]] = &[
        // Carlito — Calibri-metric-compatible, OFL-1.1
        include_bytes!("../../../fonts/carlito_regular.ttf"),
        include_bytes!("../../../fonts/carlito_bold.ttf"),
        include_bytes!("../../../fonts/carlito_italic.ttf"),
        include_bytes!("../../../fonts/carlito_bolditalic.ttf"),
        // Inter — good Unicode / Cyrillic coverage
        include_bytes!("../../../fonts/inter_regular.ttf"),
        include_bytes!("../../../fonts/inter_bold.ttf"),
        // Source Serif 4 — editorial / academic serif
        include_bytes!("../../../fonts/source_serif4_regular.ttf"),
        include_bytes!("../../../fonts/source_serif4_bold.ttf"),
        include_bytes!("../../../fonts/source_serif4_italic.ttf"),
        // Manrope — Swiss Minimal template
        include_bytes!("../../../fonts/manrope_regular.ttf"),
        include_bytes!("../../../fonts/manrope_bold.ttf"),
    ];

    let mut fonts: Vec<Font> = Vec::new();

    for &bytes in raw {
        let b = Bytes::new(bytes);
        // Font::iter handles TTC collections (returns one Font per face).
        for font in Font::iter(b) {
            fonts.push(font);
        }
    }

    let book = LazyHash::new(FontBook::from_fonts(&fonts));
    LoadedFonts { fonts, book }
});

/// In-memory Typst world for resume/cover-letter rendering.
///
/// Constructed from a Typst source string plus an optional in-memory data
/// blob (JSON) that the template can access via `#let data = json("data.json")`.
/// An optional photo (clean PNG bytes from [`resolve_photo`]) can be served as
/// the virtual file `/photo.png` so templates can embed it via
/// `image("photo.png", …)`.
///
/// All methods are pure — no filesystem access, no network I/O. `today` returns
/// a fixed date so rendered output is deterministic across machines and time.
///
/// Offline hard-wall: `source()` serves only `/main.typ`; `file()` serves only
/// `/data.json` (when provided) and `/photo.png` (when provided) — everything
/// else returns `NotFound`.
pub struct ResumeWorld {
    library: LazyHash<Library>,
    main_id: FileId,
    main_source: Source,
    /// Optional in-memory data.json bytes served to the template.
    data_id: FileId,
    data_bytes: Option<Bytes>,
    /// Optional in-memory photo PNG bytes served as `/photo.png`.
    photo_id: FileId,
    photo_bytes: Option<Bytes>,
}

impl ResumeWorld {
    /// Create a world that will compile `source_text` as its main document,
    /// with no data file attached.
    pub fn new(source_text: &str) -> Self {
        Self::with_data(source_text, None)
    }

    /// Create a world with an in-memory `data.json` the template can read via
    /// `#let data = json("data.json")`. The `data_json` bytes must be valid UTF-8
    /// JSON; they are served verbatim from memory with no disk read.
    pub fn with_data(source_text: &str, data_json: Option<Vec<u8>>) -> Self {
        Self::with_data_and_photo(source_text, data_json, None)
    }

    /// Create a world with both an in-memory `data.json` and an optional
    /// `/photo.png`.  `photo_png` should be the clean PNG bytes returned by
    /// [`super::photo::resolve_photo`] — already sanitised, EXIF-stripped, and
    /// dimension-capped.  Pass `None` when no photo is available; templates
    /// must guard with `data.opts.has_photo` before calling `image("photo.png")`.
    pub fn with_data_and_photo(
        source_text: &str,
        data_json: Option<Vec<u8>>,
        photo_png: Option<Vec<u8>>,
    ) -> Self {
        let main_id = FileId::new(None, VirtualPath::new(MAIN_PATH));
        let main_source = Source::new(main_id, source_text.to_owned());
        let data_id = FileId::new(None, VirtualPath::new(DATA_PATH));
        let data_bytes = data_json.map(Bytes::new);
        let photo_id = FileId::new(None, VirtualPath::new(PHOTO_PATH));
        let photo_bytes = photo_png.map(Bytes::new);
        // Force the font set to load (parsed once per process; subsequent ResumeWorld constructions reuse it).
        let _ = &*LOADED_FONTS;
        let library = LazyHash::new(Library::default());

        Self {
            library,
            main_id,
            main_source,
            data_id,
            data_bytes,
            photo_id,
            photo_bytes,
        }
    }
}

impl World for ResumeWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &LOADED_FONTS.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.main_source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.data_id {
            match &self.data_bytes {
                Some(b) => Ok(b.clone()),
                None => Err(FileError::NotFound(id.vpath().as_rootless_path().into())),
            }
        } else if id == self.photo_id {
            match &self.photo_bytes {
                Some(b) => Ok(b.clone()),
                None => Err(FileError::NotFound(id.vpath().as_rootless_path().into())),
            }
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        LOADED_FONTS.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        Datetime::from_ymd(FIXED_YEAR, FIXED_MONTH, FIXED_DAY)
    }
}
