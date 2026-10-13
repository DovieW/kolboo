//! A durable receipt is written before any paid job submission. Dropping this
//! future cancels local work only; it never claims to cancel provider processing.
use super::*;
use crate::stt::UploadPurpose;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Clone, Serialize, Deserialize)]
struct Receipt {
    fingerprint: String,
    nonce: uuid::Uuid,
    dispatched: bool,
    job_id: Option<String>,
}

fn storage_error() -> SttError {
    SttError::Config("The protected upload checkpoint could not be verified or saved. No new job will be submitted; your recording is retained.".into())
}
// All filesystem, lock and receipt-decoding failures share the same private
// recovery contract. Paths and serialized receipt contents never reach logs/UI.
fn storage_failure<T>(_error: T) -> SttError {
    storage_error()
}

#[cfg(unix)]
fn sync_directory(path: &std::path::Path) -> Result<(), SttError> {
    use std::os::unix::fs::OpenOptionsExt;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(storage_failure)?;
    directory.sync_all().map_err(storage_failure)
}

fn validate_private_entry(metadata: &std::fs::Metadata, directory: bool) -> Result<(), SttError> {
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(storage_error());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(storage_error());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Reject all reparse points, including directory junctions, not only
        // symbolic links. Native Windows verification remains required.
        if metadata.file_attributes() & 0x00000400 != 0 {
            return Err(storage_error());
        }
    }
    Ok(())
}
fn pending_error() -> SttError {
    SttError::Api("YiddishLabs submission is uncertain. Processing or charges may have started. The recording and saved job key are retained; do not submit it as a new upload.".into())
}

struct Checkpoint {
    file: File,
    receipt: Receipt,
}

/// One private directory per recording, with stable per-source/chunk filenames.
/// No key, bearer, account identity, audio or text enters these names.
pub(crate) fn checkpoint_path(
    base: &std::path::Path,
    managed: bool,
    audio: &[u8],
    chunk_offset: u64,
) -> Result<std::path::PathBuf, SttError> {
    let directory = base.with_extension("yiddish-jobs");
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(storage_error()),
    }
    let metadata = std::fs::symlink_metadata(&directory).map_err(storage_failure)?;
    validate_private_entry(&metadata, true)?;
    #[cfg(unix)]
    sync_directory(directory.parent().ok_or_else(storage_error)?)?;
    let mut identity = Sha256::new();
    identity.update(chunk_offset.to_le_bytes());
    identity.update(audio);
    Ok(directory.join(format!(
        "{}.{:x}.job",
        if managed { "managed" } else { "byok" },
        identity.finalize()
    )))
}

/// Deletion is local only, and refuses active/foreign files before removing any
/// receipt. Keep these locks alive until the caller also removes owned audio.
pub(crate) fn delete_owned_checkpoints(base: &std::path::Path) -> Result<Vec<File>, SttError> {
    let directory = base.with_extension("yiddish-jobs");
    // Windows may report a child of a regular file as NotFound rather than
    // NotADirectory. Verify the nearest existing parent before accepting an
    // absent receipt directory. Missing directory trees remain an idempotent
    // no-op; files and links are never treated as valid storage parents.
    for parent in directory.ancestors().skip(1) {
        match std::fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => return Err(storage_error()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(storage_error()),
        }
    }
    let metadata = match std::fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(storage_error()),
    };
    validate_private_entry(&metadata, true)?;
    let mut owned = Vec::new();
    for entry in std::fs::read_dir(&directory).map_err(storage_failure)? {
        let entry = entry.map_err(storage_failure)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(storage_error)?;
        let hash = name
            .strip_prefix("managed.")
            .or_else(|| name.strip_prefix("byok."))
            .and_then(|name| name.strip_suffix(".job"))
            .ok_or_else(storage_error)?;
        if hash.len() != 64
            || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !entry.file_type().map_err(storage_failure)?.is_file()
        {
            return Err(storage_error());
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000);
        }
        let file = options.open(entry.path()).map_err(storage_failure)?;
        validate_private_entry(&file.metadata().map_err(storage_failure)?, false)?;
        file.try_lock().map_err(storage_failure)?;
        owned.push((entry.path(), file));
    }
    // All names and locks have been verified. Never recursively delete a path.
    for (path, _) in &owned {
        std::fs::remove_file(path).map_err(storage_failure)?;
    }
    std::fs::remove_dir(&directory).map_err(storage_failure)?;
    Ok(owned.into_iter().map(|(_, file)| file).collect())
}
impl Checkpoint {
    fn open(path: &std::path::Path, fingerprint: String) -> Result<Self, SttError> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000); // Open the reparse point itself, never its target.
        }
        let mut file = options.open(path).map_err(storage_failure)?;
        file.try_lock().map_err(storage_failure)?;
        let metadata = file.metadata().map_err(storage_failure)?;
        validate_private_entry(&metadata, false)?;
        if metadata.len() > 65536 {
            return Err(storage_error());
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(storage_failure)?;
        let receipt = if bytes.is_empty() {
            Receipt {
                fingerprint: fingerprint.clone(),
                nonce: uuid::Uuid::new_v4(),
                dispatched: false,
                job_id: None,
            }
        } else {
            let end = bytes
                .iter()
                .rposition(|byte| *byte == b'\n')
                .ok_or_else(storage_error)?;
            let last = bytes[..end]
                .rsplit(|byte| *byte == b'\n')
                .next()
                .ok_or_else(storage_error)?;
            let receipt: Receipt = serde_json::from_slice(last).map_err(storage_failure)?;
            if receipt.fingerprint != fingerprint {
                return Err(storage_error());
            }
            if receipt.job_id.as_ref().is_some_and(|id| !valid_job_id(id)) {
                return Err(storage_error());
            }
            // Truncating an incomplete last append cannot erase the previous
            // durable dispatch claim. Unknown BYOK dispatch stays uncertain.
            file.set_len((end + 1) as u64).map_err(storage_failure)?;
            receipt
        };
        let mut checkpoint = Self { file, receipt };
        if bytes.is_empty() {
            checkpoint.save()?;
        }
        // sync_all on the receipt commits its bytes, not the containing entry.
        // Persist that entry before any network dispatch can use the nonce.
        #[cfg(unix)]
        sync_directory(path.parent().ok_or_else(storage_error)?)?;
        Ok(checkpoint)
    }
    fn save(&mut self) -> Result<(), SttError> {
        let mut bytes = serde_json::to_vec(&self.receipt).map_err(storage_failure)?;
        bytes.push(b'\n');
        self.file.seek(SeekFrom::End(0)).map_err(storage_failure)?;
        self.file.write_all(&bytes).map_err(storage_failure)?;
        self.file.sync_all().map_err(storage_failure)
    }
}

fn valid_job_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 160
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}
fn managed_identity_error() -> SttError {
    SttError::Config("The managed session identity could not be verified for this upload. Sign in again; the saved recording is unchanged.".into())
}
fn managed_identity_failure<T>(_error: T) -> SttError {
    managed_identity_error()
}
fn credential_owner(provider: &YiddishLabsSttProvider) -> Result<String, SttError> {
    if provider.api_key.trim().is_empty() {
        return Err(SttError::Config(
            "Add the YiddishLabs Standard API key in secure key settings".into(),
        ));
    }
    if provider.managed_gateway.is_none() {
        return Ok(format!("{:x}", Sha256::digest(provider.api_key.as_bytes())));
    }
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    if provider.api_key.len() > 65536 {
        return Err(managed_identity_error());
    }
    let payload = provider
        .api_key
        .split('.')
        .nth(1)
        .ok_or_else(managed_identity_error)?;
    let claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(managed_identity_failure)?,
    )
    .map_err(managed_identity_failure)?;
    let sub = claims
        .get("sub")
        .and_then(Value::as_str)
        .filter(|sub| !sub.is_empty())
        .ok_or_else(managed_identity_error)?;
    let iss = claims
        .get("iss")
        .and_then(Value::as_str)
        .filter(|iss| !iss.is_empty())
        .ok_or_else(managed_identity_error)?;
    let organization = claims
        .get("org_id")
        .or_else(|| claims.get("organization_id"))
        .or_else(|| {
            claims.get("app_metadata").and_then(|metadata| {
                metadata
                    .get("org_id")
                    .or_else(|| metadata.get("organization_id"))
            })
        });
    // Continuity check only: Edge verifies the actual bearer on every request.
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::json!([sub, iss, organization])
                .to_string()
                .as_bytes()
        )
    ))
}

pub(super) async fn transcribe(
    provider: &YiddishLabsSttProvider,
    audio: &[u8],
    context: &SttUploadContext,
) -> Result<String, SttError> {
    transcribe_with_poll_interval(provider, audio, context, Duration::from_secs(2)).await
}

async fn transcribe_with_poll_interval(
    provider: &YiddishLabsSttProvider,
    audio: &[u8],
    context: &SttUploadContext,
    poll_interval: Duration,
) -> Result<String, SttError> {
    let language = language_hint(provider.language.as_deref(), true)?;
    let owner = credential_owner(provider)?;
    if audio.is_empty() || audio.len() > 25 * 1024 * 1024 {
        return Err(SttError::Audio(
            "Use the file/meeting chunked upload flow for YiddishLabs Standard".into(),
        ));
    }
    if provider
        .prompt
        .as_ref()
        .is_some_and(|prompt| prompt.len() > 8000)
    {
        return Err(SttError::Config(
            "The YiddishLabs transcription context is too long".into(),
        ));
    }
    let endpoint = provider
        .managed_gateway
        .as_ref()
        .map(|gateway| crate::http::join_base_url(gateway, "/v1/managed/transcription/jobs"))
        .unwrap_or_else(|| provider.standard_endpoint.clone());
    let purpose = match context.purpose {
        #[cfg(test)]
        UploadPurpose::File => "file",
        UploadPurpose::Meeting => "meeting",
    };
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                &endpoint,
                owner,
                language,
                &provider.prompt,
                purpose,
                context.chunk_offset,
                format!("{:x}", Sha256::digest(audio))
            ))
            .map_err(storage_failure)?
        )
    );
    let mut checkpoint = Checkpoint::open(&context.checkpoint_path, fingerprint)?;
    let managed = provider.managed_gateway.is_some();
    if checkpoint.receipt.dispatched && checkpoint.receipt.job_id.is_none() && !managed {
        return Err(pending_error());
    }
    if checkpoint.receipt.job_id.is_none() {
        checkpoint.receipt.dispatched = true;
        checkpoint.save()?;
        let mut form = multipart::Form::new()
            .part(
                "file",
                multipart::Part::bytes(audio.to_vec()).file_name("recording.wav"),
            )
            .text("language", language.to_owned());
        if let Some(prompt) = &provider.prompt {
            form = form.text("context", prompt.clone());
        }
        let mut request = provider
            .client
            .post(&endpoint)
            .timeout(Duration::from_secs(120));
        if managed {
            form = form.text("model", STANDARD_MODEL).text("purpose", purpose);
            request = request
                .bearer_auth(&provider.api_key)
                .header("x-kolboo-model-provider", "yiddishlabs")
                .header("idempotency-key", checkpoint.receipt.nonce.to_string());
        } else {
            request = request.header("X-API-KEY", &provider.api_key);
        }
        let response = crate::http::with_cloudflare_access_headers_if_target(request, &endpoint)
            .multipart(form)
            .send()
            .await
            .map_err(|_| pending_error())?;
        if !response.status().is_success() {
            if managed && response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(SttError::Api("401 auth_invalid_token: the saved upload key is retained for a same-owner session refresh".into()));
            }
            return Err(pending_error());
        }
        let value = response_json(response).await.map_err(|_| pending_error())?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| valid_job_id(id))
            .ok_or_else(pending_error)?;
        if managed && uuid::Uuid::parse_str(id).is_err() {
            return Err(pending_error());
        }
        checkpoint.receipt.job_id = Some(id.into());
        checkpoint.save()?;
        if let Some(result) = terminal_result(&value)? {
            return Ok(result);
        }
    }
    let id = checkpoint
        .receipt
        .job_id
        .as_ref()
        .ok_or_else(pending_error)?;
    loop {
        let mut request = provider
            .client
            .get(format!("{endpoint}/{id}"))
            .timeout(Duration::from_secs(15));
        request = if managed {
            request.bearer_auth(&provider.api_key)
        } else {
            request.header("X-API-KEY", &provider.api_key)
        };
        let response = crate::http::with_cloudflare_access_headers_if_target(request, &endpoint)
            .send()
            .await
            .map_err(|_| pending_error())?;
        if !response.status().is_success() {
            if managed && response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(SttError::Api("401 auth_invalid_token: the saved job is retained for a same-owner session refresh".into()));
            }
            return Err(pending_error());
        }
        let value = response_json(response).await?;
        if value.get("id").and_then(Value::as_str) != Some(id) {
            return Err(pending_error());
        }
        if let Some(result) = terminal_result(&value)? {
            return Ok(result);
        }
        tokio::time::sleep(poll_interval).await;
    }
}

fn terminal_result(value: &Value) -> Result<Option<String>, SttError> {
    match value.get("status").and_then(Value::as_str) {
        Some("completed") => value.get("text").and_then(Value::as_str).map(|text| Some(text.to_owned())).ok_or_else(pending_error),
        Some("queued" | "processing" | "pending") => Ok(None),
        Some("failed" | "rejected") => Err(SttError::Api("The submitted YiddishLabs job failed. Its recording and job receipt are retained; provider processing or charges are not cancelled.".into())),
        _ => Err(pending_error()),
    }
}

#[cfg(test)]
#[path = "tests/standard_jobs.rs"]
mod tests;
