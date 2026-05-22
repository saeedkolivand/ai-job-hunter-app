use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedInSessionData {
    pub cookies: Vec<Cookie>,
    pub li_at: String,
    #[serde(rename = "JSESSIONID")]
    pub jsession_id: Option<String>,
    #[serde(rename = "csrfToken")]
    pub csrf_token: Option<String>,
    #[serde(rename = "lastUpdated")]
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<f64>,
}

