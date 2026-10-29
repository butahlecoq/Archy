# D1-D2 implementation plan and risks

Implement the supplied task-01 contract in tests first, then strict shared core parsing and normalization, CLI init/validate, schemas, fixtures, CI and ADRs. No commits until independent review.

Interfaces: Project (strict input), NormalizedSpec (canonical SI), parse_project(text, InputFormat), normalize(Project), ArchyError {code,message,field,retriable}. Generic dimension names obey units m/mm/cm/ft; SI-suffixed aliases are accepted only with units=m and cannot coexist with generic names. Width/depth shorthand means origin-zero rectangle; polygon_xy accepts explicit closed translated axis-aligned rectangles. Floor height is required; target GFA is soft unless target_gfa_hard=true. Missing authority is an explicit assumption, never verified law. North defaults to zero. Generation defaults to count=3, seed=42 and rectangle/l_shape.

Risks: duplicate keys must fail before lossy maps; YAML aliases and tags are rejected by parser options if possible; bounded input prevents expansion abuse. Floating-point overflow, tolerance and rectangularity require analytic fixtures. Schemas describe structure; semantic geometry/cross-field rules remain core validation. Existing directories are refused by init; create-new writes prevent overwrite. No generator/worker/MCP behavior.

Cargo is blocked by Windows Application Control (../.tools/toolchain-status.md). Tests will be written before implementation and execution attempted, but no red/green or compilation success will be asserted without an executable toolchain. No fabricated Cargo.lock. Static Python JSON/TOML/schema checks and git whitespace checks will be recorded. The slice remains unverified until Linux CI or another permitted toolchain passes.
