#[cfg(desktop)]
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
#[cfg(desktop)]
use tauri::AppHandle;

/// Best-effort detection of whether it is safe to send a media play/pause toggle.
///
/// On Windows we attempt to detect active audio sessions so we don't accidentally
/// start playback when nothing is playing.
#[cfg(target_os = "windows")]
pub(crate) fn is_non_system_audio_session_active() -> Result<bool, String> {
    use windows::Win32::Media::Audio::{
        eConsole, eRender, AudioSessionStateActive, IAudioSessionManager2, IMMDevice,
        IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    unsafe {
        // Initialize COM (ignore error if already initialized)
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

        let device: IMMDevice = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("Failed to get default audio endpoint: {}", e))?;

        // Enumerate sessions on the default render endpoint.
        let session_manager = device
            .Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)
            .map_err(|e| format!("Failed to activate session manager: {}", e))?;

        let sessions = session_manager
            .GetSessionEnumerator()
            .map_err(|e| format!("Failed to get session enumerator: {}", e))?;

        let count = sessions
            .GetCount()
            .map_err(|e| format!("Failed to get session count: {}", e))?;

        for i in 0..count {
            let session = sessions
                .GetSession(i)
                .map_err(|e| format!("Failed to get session {}: {}", i, e))?;

            let state = session
                .GetState()
                .map_err(|e| format!("Failed to get session state: {}", e))?;
            if state == AudioSessionStateActive {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn is_non_system_audio_session_active() -> Result<bool, String> {
    // Best-effort on non-Windows platforms: we don't currently have a reliable
    // cross-platform way to detect whether audio is actively playing.
    Ok(true)
}

/// Toggle OS media play/pause.
///
/// On macOS, Enigo requires running on the main thread.
#[cfg(desktop)]
pub(crate) fn toggle_media_play_pause(app: &AppHandle) -> Result<(), String> {
    // On macOS, enigo requires running on the main thread.
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        app.run_on_main_thread(move || {
            let mut enigo = match Enigo::new(&Settings::default()) {
                Ok(e) => e,
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    return;
                }
            };

            let result = enigo
                .key(Key::MediaPlayPause, Direction::Click)
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
        return rx.recv().map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        // `app` is only needed on macOS (main-thread requirement). Silence the
        // unused-parameter warning on other platforms without changing behavior.
        let _ = app;
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        enigo
            .key(Key::MediaPlayPause, Direction::Click)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
