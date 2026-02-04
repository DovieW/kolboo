# Data Model: Backend CLI Subcommand

## Entities

### Command
- **Purpose**: A named CLI action invoked by the user.
- **Fields**:
  - `name` (string): Command name (e.g., `pipeline run`).
  - `args` (map<string, string | number | boolean>): Parsed arguments and flags.
  - `output_format` (enum: `json`, `human`): Output format selection.

### CommandResult
- **Purpose**: Standardized output for all commands.
- **Fields**:
  - `success` (boolean)
  - `code` (number): Exit code.
  - `message` (string | null): Human-friendly summary.
  - `data` (object | null): Command-specific payload.
  - `warnings` (string[])

### PipelineRun
- **Purpose**: Represents a headless pipeline run.
- **Fields**:
  - `id` (string)
  - `state` (string): Current pipeline state.
  - `started_at` (string, ISO-8601)
  - `updated_at` (string, ISO-8601)
  - `progress` (object | null): Optional progress details.

### Settings
- **Purpose**: Persisted configuration values used by the backend.
- **Fields**:
  - `key` (string)
  - `value` (any JSON value)
  - `source` (enum: `default`, `user`, `profile`)

### Profile
- **Purpose**: Named bundle of settings.
- **Fields**:
  - `id` (string)
  - `name` (string)
  - `is_active` (boolean)
  - `settings_overrides` (object)

### DiagnosticsReport
- **Purpose**: Structured health and environment info.
- **Fields**:
  - `app_version` (string)
  - `os` (string)
  - `capabilities` (object)
  - `checks` (array of { `name`, `status`, `details` })

### ConfigurationExport
- **Purpose**: Effective configuration snapshot.
- **Fields**:
  - `active_profile` (Profile)
  - `settings` (object)
  - `generated_at` (string, ISO-8601)

## Relationships

- A **Command** produces a **CommandResult**.
- A **PipelineRun** is created by the `pipeline run` command.
- **Profiles** reference **Settings** overrides and determine effective configuration.
- **ConfigurationExport** references the active **Profile** and current **Settings**.

## Validation Rules

- `Command.name` must match a supported CLI command/subcommand.
- `Settings.key` must match a known setting key; unknown keys return validation errors.
- `Profile.id` and `Profile.name` must be non-empty strings.
- `PipelineRun.state` must be one of the known pipeline states.
