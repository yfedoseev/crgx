fn main() {
    println!(
        "cargo:rustc-env=CRGX_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
