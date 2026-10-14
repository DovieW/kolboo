use super::*;

#[test]
fn refreshes_are_coalesced_and_shutdown_handoff_never_loses_changes() {
    let queue = crate::state::TrayRefresh::default();
    assert!(queue.request());
    assert!(!queue.request());
    queue.begin();
    assert!(!queue.finish());
    assert!(queue.request());
    queue.begin();
    assert!(!queue.request());
    assert!(queue.finish());
    queue.begin();
    assert!(!queue.finish());
    assert!(queue.request());
}

#[test]
fn selections_keep_stable_ids_duplicates_and_missing_devices_explicit() {
    let devices = vec![
        AudioInputDeviceInfo {
            id: "mic:v1:a:0".into(),
            name: "USB Mic".into(),
        },
        AudioInputDeviceInfo {
            id: "mic:v1:a:1".into(),
            name: "USB Mic".into(),
        },
        AudioInputDeviceInfo {
            id: "mic:v1:b:0".into(),
            name: "Built-in".into(),
        },
    ];
    for selected in [None, Some(""), Some("default"), Some(" default ")] {
        let options = microphone_options(&devices, selected, true);
        assert!(options[0].selected);
        assert_eq!(options.iter().filter(|option| option.selected).count(), 1);
    }
    let options = microphone_options(&devices, Some("mic:v1:a:1"), true);
    assert_eq!(options[1].label, "USB Mic · Device 1 of 2");
    assert_eq!(options[2].label, "USB Mic · Device 2 of 2");
    assert_eq!(options[3].label, "Built-in");
    assert!(options[2].selected);
    assert_eq!(options[2].id, "mic:v1:a:1");
    let legacy = microphone_options(&devices, Some("USB Mic"), false);
    assert!(legacy[1].selected);
    assert!(!legacy[2].selected);
    assert!(legacy.iter().all(|option| !option.enabled));
    let partial = microphone_options(&devices, Some("USB"), true);
    assert!(partial[1].selected);
    assert!(!partial[2].selected);
    let missing = microphone_options(&[], Some("mic:v1:gone:0"), true);
    assert_eq!(missing[1].label, "Selected microphone unavailable");
    assert!(missing[1].selected);
    assert!(!missing[1].enabled);
}

#[test]
fn tray_reads_latest_used_context_without_changing_settings() {
    let store = crate::request_log::RequestLogStore::new();
    assert_eq!(store.latest_profile_preset(), None);
    store.start_request_with("mock".into(), None, |log| {
        log.profile_name = Some("Writing".into());
        log.preset_name = Some("Email".into());
    });
    assert_eq!(
        store.latest_profile_preset(),
        Some((Some("Writing".into()), Some("Email".into())))
    );
    store.start_request("mock".into(), None);
    assert_eq!(
        store.latest_profile_preset().unwrap().0.as_deref(),
        Some("Writing")
    );
    store.start_request_with("mock".into(), None, |log| {
        log.profile_name = Some("Default".into())
    });
    assert_eq!(
        store.latest_profile_preset(),
        Some((Some("Default".into()), None))
    );
}

pub(crate) fn native_controls(app: &AppHandle) {
    assert_eq!(
        std::env::var("KOLBOO_NATIVE_WINDOW_TEST").as_deref(),
        Ok("1")
    );
    use tauri_plugin_store::StoreExt;
    let menu = build(
        app,
        &[AudioInputDeviceInfo {
            id: "mic:v1:fixture:0".into(),
            name: "Fixture microphone".into(),
        }],
    )
    .unwrap();
    assert_eq!(menu.items().unwrap().len(), 3);
    assert_eq!(
        context_items(app).unwrap()[0].text().unwrap(),
        "Last profile: Not used yet"
    );
    if app
        .try_state::<crate::request_log::RequestLogStore>()
        .is_none()
    {
        app.manage(crate::request_log::RequestLogStore::new());
    }
    let logs = app.state::<crate::request_log::RequestLogStore>();
    logs.start_request_with("mock".into(), None, |log| {
        log.profile_name = Some("Fixture profile".into());
        log.preset_name = Some("Fixture preset".into());
    });
    let items = context_items(app).unwrap();
    assert_eq!(items[0].text().unwrap(), "Last profile: Fixture profile");
    assert_eq!(items[1].text().unwrap(), "Last preset: Fixture preset");
    let settings = app.store("settings.json").unwrap();
    settings.set("selected_mic_id", "missing");
    let missing = build(app, &[]).unwrap();
    assert_eq!(missing.items().unwrap().len(), 3);
    // Missing pipeline/busy states fail closed; a stale device never gets persisted.
    let result = tauri::async_runtime::block_on(select_microphone(app, "mic:v1:gone:0"));
    assert!(result.is_err());
    assert_eq!(
        settings.get("selected_mic_id"),
        Some(serde_json::json!("missing"))
    );
    settings.delete("selected_mic_id");
    let meter = app.state::<crate::state::MicTestMeterState>();
    let cancel = tokio_util::sync::CancellationToken::new();
    meter.inner.lock().unwrap().cancel = Some(cancel.clone());
    assert!(!idle(app));
    assert!(
        tauri::async_runtime::block_on(select_microphone(app, "default"))
            .unwrap_err()
            .contains("Finish")
    );
    cancel.cancel();
    assert!(idle(app));
    {
        let _guard = meter.inner.lock().unwrap();
        assert!(!idle(app), "a busy meter lock fails closed");
    }
    meter.inner.lock().unwrap().cancel = None;
    let pipeline = app.state::<SharedPipeline>();
    let original = pipeline.config();
    pipeline.set_runtime_state_for_native_test(PipelineState::Recording);
    assert!(!idle(app));
    assert!(tauri::async_runtime::block_on(select_microphone(app, "default")).is_err());
    pipeline.set_runtime_state_for_native_test(PipelineState::Error);
    assert!(idle(app));
    pipeline.set_runtime_state_for_native_test(PipelineState::Idle);
    let devices = [AudioInputDeviceInfo {
        id: "mic:v1:fixture:0".into(),
        name: "Synthetic device snapshot".into(),
    }];
    tauri::async_runtime::block_on(apply_microphone_selection(app, &devices[0].id, &devices))
        .unwrap();
    assert_eq!(
        settings.get("selected_mic_id"),
        Some(serde_json::json!(devices[0].id))
    );
    assert_eq!(
        pipeline.config().input_device_name.as_deref(),
        Some(devices[0].id.as_str())
    );
    tauri::async_runtime::block_on(select_microphone(app, "default")).unwrap();
    assert_eq!(
        settings.get("selected_mic_id"),
        Some(serde_json::Value::Null)
    );
    assert_eq!(pipeline.config().input_device_name, None);
    // Disk failure must roll back the in-memory settings transaction.
    settings.set("selected_mic_id", "preserve-on-save-failure");
    settings.save().unwrap();
    let path = app.path().app_data_dir().unwrap().join("settings.json");
    let backup = tempfile::NamedTempFile::new_in(path.parent().unwrap()).unwrap();
    std::fs::rename(&path, backup.path()).unwrap();
    std::fs::create_dir(&path).unwrap();
    let rejected = tauri::async_runtime::block_on(select_microphone(app, "default"));
    std::fs::remove_dir(&path).unwrap();
    std::fs::rename(backup.path(), &path).unwrap();
    assert_eq!(
        rejected.unwrap_err(),
        "Microphone settings could not be saved"
    );
    assert_eq!(
        settings.get("selected_mic_id"),
        Some(serde_json::json!("preserve-on-save-failure"))
    );
    // Interrupt runtime access after settings are saved, not before the idle guard.
    use tauri::Listener;
    let runtime = pipeline.inner().clone();
    let listener = app.listen(crate::events::EVENT_SETTINGS_CHANGED, move |_| {
        runtime.poison_runtime_for_native_test();
    });
    let rejected = tauri::async_runtime::block_on(select_microphone(app, "default"));
    app.unlisten(listener);
    pipeline.recover_runtime_for_native_test();
    assert_eq!(
        rejected.unwrap_err(),
        "Microphone runtime could not be updated"
    );
    assert_eq!(
        settings.get("selected_mic_id"),
        Some(serde_json::Value::Null)
    );
    pipeline.update_config(original).unwrap();
    settings.delete("selected_mic_id");
    on_menu_event(
        app,
        tauri::menu::MenuEvent {
            id: tauri::menu::MenuId::new("unrelated"),
        },
    );
    logs.clear();
}
