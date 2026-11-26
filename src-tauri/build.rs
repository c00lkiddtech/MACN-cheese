fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| std::env::var("TARGET_TRIPLE").unwrap_or_default());
    println!("cargo:rustc-env=TARGET_TRIPLE={target}");
    tauri_build::build();
}
