# Export Format Issues - Resume & Cover Letter

## Problem Summary

The AI Generate page exports resumes and cover letters in 3 formats (DOCX, PDF, TXT), but users report **malformed text and bad formatting**. The current implementation is entirely in the frontend using JavaScript libraries (`docx`, `jspdf`), which has several limitations.

---

## Current Architecture

### Export Flow

```
AI Generated Text (with **bold** markers)
    ↓
parseDocument() / parseInlineMd() - Parse structure
    ↓
buildResumeDocx() / buildCoverLetterDocx() - DOCX generation
exportResumePDF() / exportCoverLetterPDF() - PDF generation
exportTXT() - Plain text (strips markdown)
```

### Libraries Used

- **DOCX**: `docx` npm package (~500KB)
- **PDF**: `jspdf` npm package (~200KB)
- **Parsing**: Custom regex-based parser

---

## Identified Issues

### 1. **Text Parsing Problems**

**Issue**: The `parseLine()` function uses heuristics that can misidentify content:

```typescript
// Problematic heuristics:
- First line = always "name" (wrong if resume has header)
- 3+ spaces = job entry with date (fails with formatted text)
- ALL CAPS = section header (fails with acronyms, company names)
- Bullet detection: /^[•\-–*·▪▸►]\s/ (misses other bullet styles)
```

**Example Failures**:

- "NASA ENGINEER" → Detected as section header (all caps)
- "Senior Developer Remote" → Detected as job entry (3 spaces)
- Bullet points with tabs → Not detected as bullets
- Multi-line job titles → Split incorrectly

### 2. **DOCX Generation Issues**

**Problems**:

- **Font inconsistencies**: Hardcoded "Calibri" may not render correctly on all systems
- **Spacing issues**: Fixed spacing values don't adapt to content length
- **Bold markers**: `**keyword**` sometimes not parsed correctly if nested or malformed
- **Line breaks**: Extra blank lines or missing breaks
- **Contact info**: Email/phone detection regex can fail with international formats

**Code Location**: Lines 520-744 in `generate-ai.ts`

### 3. **PDF Generation Issues**

**Problems**:

- **Text overflow**: No proper word wrapping for long lines
- **Page breaks**: `checkY()` logic can break mid-sentence
- **Font rendering**: Limited to Helvetica (no custom fonts)
- **Bold rendering**: `drawMixedText()` can misalign bold segments
- **Character encoding**: Special characters (é, ñ, ü) may not render
- **Bullet alignment**: Hardcoded positions don't adapt to content

**Code Location**: Lines 978-1298 in `generate-ai.ts`

### 4. **Template Issues**

**Current Templates**:

1. **Classic** - ATS-safe, no color
2. **Modern** - Blue accents
3. **Executive** - Purple accents

**Problems**:

- Templates are hardcoded with fixed spacing
- No customization options
- Colors may not print well
- Margins don't adapt to content length

---

## Why Frontend-Only Export Is Problematic

### JavaScript Library Limitations

1. **DOCX (`docx` package)**:
   - ❌ Limited styling options
   - ❌ No advanced layout features
   - ❌ Large bundle size (~500KB)
   - ❌ Complex API, hard to maintain
   - ❌ No native font embedding

2. **PDF (`jspdf`)**:
   - ❌ Manual text positioning (error-prone)
   - ❌ No automatic text wrapping
   - ❌ Limited font support
   - ❌ No complex layouts
   - ❌ Large bundle size (~200KB)
   - ❌ Poor Unicode support

3. **Parsing**:
   - ❌ Regex-based (fragile)
   - ❌ Doesn't handle edge cases
   - ❌ Language-specific assumptions
   - ❌ Hard to debug

---

## Proposed Solutions

### Option 1: Move to Rust Backend (RECOMMENDED) ⭐

**Benefits**:

- ✅ **Better libraries**: Use `docx-rs`, `printpdf`, or `typst`
- ✅ **Performance**: 10-100x faster than JavaScript
- ✅ **Reliability**: Proper error handling, type safety
- ✅ **Smaller bundle**: Remove 700KB+ from frontend
- ✅ **Better parsing**: Use proper parsers (pest, nom)
- ✅ **Font embedding**: Include custom fonts
- ✅ **Advanced layouts**: Tables, columns, headers/footers
- ✅ **Better Unicode**: Full UTF-8 support

**Implementation**:

```rust
// Tauri command
#[tauri::command]
async fn export_resume(
    text: String,
    format: ExportFormat,
    template: TemplateId,
    meta: GenerationMeta,
) -> Result<Vec<u8>, String> {
    match format {
        ExportFormat::Docx => generate_docx(&text, &template, &meta),
        ExportFormat::Pdf => generate_pdf(&text, &template, &meta),
        ExportFormat::Txt => Ok(text.into_bytes()),
    }
}
```

**Rust Libraries to Use**:

- **DOCX**: `docx-rs` (pure Rust, full Office compatibility)
- **PDF**: `printpdf` or `typst` (professional quality)
- **Parsing**: `pest` (PEG parser, robust)
- **Markdown**: `pulldown-cmark` (CommonMark compliant)

**Estimated Effort**: 2-3 days

---

### Option 2: Improve Frontend Implementation

If Rust migration is not feasible, improve the current system:

#### 2.1 Fix Parsing Logic

**Replace heuristics with structured parsing**:

```typescript
// Instead of regex heuristics, use a proper state machine
class ResumeParser {
  private state: 'header' | 'section' | 'content' = 'header';

  parseLine(line: string, context: ParserContext): ParsedLine {
    // Use context-aware parsing instead of isolated line checks
    // Track previous lines, section context, indentation
  }
}
```

**Improvements**:

- Track parser state (header, section, content)
- Use indentation for structure detection
- Validate against expected patterns
- Handle multi-line content properly

#### 2.2 Fix DOCX Generation

```typescript
// Use better spacing calculations
const calculateSpacing = (contentType: string, previousType: string) => {
  // Dynamic spacing based on content flow
  if (previousType === 'sectionHeader' && contentType === 'bullet') {
    return { before: 60, after: 40 };
  }
  // ... more rules
};

// Better bold parsing
const parseBoldMarkers = (text: string): MdSegment[] => {
  // Handle nested markers, escaped asterisks, edge cases
  const tokens = tokenize(text);
  return buildSegments(tokens);
};
```

#### 2.3 Fix PDF Generation

```typescript
// Implement proper text wrapping
const wrapText = (text: string, maxWidth: number, fontSize: number): string[] => {
  // Use canvas measureText for accurate width
  // Break at word boundaries
  // Handle hyphenation
};

// Better page break logic
const smartPageBreak = (currentY: number, contentHeight: number, contentType: string) => {
  // Don't break in middle of bullets
  // Keep job entries together
  // Add orphan/widow control
};
```

#### 2.4 Add Template Customization

```typescript
interface TemplateConfig {
  fonts: {
    body: string;
    heading: string;
  };
  spacing: {
    sectionGap: number;
    bulletGap: number;
    paragraphGap: number;
  };
  margins: {
    top: number;
    bottom: number;
    left: number;
    right: number;
  };
  // ... more options
}
```

**Estimated Effort**: 3-4 days

---

### Option 3: Hybrid Approach

**Use Rust for PDF, keep JavaScript for DOCX**:

- PDF generation is more problematic → Move to Rust
- DOCX generation is acceptable → Keep in frontend but improve
- Parsing → Move to Rust (shared by both)

**Benefits**:

- ✅ Best of both worlds
- ✅ Incremental migration
- ✅ Immediate improvement for PDF
- ✅ Smaller scope than full migration

**Estimated Effort**: 2 days

---

## Recommended Approach

### Phase 1: Quick Fixes (1 day)

1. **Fix critical parsing bugs**:
   - Improve bullet detection
   - Fix all-caps detection (exclude known patterns)
   - Better date range detection
   - Handle multi-line content

2. **Fix spacing issues**:
   - Add dynamic spacing calculations
   - Fix page break logic
   - Improve text wrapping

3. **Add error handling**:
   - Validate parsed structure
   - Show warnings for malformed content
   - Fallback to plain text if parsing fails

### Phase 2: Rust Migration (2-3 days)

1. **Create Rust export module**:

   ```
   src-tauri/
     src/
       export/
         mod.rs          - Main export logic
         parser.rs       - Resume structure parser
         docx.rs         - DOCX generation
         pdf.rs          - PDF generation
         templates.rs    - Template definitions
   ```

2. **Implement Tauri commands**:
   - `export_resume_docx`
   - `export_resume_pdf`
   - `export_cover_letter_docx`
   - `export_cover_letter_pdf`

3. **Update frontend**:
   - Remove `docx` and `jspdf` dependencies
   - Call Tauri commands instead
   - Handle binary data download

4. **Add tests**:
   - Unit tests for parser
   - Integration tests for exports
   - Visual regression tests for templates

### Phase 3: Polish (1 day)

1. **Add template customization UI**
2. **Support more export formats** (HTML, Markdown)
3. **Add export preview**
4. **Improve error messages**

---

## Expected Improvements

### With Rust Migration:

**Performance**:

- Export time: 2-5s → 0.1-0.5s (10-50x faster)
- Bundle size: -700KB frontend
- Memory usage: -50MB during export

**Quality**:

- ✅ Perfect text wrapping
- ✅ Proper page breaks
- ✅ Professional typography
- ✅ Full Unicode support
- ✅ Custom fonts
- ✅ Advanced layouts

**Reliability**:

- ✅ Robust parsing (no regex hacks)
- ✅ Type-safe generation
- ✅ Better error handling
- ✅ Easier to maintain

---

## Implementation Priority

1. **HIGH**: Fix critical parsing bugs (Phase 1)
2. **HIGH**: Migrate PDF generation to Rust
3. **MEDIUM**: Migrate DOCX generation to Rust
4. **LOW**: Add template customization
5. **LOW**: Add more export formats

---

## Testing Strategy

### Manual Testing Checklist:

- [ ] Resume with 1 page
- [ ] Resume with 5+ pages
- [ ] Resume with special characters (é, ñ, ü, 中文)
- [ ] Resume with long bullet points
- [ ] Resume with multiple jobs
- [ ] Resume with ALL CAPS company names
- [ ] Cover letter with international addresses
- [ ] Cover letter with long paragraphs
- [ ] All 3 templates (Classic, Modern, Executive)
- [ ] All 3 formats (DOCX, PDF, TXT)

### Automated Tests:

```rust
#[test]
fn test_parse_resume_structure() {
    let input = include_str!("../fixtures/sample_resume.txt");
    let parsed = parse_resume(input);
    assert_eq!(parsed.sections.len(), 5);
    assert_eq!(parsed.sections[0].kind, SectionKind::Header);
}

#[test]
fn test_export_docx_valid() {
    let resume = create_test_resume();
    let bytes = export_docx(&resume, TemplateId::Modern).unwrap();
    assert!(is_valid_docx(&bytes));
}
```

---

## Conclusion

**Recommendation**: Implement **Phase 1 (Quick Fixes)** immediately, then proceed with **Rust migration (Phase 2)** for a robust, long-term solution.

The Rust approach will:

- ✅ Solve all current issues
- ✅ Provide better performance
- ✅ Be easier to maintain
- ✅ Enable future enhancements

**Estimated Total Effort**: 4-5 days for complete solution
