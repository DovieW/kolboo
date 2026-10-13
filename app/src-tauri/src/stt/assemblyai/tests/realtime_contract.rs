use super::*;
use futures_util::{SinkExt, StreamExt};

#[tokio::test]
async fn turn_delivery_requires_complete_audio_and_a_termination_ack() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(4);
    let (partials, mut output) = mpsc::channel(32);
    let store = RequestLogStore::new();
    store.start_request("assemblyai".into(), None);
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
                if serde_json::from_str::<JsonValue>(&text).unwrap()["type"] == "Terminate" =>
            {
                break
            }
            _ => {}
        }
    }
    assert_eq!(received, f32_to_pcm_s16le(&vec![0.1; 1601]));
    for event in [
        json!({"type":"Begin","id":"synthetic"}),
        json!({"type":"future-event"}),
        json!({"type":"Turn","turn_order":0,"transcript":"draft","turn_is_formatted":false}),
        json!({"type":"Turn","turn_order":0,"transcript":"hello","end_of_turn":true,"turn_is_formatted":true}),
        json!({"type":"Turn","turn_order":0,"transcript":"duplicate","end_of_turn":true,"turn_is_formatted":true}),
        json!({"type":"Turn","turn_order":2,"transcript":"third","end_of_turn":true,"turn_is_formatted":true}),
        json!({"type":"Turn","turn_order":1,"transcript":"second","end_of_turn":true,"turn_is_formatted":true}),
    ] {
        peer.send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
    }
    peer.send(Message::Binary(vec![0].into())).await.unwrap();
    peer.send(Message::Text(
        json!({"type":"Termination"}).to_string().into(),
    ))
    .await
    .unwrap();
    assert_eq!(task.await.unwrap().unwrap(), "hello second third");
    let mut committed = Vec::new();
    while let Some(partial) = output.recv().await {
        if let Some(text) = partial.committed_text {
            committed.push(text);
        }
    }
    assert_eq!(committed, ["hello", "second third"]);
    assert_eq!(
        store.get_logs(None)[0].raw_transcript.as_deref(),
        Some("hello second third")
    );
}

#[tokio::test]
async fn invalid_turns_and_premature_termination_cannot_report_success() {
    for event in [
        json!({"type":"Turn","transcript":"private"}),
        json!({"type":"Termination"}),
    ] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (_audio, input) = mpsc::channel(1);
        let (partials, _output) = mpsc::channel(1);
        let task = tokio::spawn(run_streaming_task(
            write, read, input, partials, 16000, None,
        ));
        peer.send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
        assert!(!task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("private"));
    }
}

#[tokio::test]
async fn timeout_in_either_phase_is_a_failed_stream() {
    for finalized in [false, true] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(1);
        let (partials, _output) = mpsc::channel(1);
        let task = tokio::spawn(run_streaming_task(
            write, read, input, partials, 16000, None,
        ));
        if finalized {
            drop(audio);
            assert!(matches!(
                peer.next().await.unwrap().unwrap(),
                Message::Text(_)
            ));
        }
        tokio::time::pause();
        tokio::task::yield_now().await;
        tokio::time::advance(
            if finalized {
                AssemblyAiSttProvider::POST_TERMINATE_TIMEOUT
            } else {
                AssemblyAiSttProvider::DEFAULT_WS_TIMEOUT
            } + Duration::from_secs(1),
        )
        .await;
        assert!(matches!(task.await.unwrap(), Err(SttError::Timeout)));
        tokio::time::resume();
    }
}
