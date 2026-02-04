# Quickstart: Backend CLI Subcommand

## Goal

Use the Kolboo backend CLI to run the pipeline headlessly, inspect status, manage settings/profiles, and export diagnostics/configuration.

## Prerequisites

- Kolboo is installed and available on your PATH.
- You have permission to access any required local resources.

## Common Tasks

### Run the pipeline

- Start a headless run and return JSON output.
- Example:

```
kolboo pipeline run --output json
```

### Check pipeline status

```
kolboo pipeline status --output json
```

### Stop recording and transcribe

```
kolboo pipeline stop --output json
```

### Transcribe a WAV file (test core pipeline)

```
kolboo pipeline transcribe --file "C:\path\to\sample.wav" --output json
```

### Read or update a setting

```
kolboo settings get --key transcription_language --output json
kolboo settings set --key transcription_language --value en-US --output json
```

### List or select profiles

```
kolboo profiles list --output json
kolboo profiles use --profile default --output json
```

### Diagnostics and configuration export

```
kolboo diagnostics --output json
kolboo config export --output json
```

## Output Format

- Default output is JSON for scripting.
- Use `--output human` for human-readable output.

## Exit Codes

- `0`: Success
- `2`: Validation error
- `3`: Runtime failure
