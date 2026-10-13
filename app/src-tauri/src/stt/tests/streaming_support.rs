use crate::stt::streaming::{WsRead, WsWrite};
use futures_util::StreamExt;

pub(crate) async fn peers() -> (
    WsWrite,
    WsRead,
    tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (client, server) = tokio::join!(
        tokio_tungstenite::connect_async(format!("ws://{address}")),
        async {
            tokio_tungstenite::accept_async(listener.accept().await.unwrap().0)
                .await
                .unwrap()
        }
    );
    let (write, read) = client.unwrap().0.split();
    (write, read, server)
}
