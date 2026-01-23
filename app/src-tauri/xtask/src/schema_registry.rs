use schemars::schema::RootSchema;

pub(crate) struct SchemaSpec {
	pub out_file: &'static str,
	pub label: &'static str,
	pub generator: fn() -> RootSchema,
}

fn gen_export_audio_capture_diagnostics_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::AudioCaptureDiagnostics)
}

fn gen_export_audio_level_stats_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::AudioLevelStats)
}

fn gen_export_audio_settings_test_wavs_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::AudioSettingsTestWavs)
}

fn gen_export_available_providers_response_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::AvailableProvidersResponse)
}

fn gen_export_cache_router_embeddings_response_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::CacheRouterEmbeddingsResponse)
}

fn gen_export_connection_state_changed_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::ConnectionStateChangedPayload)
}

fn gen_export_cost_by_provider_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::CostByProviderResponse)
}

fn gen_export_cost_summary_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::CostSummaryResponse)
}

fn gen_export_data_storage_summary_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::DataStorageSummary)
}

fn gen_export_default_sections_response_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::DefaultSectionsResponse)
}

fn gen_export_history_changed_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_history_delete_mode_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::HistoryDeleteMode)
}

fn gen_export_history_delete_options_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::HistoryDeleteOptions)
}

fn gen_export_history_delete_result_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::HistoryDeleteResult)
}

fn gen_export_history_page_query_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::HistoryPageQuery)
}

fn gen_export_history_page_result_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::HistoryPageResult)
}

fn gen_export_hotkey_config_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::HotkeyConfig)
}

fn gen_export_intent_router_settings_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::IntentRouterSettings)
}

fn gen_export_iterate_rewrite_prompt_response_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::IterateRewritePromptResponse)
}

fn gen_export_llm_complete_response_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::LlmCompleteResponse)
}

fn gen_export_llm_provider_info_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::LlmProviderInfo)
}

fn gen_export_local_whisper_backend_status_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::LocalWhisperBackendStatus)
}

fn gen_export_local_whisper_model_load_event_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::LocalWhisperModelLoadEvent)
}

fn gen_export_mic_test_audio_level_payload_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::MicTestAudioLevelPayload)
}

fn gen_export_model_option_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::ModelOption)
}

fn gen_export_model_pricing_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::ModelPricingResponse)
}

fn gen_export_open_window_info_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::OpenWindowInfo)
}

fn gen_export_overlay_audio_level_payload_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::OverlayAudioLevelPayload)
}

fn gen_export_overlay_hide_requested_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_pipeline_cancelled_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_pipeline_error_payload_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::PipelineErrorPayload)
}

fn gen_export_pipeline_recording_started_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_pipeline_reset_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_pipeline_rewriting_started_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_pipeline_routing_started_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_pipeline_state_changed_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::PipelineStateEvent)
}

fn gen_export_pipeline_transcript_ready_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::PipelineTranscriptReadyPayload)
}

fn gen_export_pipeline_transcription_started_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_proxy_settings_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::ProxySettings)
}

fn gen_export_quick_ask_answer_payload_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::QuickAskAnswerPayload)
}

fn gen_export_quick_ask_started_payload_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::QuickAskStartedPayload)
}

fn gen_export_recording_start_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_recording_stop_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_recordings_stats_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::RecordingsStats)
}

fn gen_export_request_log_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::RequestLog)
}

fn gen_export_rewrite_preset_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::RewritePreset)
}

fn gen_export_rewrite_program_profile_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::RewriteProgramPromptProfile)
}

fn gen_export_settings_changed_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::SettingsChangedPayload)
}

fn gen_export_stats_changed_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::EmptyEventPayload)
}

fn gen_export_system_event_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::SystemEvent)
}

fn gen_export_system_proxy_info_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::SystemProxyInfo)
}

fn gen_export_test_llm_rewrite_response_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::TestLlmRewriteResponse)
}

fn gen_export_test_rewrite_with_prompt_response_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::TestRewriteWithPromptResponse)
}

fn gen_export_whisper_model_download_progress_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::WhisperModelDownloadProgress)
}

fn gen_export_whisper_model_download_status_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::WhisperModelDownloadStatus)
}

fn gen_export_whisper_model_info_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::WhisperModelInfo)
}

fn gen_export_windows_internet_proxy_settings_schema() -> RootSchema {
	schemars::schema_for!(kolboo_lib::WindowsInternetProxySettings)
}

pub(crate) const SCHEMAS: &[SchemaSpec] = &[
	SchemaSpec {
		out_file: "audio-capture-diagnostics.schema.json",
		label: "AudioCaptureDiagnostics",
		generator: gen_export_audio_capture_diagnostics_schema,
	},
	SchemaSpec {
		out_file: "audio-level-stats.schema.json",
		label: "AudioLevelStats",
		generator: gen_export_audio_level_stats_schema,
	},
	SchemaSpec {
		out_file: "audio-settings-test-wavs.schema.json",
		label: "AudioSettingsTestWavs",
		generator: gen_export_audio_settings_test_wavs_schema,
	},
	SchemaSpec {
		out_file: "available-providers-response.schema.json",
		label: "AvailableProvidersResponse",
		generator: gen_export_available_providers_response_schema,
	},
	SchemaSpec {
		out_file: "cache-router-embeddings-response.schema.json",
		label: "CacheRouterEmbeddingsResponse",
		generator: gen_export_cache_router_embeddings_response_schema,
	},
	SchemaSpec {
		out_file: "connection-state-changed.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_CONNECTION_STATE_CHANGED),
		generator: gen_export_connection_state_changed_schema,
	},
	SchemaSpec {
		out_file: "cost-by-provider.schema.json",
		label: "CostByProviderResponse",
		generator: gen_export_cost_by_provider_schema,
	},
	SchemaSpec {
		out_file: "cost-summary.schema.json",
		label: "CostSummaryResponse",
		generator: gen_export_cost_summary_schema,
	},
	SchemaSpec {
		out_file: "data-storage-summary.schema.json",
		label: "DataStorageSummary",
		generator: gen_export_data_storage_summary_schema,
	},
	SchemaSpec {
		out_file: "default-sections-response.schema.json",
		label: "DefaultSectionsResponse",
		generator: gen_export_default_sections_response_schema,
	},
	SchemaSpec {
		out_file: "history-changed.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_HISTORY_CHANGED),
		generator: gen_export_history_changed_schema,
	},
	SchemaSpec {
		out_file: "history-delete-mode.schema.json",
		label: "HistoryDeleteMode",
		generator: gen_export_history_delete_mode_schema,
	},
	SchemaSpec {
		out_file: "history-delete-options.schema.json",
		label: "HistoryDeleteOptions",
		generator: gen_export_history_delete_options_schema,
	},
	SchemaSpec {
		out_file: "history-delete-result.schema.json",
		label: "HistoryDeleteResult",
		generator: gen_export_history_delete_result_schema,
	},
	SchemaSpec {
		out_file: "history-page-query.schema.json",
		label: "HistoryPageQuery",
		generator: gen_export_history_page_query_schema,
	},
	SchemaSpec {
		out_file: "history-page-result.schema.json",
		label: "HistoryPageResult",
		generator: gen_export_history_page_result_schema,
	},
	SchemaSpec {
		out_file: "hotkey-config.schema.json",
		label: "HotkeyConfig",
		generator: gen_export_hotkey_config_schema,
	},
	SchemaSpec {
		out_file: "intent-router-settings.schema.json",
		label: "IntentRouterSettings",
		generator: gen_export_intent_router_settings_schema,
	},
	SchemaSpec {
		out_file: "iterate-rewrite-prompt-response.schema.json",
		label: "IterateRewritePromptResponse",
		generator: gen_export_iterate_rewrite_prompt_response_schema,
	},
	SchemaSpec {
		out_file: "llm-complete-response.schema.json",
		label: "LlmCompleteResponse",
		generator: gen_export_llm_complete_response_schema,
	},
	SchemaSpec {
		out_file: "llm-provider-info.schema.json",
		label: "LlmProviderInfo",
		generator: gen_export_llm_provider_info_schema,
	},
	SchemaSpec {
		out_file: "local-whisper-backend-status.schema.json",
		label: "LocalWhisperBackendStatus",
		generator: gen_export_local_whisper_backend_status_schema,
	},
	SchemaSpec {
		out_file: "local-whisper-model-load-event.schema.json",
		label: "LocalWhisperModelLoadEvent",
		generator: gen_export_local_whisper_model_load_event_schema,
	},
	SchemaSpec {
		out_file: "mic-test-audio-level-payload.schema.json",
		label: "MicTestAudioLevelPayload",
		generator: gen_export_mic_test_audio_level_payload_schema,
	},
	SchemaSpec {
		out_file: "model-option.schema.json",
		label: "ModelOption",
		generator: gen_export_model_option_schema,
	},
	SchemaSpec {
		out_file: "model-pricing.schema.json",
		label: "ModelPricingResponse",
		generator: gen_export_model_pricing_schema,
	},
	SchemaSpec {
		out_file: "open-window-info.schema.json",
		label: "OpenWindowInfo",
		generator: gen_export_open_window_info_schema,
	},
	SchemaSpec {
		out_file: "overlay-audio-level-payload.schema.json",
		label: "OverlayAudioLevelPayload",
		generator: gen_export_overlay_audio_level_payload_schema,
	},
	SchemaSpec {
		out_file: "overlay-hide-requested.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_OVERLAY_HIDE_REQUESTED),
		generator: gen_export_overlay_hide_requested_schema,
	},
	SchemaSpec {
		out_file: "pipeline-cancelled.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_PIPELINE_CANCELLED),
		generator: gen_export_pipeline_cancelled_schema,
	},
	SchemaSpec {
		out_file: "pipeline-error-payload.schema.json",
		label: "PipelineErrorPayload",
		generator: gen_export_pipeline_error_payload_schema,
	},
	SchemaSpec {
		out_file: "pipeline-recording-started.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_PIPELINE_RECORDING_STARTED),
		generator: gen_export_pipeline_recording_started_schema,
	},
	SchemaSpec {
		out_file: "pipeline-reset.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_PIPELINE_RESET),
		generator: gen_export_pipeline_reset_schema,
	},
	SchemaSpec {
		out_file: "pipeline-rewriting-started.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_PIPELINE_REWRITING_STARTED),
		generator: gen_export_pipeline_rewriting_started_schema,
	},
	SchemaSpec {
		out_file: "pipeline-routing-started.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_PIPELINE_ROUTING_STARTED),
		generator: gen_export_pipeline_routing_started_schema,
	},
	SchemaSpec {
		out_file: "pipeline-state-changed.schema.json",
		label: "PipelineStateEvent",
		generator: gen_export_pipeline_state_changed_schema,
	},
	SchemaSpec {
		out_file: "pipeline-transcript-ready.schema.json",
		label: "PipelineTranscriptReadyPayload",
		generator: gen_export_pipeline_transcript_ready_schema,
	},
	SchemaSpec {
		out_file: "pipeline-transcription-started.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_PIPELINE_TRANSCRIPTION_STARTED),
		generator: gen_export_pipeline_transcription_started_schema,
	},
	SchemaSpec {
		out_file: "proxy-settings.schema.json",
		label: "ProxySettings",
		generator: gen_export_proxy_settings_schema,
	},
	SchemaSpec {
		out_file: "quick-ask-answer-payload.schema.json",
		label: "QuickAskAnswerPayload",
		generator: gen_export_quick_ask_answer_payload_schema,
	},
	SchemaSpec {
		out_file: "quick-ask-started-payload.schema.json",
		label: "QuickAskStartedPayload",
		generator: gen_export_quick_ask_started_payload_schema,
	},
	SchemaSpec {
		out_file: "recording-start.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_RECORDING_START),
		generator: gen_export_recording_start_schema,
	},
	SchemaSpec {
		out_file: "recording-stop.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_RECORDING_STOP),
		generator: gen_export_recording_stop_schema,
	},
	SchemaSpec {
		out_file: "recordings-stats.schema.json",
		label: "RecordingsStats",
		generator: gen_export_recordings_stats_schema,
	},
	SchemaSpec {
		out_file: "request-log.schema.json",
		label: "RequestLog",
		generator: gen_export_request_log_schema,
	},
	SchemaSpec {
		out_file: "rewrite-preset.schema.json",
		label: "RewritePreset",
		generator: gen_export_rewrite_preset_schema,
	},
	SchemaSpec {
		out_file: "rewrite-program-profile.schema.json",
		label: "RewriteProgramPromptProfile",
		generator: gen_export_rewrite_program_profile_schema,
	},
	SchemaSpec {
		out_file: "settings-changed.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_SETTINGS_CHANGED),
		generator: gen_export_settings_changed_schema,
	},
	SchemaSpec {
		out_file: "stats-changed.schema.json",
		label: stringify!(kolboo_lib::events::EVENT_STATS_CHANGED),
		generator: gen_export_stats_changed_schema,
	},
	SchemaSpec {
		out_file: "system-event.schema.json",
		label: "SystemEvent",
		generator: gen_export_system_event_schema,
	},
	SchemaSpec {
		out_file: "system-proxy-info.schema.json",
		label: "SystemProxyInfo",
		generator: gen_export_system_proxy_info_schema,
	},
	SchemaSpec {
		out_file: "test-llm-rewrite-response.schema.json",
		label: "TestLlmRewriteResponse",
		generator: gen_export_test_llm_rewrite_response_schema,
	},
	SchemaSpec {
		out_file: "test-rewrite-with-prompt-response.schema.json",
		label: "TestRewriteWithPromptResponse",
		generator: gen_export_test_rewrite_with_prompt_response_schema,
	},
	SchemaSpec {
		out_file: "whisper-model-download-progress.schema.json",
		label: "WhisperModelDownloadProgress",
		generator: gen_export_whisper_model_download_progress_schema,
	},
	SchemaSpec {
		out_file: "whisper-model-download-status.schema.json",
		label: "WhisperModelDownloadStatus",
		generator: gen_export_whisper_model_download_status_schema,
	},
	SchemaSpec {
		out_file: "whisper-model-info.schema.json",
		label: "WhisperModelInfo",
		generator: gen_export_whisper_model_info_schema,
	},
	SchemaSpec {
		out_file: "windows-internet-proxy-settings.schema.json",
		label: "WindowsInternetProxySettings",
		generator: gen_export_windows_internet_proxy_settings_schema,
	},
];
