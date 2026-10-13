use super::*;
use futures_util::{SinkExt, StreamExt};

#[tokio::test]
async fn startup_errors_and_closed_connections_never_expose_vendor_content() {
    for rejected in [true, false] {
        let (_write, mut read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let task =
            tokio::spawn(async move { wait_for_recognition_started(&mut read, "test").await });
        if rejected {
            peer.send(Message::Text(
                json!({"message":"Error","reason":"private vendor response"})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        } else {
            peer.close(None).await.unwrap();
        }
        let error = task.await.unwrap().unwrap_err();
        assert!(!error.to_string().contains("private"));
        assert!(if rejected {
            matches!(error, SttError::Api(_))
        } else {
            matches!(error, SttError::NetworkMessage(_))
        });
    }
}

#[tokio::test]
async fn startup_heartbeats_cannot_extend_the_handshake_deadline() {
    let (_write, mut read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let task = tokio::spawn(async move { wait_for_recognition_started(&mut read, "test").await });
    peer.send(Message::Text(
        json!({"message":"Heartbeat"}).to_string().into(),
    ))
    .await
    .unwrap();
    tokio::task::yield_now().await;
    tokio::time::pause();
    tokio::time::advance(DEFAULT_WS_TIMEOUT + Duration::from_secs(1)).await;
    assert!(matches!(task.await.unwrap(), Err(SttError::Timeout)));
}

#[tokio::test]
async fn closed_buffered_audio_delivery_is_an_error_not_a_short_transcript() {
    let (audio, input) = mpsc::channel(1);
    drop(input);
    let (_partials, output) = mpsc::channel(1);
    let provider = tokio::spawn(async { Ok("must not be accepted".into()) });
    let session = StreamingSttSession::new(audio, output, provider);
    let error = transcribe_buffered_session(session, &[0; 320], 16000, 1)
        .await
        .unwrap_err();
    assert!(matches!(error, SttError::NetworkMessage(_)));
    assert!(error.to_string().contains("before all upload audio"));
}

#[tokio::test]
async fn overlapping_final_results_fail_instead_of_pasting_a_corrupt_transcript() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (_audio, input) = mpsc::channel(1);
    let (partials, mut output) = mpsc::channel(4);
    let task = tokio::spawn(run_streaming_task(
        write, read, input, partials, 16000, None,
    ));
    peer.send(Message::Text(
        transcript("AddTranscript", "first", true)
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    assert_eq!(
        output.recv().await.unwrap().committed_text.as_deref(),
        Some("first")
    );
    let mut overlapping = transcript("AddTranscript", "private overlapping", true);
    overlapping["metadata"]["start_time"] = json!(0.05);
    overlapping["metadata"]["end_time"] = json!(0.2);
    peer.send(Message::Text(overlapping.to_string().into()))
        .await
        .unwrap();
    let error = task.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("overlapping"));
    assert!(!error.to_string().contains("private"));
}

#[tokio::test]
async fn complete_adapter_handshakes_and_delivers_buffered_and_live_audio() {
    use crate::stt::{AudioEncoding, AudioFormat, SttProvider};
    for live in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut peer = tokio_tungstenite::accept_hdr_async(
                socket,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                    assert_eq!(request.headers()["authorization"], "Bearer synthetic-key");
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let start: JsonValue =
                serde_json::from_str(peer.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            assert_eq!(start["message"], "StartRecognition");
            assert_eq!(start["transcription_config"]["language"], "he");
            assert_eq!(start["transcription_config"]["enable_partials"], live);
            peer.send(Message::Text(
                json!({"message":"RecognitionStarted"}).to_string().into(),
            ))
            .await
            .unwrap();
            let mut delivered = Vec::new();
            loop {
                match peer.next().await.unwrap().unwrap() {
                    Message::Binary(bytes) => delivered.extend_from_slice(&bytes),
                    Message::Text(text)
                        if serde_json::from_str::<JsonValue>(&text).unwrap()["message"]
                            == "EndOfStream" =>
                    {
                        break
                    }
                    _ => {}
                }
            }
            assert_eq!(delivered, 8192_i16.to_le_bytes().repeat(1601));
            peer.send(Message::Text(
                transcript("AddTranscript", "שלום\n  וועלט!", true)
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            peer.send(Message::Text(
                json!({"message":"EndOfTranscript"}).to_string().into(),
            ))
            .await
            .unwrap();
        });
        let mut provider =
            SpeechmaticsSttProvider::new("synthetic-key".into(), None, Some("he".into()));
        provider.batch_ws_url = endpoint.clone();
        provider.streaming_ws_url = endpoint;
        let store = RequestLogStore::new();
        store.start_request("speechmatics".into(), None);
        provider.request_log_store = Some(store.clone());
        assert!(provider
            .with_renewed_managed_token("must-not-promote-byok")
            .is_none());
        let directory = tempfile::tempdir().unwrap();
        let upload = crate::stt::SttUploadContext {
            purpose: crate::stt::UploadPurpose::Meeting,
            chunk_offset: 0,
            checkpoint_path: directory.path().join("unused-checkpoint"),
        };
        let text = if live {
            let session = provider.start_streaming(16000).await.unwrap();
            transcribe_buffered_session(session, &8192_i16.to_le_bytes().repeat(1601), 16000, 1)
                .await
                .unwrap()
        } else {
            provider
                .transcribe_upload(
                    &8192_i16.to_le_bytes().repeat(1601),
                    &AudioFormat {
                        sample_rate: 16000,
                        channels: 1,
                        encoding: AudioEncoding::Pcm16,
                    },
                    &upload,
                )
                .await
                .unwrap()
                .text
        };
        assert_eq!(text, "שלום\n  וועלט!");
        assert!(!upload.checkpoint_path.exists());
        assert!(store.get_logs(None)[0].stt_request_json.is_some());
        server.await.unwrap();
    }
}

#[test]
fn transcript_metadata_rejects_missing_fields_and_invalid_audio_ranges() {
    for value in [
        json!({}),
        json!({"metadata":{}}),
        json!({"metadata":{"start_time":0,"end_time":1}}),
        json!({"metadata":{"start_time":-1,"end_time":1,"transcript":"private"}}),
        json!({"metadata":{"start_time":2,"end_time":1,"transcript":"private"}}),
    ] {
        let error = streaming_transcript_metadata(&value)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private"));
    }
}

#[tokio::test]
async fn buffered_input_uses_the_same_owned_engine_and_preserves_formatted_rtl() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(4);
    let (partials, output) = mpsc::channel(32);
    let task = tokio::spawn(run_streaming_task(
        write, read, input, partials, 16000, None,
    ));
    let session = StreamingSttSession::new(audio, output, task);
    let pcm = [16384_i16, 0]
        .repeat(1601)
        .into_iter()
        .flat_map(i16::to_le_bytes)
        .collect::<Vec<_>>();
    let feed =
        tokio::spawn(async move { transcribe_buffered_session(session, &pcm, 16000, 2).await });
    let mut delivered = Vec::new();
    loop {
        match peer.next().await.unwrap().unwrap() {
            Message::Binary(bytes) => delivered.extend_from_slice(&bytes),
            Message::Text(text)
                if serde_json::from_str::<JsonValue>(&text).unwrap()["message"]
                    == "EndOfStream" =>
            {
                break
            }
            _ => {}
        }
    }
    assert_eq!(delivered, 8192_i16.to_le_bytes().repeat(1601));
    for (start, end, text) in [(0.0, 0.1, "שלום\n"), (0.1, 0.2, "  וועלט!")] {
        let mut event = transcript("AddTranscript", text, true);
        event["metadata"]["start_time"] = json!(start);
        event["metadata"]["end_time"] = json!(end);
        peer.send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
    }
    peer.send(Message::Text(
        json!({"message":"EndOfTranscript"}).to_string().into(),
    ))
    .await
    .unwrap();
    assert_eq!(feed.await.unwrap().unwrap(), "שלום\n  וועלט!");
}

#[tokio::test]
async fn auto_language_and_incomplete_audio_fail_before_connecting() {
    let unavailable = SpeechmaticsSttProvider::new(
        "synthetic".into(),
        Some("missing-model".into()),
        Some("he".into()),
    );
    let error = start_session(&unavailable, 16000, "not-a-url", true)
        .await
        .err()
        .unwrap();
    assert!(matches!(error, SttError::Config(_)));
    assert!(error.to_string().contains("no model will be substituted"));
    for hint in [None, Some("auto".into())] {
        let provider = SpeechmaticsSttProvider::new("synthetic".into(), None, hint);
        assert!(matches!(
            start_session(&provider, 16000, "not-a-url", true).await,
            Err(SttError::Config(_))
        ));
        assert_eq!(provider.language, "auto");
    }
    let provider = SpeechmaticsSttProvider::new("synthetic".into(), None, Some("he".into()));
    assert_eq!(provider.language, "he");
    for (pcm, rate, channels) in [(vec![0], 16000, 1), (vec![], 0, 1), (vec![], 16000, 0)] {
        assert!(matches!(
            transcribe_ws(&provider, &pcm, rate, channels).await,
            Err(SttError::Audio(_))
        ));
    }
}

fn transcript(message: &str, text: &str, eos: bool) -> JsonValue {
    json!({"message":message,"metadata":{"start_time":0.0,"end_time":0.1,"transcript":text},"results":[{"type":"word","alternatives":[{"content":text}],"is_eos":eos}]})
}

#[tokio::test]
async fn streaming_delivers_complete_audio_and_flushes_only_final_text() {
    for sentence_boundary in [false, true] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(4);
        let (partials, mut output) = mpsc::channel(32);
        let store = RequestLogStore::new();
        store.start_request("speechmatics".into(), None);
        let task = tokio::spawn(run_streaming_task(
            write,
            read,
            input,
            partials,
            16000,
            Some(store.clone()),
        ));
        audio.send(vec![0.1; 1601]).await.unwrap();
        drop(audio);
        let mut received = Vec::new();
        loop {
            match peer.next().await.unwrap().unwrap() {
                Message::Binary(bytes) => received.extend_from_slice(&bytes),
                Message::Text(text)
                    if serde_json::from_str::<JsonValue>(&text).unwrap()["message"]
                        == "EndOfStream" =>
                {
                    assert_eq!(
                        serde_json::from_str::<JsonValue>(&text).unwrap()["last_seq_no"],
                        2
                    );
                    break;
                }
                _ => {}
            }
        }
        assert_eq!(received, f32_to_pcm_s16le(&vec![0.1; 1601]));
        for event in [
            json!({"message":"RecognitionStarted"}),
            json!({"message":"AudioAdded","seq_no":2}),
            json!({"message":"future-event"}),
            transcript("AddPartialTranscript", "draft", false),
            transcript("AddTranscript", "hello", sentence_boundary),
            transcript("AddTranscript", "hello", sentence_boundary),
            transcript("AddPartialTranscript", "stale draft", false),
            json!({"message":"EndOfTranscript"}),
        ] {
            peer.send(Message::Text(event.to_string().into()))
                .await
                .unwrap();
        }
        assert_eq!(task.await.unwrap().unwrap(), "hello");
        let mut commits = Vec::new();
        while let Some(partial) = output.recv().await {
            if let Some(commit) = partial.committed_text {
                commits.push(commit);
            }
        }
        assert_eq!(commits, ["hello"]);
        let log = store.get_logs(None).remove(0);
        assert_eq!(log.raw_transcript.as_deref(), Some("hello"));
        assert_eq!(log.stt_response_json.unwrap()["chunks_sent"], 2);
    }
}

#[tokio::test]
async fn early_finalization_malformed_and_closed_streams_are_recoverable_errors() {
    for event in [
        "private-not-json".to_string(),
        json!({"message":"Error","detail":"private-content"}).to_string(),
        json!({"message":"EndOfTranscript"}).to_string(),
    ] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (_audio, input) = mpsc::channel(1);
        let (partials, _output) = mpsc::channel(1);
        let task = tokio::spawn(run_streaming_task(
            write, read, input, partials, 16000, None,
        ));
        peer.send(Message::Text(event.into())).await.unwrap();
        assert!(!task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("private"));
    }
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (_audio, input) = mpsc::channel(1);
    let (partials, _output) = mpsc::channel(1);
    peer.close(None).await.unwrap();
    assert!(
        run_streaming_task(write, read, input, partials, 16000, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn active_and_finalization_timeouts_do_not_return_a_provisional_prefix() {
    for finalized in [false, true] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(1);
        let (partials, _output) = mpsc::channel(1);
        let task = tokio::spawn(run_streaming_task(
            write, read, input, partials, 16000, None,
        ));
        if finalized {
            drop(audio);
            let message = peer.next().await.unwrap().unwrap();
            assert_eq!(
                serde_json::from_str::<JsonValue>(message.to_text().unwrap()).unwrap()["message"],
                "EndOfStream"
            );
        }
        tokio::time::pause();
        tokio::task::yield_now().await;
        tokio::time::advance(
            if finalized {
                POST_EOS_TIMEOUT
            } else {
                DEFAULT_WS_TIMEOUT
            } + Duration::from_secs(1),
        )
        .await;
        assert!(matches!(task.await.unwrap(), Err(SttError::Timeout)));
        tokio::time::resume();
    }
}
