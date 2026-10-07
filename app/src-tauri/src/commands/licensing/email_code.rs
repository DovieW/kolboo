use super::*;

fn validated_email(email: &str) -> CommandResult<String> {
    let email = email.trim().to_lowercase();
    if email.len() > 320 || !email.contains('@') || email.chars().any(char::is_whitespace) {
        return Err(CommandError::new("Enter a valid email address.", "auth")
            .with_code("auth_email_invalid"));
    }
    Ok(email)
}

fn email_code_error(status: StatusCode, verify: bool) -> CommandError {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return CommandError::new("Please wait before requesting another code.", "auth")
            .with_code("auth_code_rate_limited");
    }
    if verify
        && matches!(
            status,
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        )
    {
        return CommandError::new(
            "That code is invalid or expired. Try again or request a new code.",
            "auth",
        )
        .with_code("auth_code_invalid");
    }
    CommandError::new(
        if verify {
            "Unable to verify the code right now. Please try again."
        } else {
            "Unable to send a code. New accounts require an approved beta email."
        },
        "auth",
    )
    .with_code("auth_code_request_failed")
    .with_retryable(status.is_server_error())
}

async fn send_email_code_request(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    email: &str,
    code: Option<&str>,
) -> CommandResult<reqwest::Response> {
    let payload = match code {
        Some(code) => json!({"email": email, "token": code, "type": "email"}),
        None => json!({"email": email, "create_user": true}),
    };
    let path = if code.is_some() { "verify" } else { "otp" };
    let response = client
        .post(format!("{base}/auth/v1/{path}"))
        .timeout(Duration::from_secs(20))
        .header("apikey", key)
        .json(&payload)
        .send()
        .await
        .map_err(|_| {
            CommandError::new(
                "Unable to reach the account service. Check your connection and try again.",
                "auth",
            )
            .with_code("auth_service_unavailable")
            .with_retryable(true)
        })?;
    if !response.status().is_success() {
        return Err(email_code_error(response.status(), code.is_some()));
    }
    Ok(response)
}

#[tauri::command]
pub async fn license_request_email_code(email: String) -> CommandResult<()> {
    let email = validated_email(&email)?;
    let (base, key) = supabase_auth_config()?;
    send_email_code_request(&license_api_client(), &base, &key, &email, None).await?;
    Ok(())
}

#[tauri::command]
pub async fn license_verify_email_code(
    app: AppHandle,
    email: String,
    code: String,
) -> CommandResult<LicenseState> {
    let ticket = SESSION_OWNER.begin();
    let email = validated_email(&email)?;
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(
            CommandError::new("Enter the six-digit code from your email.", "auth")
                .with_code("auth_code_invalid"),
        );
    }
    let (base, key) = supabase_auth_config()?;
    let response =
        send_email_code_request(&license_api_client(), &base, &key, &email, Some(code)).await?;
    let auth = response
        .json::<SupabaseSessionAuthResponse>()
        .await
        .map_err(|_| {
            CommandError::new(
                "The account service returned an invalid sign-in response.",
                "auth",
            )
            .with_code("auth_response_parse_failed")
        })?;
    persist_session_and_hydrate_license_state(
        &app,
        auth,
        Method::GET,
        "/v1/license/state",
        "email_code_login_success",
        "Email code login",
        ticket,
    )
    .await
}

#[cfg(test)]
#[path = "tests/email_code.rs"]
mod tests;
