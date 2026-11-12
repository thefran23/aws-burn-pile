use crate::home_page::HomePage;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonthSpend {
    pub start: String,
    pub end: String,
    pub total: f64,
    pub unit: String,
}

/// Logs a server error (shows up in CloudWatch) and passes it on to the browser.
#[cfg(feature = "ssr")]
fn server_error(e: anyhow::Error) -> ServerFnError {
    let message = format!("{e:#}");
    tracing::error!("{message}");
    ServerFnError::new(message)
}

/// Runs on the server only; the browser calls it over HTTP.
/// Sums the month-to-date spend of every account in burn-pile.toml.
#[server]
pub async fn get_month_spend() -> Result<MonthSpend, ServerFnError> {
    use chrono::{Datelike, Local};

    let config = crate::config::load("burn-pile.toml").map_err(server_error)?;

    let today = Local::now().date_naive();
    let start = today.with_day(1).expect("Each month has a 1st");

    let mut total = 0.0;
    let mut unit = String::from("USD");
    for account in &config.accounts {
        let (amount, u) = crate::cost::month_spend(&account.profile, start, today)
            .await
            .map_err(server_error)?;
        total += amount;
        unit = u;
    }

    tracing::info!("month spend {start}..{today}: {total:.2} {unit} over {} account(s)", config.accounts.len());
    Ok(MonthSpend {
        start: start.to_string(),
        end: today.to_string(),
        total,
        unit,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceCost {
    pub name: String,
    pub this_month: f64,
    pub last_month: f64,
    /// Average per full month since the account first had costs (max 12 months back).
    pub avg: f64,
    pub url: String,
}

/// Services across all accounts with this month, last month and the monthly average.
#[server]
pub async fn get_services() -> Result<Vec<ServiceCost>, ServerFnError> {
    use chrono::{Datelike, Local, Months};
    use std::collections::{BTreeSet, HashMap};

    let config = crate::config::load("burn-pile.toml").map_err(server_error)?;
    let today = Local::now().date_naive();
    let this_start = today.with_day(1).expect("Each month has a 1st");
    let history_start = this_start - Months::new(12);
    let this_key = this_start.to_string();
    let last_key = (this_start - Months::new(1)).to_string();

    // service -> month -> cost, added up over all accounts.
    let mut costs: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for account in &config.accounts {
        let rows = crate::cost::services_by_month(&account.profile, history_start, today)
            .await
            .map_err(server_error)?;
        for (month, name, cost) in rows {
            *costs.entry(name).or_default().entry(month).or_default() += cost;
        }
    }

    // Full months (not the current one) from the first month that had any cost.
    let full_months: BTreeSet<&String> = costs.values().flat_map(|m| m.keys()).filter(|m| **m != this_key).collect();
    let month_count = full_months
        .first()
        .map(|first| {
            let first = chrono::NaiveDate::parse_from_str(first, "%Y-%m-%d").unwrap_or(this_start);
            let months = (this_start.year() - first.year()) * 12 + this_start.month() as i32 - first.month() as i32;
            months.max(1) as f64
        })
        .unwrap_or(1.0);

    let mut list: Vec<ServiceCost> = costs
        .iter()
        .map(|(name, months)| {
            let get = |key: &str| months.get(key).copied().unwrap_or(0.0);
            let past: f64 = months.iter().filter(|(m, _)| **m != this_key).map(|(_, c)| c).sum();
            ServiceCost {
                name: name.clone(),
                this_month: get(&this_key),
                last_month: get(&last_key),
                avg: past / month_count,
                url: crate::cost::console_url(name).to_string(),
            }
        })
        .collect();
    list.sort_by(|a, b| b.this_month.total_cmp(&a.this_month).then(b.avg.total_cmp(&a.avg)));
    tracing::info!("services: {} found over {} account(s)", list.len(), config.accounts.len());
    Ok(list)
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="color-scheme" content="light dark" />
                <title>"AWS Burn Pile"</title>
                <style>
                    "body { margin: 0; background: #f5f5f7; }
                    @media (prefers-color-scheme: dark) { body { background: #000; } }"
                </style>
                <AutoReload options=options.clone() />
                <HydrationScripts options />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    view! { <HomePage /> }
}
