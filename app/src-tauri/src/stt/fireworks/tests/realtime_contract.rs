use super::*;
use futures_util::{SinkExt, StreamExt};

#[tokio::test]
async fn invalid_segments_and_lost_final_delivery_fail_the_actual_session() {
    for case in ["range", "gap", "delivery"] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(1);
        drop(audio);
        let (partials, output) = mpsc::channel(1);
        let task = tokio::spawn(run_streaming_task(
            write, read, input, partials, 16000, None,
        ));
        let message = peer.next().await.unwrap().unwrap();
        assert!(message.to_text().unwrap().contains("final"));
        let id = match case {
            "range" => 16384,
            "gap" => 1,
            _ => 0,
        };
        peer.send(Message::Text(
            json!({"segments":[{"id":id,"text":"synthetic"}]})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
        peer.send(Message::Text(
            json!({"checkpoint_id":"final"}).to_string().into(),
        ))
        .await
        .unwrap();
        let error = task.await.unwrap().unwrap_err().to_string();
        assert!(error.contains(match case {
            "range" => "invalid segment",
            "gap" => "every transcript segment",
            _ => "delivery failed",
        }));
        drop(output);
    }
}

#[tokio::test]
async fn checkpoint_flushes_complete_audio_and_commits_the_remaining_text_once() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(4);
    let (partials, mut output) = mpsc::channel(32);
    let store = RequestLogStore::new();
    store.start_request("fireworks".into(), None);
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
                if serde_json::from_str::<JsonValue>(&text).unwrap()["checkpoint_id"]
                    == "final" =>
            {
                break
            }
            _ => {}
        }
    }
    assert_eq!(received, f32_to_pcm_s16le(&vec![0.1; 1601]));
    peer.send(Message::Binary(vec![0].into())).await.unwrap();
    for event in [
        json!({"future-event":true}),
        json!({"segments":[{"id":0,"text":"hello"}]}),
        json!({"checkpoint_id":"final"}),
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

#[tokio::test]
async fn early_checkpoint_malformed_and_closed_streams_do_not_report_success() {
    for event in [
        "private-not-json".to_string(),
        json!({"error":"private-content"}).to_string(),
        json!({"checkpoint_id":"final"}).to_string(),
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
async fn active_and_checkpoint_timeouts_fail_instead_of_accepting_a_prefix() {
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
                serde_json::from_str::<JsonValue>(message.to_text().unwrap()).unwrap()
                    ["checkpoint_id"],
                "final"
            );
        }
        tokio::time::pause();
        tokio::task::yield_now().await;
        tokio::time::advance(
            if finalized {
                POST_CHECKPOINT_TIMEOUT
            } else {
                DEFAULT_WS_TIMEOUT
            } + Duration::from_secs(1),
        )
        .await;
        assert!(matches!(task.await.unwrap(), Err(SttError::Timeout)));
        tokio::time::resume();
    }
}
