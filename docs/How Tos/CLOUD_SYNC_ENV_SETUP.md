# Cloud sync env setup

Cloud sync endpoint configuration for the desktop app is read from app runtime env vars.

## Variables

- `KOLBOO_SYNC_BASE_URL` (preferred)
- `VITE_SYNC_BASE_URL` (fallback)

If both are set, `KOLBOO_SYNC_BASE_URL` takes precedence.

## Local setup

1. Copy `app/.env.example` to `app/.env`.
2. Set the base URL to your API edge host, for example:
   - `KOLBOO_SYNC_BASE_URL=https://api.kolboo.example`

## Notes

- These variables are consumed by `app/src-tauri/src/commands/sync.rs`.
- They are non-secret runtime config values.
