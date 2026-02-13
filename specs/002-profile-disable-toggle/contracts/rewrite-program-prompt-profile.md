# Contract: RewriteProgramPromptProfile (settings.json)

This contract describes the persisted shape under the `rewrite_program_prompt_profiles` key in `settings.json` and how it is interpreted.

## Persisted location

- Store file: `settings.json`
- Key: `rewrite_program_prompt_profiles`
- Type: array of `RewriteProgramPromptProfile`

## Field: `disabled`

- **Type**: boolean
- **Optional on disk**: yes (for backward compatibility)
- **Default when missing/invalid**: `false`
- **Semantics**:
	- If `disabled === true`, the profile MUST NOT be eligible for activation.
	- If `disabled === true`, the profile MUST remain visible/editable in UI.

## Backward compatibility rules

- Profiles stored before this feature will not have `disabled`. They are treated as `disabled=false`.

## Example JSON

```json
{
  "rewrite_program_prompt_profiles": [
    {
      "id": "abc123",
      "name": "Obsidian",
      "program_paths": ["obsidian.exe"],
      "disabled": false,
      "cleanup_prompt_sections": null,
      "stt_provider": null,
      "llm_provider": null,
      "presets": []
    }
  ]
}
```

## Non-goals

- This contract does not introduce new commands/events.
- This contract does not change the meaning of existing override fields.
