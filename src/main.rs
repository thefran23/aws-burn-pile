#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use aws_burn_pile::app::{shell, App};
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};

    // Plain-text logs to stdout; on Lambda these end up in CloudWatch Logs.
    // Change the level with RUST_LOG, e.g. RUST_LOG=debug.
    // aws_config is noisy at info (logs on every credential load), so it's limited to warnings.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,aws_config=warn")),
        )
        .init();

    // Logs one line per request: method, path, status and duration.
    async fn log_request(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
        let method = req.method().clone();
        let path = req.uri().path().to_owned();
        let start = std::time::Instant::now();
        let response = next.run(req).await;
        tracing::info!("{method} {path} -> {} in {:?}", response.status().as_u16(), start.elapsed());
        response
    }

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .layer(axum::middleware::from_fn(log_request))
        .with_state(leptos_options);

    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
