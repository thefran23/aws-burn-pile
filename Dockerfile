FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown \
 && curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
 && cargo binstall -y cargo-leptos
WORKDIR /app
COPY . .
RUN cargo leptos build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
# Lambda Web Adapter: forwards Lambda invocations to the HTTP server below.
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.9.1 /lambda-adapter /opt/extensions/lambda-adapter
WORKDIR /app
COPY --from=build /app/target/release/aws-burn-pile /app/aws-burn-pile
COPY --from=build /app/target/site /app/site
COPY burn-pile.toml /app/burn-pile.toml
ENV LEPTOS_OUTPUT_NAME=aws-burn-pile \
    LEPTOS_SITE_ROOT=site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:8080 \
    LEPTOS_ENV=PROD \
    AWS_LWA_PORT=8080
CMD ["/app/aws-burn-pile"]
