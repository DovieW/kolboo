# Profile Configuration Presets System

This document outlines the research findings and recommendations for implementing a configuration preset system on top of the existing profile system in Kolboo.

**Status**: Research & Design  
**Date**: 2026-01-03  
**Author**: System Research

---

## Executive Summary

Currently, Kolboo has a robust **profile system** that allows per-program customization of AI settings, prompts, UI preferences, and audio settings. However, users cannot quickly swap between different configuration sets for the same program without manually changing settings or editing the profile.

This document proposes adding a **Configuration Preset** layer that sits above profiles, enabling users to:
- Store multiple named configuration sets per profile
- Quickly switch between presets on-the-fly
- Temporarily override settings without losing the default configuration
- Create use-case-specific presets (e.g., "Brief notes", "Detailed transcription", "Code dictation")

---

## Current System Architecture

### 1. Profile System (Existing)

**Location**: 
- Backend: `app/src-tauri/src/settings.rs` (`RewriteProgramPromptProfile`)
- Frontend: `app/src/components/settings/ProgramsModal.tsx`
- Pipeline: `app/src-tauri/src/pipeline.rs` (`select_profile_for_foreground_app`)

**What it does**:
- Matches foreground application by executable path
- Automatically activates profile when program gains focus
- Stores per-profile overrides for:
  - **AI Settings**: STT provider/model, LLM provider/model, rewrite enabled
  - **Prompts**: Main, Advanced, Dictionary sections with custom content
  - **Timeouts**: STT timeout configuration
  - **Provider-specific**: OpenAI reasoning effort, Gemini thinking budget/level, Anthropic thinking budget
  - **UI Settings**: Sound, overlay mode, widget position, output mode (clipboard, paste, etc.)
  - **Audio**: Playing audio handling behavior

**Data Structure** (`RewriteProgramPromptProfile`):
```rust
pub struct RewriteProgramPromptProfile {
    pub id: String,                    // Unique identifier
    pub name: String,                  // Display name
    pub program_paths: Vec<String>,    // Executable paths to match
    
    // AI configuration overrides (None = inherit from Default)
    pub cleanup_prompt_sections: Option<CleanupPromptSectionsSetting>,
    pub rewrite_llm_enabled: Option<bool>,
    pub stt_provider: Option<String>,
    pub stt_model: Option<String>,
    pub stt_timeout_seconds: Option<f64>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub openai_reasoning_effort: Option<String>,
    pub gemini_thinking_budget: Option<i64>,
    pub gemini_thinking_level: Option<String>,
    pub anthropic_thinking_budget: Option<i64>,
    
    // UI configuration overrides
    pub sound_enabled: Option<bool>,
    pub playing_audio_handling: Option<String>,
    pub overlay_mode: Option<String>,
    pub widget_position: Option<WidgetPosition>,
    pub output_mode: Option<String>,
}
```

**Storage**: `settings.json` via Tauri store plugin, key: `rewrite_program_prompt_profiles`

**Selection Logic**:
1. Pipeline checks foreground application path (`get_foreground_process_path()`)
2. Normalizes path (case-insensitive, backslash-normalized for Windows)
3. Iterates through profiles to find a match
4. Uses matched profile's overrides, falling back to Default for any `None` values
5. Manual override possible via `profile_id_override` in testing/retry flows

### 2. Default Profile (Global Settings)

**Location**: Various settings across `settings.json`

The "Default" profile is not a separate entity but represents the global settings that profiles inherit from when they don't override a value. This includes:
- Global STT provider/model
- Global LLM provider/model/prompts
- Global UI preferences
- Global audio settings

**Inheritance Pattern**:
```
Global Settings (Default)
    ↓ (inherits via Option::None)
Profile for VS Code
    ↓ (inherits via Option::None for non-overridden fields)
Active Configuration
```

### 3. Settings Architecture

**Settings are split across**:
- **Rust backend**: Settings definitions, validation, defaults
- **TypeScript frontend**: UI for editing, migrations, normalization
- **Tauri store**: Persistent JSON storage in user data directory

**Key Files**:
- `app/src-tauri/src/settings.rs` - Rust types and defaults
- `app/src/lib/tauri.ts` - TypeScript types and API wrappers
- `app/src/components/settings/*.tsx` - UI components for each settings category

---

## Problem Statement

### User Need

Users want to temporarily switch between different configuration sets for the same program without:
1. **Manually changing multiple settings** each time
2. **Creating duplicate profiles** for the same program
3. **Losing their default configuration** when experimenting
4. **Permanently altering the profile** when they need a one-off change

### Real-World Use Cases

1. **Writing Context Switching**
   - Normal profile: "VS Code" → Brief, casual transcription
   - Want to temporarily use: "Formal documentation" preset with detailed prompts
   
2. **Cost Management**
   - Normal profile: Uses expensive GPT-4 for quality
   - Want to temporarily use: "Budget mode" preset with cheaper models
   
3. **Testing & Experimentation**
   - Normal profile: Production settings
   - Want to temporarily use: "Test" preset with different providers/models
   
4. **Task-Specific Optimization**
   - Normal profile: General dictation
   - Want to switch to: "Code dictation", "Meeting notes", "Email drafting" presets

### Current Workarounds (and their problems)

1. **Manual setting changes**: Tedious, error-prone, can't easily revert
2. **Multiple profiles for same program**: Confusing, requires program path duplication, doesn't solve temporary override
3. **Edit Default profile**: Affects all programs, loses global baseline

---

## Proposed Solution: Configuration Presets

### Concept Overview

Add a **Configuration Preset** layer that sits between the user and the profile system:

```
User Interface
    ↓
Configuration Presets (stored per profile)
    ↓
Profile System (existing, auto-activated by program)
    ↓
Default Settings (global fallback)
```

### Terminology

**Naming considerations** (to avoid confusion):

| Term | Pro | Con | Recommendation |
|------|-----|-----|----------------|
| **Configuration Preset** | Clear, familiar (UI presets) | Verbose | ✅ Best for technical docs |
| **Config** | Short | Too generic, could mean profile itself | ❌ |
| **Preset** | Short, implies "pre-configured" | Might be confused with templates | ⚠️ Use in UI as shorthand |
| **Mode** | Very short | Implies mutually exclusive states | ⚠️ Could work ("Budget mode") |
| **Variant** | Clear hierarchy | Less familiar to users | ❌ |

**Recommended naming**:
- **Code/docs**: "Configuration Preset" or "Preset"
- **UI**: "Preset" or "Config preset"
- **User-facing help**: "Configuration preset" with examples

### Data Structure

#### New: ConfigurationPreset

```rust
pub struct ConfigurationPreset {
    pub id: String,                    // Unique identifier
    pub name: String,                  // Display name (e.g., "Budget mode", "Detailed notes")
    pub description: Option<String>,   // Optional user note
    
    // All the same override fields as RewriteProgramPromptProfile
    // (but applied at preset selection time, not program match time)
    pub cleanup_prompt_sections: Option<CleanupPromptSectionsSetting>,
    pub rewrite_llm_enabled: Option<bool>,
    pub stt_provider: Option<String>,
    pub stt_model: Option<String>,
    pub stt_timeout_seconds: Option<f64>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub openai_reasoning_effort: Option<String>,
    pub gemini_thinking_budget: Option<i64>,
    pub gemini_thinking_level: Option<String>,
    pub anthropic_thinking_budget: Option<i64>,
    pub sound_enabled: Option<bool>,
    pub playing_audio_handling: Option<String>,
    pub overlay_mode: Option<String>,
    pub widget_position: Option<WidgetPosition>,
    pub output_mode: Option<String>,
}

pub struct RewriteProgramPromptProfile {
    // ... existing fields ...
    
    // NEW: Collection of presets for this profile
    #[serde(default)]
    pub presets: Vec<ConfigurationPreset>,
    
    // NEW: Currently active preset ID (None = use profile's own settings)
    #[serde(default)]
    pub active_preset_id: Option<String>,
}
```

### Resolution Order (with presets)

```
1. Active Preset (if selected)
    ↓ (None values inherit)
2. Profile Settings
    ↓ (None values inherit)
3. Default/Global Settings
```

**Example**:
```
Default: STT=OpenAI Whisper, LLM=GPT-4o-mini
    ↓
Profile "VS Code": STT=None (inherits Whisper), LLM=GPT-4 (override)
    ↓
Preset "Budget": STT=None (inherits Whisper), LLM=GPT-3.5-turbo (override)
    ↓
Final: STT=OpenAI Whisper, LLM=GPT-3.5-turbo
```

### User Workflows

#### 1. Creating a Preset

```
Settings → Select Profile → [New Preset button]
    ↓
Enter preset name and description
    ↓
Configure overrides (all start as "Inherit from profile")
    ↓
Save preset
```

#### 2. Activating a Preset

**Option A: Settings UI**
```
Settings → Select Profile → Presets dropdown → Select preset
```

**Option B: Quick-switch Hotkey/Menu** (future enhancement)
```
Hotkey → Preset quick-switcher overlay → Select preset
```

**Option C: System Tray** (future enhancement)
```
Tray menu → Active Profile: VS Code → Presets → [Budget mode ✓]
```

#### 3. Deactivating a Preset

```
Select "(Default - use profile settings)" in preset dropdown
```

#### 4. Editing a Preset

```
Settings → Select Profile → Select Preset → Edit settings → Save
```

#### 5. Deleting a Preset

```
Settings → Select Profile → Select Preset → [Delete Preset button]
```

### UI Mockup (Text-based)

**Settings Screen - Prompt/AI Tab**:
```
┌─────────────────────────────────────────────────────┐
│ Profile: [VS Code ▼]  [⚙️ Configure] [➕ New]      │
│                                                     │
│ Preset:  [Default - use profile settings    ▼]     │
│          ├─ Budget mode                             │
│          ├─ Detailed transcription                  │
│          └─ Code dictation                          │
│                                                     │
│          [📝 Edit Preset]  [➕ New Preset]          │
│                                                     │
│ ───────────────────────────────────────────────────│
│                                                     │
│ Rewrite with LLM   [✓] Enabled   (🔹 from preset)  │
│                                                     │
│ STT Provider:      [OpenAI Whisper ▼] (inherited)  │
│ STT Model:         [whisper-1      ▼] (inherited)  │
│                                                     │
│ LLM Provider:      [OpenAI         ▼] (🔹 preset)  │
│ LLM Model:         [gpt-3.5-turbo  ▼] (🔹 preset)  │
│                                                     │
└─────────────────────────────────────────────────────┘

Legend:
(inherited) - from profile or default
(🔹 preset)  - overridden by active preset
(🔸 profile) - overridden by profile
```

**New Preset Modal**:
```
┌─────────────────────────────────────────┐
│ Create Preset for "VS Code"            │
├─────────────────────────────────────────┤
│                                         │
│ Name:  [Budget mode                  ]  │
│                                         │
│ Description (optional):                 │
│ [Use cheaper models for quick drafts ]  │
│                                         │
│                                         │
│ All settings start as "Inherit from    │
│ profile". Edit the preset to customize.│
│                                         │
│          [Cancel]  [Create Preset]      │
│                                         │
└─────────────────────────────────────────┘
```

---

## Implementation Recommendations

### Phase 1: Core Preset System (MVP)

**Goals**: 
- Add preset storage per profile
- Allow creating, editing, deleting presets
- Allow activating a preset within Settings UI
- Show inheritance indicators in UI

**Backend Changes**:

1. **Update `settings.rs`**:
   - Add `ConfigurationPreset` struct
   - Add `presets: Vec<ConfigurationPreset>` to `RewriteProgramPromptProfile`
   - Add `active_preset_id: Option<String>` to `RewriteProgramPromptProfile`

2. **Update preset resolution in `pipeline.rs`**:
   - Modify `select_profile_for_foreground_app` to also apply active preset
   - Create helper: `apply_preset_to_profile(profile, preset_id) -> Profile`
   - Resolution order: preset → profile → default

3. **Add Tauri commands**:
   ```rust
   // In app/src-tauri/src/commands/config.rs
   
   #[tauri::command]
   pub fn get_presets_for_profile(profile_id: String) -> Vec<ConfigurationPreset>
   
   #[tauri::command]
   pub fn create_preset_for_profile(profile_id: String, preset: ConfigurationPreset)
   
   #[tauri::command]
   pub fn update_preset(profile_id: String, preset_id: String, preset: ConfigurationPreset)
   
   #[tauri::command]
   pub fn delete_preset(profile_id: String, preset_id: String)
   
   #[tauri::command]
   pub fn set_active_preset(profile_id: String, preset_id: Option<String>)
   ```

**Frontend Changes**:

1. **Update TypeScript types** (`app/src/lib/tauri.ts`):
   ```typescript
   export interface ConfigurationPreset {
     id: string;
     name: string;
     description?: string;
     // ... all override fields same as RewriteProgramPromptProfile
   }
   
   export interface RewriteProgramPromptProfile {
     // ... existing fields ...
     presets?: ConfigurationPreset[];
     active_preset_id?: string | null;
   }
   ```

2. **Add preset management UI**:
   - New component: `PresetManager.tsx` (similar to `ProgramsModal.tsx`)
   - Add preset dropdown to settings tabs
   - Show inheritance indicators next to each setting
   - Add [New Preset], [Edit Preset], [Delete Preset] buttons

3. **Update settings components**:
   - `PromptSettings.tsx`: Add preset selector at top
   - `AudioSettings.tsx`: Add preset selector at top
   - `UiSettings.tsx`: Add preset selector at top
   - Each should show which values come from preset vs profile vs default

**Data Migration**:
```typescript
// In app/src/lib/tauri.ts - settings loading
function migrateProfilePresets(profile: any): RewriteProgramPromptProfile {
  // Existing profiles don't have presets, add empty array
  return {
    ...profile,
    presets: profile.presets ?? [],
    active_preset_id: profile.active_preset_id ?? null,
  };
}
```

**Testing**:
- Unit tests for preset resolution logic
- UI tests for creating/editing/deleting presets
- Integration test: verify correct settings used during transcription with preset active

### Phase 2: Enhanced UX (Polish)

**Goals**:
- Make preset switching more accessible
- Add visual feedback for active presets
- Improve discoverability

**Features**:

1. **System Tray Preset Menu**:
   - Show active profile and preset in tray
   - Add submenu: "Presets" with list of available presets for active profile
   - Quick-toggle presets from tray

2. **Preset Quick-Switcher** (like IDE command palette):
   - Hotkey opens overlay with preset list
   - Fuzzy search presets by name
   - Shows current active preset
   - Quick switch without opening Settings

3. **Visual Indicators**:
   - Show preset icon/badge in overlay when preset is active
   - Add subtle color coding or badge to indicate preset mode
   - Toast notification when preset auto-activates

4. **Preset Templates**:
   - Ship with some built-in preset templates:
     - "Budget mode" (cheaper models)
     - "High quality" (best models, extended thinking)
     - "Fast" (fastest models, no LLM rewrite)
     - "Code dictation" (code-optimized prompts)
   - One-click "Create from template" option

### Phase 3: Advanced Features (Future)

1. **Profile-Scoped Presets** (Already planned in Phase 1, but emphasizing here):
   - Presets are tied to their parent profile
   - Can't accidentally apply VS Code preset to Chrome profile
   - Cleaner mental model

2. **Global Presets** (Optional complexity):
   - Some presets could be marked as "global" and available across all profiles
   - Use case: "Budget mode" that works everywhere
   - Requires careful UX to avoid confusion

3. **Preset Chaining/Composition** (Advanced):
   - Allow presets to inherit from other presets
   - Example: "Budget Code Mode" = "Budget" + "Code Dictation"
   - Adds complexity, probably not worth it initially

4. **Auto-Switching Rules** (Power User):
   - Time-based: "Use Budget mode during evenings"
   - Context-based: "Use Code Dictation preset when certain windows are open"
   - Quota-based: "Switch to Budget mode after $X spent today"
   - Requires additional monitoring and logic

5. **Preset Export/Import**:
   - Allow sharing presets as JSON files
   - Community preset repository
   - Useful for power users and tutorials

---

## Technical Considerations

### 1. Settings Change Events

**Current**: `settings-changed` event emitted on updates, frontend re-queries

**With Presets**: Same pattern, but ensure preset changes also emit events:
```rust
// After setting active_preset_id
app.emit("settings-changed", {});
```

### 2. Storage Size

**Concern**: If users create many presets, `settings.json` could grow large

**Mitigation**:
- Presets only store overrides (None for inherited values)
- Typical preset is <1KB
- Even 50 presets = ~50KB, acceptable
- Monitor in future, could split to separate file if needed

### 3. Backwards Compatibility

**Approach**:
- Existing profiles without `presets` field get empty array via `#[serde(default)]`
- Existing profiles without `active_preset_id` get `None` via `#[serde(default)]`
- No migration script needed, graceful fallback

### 4. UI Complexity

**Challenge**: Three-level hierarchy (Default → Profile → Preset) can confuse users

**Mitigation**:
- Clear visual indicators showing source of each setting
- Inline help text explaining inheritance
- "Reset to profile defaults" button when preset is active
- Progressive disclosure: hide preset UI until user creates first preset

### 5. Race Conditions

**Scenario**: User changes profile while preset is active, then switches programs

**Solution**:
- `active_preset_id` is stored per profile
- Switching programs → profile switch → new profile's active_preset_id used
- No race condition, each profile has independent preset state

### 6. Testing Coverage

**Key Test Scenarios**:
1. Preset with all fields = None (inherits everything)
2. Preset with some overrides (mixed inheritance)
3. Preset with all overrides (no inheritance)
4. Switching presets mid-recording (should apply to next recording)
5. Deleting active preset (should clear `active_preset_id`)
6. Profile with no presets (should work as before)

---

## Alternative Approaches Considered

### Alternative 1: Separate Preset Storage (Not Per-Profile)

**Idea**: Store presets globally, not tied to profiles
```
presets/
  budget-mode.json
  detailed-notes.json
profiles/
  vs-code.json (references preset IDs)
```

**Pros**:
- Could reuse presets across profiles
- Easier to share presets

**Cons**:
- Complex reference management
- What if preset deleted but profile still references it?
- Mental model less clear
- Inheritance becomes more complex

**Decision**: ❌ Not recommended. Per-profile presets are simpler and match mental model.

### Alternative 2: Snapshot/Checkpoint System

**Idea**: Instead of presets, save full snapshots of settings
```
profile.snapshots = [
  { timestamp, name, full_settings_copy }
]
```

**Pros**:
- No inheritance logic needed
- Easy to implement

**Cons**:
- Large storage size (full copy per snapshot)
- No inheritance = redundancy and harder to update
- Doesn't solve temporary override use case well

**Decision**: ❌ Not recommended. Doesn't align with override/inheritance pattern.

### Alternative 3: Tag-Based Configuration

**Idea**: Tag settings with contexts, apply multiple tags
```
settings:
  - { key: "llm_model", value: "gpt-4", tags: ["quality", "default"] }
  - { key: "llm_model", value: "gpt-3.5", tags: ["budget"] }
active_tags: ["budget"]
```

**Pros**:
- Very flexible
- Could combine multiple "aspects" of config

**Cons**:
- Complex resolution when tags conflict
- Harder to understand than preset hierarchy
- Overkill for the use case

**Decision**: ❌ Not recommended. Too complex for the problem.

---

## Migration Path for Users

### From Current System to Presets

**No breaking changes required!**

1. Existing profiles continue to work exactly as before
2. `presets` field defaults to empty array
3. UI shows "No presets yet" state with "Create your first preset" call-to-action
4. Users can adopt presets gradually

**Recommended First-Run Experience**:

After updating to version with presets:
1. No automatic migration (presets are opt-in)
2. Settings UI shows inline tip: "New: Save configuration presets for quick switching"
3. Help docs updated with preset examples
4. Changelog highlights preset feature with use cases

---

## Success Metrics

**Phase 1 (MVP) Success Criteria**:
- [ ] Users can create, edit, delete presets per profile
- [ ] Users can activate presets and see settings change
- [ ] Inheritance is clearly indicated in UI
- [ ] No regressions in existing profile functionality
- [ ] Settings changes are properly persisted

**Phase 2 (Enhanced UX) Success Criteria**:
- [ ] Users can switch presets from system tray
- [ ] Quick-switcher reduces preset switching to <3 seconds
- [ ] Visual indicators make active preset obvious
- [ ] Preset templates reduce new preset creation time

**User Satisfaction Metrics** (future):
- Survey: "How often do you use presets?"
- Survey: "Has the preset system improved your workflow?"
- Telemetry: Preset creation/switch frequency (if opt-in telemetry added)

---

## Open Questions

1. **Naming**: Should we stick with "Preset" or consider "Config", "Mode", "Variant"?
   - **Recommendation**: "Preset" in UI, "ConfigurationPreset" in code

2. **Scope**: Should presets be per-profile or global?
   - **Recommendation**: Per-profile (Phase 1), consider global presets in Phase 3

3. **Discoverability**: How do we ensure users know presets exist?
   - **Recommendation**: Inline tips, changelog, help docs, "Create first preset" prompts

4. **Quick Access**: What's the fastest way to switch presets?
   - **Recommendation**: System tray menu (Phase 2) and hotkey-activated quick-switcher

5. **Deletion Safety**: What happens if user deletes active preset?
   - **Recommendation**: Clear `active_preset_id`, show toast "Preset deleted, using profile settings"

6. **Export/Import**: Should users be able to share presets?
   - **Recommendation**: Phase 3 feature, not critical for MVP

---

## Related Documentation

- **Current Profile System**: `app/src-tauri/src/settings.rs`, `app/src-tauri/src/pipeline.rs`
- **Settings Architecture**: `app/src-tauri/src/commands/config.rs`, `app/src/lib/tauri.ts`
- **UI Components**: `app/src/components/settings/ProgramsModal.tsx`, `app/src/components/settings/PromptSettings.tsx`

---

## Appendix A: Example Use Cases

### Use Case 1: Writer with Multiple Writing Styles

**Setup**:
- Profile: "Google Docs"
- Presets:
  - "Casual Blog" - Informal tone, brief cleanup, fast models
  - "Technical Article" - Formal tone, detailed cleanup, extended thinking
  - "Social Media" - Very brief, hashtag-aware, fast

**Workflow**:
1. Opens Google Docs
2. Switches to "Casual Blog" preset from tray
3. Dictates blog post with appropriate tone
4. Next day, writes technical article
5. Switches to "Technical Article" preset
6. Gets higher-quality, more formal output

### Use Case 2: Developer Managing Costs

**Setup**:
- Profile: "VS Code"
- Presets:
  - "Default" - GPT-4 for quality
  - "Budget" - GPT-3.5-turbo for cost savings
  - "Local" - Ollama for offline/free

**Workflow**:
1. Normally uses Default (GPT-4)
2. Near end of month, checks costs
3. Switches to "Budget" preset to save money
4. Still gets decent transcriptions at lower cost
5. Next month, switches back to Default

### Use Case 3: Multilingual User

**Setup**:
- Profile: "Slack"
- Presets:
  - "English" - English STT model, English prompts
  - "Spanish" - Spanish STT model, Spanish prompts
  - "French" - French STT model, French prompts

**Workflow**:
1. Joins English-speaking channel
2. Switches to "English" preset
3. Dictates message in English
4. Switches to Spanish-speaking channel
5. Switches to "Spanish" preset
6. Dictates message in Spanish

### Use Case 4: Testing New Models

**Setup**:
- Profile: "Default"
- Presets:
  - "Stable" - Known-good models
  - "Experimental" - Newest models for testing

**Workflow**:
1. Normally uses "Stable"
2. New GPT version released
3. Creates "Experimental" preset with new model
4. Switches to test new model
5. If good, updates "Stable" preset
6. If bad, switches back to "Stable" without losing settings

---

## Appendix B: Code References

### Key Files to Modify (Phase 1)

**Backend**:
1. `app/src-tauri/src/settings.rs`
   - Add `ConfigurationPreset` struct
   - Update `RewriteProgramPromptProfile` with presets fields

2. `app/src-tauri/src/pipeline.rs`
   - Update `select_profile_for_foreground_app` to apply presets
   - Add `apply_preset_to_profile` helper

3. `app/src-tauri/src/commands/config.rs`
   - Add preset management commands

**Frontend**:
1. `app/src/lib/tauri.ts`
   - Add `ConfigurationPreset` interface
   - Add preset management functions
   - Update `updateRewriteProgramPromptProfiles`

2. `app/src/lib/queries.ts`
   - Add preset-related React Query hooks

3. `app/src/components/settings/ProgramsModal.tsx`
   - Add preset list display (optional, might be separate component)

4. `app/src/components/settings/PromptSettings.tsx`
   - Add preset selector dropdown
   - Show inheritance indicators

5. New: `app/src/components/settings/PresetManager.tsx`
   - Preset CRUD UI

### Estimated Implementation Effort

**Phase 1 (MVP)**:
- Backend changes: 2-3 days
  - Data structures: 4 hours
  - Pipeline logic: 1 day
  - Commands: 4 hours
  - Testing: 4 hours

- Frontend changes: 3-4 days
  - TypeScript types: 2 hours
  - Preset manager component: 1.5 days
  - Settings integration: 1 day
  - UI polish: 0.5 days
  - Testing: 4 hours

**Total Phase 1**: ~5-7 days

**Phase 2 (Enhanced UX)**: ~3-5 days additional

**Phase 3 (Advanced)**: ~5-10 days additional (depending on features)

---

## Conclusion

The **Configuration Preset** system provides a clean, user-friendly way to manage multiple configuration sets per profile without duplicating profiles or manually changing settings. It builds naturally on top of the existing profile system, maintains backward compatibility, and can be implemented incrementally.

**Recommended next steps**:
1. Review this document with stakeholders
2. Validate terminology and UX mockups with potential users
3. Begin Phase 1 implementation
4. Gather feedback after Phase 1 before proceeding to Phase 2

**Key design principles**:
- ✅ Builds on existing profile system
- ✅ Maintains backward compatibility
- ✅ Clear inheritance hierarchy
- ✅ Progressive disclosure (advanced users benefit, basic users unaffected)
- ✅ Minimal storage overhead
- ✅ Incremental implementation path
