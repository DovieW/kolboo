# Quickstart: Active Window OCR Context

This guide explains how to run an OCR server locally and wire it into Kolboo.

## 1) Run the OCR server (vLLM)

The target model is **LightOnOCR**: `lightonai/LightOnOCR-1B-1025`.

From the model docs, the recommended vLLM invocation is:

- Serve with a single image per request:
  - `--limit-mm-per-prompt '{"image": 1}'`

You should end up with an OpenAI-compatible endpoint at:

- `http://localhost:8000/v1/chat/completions`

## 2) Configure Kolboo

In Kolboo:

1. Open **Settings**
2. Go to **Providers**
3. Find the new **OCR** provider section
4. Set **OCR Base URL**:
  - vLLM example: `http://localhost:8000`
  - OpenAI example: `https://api.openai.com`
5. Set **OCR Model**:
  - Default: `lightonai/LightOnOCR-1B-1025`
  - You can replace this with any OCR-capable model your provider supports
6. If your provider requires auth, set **OCR Auth Mode = API Key** and enter the **OCR API Key**.

## 3) Enable OCR context per tool

In Settings (wherever the controls live), each tool has a 3-state mode:

- **Disabled**: never run OCR
- **Auto**: run OCR automatically when the tool is triggered
- **Manual**: show an OCR button in the recording overlay; OCR runs only after you click it

Configure any of:

- Rewrite
- Quick Replace
- Quick Ask

## 4) Try it

1. Put focus on a window that has visible text (browser, PDF, etc.)
2. Trigger the tool you enabled (Quick Ask / Quick Replace / Rewrite)
3. Confirm the assistant request includes a clearly labeled section:

- "OCR context from the currently active window"

## Troubleshooting

- If you see "OCR context unavailable":
  - The OCR URL may be unset/invalid, or
  - The active window may be protected/un-capturable, or
  - The OCR server may be down.

## Debugging tips

- The **Request Logs** page is great for seeing high-level breadcrumbs like:
	- whether OCR was started
	- whether OCR finished
	- whether OCR text was attached to the request (presence/char count)

- The `pnpm -C app dev` logs are better for lower-level timing and state transitions (recording start/stop, pipeline reset/cancel).

- If OCR was started but later shows as `status=not_started`, that typically indicates a session lifecycle issue (OCR ownership cleared). The robust fix for that is session-owned OCR jobs.

Kolboo should still complete the tool action without OCR context.


