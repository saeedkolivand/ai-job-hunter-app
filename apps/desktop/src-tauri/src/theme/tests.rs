use super::*;

#[test]
fn template_accessor_matches_registry() {
    assert_eq!(template(TemplateId::Classic).id, TemplateId::Classic);
    assert_eq!(template(TemplateId::Atelier).id, TemplateId::Atelier);
}

/// A header band must be DROPPABLE, and the ATS-mode toggle that drops it
/// only surfaces for design-tier templates. So a band on an ATS-tier
/// template could never be turned off — it would permanently shade the
/// header of a template sold as plain. Adding one to an ATS-tier template
/// (the easy mistake: `Meridian`, whose PDF genuinely paints a 38 mm band)
/// fails here rather than shipping an undroppable tint into DOCX.
#[test]
fn header_band_templates_are_all_design_tier() {
    use crate::export::templates::{TemplateTier, CANONICAL_TEMPLATE_IDS};
    let mut banded = 0;
    for id in CANONICAL_TEMPLATE_IDS {
        if !has_header_band(id) {
            continue;
        }
        banded += 1;
        assert_eq!(
            template(id).tier,
            TemplateTier::Design,
            "{id:?} carries a header band but is ATS-tier — it never surfaces \
             the ATS toggle, so the band could never be dropped"
        );
    }
    assert_eq!(
        banded, 1,
        "expected exactly one banded template (Awesome); the roster changed"
    );
    assert!(
        has_header_band(TemplateId::Awesome),
        "Awesome is the banded template both renderers key off"
    );
    assert!(
        !has_header_band(TemplateId::Meridian),
        "Meridian's PDF band is a deliberate PDF-only divergence — see \
         `has_header_band`; enrolling it here silently changes its DOCX"
    );
}

#[test]
fn sidebar_sections_go_to_the_sidebar() {
    // Default table (Atelier / Portrait keep the full sidebar set).
    for tid in [TemplateId::Atelier, TemplateId::Portrait] {
        for id in [
            SectionId::Skills,
            SectionId::Education,
            SectionId::Languages,
            SectionId::Certifications,
        ] {
            assert_eq!(
                placement_for(tid, &id),
                Placement::Sidebar,
                "{tid:?}/{id:?} should be sidebar"
            );
        }
    }
}

#[test]
fn main_sections_stay_in_the_main_column() {
    for id in [
        SectionId::Summary,
        SectionId::Experience,
        SectionId::Projects,
    ] {
        assert_eq!(
            placement_for(TemplateId::Portrait, &id),
            Placement::Main,
            "{id:?} should be main"
        );
    }
    assert_eq!(
        placement_for(TemplateId::Portrait, &SectionId::Custom("Patents".into())),
        Placement::Main
    );
}

#[test]
fn aria_keeps_education_in_the_main_column() {
    // Aria pulls Education into the main column; the rest of the sidebar set
    // is unchanged.
    assert_eq!(
        placement_for(TemplateId::Aria, &SectionId::Education),
        Placement::Main,
        "Aria: Education should read in the main column"
    );
    for id in [
        SectionId::Skills,
        SectionId::Languages,
        SectionId::Certifications,
    ] {
        assert_eq!(
            placement_for(TemplateId::Aria, &id),
            Placement::Sidebar,
            "Aria/{id:?} should stay in the sidebar"
        );
    }
}

#[test]
fn saffron_keeps_certifications_in_the_main_column() {
    // Saffron pulls Certifications into the main column; Education stays in
    // the sidebar (unlike Aria).
    assert_eq!(
        placement_for(TemplateId::Saffron, &SectionId::Certifications),
        Placement::Main,
        "Saffron: Certifications should read in the main column"
    );
    for id in [
        SectionId::Skills,
        SectionId::Education,
        SectionId::Languages,
    ] {
        assert_eq!(
            placement_for(TemplateId::Saffron, &id),
            Placement::Sidebar,
            "Saffron/{id:?} should stay in the sidebar"
        );
    }
}

#[test]
fn default_templates_placement_is_byte_identical() {
    // Guard: adding the id parameter must NOT shift Atelier/Portrait placement.
    for tid in [TemplateId::Atelier, TemplateId::Portrait] {
        assert_eq!(
            placement_for(tid, &SectionId::Education),
            Placement::Sidebar
        );
        assert_eq!(
            placement_for(tid, &SectionId::Certifications),
            Placement::Sidebar
        );
        assert_eq!(placement_for(tid, &SectionId::Skills), Placement::Sidebar);
        assert_eq!(
            placement_for(tid, &SectionId::Languages),
            Placement::Sidebar
        );
        assert_eq!(placement_for(tid, &SectionId::Summary), Placement::Main);
        assert_eq!(placement_for(tid, &SectionId::Experience), Placement::Main);
    }
}

#[test]
fn two_column_only_for_two_column_templates() {
    for id in [
        TemplateId::Atelier,
        TemplateId::Portrait,
        TemplateId::Aria,
        TemplateId::Saffron,
    ] {
        assert!(is_two_column(id), "{id:?} is two-column");
    }
    assert!(!is_two_column(TemplateId::Classic));
    assert!(!is_two_column(TemplateId::SwissMinimal));
    assert!(
        !is_two_column(TemplateId::Lebenslauf),
        "Lebenslauf is single-column"
    );
}

#[test]
fn classic_links_are_plain_others_accented() {
    assert_eq!(
        link_style(TemplateId::Classic),
        LinkStyle {
            use_accent: false,
            underline: false
        }
    );
    let accented = link_style(TemplateId::SwissMinimal);
    assert!(accented.use_accent && accented.underline);
}
