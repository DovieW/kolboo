//! Hard budgets before signature verification. Stage on private disk, not in
//! the desktop heap, and reverify those bytes immediately before installation.
use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
use std::io::{Read, Seek, SeekFrom};
use tauri_plugin_updater::{Error, Result, Update};
use tokio::io::AsyncWriteExt;

const MAX_BYTES: u64 = 256 * 1024 * 1024;
fn failure() -> Error {
    Error::Network("Update exceeded its resource budget, could not be staged, or failed signature verification".into())
}
fn failure_from<T>(_: T) -> Error {
    failure()
}
fn public_key(value: &str) -> Result<PublicKey> {
    PublicKey::decode(
        std::str::from_utf8(&STANDARD.decode(value).map_err(failure_from)?)
            .map_err(failure_from)?,
    )
    .map_err(failure_from)
}
fn signature(value: &str) -> Result<Signature> {
    Signature::decode(
        std::str::from_utf8(&STANDARD.decode(value).map_err(failure_from)?)
            .map_err(failure_from)?,
    )
    .map_err(failure_from)
}

pub(super) struct VerifiedArchive {
    file: tempfile::NamedTempFile,
    key: String,
    signature: String,
}
impl VerifiedArchive {
    #[cfg(test)]
    pub(super) fn tamper_for_test(&self) {
        std::fs::write(self.file.path(), b"synthetic tampering").unwrap();
    }
    pub(super) fn read_verified(&self) -> Result<Vec<u8>> {
        let bytes = read_bounded(self.file.as_file(), MAX_BYTES)?;
        public_key(&self.key)?
            .verify(&bytes, &signature(&self.signature)?, true)
            .map_err(failure_from)?;
        Ok(bytes)
    }
}

fn read_bounded(file: &std::fs::File, limit: u64) -> Result<Vec<u8>> {
    let mut input = file.try_clone().map_err(failure_from)?;
    input.seek(SeekFrom::Start(0)).map_err(failure_from)?;
    let length = input.metadata().map_err(failure_from)?.len();
    read_snapshot(input, length, limit)
}

fn read_snapshot(mut input: impl Read, length: u64, limit: u64) -> Result<Vec<u8>> {
    if length > limit {
        return Err(failure());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length as usize)
        .map_err(failure_from)?;
    bytes.resize(length as usize, 0);
    input.read_exact(&mut bytes).map_err(failure_from)?;
    // Never grow the allocation if a staged file changes underneath us.
    if input.read(&mut [0; 1]).map_err(failure_from)? != 0 {
        return Err(failure());
    }
    Ok(bytes)
}

pub(super) async fn download(update: &Update, key: &str) -> Result<VerifiedArchive> {
    download_with_limit(update, key, MAX_BYTES).await
}
async fn download_with_limit(update: &Update, key: &str, limit: u64) -> Result<VerifiedArchive> {
    let public = public_key(key)?;
    let signed = signature(&update.signature)?;
    let mut verifier = match public.verify_stream(&signed) {
        Ok(verifier) => Some(verifier),
        Err(minisign_verify::Error::UnsupportedLegacyMode) => None,
        Err(_) => return Err(failure()),
    };
    let mut client = reqwest::Client::builder()
        .user_agent("Kolboo-Updater")
        .timeout(super::NETWORK_TIMEOUT);
    if update.no_proxy {
        client = client.no_proxy();
    } else if let Some(proxy) = &update.proxy {
        client = client.proxy(reqwest::Proxy::all(proxy.as_str()).map_err(failure_from)?);
    }
    let mut response = client
        .build()
        .map_err(failure_from)?
        .get(update.download_url.clone())
        .headers(update.headers.clone())
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(failure_from)?
        .error_for_status()
        .map_err(failure_from)?;
    if response.content_length().is_some_and(|size| size > limit) {
        return Err(failure());
    }
    let file = tempfile::NamedTempFile::new().map_err(failure_from)?;
    let mut output = tokio::fs::File::from_std(file.as_file().try_clone().map_err(failure_from)?);
    let mut total = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(failure_from)? {
        total = total
            .checked_add(chunk.len() as u64)
            .filter(|size| *size <= limit)
            .ok_or_else(failure)?;
        if let Some(verifier) = verifier.as_mut() {
            verifier.update(&chunk);
        }
        output.write_all(&chunk).await.map_err(failure_from)?;
    }
    output.flush().await.map_err(failure_from)?;
    drop(output);
    let archive = VerifiedArchive {
        file,
        key: key.into(),
        signature: update.signature.clone(),
    };
    if let Some(mut verifier) = verifier {
        verifier.finalize().map_err(failure_from)?;
    } else {
        archive.read_verified()?;
    }
    Ok(archive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automatic_updates::tests::{release, test_app, PAYLOAD, PUBLIC_KEY};
    #[tokio::test]
    async fn budget_and_signature_are_enforced_before_publication_and_again_at_install() {
        let server = wiremock::MockServer::start().await;
        let app = test_app(&server);
        release(&server, "99.0.0", PAYLOAD).await;
        use tauri_plugin_updater::UpdaterExt;
        let update = app
            .handle()
            .updater()
            .unwrap()
            .check()
            .await
            .unwrap()
            .unwrap();
        assert!(download_with_limit(&update, PUBLIC_KEY, 1).await.is_err());
        assert!(download(&update, "PRIVATE").await.is_err());
        let archive = download(&update, PUBLIC_KEY).await.unwrap();
        assert_eq!(archive.read_verified().unwrap(), PAYLOAD);
        std::fs::write(archive.file.path(), b"tampered").unwrap();
        assert!(archive.read_verified().is_err());
        assert!(public_key("/w==").is_err());
        assert!(signature("/w==").is_err());
        assert!(public_key(&STANDARD.encode(b"invalid")).is_err());
        assert!(signature(&STANDARD.encode(b"invalid")).is_err());
        assert!(signature("PRIVATE").is_err());
        assert!(read_snapshot(std::io::empty(), usize::MAX as u64, u64::MAX).is_err());
        assert!(
            read_snapshot(std::io::Cursor::new(b"ab"), 1, 16).is_err(),
            "a growing staged file must not extend the allocation"
        );
        assert!(
            read_snapshot(std::io::Cursor::new(b"a"), 2, 16).is_err(),
            "a truncated staged file is not complete"
        );
        assert!(read_bounded(archive.file.as_file(), 1).is_err());
        #[cfg(unix)]
        assert!(read_bounded(
            &std::fs::File::open(tempfile::tempdir().unwrap().path()).unwrap(),
            16
        )
        .is_err());
        let mut proxied = update.clone();
        proxied.no_proxy = false;
        proxied.proxy = Some(server.uri().parse().unwrap());
        assert_eq!(
            download(&proxied, PUBLIC_KEY)
                .await
                .unwrap()
                .read_verified()
                .unwrap(),
            PAYLOAD
        );
        proxied.no_proxy = true;
        assert!(download(&proxied, PUBLIC_KEY).await.is_ok());
        let mut wrong_key = String::from_utf8(STANDARD.decode(PUBLIC_KEY).unwrap())
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut bytes = STANDARD.decode(&wrong_key[1]).unwrap();
        bytes[2] ^= 1;
        wrong_key[1] = STANDARD.encode(bytes);
        assert!(download(&update, &STANDARD.encode(wrong_key.join("\n")))
            .await
            .is_err());
        let mut legacy = update.clone();
        let mut text = String::from_utf8(STANDARD.decode(&legacy.signature).unwrap())
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut bytes = STANDARD.decode(&text[1]).unwrap();
        bytes[1] = b'd';
        text[1] = STANDARD.encode(bytes);
        legacy.signature = STANDARD.encode(text.join("\n"));
        assert!(
            download(&legacy, PUBLIC_KEY).await.is_err(),
            "changing the signature algorithm must never bypass verification"
        );
        server.reset().await;
        assert!(download(&update, PUBLIC_KEY).await.is_err());
    }

    #[tokio::test]
    async fn chunked_responses_cannot_bypass_the_download_budget() {
        use std::io::{BufRead, Write};
        let server = wiremock::MockServer::start().await;
        let app = test_app(&server);
        release(&server, "99.0.0", PAYLOAD).await;
        use tauri_plugin_updater::UpdaterExt;
        let mut update = app
            .handle()
            .updater()
            .unwrap()
            .check()
            .await
            .unwrap()
            .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        update.download_url = format!("http://{}/installer", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut reader = std::io::BufReader::new(&stream);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
            }
            write!(stream, "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n", PAYLOAD.len()).unwrap();
            let _ = stream.write_all(PAYLOAD);
            let _ = stream.write_all(b"\r\n0\r\n\r\n");
        });
        assert!(download_with_limit(&update, PUBLIC_KEY, 4).await.is_err());
        peer.join().unwrap();
    }
}
