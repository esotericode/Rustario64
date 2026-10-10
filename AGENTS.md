# Agent Guide

Read [PROJECT_PLAN.md](PROJECT_PLAN.md) before working. It defines the project scope, references, acceptance criteria, and current status. Inspect the code and reconcile that status with reality.

Every committed project change, including documentation, must advance the shared
project version in `[workspace.package]` of Cargo.toml. The versioning feature
starts at 0.0.1; the next change is 0.0.2. Before each subsequent change checkpoint,
run `python3 tools/project_version.py bump` once. It updates Cargo.toml and the
workspace entries in Cargo.lock and the plan's current-version header. Patch and
minor roll over at 100: 0.0.99 becomes
0.1.0, and 0.99.99 becomes 1.0.0. Keep related edits and their documentation under
one version; bump again for another committed change. Validate with
`python3 tools/project_version.py check --base HEAD` before committing, and
`python3 tools/project_version.py check --history` after committing. Develop and
test locally, then push verified code and documentation; keep ROMs and private
artifacts excluded. Rebuild the runtime when handing the owner a changed build.

Build a new Rust engine that imports Super Mario 64 content from a user-supplied ROM. Bob-omb Battlefield is the first playable target; shared course, area, act, behavior, and warp systems must leave room for the complete game.

Preserve original movement, collision, and timing. Keep simulation at the supported reference version's cadence and interpolate presentation independently. Establish per-tick comparisons before claiming fidelity. Graphics settings must not modify authoritative gameplay state. Intentional gameplay changes require explicit optional modes.

Reference or adapt community work instead of guessing formats or reinventing established algorithms. Check source terms, pin revisions, preserve notices, and record provenance. Keep ROMs and ROM-derived assets out of version control. Temporary C integrations must have documented boundaries and a replacement plan; the intended engine and game runtime are Rust.

Choose implementation details within these goals. Deliver small, usable increments; test changes meaningfully and update PROJECT_PLAN.md, build/run instructions, and provenance records as needed. Report what works, what was tested, what remains missing, and the next useful task. Distinguish level exploration, mission completion, and complete-course support.
