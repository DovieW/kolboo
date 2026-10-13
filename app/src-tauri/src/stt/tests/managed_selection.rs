use super::*;
use crate::pipeline::stt_provider::{create_cloud_stt_provider, SttProviderParams};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn managed_transcription_requests_keep_exact_provider_and_raw_model_identity() {
    for (provider, model, endpoint, response) in [
        (
            "groq",
            "whisper-large-v3",
            "/openai/v1/audio/transcriptions",
            serde_json::json!({"text":"synthetic"}),
        ),
        (
            "openai",
            "whisper-1",
            "/v1/audio/transcriptions",
            serde_json::json!({"text":"synthetic"}),
        ),
        (
            "openai",
            "gpt-4o-transcribe-diarize",
            "/v1/audio/transcriptions",
            serde_json::json!({"text":"synthetic","segments":[]}),
        ),
        (
            "openai",
            "gpt-4o-audio-preview",
            "/v1/responses",
            serde_json::json!({"output":[{"type":"message","content":[{"type":"output_text","text":"synthetic"}]}]}),
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .expect(1)
            .mount(&server)
            .await;
        let adapter = create_cloud_stt_provider(
            reqwest::Client::new(),
            SttProviderParams {
                provider_id: provider.into(),
                model: Some(model.into()),
                language: None,
                api_key: "synthetic-session".into(),
                proxy_settings: Default::default(),
                managed_gateway_url: Some(server.uri()),
                transcription_prompt: None,
                request_log_store: None,
            },
        )
        .unwrap();
        assert_eq!(
            adapter
                .transcribe_upload(
                    b"synthetic audio",
                    &AudioFormat::default(),
                    &SttUploadContext {
                        purpose: UploadPurpose::File,
                        chunk_offset: 0,
                        checkpoint_path: std::path::PathBuf::from(
                            "unused-vendor-specific-checkpoint"
                        ),
                    }
                )
                .await
                .unwrap()
                .text,
            "synthetic"
        );
        let request = server.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(request.headers["x-kolboo-model-provider"], provider);
        assert_eq!(request.headers["authorization"], "Bearer synthetic-session");
        assert!(String::from_utf8_lossy(&request.body).contains(model));
    }
}

#[tokio::test]
async fn credential_renewal_preserves_the_original_adapter_and_cannot_convert_byok() {
    for (provider, model, endpoint) in [
        (
            "groq",
            "whisper-large-v3",
            "/openai/v1/audio/transcriptions",
        ),
        ("openai", "whisper-1", "/v1/audio/transcriptions"),
        (
            "yiddishlabs",
            "yiddishlabs-3-turbo",
            "/v1/audio/transcriptions",
        ),
    ] {
        let original = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"text":"שלום"})),
            )
            .expect(1)
            .mount(&original)
            .await;
        let build = |managed: bool| {
            create_cloud_stt_provider(
                reqwest::Client::new(),
                SttProviderParams {
                    provider_id: provider.into(),
                    model: Some(model.into()),
                    language: Some("he".into()),
                    api_key: "original-key".into(),
                    proxy_settings: Default::default(),
                    managed_gateway_url: managed.then(|| original.uri()),
                    transcription_prompt: Some("synthetic context".into()),
                    request_log_store: None,
                },
            )
            .unwrap()
        };
        assert!(build(false).with_renewed_managed_token("new-key").is_none());
        let adapter = build(true);
        assert!(adapter.with_renewed_managed_token("").is_none());
        let renewed = adapter.with_renewed_managed_token("new-key").unwrap();
        assert_eq!(
            renewed
                .transcribe(b"synthetic audio", &AudioFormat::default())
                .await
                .unwrap(),
            "שלום"
        );
        let request = original.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(request.headers["authorization"], "Bearer new-key");
        assert_eq!(request.headers["x-kolboo-model-provider"], provider);
        let body = String::from_utf8_lossy(&request.body);
        assert!(body.contains(model));
        assert!(body.contains("he"));
        assert!(body.contains("synthetic context"));
        assert!(!body.contains("original-key"));
    }
    let byok_only = DeepgramSttProvider::new("synthetic".into(), None, None);
    assert!(byok_only.with_renewed_managed_token("new-key").is_none());
}

#[tokio::test]
async fn byok_endpoints_do_not_receive_kolboo_routing_hints_and_managed_realtime_is_disabled() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"text":"synthetic"})),
        )
        .expect(2)
        .mount(&server)
        .await;
    let providers: Vec<Box<dyn SttProvider>> = vec![
        Box::new(
            OpenAiSttProvider::new("synthetic-key".into(), Some("whisper-1".into()), None, None)
                .with_api_base_url(server.uri()),
        ),
        Box::new(
            GroqSttProvider::new("synthetic-key".into(), None, None, None)
                .with_api_base_url(server.uri()),
        ),
    ];
    for provider in providers {
        assert_eq!(
            provider
                .transcribe(b"synthetic", &AudioFormat::default())
                .await
                .unwrap(),
            "synthetic"
        );
    }
    assert!(server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .all(|request| !request.headers.contains_key("x-kolboo-model-provider")));
    let realtime = OpenAiSttProvider::new(
        "synthetic-session".into(),
        Some("gpt-live-transcribe".into()),
        None,
        None,
    )
    .with_managed_gateway(server.uri());
    assert!(!realtime.supports_streaming());
    assert!(realtime.requires_streaming());
    assert!(matches!(
        realtime
            .transcribe(b"synthetic", &AudioFormat::default())
            .await,
        Err(SttError::Config(_))
    ));
}
