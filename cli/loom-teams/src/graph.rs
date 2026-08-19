//! Microsoft Graph client: identity probe, people search, free/busy.

use crate::model::{Person, ScheduleInformation};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

const GRAPH: &str = "https://graph.microsoft.com/v1.0";

pub struct Graph {
    client: reqwest::Client,
    token: String,
}

pub struct Me {
    pub user_principal_name: String,
    pub mail: Option<String>,
    pub display_name: String,
}

impl Graph {
    pub fn new(token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
        }
    }

    async fn get(&self, path: &str, headers: &[(&str, String)]) -> Result<(u16, Value)> {
        let mut req = self
            .client
            .get(format!("{GRAPH}{path}"))
            .bearer_auth(&self.token)
            .header("Accept", "application/json");
        for (k, v) in headers {
            req = req.header(*k, v);
        }
        let res = req.send().await.context("Graph request failed")?;
        let status = res.status().as_u16();
        let body = res.json::<Value>().await.unwrap_or_else(|_| json!({}));
        Ok((status, body))
    }

    async fn post(
        &self,
        path: &str,
        body: Value,
        headers: &[(&str, String)],
    ) -> Result<(u16, Value)> {
        let mut req = self
            .client
            .post(format!("{GRAPH}{path}"))
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
            .json(&body);
        for (k, v) in headers {
            req = req.header(*k, v);
        }
        let res = req.send().await.context("Graph request failed")?;
        let status = res.status().as_u16();
        let body = res.json::<Value>().await.unwrap_or_else(|_| json!({}));
        Ok((status, body))
    }

    pub async fn me(&self) -> Result<Me> {
        let (status, body) = self.get("/me", &[]).await?;
        if status != 200 {
            bail!(
                "Graph /me failed: {status} {}",
                body["error"]["message"].as_str().unwrap_or("")
            );
        }
        Ok(Me {
            user_principal_name: body["userPrincipalName"].as_str().unwrap_or("").to_string(),
            mail: body["mail"].as_str().map(String::from),
            display_name: body["displayName"].as_str().unwrap_or("").to_string(),
        })
    }

    /// Names become emails via `/me/people` relevance search; emails pass
    /// through. Lookups run concurrently over one connection.
    pub async fn resolve_people(&self, me: &Me, names: &[String]) -> Result<Vec<Person>> {
        let self_email = me
            .mail
            .clone()
            .unwrap_or_else(|| me.user_principal_name.clone());
        let mut resolved = vec![Person {
            query: "me".into(),
            email: self_email,
            name: me.display_name.clone(),
        }];
        let lookups = names.iter().map(|name| self.resolve_one(name));
        resolved.extend(futures::future::try_join_all(lookups).await?);
        Ok(resolved)
    }

    async fn resolve_one(&self, name: &str) -> Result<Person> {
        if name.contains('@') {
            return Ok(Person {
                query: name.into(),
                email: name.into(),
                name: name.into(),
            });
        }
        let quoted = format!("\"{name}\"");
        let q = percent_encoding::utf8_percent_encode(&quoted, percent_encoding::NON_ALPHANUMERIC);
        let (status, body) = self
            .get(
                &format!(
                    "/me/people?$search={q}&$select=displayName,scoredEmailAddresses,userPrincipalName"
                ),
                &[("ConsistencyLevel", "eventual".into())],
            )
            .await?;
        let hit = &body["value"][0];
        let email = hit["scoredEmailAddresses"][0]["address"]
            .as_str()
            .or_else(|| hit["userPrincipalName"].as_str());
        match email {
            Some(email) if status == 200 => Ok(Person {
                query: name.into(),
                email: email.to_string(),
                name: hit["displayName"].as_str().unwrap_or(name).to_string(),
            }),
            _ => bail!("Could not resolve \"{name}\" via Graph people search. Pass an email."),
        }
    }

    pub async fn get_schedule(
        &self,
        emails: &[String],
        start_utc: &str,
        end_utc: &str,
        interval: u32,
        tz: &str,
    ) -> Result<Vec<ScheduleInformation>> {
        let body = json!({
            "schedules": emails,
            "startTime": { "dateTime": start_utc, "timeZone": "UTC" },
            "endTime": { "dateTime": end_utc, "timeZone": "UTC" },
            "availabilityViewInterval": interval,
        });
        let (status, body) = self
            .post(
                "/me/calendar/getSchedule",
                body,
                &[("Prefer", format!("outlook.timezone=\"{tz}\""))],
            )
            .await?;
        if status != 200 {
            bail!(
                "getSchedule {status}: {}",
                body["error"]["message"].as_str().unwrap_or("unknown")
            );
        }
        let schedules: Vec<ScheduleInformation> = serde_json::from_value(body["value"].clone())
            .context("unexpected getSchedule shape")?;
        Ok(schedules)
    }
}
