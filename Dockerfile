FROM rust:1.66 AS builder
RUN mkdir src && touch src/lib.rs
COPY Cargo.* .
RUN cargo build --release
COPY src src
RUN touch /src/main.rs
RUN cargo build --release

FROM linuxserver/ffmpeg:5.1.2
COPY --from=builder /target/release/ffserve /ffserve
RUN mkdir /data && chown nobody. /data
WORKDIR /
USER nobody
ENTRYPOINT ["/ffserve"]
