pub mod clipboard;
pub mod inject;
pub(crate) mod key_inject;
pub mod selection_probe;
#[cfg(all(desktop, target_os = "linux"))]
pub(crate) mod wayland_input;
