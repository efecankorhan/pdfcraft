//! Keeps the vendored winit pen patches in the build (vendor/README.md, winit (14) and (15)):
//! pen input arrives as mouse buttons, and its tip pressure is read through
//! `winit::platform::windows::pen_pressure`. Pressing a pen needs Windows and a tablet.

#[cfg(target_os = "windows")]
#[test]
fn pen_pressure_is_none_without_a_pen_in_contact() {
    assert_eq!(winit::platform::windows::pen_pressure(), None);
}
