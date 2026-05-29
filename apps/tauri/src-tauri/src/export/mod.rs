/*!
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

pub mod links;
pub mod layout_pdf;
pub mod parser;
pub mod docx;
pub mod docx_renderer;
pub mod pdf;
pub mod pdf_renderer;
pub mod templates;
pub mod types;
pub mod commands;

