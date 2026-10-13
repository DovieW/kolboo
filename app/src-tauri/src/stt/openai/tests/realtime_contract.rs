use super::*;
use futures_util::{SinkExt, StreamExt};

fn configuration() -> StreamingTaskConfig {
    StreamingTaskConfig {
        capture_sample_rate: 24000,
        model: "gpt-live-transcribe".into(),
        language: None,
        prompt: None,
        request_log_store: None,
    }
}

#[tokio::test]
async fn final_short_audio_tail_and_results_before_acknowledgement_are_not_lost() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(1);
    let (partials, mut preview) = mpsc::channel(8);
    let task = tokio::spawn(run_streaming_task(
        write,
        read,
        input,
        partials,
        configuration(),
    ));
    peer.next().await.unwrap().unwrap();
    audio.send(vec![0.1; 1]).await.unwrap();
    drop(audio);
    let append: JsonValue =
        serde_json::from_str(peer.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(append["type"], "input_audio_buffer.append");
    assert_eq!(
        STANDARD.decode(append["audio"].as_str().unwrap()).unwrap(),
        f32_to_pcm_s16le(&[0.1])
    );
    let commit: JsonValue =
        serde_json::from_str(peer.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(commit["type"], "input_audio_buffer.commit");
    for event in [
        json!({"type":"future.event","private_content":"not logged"}),
        json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"tail","transcript":"שלום"}),
        json!({"type":"input_audio_buffer.committed","item_id":"tail","previous_item_id":null}),
    ] {
        peer.send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
    }
    assert_eq!(task.await.unwrap().unwrap(), "שלום");
    let partial = preview.recv().await.unwrap();
    assert_eq!(partial.text, "שלום");
    assert_eq!(partial.committed_text.as_deref(), Some("שלום"));
}

#[tokio::test]
async fn malformed_transcript_identity_and_vad_metadata_fail_without_returning_private_errors() {
    for event in [
        json!({"type":"input_audio_buffer.speech_stopped","item_id":"private-id"}),
        json!({"type":"input_audio_buffer.speech_stopped","item_id":"","audio_end_ms":5}),
        json!({"type":"conversation.item.input_audio_transcription.completed","transcript":"private text"}),
    ] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (_audio, input) = mpsc::channel(1);
        let (partials, _preview) = mpsc::channel(8);
        let task = tokio::spawn(run_streaming_task(
            write,
            read,
            input,
            partials,
            configuration(),
        ));
        peer.next().await.unwrap().unwrap();
        peer.send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
        let error = task.await.unwrap().unwrap_err();
        assert!(matches!(error, SttError::Api(_)));
        assert!(!error.to_string().contains("private"));
    }
}

#[tokio::test]
async fn missing_final_commit_receipt_times_out_instead_of_returning_a_prefix() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(1);
    let (partials, _preview) = mpsc::channel(8);
    let task = tokio::spawn(run_streaming_task(
        write,
        read,
        input,
        partials,
        configuration(),
    ));
    peer.next().await.unwrap().unwrap();
    audio.send(vec![0.1; 1]).await.unwrap();
    drop(audio);
    peer.next().await.unwrap().unwrap();
    peer.next().await.unwrap().unwrap();
    tokio::time::pause();
    tokio::time::advance(POST_COMMIT_TIMEOUT + Duration::from_secs(1)).await;
    assert!(matches!(task.await.unwrap(), Err(SttError::Timeout)));
}

#[tokio::test]
async fn delayed_vad_commit_cannot_finish_before_its_result_or_a_later_manual_tail() {
    for vad_end in [100_u64, 200] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(4);
        let (partials, mut preview) = mpsc::channel(32);
        let task = tokio::spawn(run_streaming_task(
            write,
            read,
            input,
            partials,
            StreamingTaskConfig {
                capture_sample_rate: 24000,
                model: "gpt-4o-transcribe-realtime".into(),
                language: None,
                prompt: None,
                request_log_store: None,
            },
        ));
        assert!(peer
            .next()
            .await
            .unwrap()
            .unwrap()
            .to_text()
            .unwrap()
            .contains("transcription_session.update"));
        audio.send(vec![0.1; 4800]).await.unwrap();
        for _ in 0..4 {
            let value: JsonValue =
                serde_json::from_str(peer.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            assert_eq!(value["type"], "input_audio_buffer.append");
        }
        for event in [
            json!({"type":"input_audio_buffer.committed","item_id":"first","previous_item_id":null}),
            json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"first","transcript":"prefix"}),
        ] {
            peer.send(Message::Text(event.to_string().into()))
                .await
                .unwrap();
        }
        assert_eq!(
            preview.recv().await.unwrap().committed_text.as_deref(),
            Some("prefix")
        );
        for event in [
            json!({"type":"input_audio_buffer.speech_stopped","item_id":"vad","audio_end_ms":vad_end}),
            json!({"type":"conversation.item.input_audio_transcription.delta","item_id":"vad","event_id":"delta-1","delta":"draft"}),
        ] {
            peer.send(Message::Text(event.to_string().into()))
                .await
                .unwrap();
        }
        assert!(preview.recv().await.unwrap().text.contains("draft"));
        drop(audio);
        if vad_end == 100 {
            let value: JsonValue =
                serde_json::from_str(peer.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            assert_eq!(value["type"], "input_audio_buffer.commit");
        }
        for event in [
            json!({"type":"input_audio_buffer.committed","item_id":"vad","previous_item_id":"first"}),
            json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"vad","transcript":"middle"}),
        ] {
            peer.send(Message::Text(event.to_string().into()))
                .await
                .unwrap();
        }
        assert_eq!(
            preview.recv().await.unwrap().committed_text.as_deref(),
            Some("middle")
        );
        if vad_end == 100 {
            // An echoed VAD acknowledgement is not the manual tail's receipt.
            for event in [
                json!({"type":"input_audio_buffer.committed","item_id":"vad","previous_item_id":"first"}),
                json!({"type":"input_audio_buffer.committed","item_id":"tail","previous_item_id":"vad"}),
                json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"tail","transcript":"tail"}),
            ] {
                peer.send(Message::Text(event.to_string().into()))
                    .await
                    .unwrap();
            }
        }
        assert_eq!(
            task.await.unwrap().unwrap(),
            if vad_end == 100 {
                "prefix middle tail"
            } else {
                "prefix middle"
            }
        );
    }
}
