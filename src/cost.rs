use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_costexplorer::types::{DateInterval, Granularity, GroupDefinition, GroupDefinitionType};
use chrono::NaiveDate;

pub async fn month_spend(profile: &str, start: NaiveDate, today: NaiveDate) -> Result<(f64, String)> {
    // Cost Explorer only lives in us-east-1.
    let sdk_config = aws_config::defaults(BehaviorVersion::latest())
        .profile_name(profile)
        .region("us-east-1")
        .load()
        .await;
    let client = aws_sdk_costexplorer::Client::new(&sdk_config);

    // Cost Explorer's end date is exclusive, so use tomorrow to include today.
    let end = today.succ_opt().unwrap();

    let period = DateInterval::builder()
        .start(start.format("%Y-%m-%d").to_string())
        .end(end.format("%Y-%m-%d").to_string())
        .build()?;

    let response = client
        .get_cost_and_usage()
        .time_period(period)
        .granularity(Granularity::Monthly)
        .metrics("UnblendedCost")
        .send()
        .await
        .with_context(|| format!("calling Cost Explorer with profile {profile}"))?;

    let mut total = 0.0;
    let mut unit = String::from("USD");
    for result in response.results_by_time() {
        if let Some(cost) = result.total().and_then(|t| t.get("UnblendedCost")) {
            total += cost.amount().unwrap_or("0").parse::<f64>()?;
            if let Some(u) = cost.unit() {
                unit = u.to_string();
            }
        }
    }

    Ok((total, unit))
}

/// Cost per (month, service) between `start` and `today`.
/// Months are identified by their first day, e.g. "2026-11-01".
pub async fn services_by_month(
    profile: &str,
    start: NaiveDate,
    today: NaiveDate,
) -> Result<Vec<(String, String, f64)>> {
    let sdk_config = aws_config::defaults(BehaviorVersion::latest())
        .profile_name(profile)
        .region("us-east-1")
        .load()
        .await;
    let client = aws_sdk_costexplorer::Client::new(&sdk_config);

    let period = DateInterval::builder()
        .start(start.format("%Y-%m-%d").to_string())
        .end(today.succ_opt().unwrap().format("%Y-%m-%d").to_string())
        .build()?;

    let response = client
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
        .await
        .with_context(|| format!("listing services with profile {profile}"))?;

    let mut rows = Vec::new();
    for result in response.results_by_time() {
        let month = result.time_period().map(|p| p.start().to_string()).unwrap_or_default();
        for group in result.groups() {
            let amount = group
                .metrics()
                .and_then(|m| m.get("UnblendedCost"))
                .and_then(|c| c.amount())
                .unwrap_or("0")
                .parse::<f64>()?;
            rows.push((month.clone(), group.keys().join(", "), amount));
        }
    }
    Ok(rows)
}

/// AWS console page for a Cost Explorer service name.
/// Unknown services fall back to the Bills page, which breaks costs down per service.
pub fn console_url(service: &str) -> &'static str {
    match service {
        "AWS Cost Explorer" => "https://console.aws.amazon.com/costmanagement/home#/cost-explorer",
        "Tax" => "https://console.aws.amazon.com/billing/home#/bills",
        "Amazon Simple Storage Service" => "https://console.aws.amazon.com/s3/home",
        "AmazonCloudWatch" => "https://console.aws.amazon.com/cloudwatch/home",
        "CloudWatch Events" => "https://console.aws.amazon.com/events/home",
        "AWS Glue" => "https://console.aws.amazon.com/glue/home",
        "AWS Key Management Service" => "https://console.aws.amazon.com/kms/home",
        "Amazon Elastic Compute Cloud - Compute" | "EC2 - Other" => "https://console.aws.amazon.com/ec2/home",
        "Amazon Relational Database Service" => "https://console.aws.amazon.com/rds/home",
        "AWS Lambda" => "https://console.aws.amazon.com/lambda/home",
        "Amazon DynamoDB" => "https://console.aws.amazon.com/dynamodbv2/home",
        "Amazon Virtual Private Cloud" => "https://console.aws.amazon.com/vpc/home",
        "Amazon Route 53" => "https://console.aws.amazon.com/route53/v2/home",
        "Amazon CloudFront" => "https://console.aws.amazon.com/cloudfront/v4/home",
        "Amazon Simple Notification Service" => "https://console.aws.amazon.com/sns/v3/home",
        "Amazon Simple Queue Service" => "https://console.aws.amazon.com/sqs/v3/home",
        "AWS Secrets Manager" => "https://console.aws.amazon.com/secretsmanager/home",
        "Amazon EC2 Container Registry (ECR)" => "https://console.aws.amazon.com/ecr/home",
        "Amazon Elastic Container Service" => "https://console.aws.amazon.com/ecs/v2/home",
        "AWS CloudTrail" => "https://console.aws.amazon.com/cloudtrail/home",
        "Amazon API Gateway" => "https://console.aws.amazon.com/apigateway/home",
        _ => "https://console.aws.amazon.com/billing/home#/bills",
    }
}
