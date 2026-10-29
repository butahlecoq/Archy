# ADR 0005: bounded review data contract

Status: partial A14 contract, 29 October 2026. This does not record or authorize a human decision.

## Context and decision

The release specification requires explicit human approval and separate agent proposals. A same-UID CLI, actor string, JSON document, private Rust constructor, or terminal confirmation cannot establish the required authority. The A16 broker and deployment design remains incomplete.

`Proposal` and `DecisionRecord` are separate versioned domain data. Both identify a project, source run, variant, revision and head hash, plus input, normalized project, variant, evaluation and comparison digests. Review evidence carries each declared constraint's kind, bound, unit, source and authority, alongside the evaluator's site, constraint and optional hard GFA checks. A selection claim can describe a failed or unknown design; it is not an approval.

`claimed_checks_passed_against` checks the exact normalized, variant and evaluation bytes against their digests, binds their project and variant IDs, compares the projected constraints and checks with those bytes, and checks the inventory and required flags against the current evaluator's normalized input. Only then does it compute the claimed result from the checks; FAIL, WARN and UNKNOWN are nonpassing. This does not recompute the evaluation: a caller can forge and rehash all the inputs. It does not verify input or comparison bytes, the history head, the run's inclusion in that history, or the relationship between original and normalized input. A future trusted integration must verify those links and independently recompute the evaluator before relying on any result.

The generated schemas cover the representable format, identifier, digest, length and basic text restrictions. Serde parsing and schema validation must be followed by `validate`; cross-field checks, the remaining Unicode visibility rules and evidence binding are runtime checks. Text limits count Unicode scalar values, matching JSON Schema character lengths. The visible-text rule rejects whitespace, controls and listed invisible formatting and combining ranges; it is not a full Unicode rendering engine. Constraint source and authority retain the upstream normalization rules rather than acquiring a new length limit.

The decision record contains actor, mechanism label, decision, timestamp and reason. Its fields are **untrusted claims** even after parsing and domain validation. SHA-256 references establish neither that the referenced bytes exist nor who supplied them. A future broker must authenticate the actor, bind the exact reviewed request and independently verify immutable evidence and history before any approved-state projection. No current public operation can write final approval or produce an authorized approval token. Proposals cannot carry actor or decision fields. Local preapproval IFC remains a historical DRAFT artifact; later selection must not rewrite it. External publication needs separate authority.

## Scope and deferred work

This slice adds only the typed contract, schema projections and validation tests. It does not change storage revision formats, persist events, add a CLI `approve` command, expose an MCP final-approval tool, or infer consent from runs and reports. A14's comparison, real decision workflow and A16's protected broker/Omarchy verification remain open. No dependencies were added.
