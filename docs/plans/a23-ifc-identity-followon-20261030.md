# IFC post-publication error identity follow-on

The combined candidate on published 18faf75 reconciles the worker pin; see a23-runtime-18faf-a13-integration-handoff-20261030.md. The original base and evidence below are retained as history.

Base tree is dbca868fcf5da36028271181eb2730162376942c, the prior IFC publication/deadline follow-on. Review identified that post-publication InternalError and other nonretriable errors named the published run and revision but omitted the resolved idempotency key and original expected project revision. If the caller used the default generated key, the resulting identity could not be reconstructed from that error.

Every post-publication error now includes both resolved identity fields. The original code and retryability are retained except for the already-existing treatment of backend unavailable, timeout and revision conflict. In particular, InternalError remains nonretriable: exposing the identity is not a claim that retrying without resolving the underlying problem is safe. Retriable pre-publication/storage errors retain the earlier conditional identity behavior.

The ordinary nonretriable unit test failed against the base and passed after the change. The separate native WSL IFC fixture injected InternalError at the actual post-commit verification point, confirmed the revision had advanced, the returned error remained nonretriable and included its generated key and expected revision, then verified exact-key replay. It also covered orphan recovery with an injected retriable backend failure. This fixture uses the separately reviewed development Python and pinned Node/web-ifc, not an installed Arch runtime; injection is Rust cfg(test) only. Strict offline CLI Clippy passed.

The prior audit, closure and this identity fix overlap IFC verifier/export code; integrate only after review, preserving published 1c292054f9409cd27f7f0d5a2ab42a83a9722441 authority/notice/CI and A12/A18 changes. A13 worker fix 49f89e3bc9611d7ef9fbd27885e1b4b0e8722383 was reported under review, so worker pin reconciliation remains pending. No release, service, target or distribution claim follows.
