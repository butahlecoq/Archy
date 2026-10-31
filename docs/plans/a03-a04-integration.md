# Versioned generation integration

The independently reviewed generation diagnostics follow-up was reconciled
with the historical v1 normalization boundary and applied on `8856407`.
The verifier retains the stored v1 normalization check and chooses the
generator from the recorded algorithm version. The combined A02/A03 source
manifest matched 24 files; the remaining core export file also includes the
independently reviewed A14 decision contract. Both exports were checked.

The isolated merge passed the focused workflow, golden, analytic, generation,
report, formatting, and schema checks. On the combined integration tree,
formatting, strict workspace Clippy, the full locked workspace tests, and
the schema checker passed. Logs are under
`.tools/a03-integration-8856407-logs` in the workspace wrapper.

The captured earlier exhausted run remains readable with its original bytes.
New runs record construction rejections and duplicate attempts under a
versioned diagnostics algorithm. A04's tested invalid-geometry behavior does
not prove a valid required check with missing evidence. Neither result is full
A02, A03, A04, backend, or Omarchy acceptance.
