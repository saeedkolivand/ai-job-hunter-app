//! Applier registry — looks up the right `Applier` by board id.

use super::boards::{
    greenhouse::GreenhouseApplier, indeed::IndeedApplier, linkedin::LinkedInApplier,
    workday::WorkdayApplier,
};
use super::types::Applier;

pub struct ApplierRegistry;

impl ApplierRegistry {
    pub fn catalog() -> Vec<(&'static str, &'static str)> {
        vec![
            ("linkedin", "LinkedIn"),
            ("indeed", "Indeed"),
            ("greenhouse", "Greenhouse"),
            ("workday", "Workday"),
        ]
    }

    pub fn get(board_id: &str) -> Option<Box<dyn Applier>> {
        match board_id {
            "linkedin" => Some(Box::new(LinkedInApplier)),
            "indeed" => Some(Box::new(IndeedApplier)),
            "greenhouse" => Some(Box::new(GreenhouseApplier)),
            "workday" => Some(Box::new(WorkdayApplier)),
            _ => None,
        }
    }
}
