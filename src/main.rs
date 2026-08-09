//! netchecker binary entry point. All logic lives in the library crate so it
//! can be reused and documented; `main` just runs it on a Tokio runtime.

#[tokio::main]
async fn main() {
    netchecker::run().await;
}
