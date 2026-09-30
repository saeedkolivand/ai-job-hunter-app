use crate::pipeline::resume::project_seed::ProjectOut;

pub(super) fn seed(name: &str, links: &[&str], stack: &[&str], description: &str) -> ProjectOut {
    ProjectOut {
        name: name.to_string(),
        links: links.iter().map(|s| s.to_string()).collect(),
        stack: stack.iter().map(|s| s.to_string()).collect(),
        description: description.to_string(),
    }
}
