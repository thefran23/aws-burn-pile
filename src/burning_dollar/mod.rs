use leptos::prelude::*;
use std::time::Duration;

// Path is relative to Cargo.toml. Class names are renamed at compile time, so
// these styles only apply to this component.
turf::style_sheet!("src/burning_dollar/burning_dollar.scss");

// Makes cargo rebuild this file when the .scss changes (turf alone doesn't).
const _: &str = include_str!("burning_dollar.scss");

const FLAMES: usize = 9;

#[component]
pub fn BurningDollar(
    /// Starting flame height in % of the dollar.
    start_height: f64,
) -> impl IntoView {
    // Same starting heights on server and client so hydration matches.
    let heights = RwSignal::new(vec![start_height; FLAMES]);

    // Effects only run in the browser, so the timer never starts on the server.
    Effect::new(move |_| {
        let handle = set_interval_with_handle(
            move || {
                heights.update(|hs| {
                    for h in hs.iter_mut() {
                        // Flicker between 60% and 140% of the requested height.
                        *h = (start_height * (0.6 + js_sys::Math::random() * 0.8)).min(100.0);
                    }
                })
            },
            Duration::from_millis(180),
        )
        .ok();
        on_cleanup(move || {
            if let Some(handle) = handle {
                handle.clear();
            }
        });
    });

    let flames = (0..FLAMES)
        .map(|i| {
            let left = (i as f64 + 0.5) * 100.0 / FLAMES as f64;
            view! {
                <span
                    class=ClassName::FLAME
                    style:left=format!("{left}%")
                    style:height=move || format!("{}%", heights.with(|h| h[i]))
                ></span>
            }
        })
        .collect_view();

    view! {
        <style>{STYLE_SHEET}</style>
        <div class=ClassName::BURN>
            <img src="/1usd.jpg" alt="1 USD" width="240"/>
            <div class=ClassName::FIRE style=format!("--burn: {start_height}%")>{flames}</div>
        </div>
    }
}
