//! uQ theme following (cpp/theme.cpp): colours, icons and font from uQ's rendered theme, live.

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("notify-panel/cpp/theme.h");
        /// Apply uQ's theme now and whenever it changes; does nothing without uQ.
        fn notifi_follow_uq_theme();
    }
}
