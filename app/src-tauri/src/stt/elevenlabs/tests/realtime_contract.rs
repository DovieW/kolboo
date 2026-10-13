use super::*;
use futures_util::{SinkExt, StreamExt};

#[tokio::test]
async fn exhausted_websocket_cannot_complete_an_active_recording() {
    let (write, mut read, mut peer) = crate::stt::streaming_test_support::peers().await;
    peer.close(None).await.unwrap();
    assert!(matches!(
        read.next().await.unwrap().unwrap(),
        Message::Close(_)
    ));
    drop(peer);
    let (_audio, input) = mpsc::channel(1);
    let (partials, _output) = mpsc::channel(1);
    assert!(
        run_streaming_task(write, read, input, partials, 16000, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn complete_adapter_delivers_buffered_audio_with_the_manual_commit_contract() {
    use crate::stt::{AudioEncoding, SttProvider};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut peer = tokio_tungstenite::accept_hdr_async(
            socket,
            |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                assert_eq!(request.headers()["xi-api-key"], "synthetic-key");
                assert!(request
                    .uri()
                    .query()
                    .unwrap()
                    .contains("commit_strategy=manual"));
                Ok(response)
            },
        )
        .await
        .unwrap();
        let mut delivered = Vec::new();
        loop {
            let value: JsonValue =
                serde_json::from_str(peer.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            delivered.extend(
                STANDARD
                    .decode(value["audio_base_64"].as_str().unwrap())
                    .unwrap(),
            );
            if value["commit"] == true {
                break;
            }
        }
        assert_eq!(delivered, 8192_i16.to_le_bytes().repeat(1601));
        peer.send(Message::Text(
            json!({"message_type":"committed_transcript","text":"שלום"})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    });
    let store = RequestLogStore::new();
    store.start_request("elevenlabs".into(), None);
    let provider = ElevenLabsSttProvider::new("synthetic-key".into(), None, None)
        .with_api_base_url(endpoint)
        .with_request_log_store(Some(store.clone()));
    assert!(provider
        .with_renewed_managed_token("must-not-promote-byok")
        .is_none());
    let directory = tempfile::tempdir().unwrap();
    let upload = crate::stt::SttUploadContext {
        purpose: crate::stt::UploadPurpose::File,
        chunk_offset: 0,
        checkpoint_path: directory.path().join("unused-checkpoint"),
    };
    let text = provider
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
        .unwrap();
    assert_eq!(text.text, "שלום");
    assert!(!upload.checkpoint_path.exists());
    assert!(store.get_logs(None)[0].stt_request_json.is_some());
    server.await.unwrap();
}

#[tokio::test]
async fn stop_after_the_full_audio_chunk_uses_a_documented_empty_commit() {
    for final_text in ["final", ""] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(1);
        let (partials, _output) = mpsc::channel(4);
        let task = tokio::spawn(run_streaming_task(
            write, read, input, partials, 16000, None,
        ));
        audio.send(vec![0.1; 1600]).await.unwrap();
        let message: JsonValue =
            serde_json::from_str(peer.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(message["commit"], false);
        assert_eq!(
            STANDARD
                .decode(message["audio_base_64"].as_str().unwrap())
                .unwrap()
                .len(),
            3200
        );
        drop(audio);
        let commit: JsonValue =
            serde_json::from_str(peer.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(commit["message_type"], "input_audio_chunk");
        assert_eq!(commit["commit"], true);
        assert_eq!(commit["audio_base_64"], "");
        peer.send(Message::Text(
            json!({"message_type":"committed_transcript","text":final_text})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
        assert_eq!(task.await.unwrap().unwrap(), final_text);
    }
}

#[tokio::test]
async fn long_recordings_wait_for_every_owned_commit_and_preserve_repeated_words() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(1);
    let (partials, mut output) = mpsc::channel(8);
    let task = tokio::spawn(run_streaming_task(
        write, read, input, partials, 16000, None,
    ));
    let samples = vec![0.1; 16000 * 20 + 1];
    audio.send(samples.clone()).await.unwrap();
    drop(audio);
    let mut received = Vec::new();
    for window in 0..2 {
        loop {
            let message = peer.next().await.unwrap().unwrap();
            let value: JsonValue = serde_json::from_str(message.to_text().unwrap()).unwrap();
            assert_eq!(value["message_type"], "input_audio_chunk");
            received.extend(
                STANDARD
                    .decode(value["audio_base_64"].as_str().unwrap())
                    .unwrap(),
            );
            if value["commit"] == true {
                break;
            }
        }
        assert!(!task.is_finished());
        assert_eq!(
            received.len(),
            if window == 0 {
                16000 * 20 * 2
            } else {
                samples.len() * 2
            }
        );
        peer.send(Message::Text(
            json!({"message_type":"committed_transcript","text":"repeat"})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    }
    assert_eq!(task.await.unwrap().unwrap(), "repeat repeat");
    assert_eq!(received, f32_to_pcm_s16le(&samples));
    let mut commits = Vec::new();
    while let Some(partial) = output.recv().await {
        if let Some(text) = partial.committed_text {
            commits.push(text);
        }
    }
    assert_eq!(commits, ["repeat", "repeat"]);
}

#[tokio::test]
async fn losing_the_last_ack_never_turns_an_intermediate_commit_into_success() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(1);
    let (partials, _output) = mpsc::channel(8);
    let task = tokio::spawn(run_streaming_task(
        write, read, input, partials, 16000, None,
    ));
    audio.send(vec![0.1; 16000 * 20 + 1]).await.unwrap();
    drop(audio);
    for window in 0..2 {
        loop {
            let message = peer.next().await.unwrap().unwrap();
            if serde_json::from_str::<JsonValue>(message.to_text().unwrap()).unwrap()["commit"]
                == true
            {
                break;
            }
        }
        if window == 0 {
            peer.send(Message::Text(
                json!({"message_type":"committed_transcript","text":"prefix"})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        } else {
            peer.close(None).await.unwrap();
        }
    }
    assert!(task.await.unwrap().is_err());
}

#[tokio::test]
async fn empty_input_and_invalid_rates_do_not_wait_for_a_nonexistent_commit() {
    for rate in [0, 16000] {
        let (write, read, _peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(1);
        drop(audio);
        let (partials, _output) = mpsc::channel(1);
        let result = run_streaming_task(write, read, input, partials, rate, None).await;
        if rate == 0 {
            assert!(matches!(result, Err(SttError::Config(_))));
        } else {
            assert_eq!(result.unwrap(), "");
        }
    }
}

#[tokio::test]
async fn manual_stream_delivers_full_audio_tail_and_ignores_timestamp_supplements() {
    for samples in [1600, 1601] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(4);
        let (partials, mut output) = mpsc::channel(32);
        let store = RequestLogStore::new();
        store.start_request("elevenlabs".into(), None);
        let task = tokio::spawn(run_streaming_task(
            write,
            read,
            input,
            partials,
            16000,
            Some(store.clone()),
        ));
        audio.send(vec![0.1; samples]).await.unwrap();
        drop(audio);
        let mut received = Vec::new();
        loop {
            let message = peer.next().await.unwrap().unwrap();
            let value: JsonValue = serde_json::from_str(message.to_text().unwrap()).unwrap();
            if let Some(encoded) = value["audio_base_64"].as_str() {
                received.extend(STANDARD.decode(encoded).unwrap());
            }
            if value["commit"] == true {
                break;
            }
        }
        assert_eq!(received, f32_to_pcm_s16le(&vec![0.1; samples]));
        peer.send(Message::Binary(vec![0].into())).await.unwrap();
        for event in [
            json!({"message_type":"session_started"}),
            json!({"message_type":"partial_transcript","text":" "}),
            json!({"message_type":"partial_transcript","text":"draft"}),
            json!({"message_type":"committed_transcript_with_timestamps","text":"hello"}),
            json!({"message_type":"committed_transcript","text":"hello"}),
        ] {
            peer.send(Message::Text(event.to_string().into()))
                .await
                .unwrap();
        }
        assert_eq!(task.await.unwrap().unwrap(), "hello");
        let mut previews = Vec::new();
        while let Some(partial) = output.recv().await {
            previews.push(partial);
        }
        assert_eq!(previews.len(), 2);
        assert_eq!(previews[0].text, "draft");
        assert_eq!(previews[0].committed_text, None);
        assert_eq!(previews[1].text, "hello");
        assert_eq!(previews[1].committed_text.as_deref(), Some("hello"));
        let log = store.get_logs(None).remove(0);
        assert_eq!(log.raw_transcript.as_deref(), Some("hello"));
        assert_eq!(
            log.stt_response_json.unwrap()["chunks_sent"],
            if samples == 1600 { 1 } else { 2 }
        );
    }
}

#[tokio::test]
async fn early_manual_commit_malformed_and_closed_streams_are_errors_without_content() {
    for event in [
        "private-not-json".to_string(),
        json!({"message_type":"error","message":"private-content"}).to_string(),
        json!({"message_type":"committed_transcript","text":"private-content"}).to_string(),
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
async fn manual_finalization_has_a_bounded_wait() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(1);
    let (partials, _output) = mpsc::channel(1);
    let task = tokio::spawn(run_streaming_task(
        write, read, input, partials, 16000, None,
    ));
    audio.send(vec![0.1; 1]).await.unwrap();
    drop(audio);
    let message = peer.next().await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<JsonValue>(message.to_text().unwrap()).unwrap()["commit"],
        true
    );
    tokio::time::pause();
    tokio::task::yield_now().await;
    tokio::time::advance(COMMIT_TIMEOUT + Duration::from_secs(1)).await;
    assert!(matches!(task.await.unwrap(), Err(SttError::Timeout)));
    tokio::time::resume();
}
