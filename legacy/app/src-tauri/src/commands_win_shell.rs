//! Windows Shell 帮助模块：宽字符包装与 ShellExecuteW 调用。
//!
//! 只被 `open_external`（打开官方链接）与 `reveal_exported_file`
//! （在文件管理器中定位导出文件）两个命令使用——两个域各持一份必然
//! 漂移，所以提为共享模块。仅 Windows 生效，非 Windows 调用方有降级分支。

#![allow(non_snake_case)]

use std::os::windows::ffi::OsStrExt;

// `ShellExecuteW` 只在 Windows 存在；返回 `HINSTANCE`（指针宽度）。
// 全项目只声明一次：两个调用点重复声明会触发 `clashing_extern_declarations`，
// 而签名不一致在 ABI 上属于未定义行为。
#[link(name = "shell32")]
extern "system" {
    pub fn ShellExecuteW(
        hwnd: *mut core::ffi::c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_cmd: i32,
    ) -> isize;
}

pub fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
