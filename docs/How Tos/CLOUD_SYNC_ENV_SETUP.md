# Cloud sync env setup

Cloud sync endpoint configuration for the desktop app is read from Tauri runtime env vars.

## Variables

- `TAURI_API_BASE_URL`

Set this to a real deployed api-edge Worker origin. Do not use `api.kolboo.example`; it is not a real Kolboo domain.

## Hosted setup

1. Copy `app/.env.example` to `app/.env`.
2. Set the base URL to your deployed API edge Worker, for example:
   - `TAURI_API_BASE_URL=https://kolboo-api-edge-dev.<your-workers-subdomain>.workers.dev`
3. If managed inference should use the same Worker, also set:
   - `TAURI_MANAGED_INFERENCE_GATEWAY_URL=https://kolboo-api-edge-dev.<your-workers-subdomain>.workers.dev`

## Notes

- These variables are consumed by `app/src-tauri/src/commands/sync.rs`.
- They are non-secret runtime config values.
- Release builds fail if `TAURI_API_BASE_URL` or `TAURI_MANAGED_INFERENCE_GATEWAY_URL` is missing or placeholder.
- For an intentional offline/internal release build only, set `KOLBOO_ALLOW_MISSING_RELEASE_CLOUD_ENDPOINTS=1`.
- Cloud sync requires a personal or enterprise plan at the edge authorization layer.
