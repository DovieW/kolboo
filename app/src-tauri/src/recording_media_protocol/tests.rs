use super::*;

#[test]
fn failed_shutdown_wakeup_does_not_join_a_blocked_listener() {
    let temp = tempfile::tempdir().unwrap();
    let mut server = RecordingMediaServer::start(temp.path().to_owned()).unwrap();
    let real_address = server.address;
    // Port zero cannot accept a connection. Exercise an OS wakeup failure
    // without consuming file descriptors or changing the user's network.
    server.address.set_port(0);
    drop(server);
    // Clean up the detached worker if it had already entered accept(). It
    // must close this connection, not serve another playback request.
    match TcpStream::connect(real_address) {
        Ok(mut connection) => {
            connection
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            // Closing an unread socket can produce EOF or RST, depending
            // on whether accept() won the shutdown race. Neither may serve data.
            match connection.read(&mut [0]) {
                Ok(length) => assert_eq!(length, 0),
                Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset),
            }
        }
        Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::ConnectionRefused),
    }
}

#[test]
fn rejects_oversized_request_lines_before_buffering_the_entire_input() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (stream, _) = listener.accept().unwrap();
    client
        .write_all(&vec![b'x'; MAX_HEADER_BYTES + 50])
        .unwrap();
    let mut line = String::new();
    assert!(read_bounded_line(&mut BufReader::new(&stream), &mut line).is_err());
    assert_eq!(line.len(), MAX_HEADER_BYTES + 1);
}

#[test]
fn rejects_non_utf8_and_closed_request_lines() {
    for input in [b"\xff\n".as_slice(), b""] {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();
        client.write_all(input).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        assert!(read_bounded_line(&mut BufReader::new(&stream), &mut String::new()).is_err());
    }
}

fn request(
    server: &RecordingMediaServer,
    method: &str,
    path: &str,
    range: Option<&str>,
) -> Vec<u8> {
    let mut stream = TcpStream::connect(server.address).unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {}\r\n",
        server.address
    )
    .unwrap();
    if let Some(range) = range {
        write!(stream, "Range: {range}\r\n").unwrap();
    }
    stream.write_all(b"Connection: close\r\n\r\n").unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    response
}

fn headers(response: &[u8]) -> String {
    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("response has a header terminator");
    String::from_utf8(response[..boundary].to_vec()).unwrap()
}

#[test]
fn serves_ranges_without_exposing_other_files() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = (0_u8..=31).collect::<Vec<_>>();
    std::fs::write(temp.path().join("recording-1.wav"), &bytes).unwrap();
    std::fs::write(temp.path().join("settings.json"), b"secret").unwrap();
    let server = RecordingMediaServer::start(temp.path().to_owned()).unwrap();

    let path = format!("/{}/recording-1", server.token);
    let response = request(&server, "GET", &path, Some("bytes=4-9"));
    assert!(response.starts_with(b"HTTP/1.1 206 Partial Content\r\n"));
    assert!(headers(&response).contains("Content-Range: bytes 4-9/32"));
    assert_eq!(
        response.split(|byte| *byte == b'\n').next_back().unwrap(),
        &bytes[4..=9]
    );

    let invalid = request(
        &server,
        "GET",
        &format!("/{}/../settings.json", server.token),
        None,
    );
    assert!(invalid.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

    let wrong_token = request(&server, "GET", "/wrong/recording-1", None);
    assert!(wrong_token.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
}

#[test]
fn supports_head_and_rejects_invalid_ranges() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("recording-1.wav"), vec![7_u8; 20]).unwrap();
    let server = RecordingMediaServer::start(temp.path().to_owned()).unwrap();
    let path = format!("/{}/recording-1", server.token);

    let head = request(&server, "HEAD", &path, None);
    assert!(head.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert!(head.ends_with(b"\r\n\r\n"));

    let invalid = request(&server, "GET", &path, Some("bytes=30-40"));
    assert!(invalid.starts_with(b"HTTP/1.1 416 Range Not Satisfiable\r\n"));
    assert!(headers(&invalid).contains("Content-Range: bytes */20"));

    assert_eq!(parse_range("bytes=10-4", 20), None);
    assert_eq!(parse_range("bytes=0-1,4-5", 20), None);
    assert_eq!(parse_range("bytes=-4", 20), Some((16, 19)));
    assert_eq!(parse_range("bytes=18-", 20), Some((18, 19)));
}
