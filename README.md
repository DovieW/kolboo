<p align="center">
  <img src="app/src-tauri/icons/icon.png" alt="Kolboo" width="128" height="128">
</p>

# Kolboo

Accurate speech-to-text using advanced STT models like Whisper (local or via API).

A fork of Tambourine mainly created so that a standalone python server is not required.

|||
|-|-|
|Home|Settings|
|![Screenshot 1](https://private-user-images.githubusercontent.com/32934685/530891802-1feb7c18-8e44-4de7-8ea5-9ca2956cb2b8.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3NjcwNzExOTEsIm5iZiI6MTc2NzA3MDg5MSwicGF0aCI6Ii8zMjkzNDY4NS81MzA4OTE4MDItMWZlYjdjMTgtOGU0NC00ZGU3LThlYTUtOWNhMjk1NmNiMmI4LnBuZz9YLUFtei1BbGdvcml0aG09QVdTNC1ITUFDLVNIQTI1NiZYLUFtei1DcmVkZW50aWFsPUFLSUFWQ09EWUxTQTUzUFFLNFpBJTJGMjAyNTEyMzAlMkZ1cy1lYXN0LTElMkZzMyUyRmF3czRfcmVxdWVzdCZYLUFtei1EYXRlPTIwMjUxMjMwVDA1MDEzMVomWC1BbXotRXhwaXJlcz0zMDAmWC1BbXotU2lnbmF0dXJlPTFkOGY3MTIyMDY4OWFlMDIwMTU2MTkzNzU2NTIxY2ZiNWFjYTA0YTg5ZTA3ODI4MjYxNzQ5OTM3NGRkYTY0NzkmWC1BbXotU2lnbmVkSGVhZGVycz1ob3N0In0.oTcL9VynyZGZT6-4CUbm1QbWghHOhKA5aiNBwldBgkA)|![Screenshot 2](https://private-user-images.githubusercontent.com/32934685/530891851-75f60eee-aec0-45ab-bf4f-19bdd0e592f8.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3NjcwNzExOTEsIm5iZiI6MTc2NzA3MDg5MSwicGF0aCI6Ii8zMjkzNDY4NS81MzA4OTE4NTEtNzVmNjBlZWUtYWVjMC00NWFiLWJmNGYtMTliZGQwZTU5MmY4LnBuZz9YLUFtei1BbGdvcml0aG09QVdTNC1ITUFDLVNIQTI1NiZYLUFtei1DcmVkZW50aWFsPUFLSUFWQ09EWUxTQTUzUFFLNFpBJTJGMjAyNTEyMzAlMkZ1cy1lYXN0LTElMkZzMyUyRmF3czRfcmVxdWVzdCZYLUFtei1EYXRlPTIwMjUxMjMwVDA1MDEzMVomWC1BbXotRXhwaXJlcz0zMDAmWC1BbXotU2lnbmF0dXJlPTk5NzM1ZDljYWZhYmI4ZGM3NmQyNWUyMTc4ODExMmJjOWM4YjdjMWIxNDIxYzdiMzUwM2U2MzE3MzEzMjQzZGImWC1BbXotU2lnbmVkSGVhZGVycz1ob3N0In0.4arrDtbaJZjeUm6cw1ZBDOVmAzWEkr38fqS9a7yDxLk)|
|Stats (WIP)|Logs|
|![Screenshot 3](https://private-user-images.githubusercontent.com/32934685/530892016-2911e930-0d99-4c51-9a2f-58c4dc4a89c5.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3NjcwNzExOTEsIm5iZiI6MTc2NzA3MDg5MSwicGF0aCI6Ii8zMjkzNDY4NS81MzA4OTIwMTYtMjkxMWU5MzAtMGQ5OS00YzUxLTlhMmYtNThjNGRjNGE4OWM1LnBuZz9YLUFtei1BbGdvcml0aG09QVdTNC1ITUFDLVNIQTI1NiZYLUFtei1DcmVkZW50aWFsPUFLSUFWQ09EWUxTQTUzUFFLNFpBJTJGMjAyNTEyMzAlMkZ1cy1lYXN0LTElMkZzMyUyRmF3czRfcmVxdWVzdCZYLUFtei1EYXRlPTIwMjUxMjMwVDA1MDEzMVomWC1BbXotRXhwaXJlcz0zMDAmWC1BbXotU2lnbmF0dXJlPTA3MjAwYTBmNzRkMGZkZGEyNzAyYjRlNDllYzYzYjA4Zjc1NjU1ODNmYTk4ZDVlMGU0ZWE1MDBmNjUzNGI5NTUmWC1BbXotU2lnbmVkSGVhZGVycz1ob3N0In0.h8Rg1As_kwIeJ5OeOOvoANr_qLJV67yGm4al0x_X_nA)|![Screenshot 4](https://private-user-images.githubusercontent.com/32934685/530891975-8a221225-7a7d-4901-b0b3-0037a36ab923.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3NjcwNzExOTEsIm5iZiI6MTc2NzA3MDg5MSwicGF0aCI6Ii8zMjkzNDY4NS81MzA4OTE5NzUtOGEyMjEyMjUtN2E3ZC00OTAxLWIwYjMtMDAzN2EzNmFiOTIzLnBuZz9YLUFtei1BbGdvcml0aG09QVdTNC1ITUFDLVNIQTI1NiZYLUFtei1DcmVkZW50aWFsPUFLSUFWQ09EWUxTQTUzUFFLNFpBJTJGMjAyNTEyMzAlMkZ1cy1lYXN0LTElMkZzMyUyRmF3czRfcmVxdWVzdCZYLUFtei1EYXRlPTIwMjUxMjMwVDA1MDEzMVomWC1BbXotRXhwaXJlcz0zMDAmWC1BbXotU2lnbmF0dXJlPTljMjNiY2YyMmFmZmJkMzE3NDQ2ZGE0MTBkMjgxMzI5NWMzNWUxYzZiYWNhZTFmZWZhNjViOWRjODQwYjI2MTUmWC1BbXotU2lnbmVkSGVhZGVycz1ob3N0In0.cub_XQeRh15Xgdnxp63wN63iThZsS78bWveG7g0W_sI)|

## Features

- Dictate using state-of-the-art speech-to-text and language models.
- Hotkeys for: Toggle recording, Hold to record, Paste last transcription.
- Pass dictation to LLMs for further enhancement.
- Create per program workflows using the profile system.
- Clean overlay while transcribing (image) with customizable sound cue.
- Comprehensive provider and model support ([full list](docs/SUPPORTED_PROVIDERS_AND_MODELS.md)): Local, Google, OpenAI, Groq, Avalon and more.
- Logging and testing system to easily troubleshoot issues and work on prompts.
- Stats page to track cost usage and more (WIP).
- Audio refinement settings.
- Customize accent color.
- And more.

## Installation

Download Windows installer from the [latest release](https://github.com/DovieW/kolboo/releases).

| Platform | Compatibility    |
| -------- | -------------    |
| Windows  | ✅               |
| macOS    | ⚠️ (need tester) |
| Linux    | ⚠️ (need tester) |

## License

[AGPL-3.0](LICENSE)
