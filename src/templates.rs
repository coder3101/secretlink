use crate::secret::SecretState;

use askama_axum::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {}

#[derive(Template)]
#[template(path = "goto.html")]
pub struct GotoTemplate {
    pub state: SecretState,
    pub key: String,
}

#[derive(Template)]
#[template(path = "result.html")]
pub struct ResultTemplate {
    pub url: String,
}

#[derive(Template)]
#[template(path = "consume.html")]
pub struct ConsumeTemplate {
    pub secret: String,
}

#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate {}

#[derive(Template)]
#[template(path = "how-it-works.html")]
pub struct HowItWorksTemplate {}
