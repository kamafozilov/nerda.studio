//! The user's system accessibility settings, read where GPUI does not read them.

/// Whether the user asked the system to reduce motion. GPUI keeps this as an app
/// flag (`App::reduce_motion`) that no platform sets, so the app reads it here.
#[cfg(target_os = "macos")]
pub fn reduce_motion() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// Whether the user turned off "Animation effects", which Windows reports as
/// client-area animation.
#[cfg(target_os = "windows")]
pub fn reduce_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };

    let mut animate: windows_sys::core::BOOL = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through `pvparam`.
    let read = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut animate).cast(), 0)
    };
    read != 0 && animate == 0
}

/// Linux desktops have no one setting for this; motion stays on.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn reduce_motion() -> bool {
    false
}

/// Whether the user asked the system to reduce transparency; glass then turns
/// solid.
#[cfg(target_os = "macos")]
pub fn reduce_transparency() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceTransparency()
}

/// Whether the user turned off "Transparency effects", which Windows keeps as
/// `EnableTransparency` under the user's personalization key.
#[cfg(target_os = "windows")]
pub fn reduce_transparency() -> bool {
    use windows_sys::{
        Win32::{
            Foundation::ERROR_SUCCESS,
            System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
        },
        w,
    };

    let mut enabled: u32 = 1;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: RRF_RT_REG_DWORD writes at most `size` bytes, one DWORD, to `enabled`.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("EnableTransparency"),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut enabled).cast(),
            &raw mut size,
        )
    };
    status == ERROR_SUCCESS && enabled == 0
}

/// Linux has no glass to reduce: its windows are solid already.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn reduce_transparency() -> bool {
    false
}
