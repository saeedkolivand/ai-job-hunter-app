# Fonts for PDF Export

This directory should contain the Calibri font files for PDF generation:

- `Calibri-Regular.ttf`
- `Calibri-Bold.ttf`
- `Calibri-Italic.ttf` (optional)
- `Calibri-BoldItalic.ttf` (optional)

## Where to get Calibri fonts

Calibri is a Microsoft font included with Windows and Microsoft Office.

### Option 1: Use system fonts (Windows)

Copy from: `C:\Windows\Fonts\`

### Option 2: Use alternative open-source fonts

If Calibri is not available, you can use these alternatives:

- **Carlito** (Calibri metric-compatible)
- **Liberation Sans**
- **Arial**

### Option 3: Download from Microsoft

Calibri is part of the Microsoft Office suite.

## Alternative: Use built-in PDF fonts

If you don't want to include font files, modify `pdf.rs` to use built-in PDF fonts:

- Helvetica
- Times-Roman
- Courier

## License Note

Calibri is proprietary. Ensure you have the right to distribute it if bundling with the app.
For open-source distribution, use metric-compatible alternatives like Carlito.
