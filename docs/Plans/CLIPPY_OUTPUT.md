```
warning: this `impl` can be derived
    --> src\audio_capture.rs:1090:1
     |
1090 | / impl Default for VadAutoStopConfig {
1091 | |     fn default() -> Self {
1092 | |         Self {
1093 | |             enabled: false,
...    |
1098 | | }
     | |_^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
     = note: `#[warn(clippy::derivable_impls)]` on by default
help: replace the manual implementation with a derive attribute
     |
1080 + #[derive(Default)]
1081 | pub struct VadAutoStopConfig {
     |

warning: this function has too many arguments (14/7)
    --> src\audio_capture.rs:1642:1
     |
1642 | / fn run_capture_thread(
1643 | |     device: cpal::Device,
1644 | |     config: cpal::StreamConfig,
1645 | |     sample_format: SampleFormat,
...    |
1656 | |     desired_device_name: Arc<StdMutex<Option<String>>>,
1657 | | ) -> Result<(), AudioCaptureError> {
     | |__________________________________^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#too_many_arguments
     = note: `#[warn(clippy::too_many_arguments)]` on by default

warning: redundant redefinition of a binding `config`
    --> src\audio_capture.rs:1663:5
     |
1663 |     let config = config;
     |     ^^^^^^^^^^^^^^^^^^^^
     |
help: `config` is initially defined here
    --> src\audio_capture.rs:1644:5
     |
1644 |     config: cpal::StreamConfig,
     |     ^^^^^^
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#redundant_locals
     = note: `#[warn(clippy::redundant_locals)]` on by default

warning: this function has too many arguments (14/7)
   --> src\commands\llm.rs:667:1
    |
667 | / pub async fn iterate_rewrite_prompt(
668 | |     app: AppHandle,
669 | |     pipeline: State<'_, SharedPipeline>,
670 | |     transcript: String,
...   |
683 | |     anthropic_thinking_budget: Option<i64>,
684 | | ) -> Result<IterateRewritePromptResponse, LlmCommandError> {
    | |__________________________________________________________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#too_many_arguments

warning: manually reimplementing `div_ceil`
  --> src\commands\network.rs:72:37
   |
72 |     let mut buf: Vec<u16> = vec![0; (data_len as usize + 1) / 2];
   |                                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: consider using `.div_ceil()`: `(data_len as usize).div_ceil(2)`
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_div_ceil
   = note: `#[warn(clippy::manual_div_ceil)]` on by default

warning: unneeded `return` statement
   --> src\commands\overlay.rs:474:9
    |
474 |         return show_overlay_with_reset_if_not_always(&app);
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_return
    = note: `#[warn(clippy::needless_return)]` on by default
help: remove `return`
    |
474 -         return show_overlay_with_reset_if_not_always(&app);
474 +         show_overlay_with_reset_if_not_always(&app)
    |

warning: the use of negated comparison operators on partially ordered types produces code that is hard to read and refactor, please consider using the `partial_cmp` method instead, to make it clear that the two values could be incomparable
   --> src\commands\recording.rs:108:16
    |
108 |             if !(value > 0.0) {
    |                ^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#neg_cmp_op_on_partial_ord
    = note: `#[warn(clippy::neg_cmp_op_on_partial_ord)]` on by default

warning: the use of negated comparison operators on partially ordered types produces code that is hard to read and refactor, please consider using the `partial_cmp` method instead, to make it clear that the two values could be incomparable
   --> src\commands\recording.rs:126:24
    |
126 |                     if !(hours > 0.0) {
    |                        ^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#neg_cmp_op_on_partial_ord

warning: unneeded `return` statement
   --> src\commands\recording.rs:154:9
    |
154 | /         return app
155 | |             .store("settings.json")
156 | |             .ok()
157 | |             .and_then(|store| store.get("transcription_retention_delete_recordings"))
158 | |             .and_then(|v| v.as_bool())
159 | |             .unwrap_or(false);
    | |_____________________________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_return
help: remove `return`
    |
154 ~         app
155 +             .store("settings.json")
156 +             .ok()
157 +             .and_then(|store| store.get("transcription_retention_delete_recordings"))
158 +             .and_then(|v| v.as_bool())
159 ~             .unwrap_or(false)
    |

warning: unneeded `return` statement
   --> src\commands\recording.rs:171:9
    |
171 | /         return app
172 | |             .store("settings.json")
173 | |             .ok()
174 | |             .and_then(|store| store.get("transcription_retention_days"))
175 | |             .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
176 | |             .unwrap_or(0u64);
    | |____________________________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_return
help: remove `return`
    |
171 ~         app
172 +             .store("settings.json")
173 +             .ok()
174 +             .and_then(|store| store.get("transcription_retention_days"))
175 +             .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
176 ~             .unwrap_or(0u64)
    |

warning: unneeded `return` statement
    --> src\commands\recording.rs:1355:9
     |
1355 |         return Ok(());
     |         ^^^^^^^^^^^^^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_return
help: remove `return`
     |
1355 -         return Ok(());
1355 +         Ok(())
     |

warning: field assignment outside of initializer for an instance created with Default::default()
    --> src\commands\recording.rs:1437:5
     |
1437 |     new_config.stt_provider = config.stt_provider.unwrap_or_else(|| "groq".to_string());
     |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     |
note: consider initializing the variable with `pipeline::PipelineConfig { stt_provider: config.stt_provider.unwrap_or_else(|| "groq".to_string()), stt_api_key: config.stt_api_key.unwrap_or_default(), stt_api_keys: HashMap::new(), stt_model: config.stt_model, max_duration_secs: config.max_duration_secs.unwrap_or(300.0), retry_config: retry_config, vad_config: vad_config, transcription_timeout: config
              .transcription_timeout_secs
              .map(Duration::from_secs)
              .unwrap_or(Duration::from_secs(60)), max_recording_bytes: config.max_recording_bytes.unwrap_or(50 * 1024 * 1024), llm_config: crate::llm::LlmConfig::default(), llm_api_keys: HashMap::new(), ..Default::default() }` and removing relevant reassignments
    --> src\commands\recording.rs:1436:5
     |
1436 |     let mut new_config = PipelineConfig::default();
     |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#field_reassign_with_default
     = note: `#[warn(clippy::field_reassign_with_default)]` on by default

warning: redundant closure
   --> src\commands\settings.rs:116:22
    |
116 |             .or_else(|| default_fn()),
    |                      ^^^^^^^^^^^^^^^ help: replace the closure with the function itself: `default_fn`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#redundant_closure
    = note: `#[warn(clippy::redundant_closure)]` on by default

warning: redundant closure
   --> src\commands\settings.rs:163:26
    |
163 |                 .or_else(|| HotkeyConfig::default_quick_ask()),
    |                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: replace the closure with the associated function itself: `HotkeyConfig::default_quick_ask`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#redundant_closure

warning: this manual char comparison can be written more succinctly
  --> src\cost\fireworks.rs:41:32
   |
41 |     for part in model_id.split(|c: char| c == '-' || c == '_' || c == '/' || c == '.') {
   |                                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: consider using an array of `char`: `['-', '_', '/', '.']`
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_pattern_char_comparison
   = note: `#[warn(clippy::manual_pattern_char_comparison)]` on by default

warning: this `if` has identical blocks
  --> src\cost\fireworks.rs:83:58
   |
83 |       } else if m.contains("deepseek") && m.contains("r1") {
   |  __________________________________________________________^
84 | |         // "DeepSeek R1 0528" on the pricing page.
85 | |         Some((1_350_000u64, 5_400_000u64))
86 | |     } else if m.contains("deepseek") && m.contains("reason") {
   | |_____^
   |
note: same as this
  --> src\cost\fireworks.rs:86:62
   |
86 |       } else if m.contains("deepseek") && m.contains("reason") {
   |  ______________________________________________________________^
87 | |         // "DeepSeek R1 0528" on the pricing page.
88 | |         Some((1_350_000u64, 5_400_000u64))
89 | |     } else if m.contains("glm-4.5") || m.contains("glm-4.6") {
   | |_____^
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#if_same_then_else
   = note: `#[warn(clippy::if_same_then_else)]` on by default

warning: this `if` has identical blocks
   --> src\cost\fireworks.rs:105:59
    |
105 |       } else if m.contains("minimax") && m.contains("m2.1") {
    |  ___________________________________________________________^
106 | |         Some((300_000u64, 1_200_000u64))
107 | |     } else if m.contains("minimax") && m.contains("m2") {
    | |_____^
    |
note: same as this
   --> src\cost\fireworks.rs:107:57
    |
107 |       } else if m.contains("minimax") && m.contains("m2") {
    |  _________________________________________________________^
108 | |         Some((300_000u64, 1_200_000u64))
109 | |     } else {
    | |_____^
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#if_same_then_else

warning: calls to `push` immediately after creation
  --> src\embeddings\cohere.rs:39:5
   |
39 | /     let mut inputs: Vec<String> = Vec::with_capacity(1);
40 | |     inputs.push(input.to_string());
   | |___________________________________^ help: consider using the `vec![]` macro: `let inputs: Vec<String> = vec![..];`
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#vec_init_then_push
   = note: `#[warn(clippy::vec_init_then_push)]` on by default

warning: calling `push_str()` using a single-character string literal
   --> src\embeddings\cohere.rs:192:9
    |
192 |         preview.push_str("…");
    |         ^^^^^^^^^^^^^^^^^^^^^ help: consider using `push` with a character literal: `preview.push('…')`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#single_char_add_str
    = note: `#[warn(clippy::single_char_add_str)]` on by default

warning: calling `push_str()` using a single-character string literal
   --> src\embeddings\fireworks.rs:109:9
    |
109 |         preview.push_str("…");
    |         ^^^^^^^^^^^^^^^^^^^^^ help: consider using `push` with a character literal: `preview.push('…')`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#single_char_add_str

warning: calling `push_str()` using a single-character string literal
   --> src\embeddings\openai.rs:104:9
    |
104 |         preview.push_str("…");
    |         ^^^^^^^^^^^^^^^^^^^^^ help: consider using `push` with a character literal: `preview.push('…')`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#single_char_add_str

warning: this `impl` can be derived
  --> src\history.rs:33:1
   |
33 | / impl Default for HistoryStatus {
34 | |     fn default() -> Self {
35 | |         // Existing history.json entries (pre-status) should be treated as success.
36 | |         HistoryStatus::Success
37 | |     }
38 | | }
   | |_^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
help: replace the manual implementation with a derive attribute and mark the default variant
   |
27 + #[derive(Default)]
28 | pub enum HistoryStatus {
29 |     InProgress,
30 ~     #[default]
31 ~     Success,
   |

warning: call to `reserve` immediately after creation
   --> src\history.rs:645:9
    |
645 | /         let mut filtered: Vec<&HistoryEntry> = Vec::new();
646 | |         filtered.reserve(entries.len().min(2048));
    | |__________________________________________________^ help: consider using `Vec::with_capacity(/* Space hint */)`: `let mut filtered: Vec<&HistoryEntry> = Vec::with_capacity(entries.len().min(2048));`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#reserve_after_initialization
    = note: `#[warn(clippy::reserve_after_initialization)]` on by default

warning: manually reimplementing `div_ceil`
   --> src\history.rs:654:27
    |
654 |         let total_pages = ((total_filtered + page_size - 1) / page_size).max(1);
    |                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: consider using `.div_ceil()`: `total_filtered.div_ceil(page_size)`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_div_ceil

warning: calling `push_str()` using a single-character string literal
   --> src\llm\cerebras.rs:243:29
    |
243 | ...                   out.push_str("\n");
    |                       ^^^^^^^^^^^^^^^^^^ help: consider using `push` with a character literal: `out.push('\n')`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#single_char_add_str

warning: this `impl` can be derived
  --> src\llm\prompts.rs:84:1
   |
84 | / impl Default for PromptSections {
85 | |     fn default() -> Self {
86 | |         Self {
87 | |             system_custom: None,
...  |
90 | | }
   | |_^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
help: replace the manual implementation with a derive attribute
   |
79 + #[derive(Default)]
80 | pub struct PromptSections {
   |

warning: the following explicit lifetimes could be elided: 'a
   --> src\pipeline.rs:152:28
    |
152 | fn select_effective_preset<'a>(
    |                            ^^
153 |     profile: &'a crate::llm::ProgramPromptProfile,
    |               ^^
154 | ) -> Option<&'a crate::llm::ProgramPreset> {
    |              ^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_lifetimes
    = note: `#[warn(clippy::needless_lifetimes)]` on by default
help: elide the lifetimes
    |
152 ~ fn select_effective_preset(
153 ~     profile: &crate::llm::ProgramPromptProfile,
154 ~ ) -> Option<&crate::llm::ProgramPreset> {
    |

warning: calling `push_str()` using a single-character string literal
   --> src\pipeline.rs:186:9
    |
186 |         preview.push_str("…");
    |         ^^^^^^^^^^^^^^^^^^^^^ help: consider using `push` with a character literal: `preview.push('…')`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#single_char_add_str

warning: this function has too many arguments (8/7)
   --> src\pipeline.rs:287:5
    |
287 | /     fn push_call(
288 | |         call_id: &mut u64,
289 | |         calls_request: &mut Vec<JsonValue>,
290 | |         calls_response: &mut Vec<JsonValue>,
...   |
295 | |         response: JsonValue,
296 | |     ) {
    | |_____^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#too_many_arguments

warning: this function has too many arguments (9/7)
   --> src\pipeline.rs:314:5
    |
314 | /     fn build_router_payloads(
315 | |         embedding_provider: &str,
316 | |         embedding_model: &str,
317 | |         pick_highest_score: bool,
...   |
323 | |         calls_response: &Vec<JsonValue>,
324 | |     ) -> (JsonValue, JsonValue) {
    | |_______________________________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#too_many_arguments

warning: this function has too many arguments (9/7)
    --> src\pipeline.rs:1614:5
     |
1614 | /     fn get_or_create_llm_provider(
1615 | |         &mut self,
1616 | |         provider_id: &str,
1617 | |         model: Option<String>,
...    |
1623 | |         anthropic_thinking_budget: Option<i64>,
1624 | |     ) -> Result<Arc<dyn LlmProvider>, PipelineError> {
     | |____________________________________________________^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#too_many_arguments

warning: this `if` statement can be collapsed
    --> src\pipeline.rs:2724:21
     |
2724 | /                     if profile_ok {
2725 | |                         if find_preset_by_id(profile, lock.preset_id.as_str()).is_some() {
2726 | |                             routed_preset_id = Some(lock.preset_id.clone());
2727 | |                         }
2728 | |                     }
     | |_____________________^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
     = note: `#[warn(clippy::collapsible_if)]` on by default
help: collapse nested if block
     |
2724 ~                     if profile_ok
2725 ~                         && find_preset_by_id(profile, lock.preset_id.as_str()).is_some() {
2726 |                             routed_preset_id = Some(lock.preset_id.clone());
2727 ~                         }
     |

warning: this `if` statement can be collapsed
    --> src\pipeline.rs:3667:21
     |
3667 | /                     if profile_ok {
3668 | |                         if find_preset_by_id(profile, lock.preset_id.as_str()).is_some() {
3669 | |                             routed_preset_id = Some(lock.preset_id.clone());
3670 | |                         }
3671 | |                     }
     | |_____________________^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
help: collapse nested if block
     |
3667 ~                     if profile_ok
3668 ~                         && find_preset_by_id(profile, lock.preset_id.as_str()).is_some() {
3669 |                             routed_preset_id = Some(lock.preset_id.clone());
3670 ~                         }
     |

warning: this `impl` can be derived
   --> src\request_log.rs:174:1
    |
174 | / impl Default for RequestKind {
175 | |     fn default() -> Self {
176 | |         Self::Transcription
177 | |     }
178 | | }
    | |_^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
help: replace the manual implementation with a derive attribute and mark the default variant
    |
168 + #[derive(Default)]
169 | pub enum RequestKind {
170 ~     #[default]
171 ~     Transcription,
    |

warning: clamp-like pattern without using clamp function
   --> src\request_log.rs:617:49
    |
617 |             RequestLogsRetentionMode::Amount => retention.amount.max(1).min(HARD_MAX_LOGS),
    |                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: replace with clamp: `retention.amount.clamp(1, HARD_MAX_LOGS)`
    |
    = note: clamp will panic if max < min
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_clamp
    = note: `#[warn(clippy::manual_clamp)]` on by default

warning: this `impl` can be derived
  --> src\settings.rs:22:1
   |
22 | / impl Default for ProxyMode {
23 | |     fn default() -> Self {
24 | |         Self::System
25 | |     }
26 | | }
   | |_^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
help: replace the manual implementation with a derive attribute and mark the default variant
   |
13 + #[derive(Default)]
14 | pub enum ProxyMode {
15 |     /// Force-disable any proxy usage (ignore env/system proxies).
16 |     NoProxy,
17 |     /// Use system defaults (env vars / OS proxy discovery).
18 ~     #[default]
19 ~     System,
   |

warning: this `impl` can be derived
  --> src\settings.rs:66:1
   |
66 | / impl Default for TrustedCaCertFormat {
67 | |     fn default() -> Self {
68 | |         Self::Pem
69 | |     }
70 | | }
   | |_^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
help: replace the manual implementation with a derive attribute and mark the default variant
   |
61 + #[derive(Default)]
62 | pub enum TrustedCaCertFormat {
63 ~     #[default]
64 ~     Pem,
   |

warning: this `impl` can be derived
   --> src\settings.rs:551:1
    |
551 | / impl Default for IntentRouterStrategy {
552 | |     fn default() -> Self {
553 | |         Self::Off
554 | |     }
555 | | }
    | |_^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
help: replace the manual implementation with a derive attribute and mark the default variant
    |
545 + #[derive(Default)]
546 | pub enum IntentRouterStrategy {
547 ~     #[default]
548 ~     Off,
    |

warning: clamp-like pattern without using clamp function
   --> src\state.rs:109:21
    |
109 |             let n = n.max(1).min(20);
    |                     ^^^^^^^^^^^^^^^^ help: replace with clamp: `n.clamp(1, 20)`
    |
    = note: clamp will panic if max < min
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_clamp

warning: unneeded `return` statement
   --> src\stats.rs:119:9
    |
119 | /         return match provider {
120 | |             "cerebras" => crate::get_setting_from_store(app, "cerebras_free_tier", true),
121 | |             "groq" => crate::get_setting_from_store(app, "groq_free_tier", true),
122 | |             "elevenlabs" => crate::get_setting_from_store(app, "elevenlabs_free_tier", true),
...   |
126 | |             _ => false,
127 | |         };
    | |_________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_return
help: remove `return`
    |
119 ~         match provider {
120 +             "cerebras" => crate::get_setting_from_store(app, "cerebras_free_tier", true),
121 +             "groq" => crate::get_setting_from_store(app, "groq_free_tier", true),
122 +             "elevenlabs" => crate::get_setting_from_store(app, "elevenlabs_free_tier", true),
123 +             "cohere" => crate::get_setting_from_store(app, "cohere_free_tier", true),
124 +             "assemblyai" => crate::get_setting_from_store(app, "assemblyai_free_tier", true),
125 +             "speechmatics" => crate::get_setting_from_store(app, "speechmatics_free_tier", true),
126 +             _ => false,
127 ~         }
    |

warning: this expression borrows a value the compiler would automatically borrow
   --> src\stats.rs:301:9
    |
301 |         (&mut *writer).write_all(b"\n").map_err(|e| e.to_string())?;
    |         ^^^^^^^^^^^^^^ help: change this to: `(*writer)`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_borrow
    = note: `#[warn(clippy::needless_borrow)]` on by default

warning: this `if` statement can be collapsed
   --> src\stats.rs:835:9
    |
835 | /         if inputs.stt_provider == "deepgram" {
836 | |             if ev.estimated_cost_usd_micros.is_none() {
837 | |                 if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
838 | |                     if let Some(micros) =
...   |
845 | |         }
    | |_________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
help: collapse nested if block
    |
835 ~         if inputs.stt_provider == "deepgram"
836 ~             && ev.estimated_cost_usd_micros.is_none() {
837 |                 if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
...
843 |                 }
844 ~             }
    |

warning: this `if` statement can be collapsed
   --> src\stats.rs:847:9
    |
847 | /         if inputs.stt_provider == "aquavoice" {
848 | |             if ev.estimated_cost_usd_micros.is_none() {
849 | |                 if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
850 | |                     if let Some(micros) =
...   |
857 | |         }
    | |_________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
help: collapse nested if block
    |
847 ~         if inputs.stt_provider == "aquavoice"
848 ~             && ev.estimated_cost_usd_micros.is_none() {
849 |                 if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
...
855 |                 }
856 ~             }
    |

warning: this `if` statement can be collapsed
   --> src\stats.rs:859:9
    |
859 | /         if inputs.stt_provider == "assemblyai" {
860 | |             if ev.estimated_cost_usd_micros.is_none() {
861 | |                 if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
862 | |                     if let Some(micros) =
...   |
869 | |         }
    | |_________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
help: collapse nested if block
    |
859 ~         if inputs.stt_provider == "assemblyai"
860 ~             && ev.estimated_cost_usd_micros.is_none() {
861 |                 if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
...
867 |                 }
868 ~             }
    |

warning: this `if` statement can be collapsed
   --> src\stats.rs:885:9
    |
885 | /         if inputs.stt_provider == "fireworks" {
886 | |             if ev.estimated_cost_usd_micros.is_none() {
887 | |                 if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
888 | |                     if let Some(micros) =
...   |
895 | |         }
    | |_________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
help: collapse nested if block
    |
885 ~         if inputs.stt_provider == "fireworks"
886 ~             && ev.estimated_cost_usd_micros.is_none() {
887 |                 if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
...
893 |                 }
894 ~             }
    |

warning: this `impl` can be derived
  --> src\vad.rs:34:1
   |
34 | / impl Default for VadAggressiveness {
35 | |     fn default() -> Self {
36 | |         VadAggressiveness::Aggressive
37 | |     }
38 | | }
   | |_^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
help: replace the manual implementation with a derive attribute and mark the default variant
   |
12 + #[derive(Default)]
13 | pub enum VadAggressiveness {
14 |     /// Quality mode - less aggressive, fewer false negatives
...
18 |     /// Aggressive mode
19 ~     #[default]
20 ~     Aggressive,
   |

warning: clamp-like pattern without using clamp function
   --> src\vad.rs:282:29
    |
282 |     let chunk_size_frames = input_len_frames.min(1024).max(1);
    |                             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: replace with clamp: `input_len_frames.clamp(1, 1024)`
    |
    = note: clamp will panic if max < min
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_clamp

warning: this `if` statement can be collapsed
   --> src\windows_modifier_hotkeys.rs:437:13
    |
437 | /             if hotkey_debug_enabled(app) {
438 | |                 if vk == VK_MENU || vk == VK_RMENU {
439 | |                     let flags: u32 = kb.flags.0;
440 | |                     let kind = if is_down { "down" } else { "up" };
...   |
455 | |             }
    | |_____________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
help: collapse nested if block
    |
437 ~             if hotkey_debug_enabled(app)
438 ~                 && (vk == VK_MENU || vk == VK_RMENU) {
439 |                     let flags: u32 = kb.flags.0;
...
453 |                     );
454 ~                 }
    |

warning: this `if` statement can be collapsed
   --> src\windows_modifier_hotkeys.rs:460:9
    |
460 | /         if is_down && ALT_RIGHT_HELD.load(Ordering::Relaxed) {
461 | |             if !is_modifier_vk(vk) {
462 | |                 ALT_RIGHT_USED_WITH_OTHER_KEY.store(true, Ordering::Relaxed);
463 | |             }
464 | |         }
    | |_________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
help: collapse nested if block
    |
460 ~         if is_down && ALT_RIGHT_HELD.load(Ordering::Relaxed)
461 ~             && !is_modifier_vk(vk) {
462 |                 ALT_RIGHT_USED_WITH_OTHER_KEY.store(true, Ordering::Relaxed);
463 ~             }
    |

warning: redundant closure
   --> src\lib.rs:132:22
    |
132 |             .or_else(|| default_fn()),
    |                      ^^^^^^^^^^^^^^^ help: replace the closure with the function itself: `default_fn`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#redundant_closure

warning: this `let...else` may be rewritten with the `?` operator
   --> src\lib.rs:520:5
    |
520 | /     let Some(history) = app.try_state::<HistoryStorage>() else {
521 | |         return None;
522 | |     };
    | |______^ help: replace it with: `let history = app.try_state::<HistoryStorage>()?;`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#question_mark
    = note: `#[warn(clippy::question_mark)]` on by default

warning: this `let...else` may be rewritten with the `?` operator
   --> src\lib.rs:524:5
    |
524 | /     let Some(store) = app.try_state::<RecordingStore>() else {
525 | |         return None;
526 | |     };
    | |______^ help: replace it with: `let store = app.try_state::<RecordingStore>()?;`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#question_mark

warning: this boolean expression can be simplified
    --> src\lib.rs:1489:51
     |
1489 |                           let should_complete_now = !is_quick_ask_session
     |  ___________________________________________________^
1490 | |                             && !(quick_replace_cfg.enabled && quick_replace_epoch != 0);
     | |_______________________________________________________________________________________^ help: try: `!(is_quick_ask_session || quick_replace_cfg.enabled && quick_replace_epoch != 0)`
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#nonminimal_bool
     = note: `#[warn(clippy::nonminimal_bool)]` on by default

warning: clamp-like pattern without using clamp function
    --> src\lib.rs:1741:37
     |
1741 | ...                   (quick_ask_conversation_history_count_raw.max(1).min(20))
     |                       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: replace with clamp: `quick_ask_conversation_history_count_raw.clamp(1, 20)`
     |
     = note: clamp will panic if max < min
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_clamp

warning: this `if let` can be collapsed into the outer `if let`
    --> src\lib.rs:2108:49
     |
2108 | / ...                   if let serde_json::Value::Object(map) = req {
2109 | | ...                       map.insert(
2110 | | ...                           "context_present".to_string(),
2111 | | ...                           serde_json::Value::Bool(
...    |
2144 | | ...                       );
2145 | | ...                   }
     | |_______________________^
     |
help: the outer pattern can be modified to include the inner pattern
    --> src\lib.rs:2106:57
     |
2106 | ...                   if let Some(req) = log.quick_ask_request_json.as_mut() {
     |                                   ^^^ replace this binding
2107 | ...                       // If it's our JSON object, add a couple extra fields.
2108 | ...                       if let serde_json::Value::Object(map) = req {
     |                                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ with this pattern
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_match
     = note: `#[warn(clippy::collapsible_match)]` on by default

warning: returning the result of a `let` binding from a block
    --> src\lib.rs:2280:45
     |
2273 | / ...                   let x = match lock_result {
2274 | | ...                       Ok(probe) if probe.epoch == quick_replace_epoch => {
2275 | | ...                           (probe.ready, probe.selection_text.clone())
...    |
2278 | | ...                   };
     | |________________________- unnecessary `let` binding
2279 |   ...
2280 |   ...                   x
     |                         ^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#let_and_return
     = note: `#[warn(clippy::let_and_return)]` on by default
help: return the expression directly
     |
2273 ~
2274 |
2275 ~                                             match lock_result {
2276 +                                                 Ok(probe) if probe.epoch == quick_replace_epoch => {
2277 +                                                     (probe.ready, probe.selection_text.clone())
2278 +                                                 }
2279 +                                                 _ => (true, None),
2280 +                                             }
     |

warning: this pattern creates a reference to a reference
    --> src\lib.rs:1614:34
     |
1614 |                     if let (Some(ref req_id), Some(store)) =
     |                                  ^^^^^^^^^^ help: try: `req_id`
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_borrow

warning: this pattern creates a reference to a reference
    --> src\lib.rs:2850:34
     |
2850 |                     if let (Some(ref req_id), Some(store)) =
     |                                  ^^^^^^^^^^ help: try: `req_id`
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_borrow

warning: redundant closure
    --> src\lib.rs:3138:26
     |
3138 |                 .or_else(|| HotkeyConfig::default_quick_ask()),
     |                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: replace the closure with the associated function itself: `HotkeyConfig::default_quick_ask`
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#redundant_closure

warning: clamp-like pattern without using clamp function
    --> src\lib.rs:3931:29
     |
3931 |                     amount: amount.max(1).min(200) as usize,
     |                             ^^^^^^^^^^^^^^^^^^^^^^ help: replace with clamp: `amount.clamp(1, 200)`
     |
     = note: clamp will panic if max < min
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_clamp

warning: unneeded `return` statement
    --> src\lib.rs:4765:9
     |
4765 |         return;
     |         ^^^^^^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_return
help: remove `return`
     |
4763 -         }
4764 -
4765 -         return;
4763 +         }
     |

warning: redundant closure
    --> src\lib.rs:4289:26
     |
4289 |                 .or_else(|| HotkeyConfig::default_quick_ask()),
     |                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: replace the closure with the associated function itself: `HotkeyConfig::default_quick_ask`
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#redundant_closure

warning: this `else { if .. }` block can be collapsed
    --> src\lib.rs:4513:16
     |
4513 |           } else {
     |  ________________^
4514 | |             if state.ptt_key_held.swap(false, Ordering::SeqCst) {
4515 | |                 let is_recording = app
4516 | |                     .try_state::<pipeline::SharedPipeline>()
...    |
4531 | |         }
     | |_________^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_else_if
     = note: `#[warn(clippy::collapsible_else_if)]` on by default
help: collapse nested if block
     |
4513 ~         } else if state.ptt_key_held.swap(false, Ordering::SeqCst) {
4514 +             let is_recording = app
4515 +                 .try_state::<pipeline::SharedPipeline>()
4516 +                 .map(|p| p.state() == pipeline::PipelineState::Recording)
4517 +                 .unwrap_or(false);
4518 +             if is_recording {
4519 +                 stop_recording(
4520 +                     app,
4521 +                     &state,
4522 +                     sound_enabled,
4523 +                     audio_cue,
4524 +                     &audio_mute_manager,
4525 +                     playing_audio_handling,
4526 +                     &hold_label,
4527 +                 );
4528 +             }
4529 +         }
     |

warning: this `else { if .. }` block can be collapsed
    --> src\lib.rs:4655:16
     |
4655 |           } else {
     |  ________________^
4656 | |             if state.quick_ask_key_held.swap(false, Ordering::SeqCst) {
4657 | |                 let is_recording = app
4658 | |                     .try_state::<pipeline::SharedPipeline>()
...    |
4677 | |         }
     | |_________^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_else_if
help: collapse nested if block
     |
4655 ~         } else if state.quick_ask_key_held.swap(false, Ordering::SeqCst) {
4656 +             let is_recording = app
4657 +                 .try_state::<pipeline::SharedPipeline>()
4658 +                 .map(|p| p.state() == pipeline::PipelineState::Recording)
4659 +                 .unwrap_or(false);
4660 +             if is_recording {
4661 +                 stop_recording(
4662 +                     app,
4663 +                     &state,
4664 +                     sound_enabled,
4665 +                     audio_cue,
4666 +                     &audio_mute_manager,
4667 +                     playing_audio_handling,
4668 +                     &quick_ask_hold_label,
4669 +                 );
4670 +             } else {
4671 +                 state
4672 +                     .quick_ask_session_active
4673 +                     .store(false, Ordering::SeqCst);
4674 +             }
4675 +         }
     |

warning: use of `unwrap_or_else` to construct default value
    --> src\lib.rs:5158:10
     |
5158 |         .unwrap_or_else(llm::PromptSections::default);
     |          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: try: `unwrap_or_default()`
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#unwrap_or_default
     = note: `#[warn(clippy::unwrap_or_default)]` on by default

warning: redundant closure
    --> src\lib.rs:5438:26
     |
5438 |                 .or_else(|| HotkeyConfig::default_quick_ask()),
     |                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: replace the closure with the associated function itself: `HotkeyConfig::default_quick_ask`
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#redundant_closure

warning: `kolboo` (lib) generated 66 warnings (run `cargo clippy --fix --lib -p kolboo` to apply 44 suggestions)
warning: `kolboo` (lib test) generated 66 warnings (66 duplicates)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 24s
```
