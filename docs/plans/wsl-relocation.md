# WSL relocation checkpoint

The founder requested moving the entire workspace from C:/work/archy to /home/butahlecoq/work/archy in Ubuntu WSL. Preserve the existing layout: the Git checkout is /home/butahlecoq/work/archy/repository, with sibling .tools and original workspace inputs/notes.

Task01 validation is independently reviewed PASS; code c11157b has passing CI. Task02 massing/evaluation implementation was interrupted for this relocation. Its current files are an unfinished WIP checkpoint, not a completed or reviewed feature. Resume from task-02.md and task-02-implementation.md, inspect current code and run checks before any completion claim. No massing agent should keep writing the Windows source during migration.

Use native WSL paths for subsequent commands: cd /home/butahlecoq/work/archy/repository; CARGO_TARGET_DIR=/home/butahlecoq/work/archy/.tools/cargo-target. Cached build depfiles may refer to the old /mnt/c path and require rebuilding; source and Git history are authoritative. Actual Omarchy execution remains deferred; Ubuntu WSL is development only. Do not modify Windows security settings.
