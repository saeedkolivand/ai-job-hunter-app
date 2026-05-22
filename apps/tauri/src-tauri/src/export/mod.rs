/**
 * Export module for generating professional resumes and cover letters.
 * 
 * Architecture:
 * - parser.rs: Parse resume text into structured format
 * - docx.rs: Generate DOCX files
 * - pdf.rs: Generate PDF files
 * - templates.rs: Template definitions and styling
 * - types.rs: Shared types and structures
 * - commands.rs: Tauri commands for frontend integration
 */

pub mod parser;
pub mod docx;
pub mod pdf;
pub mod templates;
pub mod types;
pub mod commands;

pub use types::{ExportFormat, ExportRequest, ExportResult, TemplateId};
pub use parser::parse_resume;
pub use docx::generate_docx;
pub use pdf::generate_pdf;
pub use commands::export_document;
