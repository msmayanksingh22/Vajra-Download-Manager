//! Captcha Solving Module
//!
//! Integrates with external Captcha solving APIs like 2captcha or Anti-Captcha.

use reqwest::Client;

pub struct CaptchaSolver {
    api_key: String,
    client: Client,
}

impl CaptchaSolver {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    /// Submit a reCAPTCHA v2 solving request and wait for the response token.
    pub async fn solve_recaptcha_v2(
        &self,
        site_key: &str,
        page_url: &str,
    ) -> anyhow::Result<String> {
        let submit_url = format!(
            "http://2captcha.com/in.php?key={}&method=userrecaptcha&googlekey={}&pageurl={}&json=1",
            self.api_key, site_key, page_url
        );

        #[derive(serde::Deserialize)]
        struct SubmitResponse {
            status: i32,
            request: String, // Captcha ID or Error string
        }

        let res: SubmitResponse = self.client.get(&submit_url).send().await?.json().await?;
        if res.status != 1 {
            anyhow::bail!("Failed to submit captcha: {}", res.request);
        }

        let captcha_id = res.request;
        let mut attempts = 0;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            attempts += 1;
            if attempts > 24 {
                anyhow::bail!("Captcha solving timed out");
            }

            let poll_url = format!(
                "http://2captcha.com/res.php?key={}&action=get&id={}&json=1",
                self.api_key, captcha_id
            );

            let poll_res: SubmitResponse = self.client.get(&poll_url).send().await?.json().await?;
            if poll_res.status == 1 {
                return Ok(poll_res.request); // This is the solved token
            }
            if poll_res.request != "CAPCHA_NOT_READY" {
                anyhow::bail!("Captcha solving error: {}", poll_res.request);
            }
        }
    }
}
