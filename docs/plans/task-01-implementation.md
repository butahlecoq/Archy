# D1-D2 implementation plan and risks

Implement the supplied task-01 contract in tests first, then strict shared core parsing and normalization, CLI init/validate, schemas, fixtures, CI and ADRs. Commit/push authored work under the founder instruction; independent review remains required before integration.

Interfaces: Project (strict input), NormalizedSpec (canonical SI), parse_project(text, InputFormat), normalize(Project), ArchyError {code,message,field,retriable}. Generic dimension names obey units m/mm/cm/ft; SI-suffixed fields always mean SI regardless of global/constraint units and cannot coexist with generic names. Width/depth shorthand means origin-zero rectangle; polygon_xy accepts explicit closed translated axis-aligned rectangles. Floor height is required; target GFA is soft unless target_gfa_hard=true. Missing authority is an explicit assumption, never verified law. North defaults to zero. Generation defaults to count=3, seed=42 and rectangle/l_shape.

Risks: duplicate keys must fail before lossy maps; YAML aliases and tags are rejected by parser options if possible; bounded input prevents expansion abuse. Floating-point overflow, tolerance and rectangularity require analytic fixtures. Schemas describe structure; semantic geometry/cross-field rules remain core validation. Existing directories are refused by init; create-new writes prevent overwrite. No generator/worker/MCP behavior.

Historical Windows Cargo execution was blocked by Application Control. Ubuntu WSL now provides Rust 1.99.1 without changing Windows security settings. The new regression tests were observed failing before fixes; current executable evidence and limitations are in task-01-report.md. Cargo.lock is resolved, not fabricated. Actual Omarchy execution remains deferred.
