//! Live insertion needs an identifiable focused text control, not merely an
//! application name. These queries read identity/state only, never text or
//! window titles. Missing accessibility support disables live insertion.

#[cfg(target_os = "linux")]
mod native {
    use dbus::blocking::Connection;
    use std::time::{Duration, Instant};
    type Object = (String, dbus::Path<'static>);
    pub(crate) struct Target {
        object: Object,
        window: Object,
        address: String,
    }

    fn state(connection: &Connection, object: &Object) -> Option<u64> {
        let (words,): (Vec<u32>,) = connection
            .with_proxy(
                object.0.as_str(),
                object.1.clone(),
                Duration::from_millis(100),
            )
            .method_call("org.a11y.atspi.Accessible", "GetState", ())
            .ok()?;
        Some(words.first().copied()? as u64 | ((words.get(1).copied().unwrap_or(0) as u64) << 32))
    }
    fn children(connection: &Connection, object: &Object) -> Option<Vec<Object>> {
        let (children,): (Vec<Object>,) = connection
            .with_proxy(
                object.0.as_str(),
                object.1.clone(),
                Duration::from_millis(100),
            )
            .method_call("org.a11y.atspi.Accessible", "GetChildren", ())
            .ok()?;
        Some(children)
    }
    fn editable_focused(bits: u64) -> bool {
        bits & ((1 << 7) | (1 << 8) | (1 << 12)) == ((1 << 7) | (1 << 8) | (1 << 12))
            && bits & ((1 << 6) | (1 << 43)) == 0
    }

    impl Target {
        pub(crate) fn capture() -> Option<Self> {
            let session = Connection::new_session().ok()?;
            let (address,): (String,) = session
                .with_proxy("org.a11y.Bus", "/org/a11y/bus", Duration::from_millis(100))
                .method_call("org.a11y.Bus", "GetAddress", ())
                .ok()?;
            let bus = Connection::new_address(&address).ok()?;
            let root = (
                "org.a11y.atspi.Registry".into(),
                dbus::Path::from("/org/a11y/atspi/accessible/root"),
            );
            let mut queue = std::collections::VecDeque::from([(root, None::<Object>)]);
            let mut seen = std::collections::HashSet::new();
            let start = Instant::now();
            while let Some((object, active)) = queue.pop_front() {
                if seen.len() >= 256 || start.elapsed() >= Duration::from_millis(750) {
                    return None;
                }
                if !seen.insert(object.clone()) {
                    continue;
                }
                let bits = state(&bus, &object)?;
                let active = if bits & (1 << 1) != 0 {
                    Some(object.clone())
                } else {
                    active
                };
                if editable_focused(bits) {
                    if let Some(window) = active.clone() {
                        return Some(Self {
                            object,
                            window,
                            address,
                        });
                    }
                }
                for child in children(&bus, &object)? {
                    queue.push_back((child, active.clone()));
                }
            }
            None
        }
        pub(crate) fn is_current(&self) -> bool {
            let Ok(bus) = Connection::new_address(&self.address) else {
                return false;
            };
            state(&bus, &self.object).is_some_and(editable_focused)
                && state(&bus, &self.window).is_some_and(|bits| bits & (1 << 1) != 0)
        }
    }
    #[cfg(test)]
    mod native_fixture {
        include!("tests/live_target.rs");
    }
    #[cfg(test)]
    pub(super) fn native_test(app: &tauri::AppHandle) {
        native_fixture::native_test(app);
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn only_enabled_editable_focused_nondefunct_controls_allow_live_output() {
            let valid = (1 << 7) | (1 << 8) | (1 << 12);
            assert!(editable_focused(valid));
            for bit in [7, 8, 12] {
                assert!(!editable_focused(valid & !(1 << bit)));
            }
            for bit in [6, 43] {
                assert!(!editable_focused(valid | (1 << bit)));
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod native {
    use crate::windows_uia::{client::UiaClient, com::initialize_com_mta};
    use windows::Win32::System::Ole::{
        SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound,
        SafeArrayGetUBound,
    };
    pub(crate) struct Target(Vec<i32>);
    impl Target {
        pub(crate) fn capture() -> Option<Self> {
            let _com = initialize_com_mta().ok()?;
            let client = UiaClient::new().ok()?;
            let element = client.get_focused_element_with_retry(0, 0).ok()?;
            // SAFETY: UIA supplies an owned SAFEARRAY of 32-bit runtime IDs.
            // Bound every access and destroy it on all success/error paths.
            unsafe {
                if element.CurrentIsPassword().ok()?.as_bool() {
                    return None;
                }
                let array = element.GetRuntimeId().ok()?;
                if array.is_null() {
                    return None;
                }
                let result = (|| {
                    if SafeArrayGetDim(array) != 1 {
                        return None;
                    }
                    let lo = SafeArrayGetLBound(array, 1).ok()?;
                    let hi = SafeArrayGetUBound(array, 1).ok()?;
                    if hi < lo || i64::from(hi) - i64::from(lo) > 63 {
                        return None;
                    }
                    let mut identity = Vec::new();
                    for index in lo..=hi {
                        let mut value = 0i32;
                        SafeArrayGetElement(array, &index, (&mut value as *mut i32).cast()).ok()?;
                        identity.push(value);
                    }
                    Some(Self(identity))
                })();
                let _ = SafeArrayDestroy(array);
                result
            }
        }
        pub(crate) fn is_current(&self) -> bool {
            Self::capture().is_some_and(|current| current.0 == self.0)
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use std::ffi::c_void;
    type Ref = *const c_void;
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> Ref;
        fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
        fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(value: Ref);
        fn CFEqual(left: Ref, right: Ref) -> u8;
        fn CFStringCreateWithCString(
            allocator: Ref,
            text: *const std::ffi::c_char,
            encoding: u32,
        ) -> Ref;
    }
    // Owned immutable CF reference. Identity comparisons run on the GUI thread;
    // CF retain/release ownership is not tied to that thread.
    pub(crate) struct Target(usize);
    impl Drop for Target {
        fn drop(&mut self) {
            unsafe {
                CFRelease(self.0 as Ref);
            }
        }
    }
    impl Target {
        pub(crate) fn capture() -> Option<Self> {
            unsafe {
                let system = AXUIElementCreateSystemWide();
                if system.is_null() {
                    return None;
                }
                let _ = AXUIElementSetMessagingTimeout(system, 0.1);
                let attribute = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"AXFocusedUIElement".as_ptr(),
                    0x08000100,
                );
                if attribute.is_null() {
                    CFRelease(system);
                    return None;
                }
                let mut focused = std::ptr::null();
                let result = AXUIElementCopyAttributeValue(system, attribute, &mut focused);
                CFRelease(attribute);
                CFRelease(system);
                if result != 0 || focused.is_null() {
                    return None;
                }
                Some(Self(focused as usize))
            }
        }
        pub(crate) fn is_current(&self) -> bool {
            Self::capture()
                .is_some_and(|other| unsafe { CFEqual(self.0 as Ref, other.0 as Ref) != 0 })
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod native {
    pub(crate) struct Target;
    impl Target {
        pub(crate) fn capture() -> Option<Self> {
            None
        }
        pub(crate) fn is_current(&self) -> bool {
            false
        }
    }
}
pub(crate) use native::Target as LiveOutputTarget;

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn native_test(app: &tauri::AppHandle) {
    native::native_test(app);
}
