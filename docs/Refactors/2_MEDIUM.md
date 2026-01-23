# Medium-priority refactors

- Replace remaining settings selects that implement the “Default + hint” UI manually with `HintSelectWithDefaultHint`.
	- Goal: reduce repeated `renderSelected`/`renderOption` blocks while keeping behavior explicit.

- Continue extracting truly-shared HTTP/provider plumbing in Rust beyond just `(status, body)` reads.
	- Keep it small (helpers), avoid over-abstracting provider-specific request schemas.

