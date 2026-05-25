use super::types::TemplateId;

/// Template styling configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Template {
    pub id: TemplateId,
    pub name: &'static str,
    
    // Colors (RGB tuples)
    pub name_color: (u8, u8, u8),
    pub section_color: (u8, u8, u8),
    pub accent_color: (u8, u8, u8),
    pub body_color: (u8, u8, u8),
    pub date_color: (u8, u8, u8),
    pub emphasis_color: (u8, u8, u8),
    pub rule_color: (u8, u8, u8),
    
    // Font sizes (points)
    pub name_pt: f32,
    pub section_pt: f32,
    pub body_pt: f32,
    
    // Margins (inches)
    pub margin_in: f32,
    
    // Spacing
    pub line_spacing: f32,
    pub section_spacing_before: f32,
    
    // Style options
    pub name_centered: bool,
    pub section_all_caps: bool,
    pub section_style: SectionStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SectionStyle {
    RuledBottom,
    Underline,
    BoldOnly,
}

impl Template {
    /// Get template by ID
    pub fn get(id: TemplateId) -> Self {
        match id {
            TemplateId::Classic => Self::classic(),
            TemplateId::Modern => Self::modern(),
            TemplateId::Executive => Self::executive(),
        }
    }

    /// ATS Classic — maximum compatibility, no color, safe for all ATS parsers
    fn classic() -> Self {
        Self {
            id: TemplateId::Classic,
            name: "ATS Classic",
            name_color: (17, 17, 17),
            section_color: (17, 17, 17),
            accent_color: (34, 34, 34),
            body_color: (34, 34, 34),
            date_color: (85, 85, 85),
            emphasis_color: (0, 0, 0),
            rule_color: (170, 170, 170),
            name_pt: 20.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 1.0,
            line_spacing: 1.15,
            section_spacing_before: 12.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::Underline,
        }
    }

    /// Modern Technical — clean navy, professional, best for tech roles
    fn modern() -> Self {
        Self {
            id: TemplateId::Modern,
            name: "Modern Technical",
            name_color: (13, 31, 60),
            section_color: (13, 31, 60),
            accent_color: (26, 58, 107),
            body_color: (26, 26, 46),
            date_color: (107, 107, 138),
            emphasis_color: (13, 61, 107),
            rule_color: (184, 196, 220),
            name_pt: 22.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 1.0,
            line_spacing: 1.2,
            section_spacing_before: 13.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
        }
    }

    /// Executive — minimalist, charcoal, premium whitespace for senior roles
    fn executive() -> Self {
        Self {
            id: TemplateId::Executive,
            name: "Executive",
            name_color: (28, 28, 28),
            section_color: (44, 44, 44),
            accent_color: (68, 68, 68),
            body_color: (44, 44, 44),
            date_color: (128, 128, 128),
            emphasis_color: (28, 28, 28),
            rule_color: (204, 204, 204),
            name_pt: 24.0,
            section_pt: 10.5,
            body_pt: 10.5,
            margin_in: 1.1,
            line_spacing: 1.25,
            section_spacing_before: 15.0,
            name_centered: true,
            section_all_caps: false,
            section_style: SectionStyle::RuledBottom,
        }
    }
}

/// Calculate dynamic spacing based on content type and context
pub fn calculate_spacing(current_kind: &super::types::LineKind, previous_kind: Option<&super::types::LineKind>) -> (f32, f32) {
    use super::types::LineKind;
    
    // Returns (before, after) in points
    match current_kind {
        LineKind::SectionHeader => (12.0, 3.0),
        LineKind::JobEntry => {
            match previous_kind {
                Some(LineKind::Bullet) | Some(LineKind::JobTitle) => (8.0, 1.0),
                _ => (6.0, 1.0),
            }
        }
        LineKind::JobTitle => (0.0, 3.0),
        LineKind::Bullet => {
            match previous_kind {
                Some(LineKind::Bullet) => (0.0, 2.0),
                _ => (3.0, 2.0),
            }
        }
        LineKind::Contact => (0.0, 0.0),
        LineKind::Name => (0.0, 2.0),
        _ => (0.0, 4.0),
    }
}

#[cfg(test)]
mod test;
