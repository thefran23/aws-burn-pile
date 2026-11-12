# AWS Burn Pile — Roadmap

A learning project for **Rust** and **AWS**. It connects to one or more AWS accounts,
checks spending, and sends alerts to Discord, Slack, or Microsoft Teams when costs rise.

Each phase lists what to build, the Rust and AWS concepts it teaches, and when the phase is done.

---

## Phase 0 — Setup

- [ ] Install the AWS CLI v2 and run `aws configure sso` (or `aws configure`) to create a profile
- [ ] Turn on **Cost Explorer** in the AWS Billing console. Data can take up to 24 hours to appear the first time
- [ ] Create a read-only IAM user (e.g. `burn-pile`) with `AWSBillingReadOnlyAccess` and an access key, then `aws configure --profile burn-pile`
- [ ] Check that access works: `aws ce get-cost-and-usage --time-period Start=2026-10-01,End=2026-10-28 --granularity DAILY --metrics UnblendedCost --profile <name>`
- [ ] Run `git init` and add a `.gitignore` with `/target` and `burn-pile.toml`, because the config will hold webhook URLs

> ⚠️ Each Cost Explorer API call costs **$0.01**. Cache results while developing; don't call the API in a tight loop.

**Rust:** `cargo`, `cargo add`, `Cargo.toml`, editions.

---

## Phase 1 — Load the config file

Create `burn-pile.toml` and add a committed `burn-pile.example.toml`:

```toml
threshold_percent = 20.0   # alert if yesterday is 20% above the baseline
baseline_days = 7

[[accounts]]
name = "prod"
profile = "prod-readonly"

[[notifiers]]
kind = "discord"
webhook_url = "https://discord.com/api/webhooks/..."
```

- [ ] Add `src/config.rs` with `#[derive(Deserialize)]` structs
- [ ] Load the file with `std::fs::read_to_string` and `toml::from_str`
- [ ] Return `anyhow::Result<Config>` and add context to errors with `.context("...")`

**Rust:** modules (`mod config;`), structs, `serde` derive, `Result` and the `?` operator, `Vec<T>`, enums with `#[serde(rename_all = "lowercase")]`.

**Done when:** `cargo run` prints the parsed config with `{:#?}`.

---

## Phase 2 — Get costs from AWS

- [ ] Add `src/cost.rs`
- [ ] Build an SDK config for each profile:
  `aws_config::defaults(BehaviorVersion::latest()).profile_name(p).region("us-east-1").load().await`
  The Cost Explorer endpoint is in `us-east-1`
- [ ] Call `GetCostAndUsage` with daily granularity, the `UnblendedCost` metric, and the last `baseline_days + 1` days
- [ ] Turn the response into `Vec<(NaiveDate, f64)>`
- [ ] Optional: group by `SERVICE` so alerts can name what caused the increase

**Rust:** `async`/`await`, `#[tokio::main]`, `Option` versus `Result`, iterators (`iter().map().collect()`), `chrono::NaiveDate`, parsing strings into `f64`.

**AWS:** credential chain and profiles, SSO, regions, Cost Explorer concepts such as granularity, metrics, and `GroupBy`.

**Done when:** the tool prints daily costs for each account.

---

## Phase 3 — Decide when to alert

- [ ] Add `src/analysis.rs` with a pure function:
  `fn check(daily: &[f64], threshold_pct: f64) -> Option<Spike>`
- [ ] Use the average of the previous N days as the baseline and compare yesterday against it
- [ ] Write unit tests with `#[cfg(test)]` and run them with `cargo test`. They need no AWS access
- [ ] Ideas for later: compare month-to-date spend with last month, use `GetCostForecast`, or set an absolute dollar threshold

**Rust:** borrowing (`&[f64]`), pure functions, unit tests, and returning a `struct` inside `Option`.

**Done when:** tests pass for no-change, spike, and empty-data cases.

---

## Phase 4 — Send alerts

- [ ] `cargo add reqwest --no-default-features --features json,rustls-tls`
- [ ] Add `src/notify.rs` with an `enum Notifier { Discord, Slack, Teams }`, each holding a URL
- [ ] Send these payloads:
  | Target | Payload |
  |---|---|
  | Discord | `{"content": "..."}` (max 2000 characters) |
  | Slack | `{"text": "..."}` |
  | Teams | Workflows webhook with an Adaptive Card. Old O365 connectors are deprecated |
- [ ] Use `serde_json::json!` to build payloads and check the result with `error_for_status()`
- [ ] Add a `--dry-run` flag that prints alerts instead of sending them

**Rust:** enums with data, `match`, traits as an optional refactor (`trait Notifier { async fn send(...) }`), and an HTTP client.

**Done when:** an alert reaches a test Discord channel.

---

## Phase 5 — Polish the CLI

- [ ] Add `clap` with derive for arguments such as `--config` and `--dry-run`
- [ ] Add `tracing` and `tracing-subscriber` for logging
- [ ] Check accounts concurrently with `futures::future::join_all` or `tokio::spawn`
- [ ] If one account fails, keep checking the others and report the error

**Rust:** concurrency, `Send`/`'static` bounds, and error-handling strategy.

---

## Phase 6 — Run it in AWS

Pick one option:

- **Option A: Lambda.** Use `cargo lambda` to build for `provided.al2023` and schedule it daily with an **EventBridge Scheduler** rule
- **Option B: ECS or a cron job on a small server.** Simpler, but you manage the server

Then:
- [ ] Move webhook URLs to **SSM Parameter Store** or **Secrets Manager**
- [ ] For multiple accounts, use an IAM role in each account and `sts:AssumeRole` instead of local profiles. If you have AWS Organizations, querying the payer account covers all linked accounts; group by `LINKED_ACCOUNT`
- [ ] Write the infrastructure as code with **AWS CDK**, **Terraform**, or **SAM**

**AWS:** Lambda, EventBridge, IAM roles and trust policies, STS, SSM, and CloudWatch Logs.

---

## Phase 7 — Stretch goals

- [ ] Store history in DynamoDB to avoid repeated Cost Explorer calls and alerts
- [ ] Set a cooldown so the same spike doesn't alert every day
- [ ] Create a weekly summary report with top services and the week-over-week change
- [ ] Use AWS Budgets or Cost Anomaly Detection and compare them with your own logic
- [ ] Add a GitHub Actions CI job that runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`

---

## Target layout

```
src/
  main.rs       # wiring: load config → fetch → analyze → notify
  config.rs
  cost.rs
  analysis.rs
  notify.rs
burn-pile.example.toml
```

## Resources

- The Rust Book: https://doc.rust-lang.org/book/
- AWS SDK for Rust: https://docs.aws.amazon.com/sdk-for-rust/
- Cost Explorer API: https://docs.aws.amazon.com/aws-cost-management/latest/APIReference/API_GetCostAndUsage.html
- cargo-lambda: https://www.cargo-lambda.info/
