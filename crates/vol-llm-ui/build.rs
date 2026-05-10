fn main() {
    let build_time = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
}
