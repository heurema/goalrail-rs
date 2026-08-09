FROM rust:1.97.1-alpine AS build

RUN apk add --no-cache bash
RUN rustup target add wasm32-unknown-unknown

WORKDIR /app
COPY . .

RUN cargo test --package gr-site
RUN ./scripts/build-site.sh \
    && test -f dist/site/index.html \
    && test -f dist/site/install.md \
    && test -f dist/site/gr_site.wasm

FROM nginx:1.27-alpine

LABEL org.opencontainers.image.source="https://github.com/heurema/goalrail-rs"
LABEL org.opencontainers.image.description="Goalrail public site"

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist/site /srv/goalrail

RUN nginx -t
