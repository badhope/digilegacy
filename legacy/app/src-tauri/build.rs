fn main() {
    tauri_build::build();

    // Windows 上用静态 OpenSSL（SQLCipher 的加密后端）时，libcrypto 引用
    // `GetProcessWindowStation` / `GetUserObjectInformationW`，这两个符号在
    // user32.lib 里。vendored 路径由 openssl-sys 自带这条链接指令；改用外部
    // 提供的 OpenSSL 后必须自己补（见 Cargo.toml 里 rusqlite feature 的说明）。
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-lib=user32");
}
