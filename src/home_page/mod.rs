use crate::app::{get_month_spend, get_services, ServiceCost};
use crate::burning_dollar::BurningDollar;
use leptos::prelude::*;

turf::style_sheet!("src/home_page/home_page.scss");
const _: &str = include_str!("home_page.scss");

#[component]
pub fn HomePage() -> impl IntoView {
    let spend = Resource::new(|| (), |_| get_month_spend());
    let services = Resource::new(|| (), |_| get_services());

    view! {
        <style>{STYLE_SHEET}</style>
        <div class=ClassName::PAGE>
            <nav class=ClassName::NAV>
                <div class=ClassName::NAV_INNER>
                    <span class=ClassName::BRAND>"🔥 Burn Pile"</span>
                    <span class=ClassName::NAV_NOTE>"AWS spend"</span>
                </div>
            </nav>

            <main class=ClassName::MAIN>
                <Suspense fallback=|| view! { <Loading /> }>
                    {move || Suspend::new(async move {
                        let spend = spend.await;
                        let services = services.await;
                        match (spend, services) {
                            (Ok(s), Ok(list)) => {
                                let last: f64 = list.iter().map(|x| x.last_month).sum();
                                let avg: f64 = list.iter().map(|x| x.avg).sum();
                                let billed = list.iter().filter(|x| x.this_month > 0.0).count();
                                view! {
                                    <section class=ClassName::HERO>
                                        <p class=ClassName::EYEBROW>"Spent this month"</p>
                                        <h1 class=ClassName::TOTAL>{usd(s.total)}</h1>
                                        <p class=ClassName::SUBTITLE>{date_range(&s.start, &s.end)}</p>
                                        <div class=ClassName::HERO_META>
                                            <Trend now=s.total compare=avg />
                                            <span>"vs. monthly average"</span>
                                        </div>
                                    </section>

                                    <section class=ClassName::TILES>
                                        <Tile label="Last month" value=usd(last) />
                                        <Tile label="Monthly average" value=usd(avg) />
                                        <Tile label="Services billed" value=billed.to_string() />
                                    </section>

                                    <section class=ClassName::CARD>
                                        <div class=ClassName::CARD_HEAD>
                                            <h2 class=ClassName::CARD_TITLE>"Burn pile"</h2>
                                            <p class=ClassName::CARD_NOTE>
                                                "This month's spend, one dollar bill at a time."
                                            </p>
                                        </div>
                                        <div class=ClassName::DOLLARS>
                                            {bill_percents(s.total)
                                                .into_iter()
                                                .map(|p| view! { <BurningDollar start_height=p /> })
                                                .collect_view()}
                                        </div>
                                    </section>

                                    <ServicesCard list billed />
                                }
                                    .into_any()
                            }
                            (Err(e), _) | (_, Err(e)) => view! { <ErrorCard message=e.to_string() /> }.into_any(),
                        }
                    })}
                </Suspense>
            </main>

            <footer class=ClassName::FOOTER>
                "Data from AWS Cost Explorer. New costs can take up to 24 hours to appear."
            </footer>
        </div>
    }
}

#[component]
fn ServicesCard(list: Vec<ServiceCost>, billed: usize) -> impl IntoView {
    let total = list.len();
    let secondary = format!("{} {}", ClassName::NUM, ClassName::HIDE_SM);
    view! {
        <section class=ClassName::CARD>
            <div class=ClassName::CARD_HEAD>
                <h2 class=ClassName::CARD_TITLE>"Services"</h2>
                <p class=ClassName::CARD_NOTE>
                    {billed} " billed this month · " {total} " in the last 12 months"
                </p>
            </div>
            <table class=ClassName::TABLE>
                <thead>
                    <tr>
                        <th class=ClassName::NAME>"Service"</th>
                        <th class=ClassName::NUM>"This month"</th>
                        <th class=secondary.clone()>"Last month"</th>
                        <th class=secondary.clone()>"Monthly avg"</th>
                        <th class=ClassName::NUM>"vs. avg"</th>
                    </tr>
                </thead>
                <tbody>
                    {list
                        .into_iter()
                        .map(|s| {
                            let idle = s.this_month == 0.0;
                            let title = s.name.clone();
                            view! {
                                <tr class=if idle { ClassName::IDLE } else { "" }>
                                    <td class=ClassName::NAME>
                                        <a href=s.url target="_blank" rel="noopener" title=title>
                                            {s.name}
                                        </a>
                                    </td>
                                    <td class=ClassName::NUM>{usd(s.this_month)}</td>
                                    <td class=secondary.clone()>{usd(s.last_month)}</td>
                                    <td class=secondary.clone()>{usd(s.avg)}</td>
                                    <td class=ClassName::NUM>
                                        <Trend now=s.this_month compare=s.avg />
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </section>
    }
}

/// Small label + value card.
#[component]
fn Tile(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class=ClassName::TILE>
            <span class=ClassName::TILE_LABEL>{label}</span>
            <span class=ClassName::TILE_VALUE>{value}</span>
        </div>
    }
}

/// Red pill when spending is up versus `compare`, green when down.
#[component]
fn Trend(now: f64, compare: f64) -> impl IntoView {
    let (tone, text) = trend(now, compare);
    let class = match tone {
        Tone::Up => format!("{} {}", ClassName::PILL, ClassName::UP),
        Tone::Down => format!("{} {}", ClassName::PILL, ClassName::DOWN),
        Tone::Flat => ClassName::PILL.to_string(),
    };
    view! { <span class=class>{text}</span> }
}

#[component]
fn Loading() -> impl IntoView {
    view! {
        <div class=ClassName::CENTER>
            <div class=ClassName::SPINNER role="status" aria-label="Loading"></div>
        </div>
    }
}

#[component]
fn ErrorCard(message: String) -> impl IntoView {
    view! {
        <section class=format!("{} {}", ClassName::CARD, ClassName::ERROR)>
            <h2 class=ClassName::CARD_TITLE>"Couldn't load AWS data"</h2>
            <p class=ClassName::ERROR_MESSAGE>{message}</p>
        </section>
    }
}

/// Small amounts get more decimals so cents of a cent are still visible.
fn usd(v: f64) -> String {
    if v == 0.0 || v.abs() >= 1.0 { format!("${v:.2}") } else { format!("${v:.4}") }
}

/// "2026-11-01", "2026-11-11" -> "Nov 1 – Nov 11, 2026". Falls back to the raw dates.
fn date_range(start: &str, end: &str) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let parse = |d: &str| -> Option<(u32, &str, u32)> {
        let mut parts = d.split('-');
        let year = parts.next()?.parse().ok()?;
        let month = MONTHS.get(parts.next()?.parse::<usize>().ok()?.checked_sub(1)?)?;
        let day = parts.next()?.parse().ok()?;
        Some((day, *month, year))
    };
    match (parse(start), parse(end)) {
        (Some((d1, m1, _)), Some((d2, m2, y))) => format!("{m1} {d1} – {m2} {d2}, {y}"),
        _ => format!("{start} – {end}"),
    }
}

enum Tone {
    Up,
    Down,
    Flat,
}

/// Tone and label for `now` compared to `compare`.
fn trend(now: f64, compare: f64) -> (Tone, String) {
    if compare == 0.0 {
        return if now == 0.0 { (Tone::Flat, "–".into()) } else { (Tone::Up, "new".into()) };
    }
    let change = (now - compare) / compare * 100.0;
    let tone = if change > 10.0 {
        Tone::Up
    } else if change < -10.0 {
        Tone::Down
    } else {
        Tone::Flat
    };
    let arrow = if change >= 0.0 { "▲" } else { "▼" };
    (tone, format!("{arrow} {:.0}%", change.abs()))
}

/// Spreads `total` USD evenly over `floor(total) + 1` bills.
/// Returns the fire % for each bill, e.g. 1.0 -> [50, 50].
fn bill_percents(total: f64) -> Vec<f64> {
    let total = total.max(0.0);
    let bills = total.floor() as usize + 1;
    vec![total / bills as f64 * 100.0; bills]
}

#[cfg(test)]
mod tests {
    use super::{bill_percents, date_range};

    #[test]
    fn spreads_spend_over_bills() {
        assert_eq!(bill_percents(0.0), vec![0.0]);
        assert_eq!(bill_percents(0.5), vec![50.0]);
        assert_eq!(bill_percents(1.0), vec![50.0, 50.0]);
        assert_eq!(bill_percents(2.4), vec![80.0, 80.0, 80.0]);
        assert_eq!(bill_percents(-1.0), vec![0.0]);
    }

    #[test]
    fn formats_date_range() {
        assert_eq!(date_range("2026-11-01", "2026-11-11"), "Nov 1 – Nov 11, 2026");
        assert_eq!(date_range("2026-12-01", "2026-12-31"), "Dec 1 – Dec 31, 2026");
        assert_eq!(date_range("bad", "2026-11-11"), "bad – 2026-11-11");
        assert_eq!(date_range("2026-13-01", "2026-11-11"), "2026-13-01 – 2026-11-11");
    }
}
