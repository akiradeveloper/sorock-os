FROM 'rust:1.59' as env

RUN rustup component add rustfmt
RUN cargo install cargo-chef --locked

WORKDIR /work

FROM env AS chef
COPY . /work
RUN cargo chef prepare --recipe-path=recipe.json

FROM env AS build 
COPY --from=chef /work/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path=recipe.json
COPY . /work
RUN cargo build --release --bin sorock

FROM 'rust:1.59-slim'
COPY --from=build /work/target/release/sorock .
EXPOSE 50000
ENTRYPOINT [ "./sorock" ]