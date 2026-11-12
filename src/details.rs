use anyhow::{Context, Result};
use aws_config::{BehaviorVersion, SdkConfig};
use aws_sdk_costexplorer::types::{DateInterval, Granularity, GroupDefinition, GroupDefinitionType};
use aws_sdk_invoicing::types::{
    BillingPeriod, InvoiceSummariesFilter, InvoiceSummariesSelector, ListInvoiceSummariesResourceType,
};
use chrono::{Datelike, Local, Months, NaiveDate};

/// Collects billing info for one AWS profile as plain text lines.
/// Each section reports its own error so one failure doesn't hide the rest.
pub async fn details(profile: &str) -> Vec<String> {
    // Billing APIs only live in us-east-1.
    let sdk = aws_config::defaults(BehaviorVersion::latest())
        .profile_name(profile)
        .region("us-east-1")
        .load()
        .await;
    let today = Local::now().date_naive();
    let start = today.with_day(1).expect("Each month has a 1st");

    let mut lines = vec![format!("=== Profile: {profile} ===")];
    section(&mut lines, "Cost by service (month to date)", cost_by_service(&sdk, start, today).await);
    match account_id(&sdk).await {
        Ok(id) => {
            section(&mut lines, "Budgets", budgets(&sdk, &id).await);
            section(&mut lines, "Invoices", invoices(&sdk, &id).await);
        }
        Err(e) => lines.push(format!("Account ID: error: {e:#}")),
    }
    section(&mut lines, "Free tier usage", free_tier(&sdk).await);
    lines
}

fn section(lines: &mut Vec<String>, title: &str, result: Result<Vec<String>>) {
    lines.push(String::new());
    lines.push(format!("--- {title} ---"));
    match result {
        Ok(items) if items.is_empty() => lines.push("(none)".into()),
        Ok(items) => lines.extend(items),
        Err(e) => lines.push(format!("error: {e:#}")),
    }
}

fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

async fn account_id(sdk: &SdkConfig) -> Result<String> {
    let resp = aws_sdk_sts::Client::new(sdk).get_caller_identity().send().await?;
    resp.account().map(String::from).context("no account id returned")
}

async fn cost_by_service(sdk: &SdkConfig, start: NaiveDate, today: NaiveDate) -> Result<Vec<String>> {
    let period = DateInterval::builder()
        .start(ymd(start))
        .end(ymd(today.succ_opt().unwrap()))
        .build()?;
    let resp = aws_sdk_costexplorer::Client::new(sdk)
        .get_cost_and_usage()
        .time_period(period)
        .granularity(Granularity::Monthly)
        .metrics("UnblendedCost")
        .group_by(
            GroupDefinition::builder()
                .r#type(GroupDefinitionType::Dimension)
                .key("SERVICE")
                .build(),
        )
        .send()
        .await?;

    let mut rows = Vec::new();
    for result in resp.results_by_time() {
        for group in result.groups() {
            if let Some(cost) = group.metrics().and_then(|m| m.get("UnblendedCost")) {
                let amount: f64 = cost.amount().unwrap_or("0").parse()?;
                rows.push((group.keys().join(", "), amount, cost.unit().unwrap_or("USD").to_string()));
            }
        }
    }
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    Ok(rows.into_iter().map(|(service, amount, unit)| format!("{service}: {amount} {unit}")).collect())
}

async fn budgets(sdk: &SdkConfig, account_id: &str) -> Result<Vec<String>> {
    let resp = aws_sdk_budgets::Client::new(sdk)
        .describe_budgets()
        .account_id(account_id)
        .send()
        .await?;
    let spend = |s: Option<&aws_sdk_budgets::types::Spend>| {
        s.map(|s| format!("{} {}", s.amount(), s.unit())).unwrap_or_else(|| "-".into())
    };
    Ok(resp
        .budgets()
        .iter()
        .map(|b| {
            let calc = b.calculated_spend();
            format!(
                "{}: actual {} / forecast {} / limit {}",
                b.budget_name(),
                spend(calc.and_then(|c| c.actual_spend())),
                spend(calc.and_then(|c| c.forecasted_spend())),
                spend(b.budget_limit()),
            )
        })
        .collect())
}

async fn invoices(sdk: &SdkConfig, account_id: &str) -> Result<Vec<String>> {
    let selector = InvoiceSummariesSelector::builder()
        .resource_type(ListInvoiceSummariesResourceType::AccountId)
        .value(account_id)
        .build()?;
    let client = aws_sdk_invoicing::Client::new(sdk);
    let this_month = Local::now().date_naive().with_day(1).unwrap();

    // The API only allows one month per request; check the last 3 billing months.
    let mut lines = Vec::new();
    for back in 1..=3 {
        let month = this_month - Months::new(back);
        let period = BillingPeriod::builder()
            .month(month.month() as i32)
            .year(month.year())
            .build()?;
        let resp = client
            .list_invoice_summaries()
            .selector(selector.clone())
            .filter(InvoiceSummariesFilter::builder().billing_period(period).build())
            .send()
            .await?;
        for i in resp.invoice_summaries() {
            let amount = i.base_currency_amount();
            lines.push(format!(
                "{}-{:02} {}: {} {}",
                month.year(),
                month.month(),
                i.invoice_id().unwrap_or("?"),
                amount.and_then(|a| a.total_amount()).unwrap_or("?"),
                amount.and_then(|a| a.currency_code()).unwrap_or(""),
            ));
        }
    }
    Ok(lines)
}

async fn free_tier(sdk: &SdkConfig) -> Result<Vec<String>> {
    let resp = aws_sdk_freetier::Client::new(sdk)
        .get_free_tier_usage()
        .max_results(1000)
        .send()
        .await?;
    Ok(resp
        .free_tier_usages()
        .iter()
        .map(|u| {
            format!(
                "{} [{}]: {} / {} {}",
                u.service().unwrap_or("?"),
                u.usage_type().unwrap_or("?"),
                u.actual_usage_amount(),
                u.limit(),
                u.unit().unwrap_or(""),
            )
        })
        .collect())
}
