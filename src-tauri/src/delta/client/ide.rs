use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

use crate::delta::{
    constants::DF_REFERER,
    error::DeltaError,
};

/// Common IDE gateway form-post call used by game data endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct IdeCall<'a> {
    pub chart_id: u64,
    pub ide_token: &'a str,
    pub method: Option<&'a str>,
    pub source: Option<&'a str>,
    pub param: Value,
}

impl<'a> IdeCall<'a> {
    pub fn new(chart_id: u64, ide_token: &'a str, param: Value) -> Self {
        Self { chart_id, ide_token, method: None, source: None, param }
    }

    pub fn with_method(mut self, method: &'a str) -> Self {
        self.method = Some(method);
        self
    }

    pub fn with_source(mut self, source: &'a str) -> Self {
        self.source = Some(source);
        self
    }

    pub async fn execute_with_url(
        self,
        client: &Client,
        gateway: &str,
    ) -> Result<Value, DeltaError> {
        let chart = self.chart_id.to_string();
        let param_str = serde_json::to_string(&self.param)?;
        let mut form: Vec<(&str, String)> = Vec::new();
        form.push(("iChartId", chart));
        form.push(("sIdeToken", self.ide_token.to_string()));
        if let Some(method) = self.method {
            form.push(("method", method.to_string()));
        }
        if let Some(source) = self.source {
            form.push(("source", source.to_string()));
        }
        form.push(("param", param_str));

        let resp = client
            .post(gateway)
            .header("Referer", DF_REFERER)
            .form(&form)
            .send()
            .await?
            .error_for_status()?;
        let body: Value = resp.json().await?;
        Ok(body)
    }
}
