Important info
1. check out Makefile before you want to use cargo. always use make, not cargo. if you need extra cargo operations, update Makefile.
2. files in SPEC/ are good reference, but not up-to-date. the code is always the source of fact.

This is a native desktop GUI application written in Rust with iced.

Priorities, in order:

1. Correctness
2. Simple and maintainable code
3. Low CPU/memory overhead
4. Native desktop behavior
5. Minimal dependencies
6. Small, focused changes

Before editing

Read only the files relevant to the requested change.

Do not scan the entire repository unless necessary.

First identify:

- entry point
- relevant module
- relevant widget/view function
- message/update path
- state affected by the change

Prefer local changes over cross-project refactors.

iced architecture
Use the normal iced flow:
Event
  -> Message
  -> update()
  -> State
  -> view()
  -> renderer
  
Keep these responsibilities separate:
- Message: user/UI events
- update(): state transitions and commands/tasks
- view(): UI description only
- backend/system code: filesystem, OS, process, etc.

Do not perform I/O, filesystem operations, expensive computation, or blocking work inside view().
view() should be cheap and deterministic.

Avoid rebuilding expensive data merely because the UI was redrawn.

Rendering and performance

Assume that mouse movement, scrolling, hover changes, window resizing, and animation can cause frequent UI work.

Do not perform expensive work on every event.

Avoid:

update() -> expensive computation -> state change -> redraw

when the expensive computation can be cached or performed asynchronously.

Avoid allocating large temporary collections inside view().

Avoid repeatedly:

- reading directories
- querying file metadata
- decoding images
- loading thumbnails
- formatting large amounts of text
- sorting large collections
- constructing expensive data structures


during rendering.

For large lists, prefer virtualization or another strategy that limits work to visible items.

If CPU usage is unexpectedly high, test these separately:
1. static UI
2. mouse movement
3. scrolling
4. hover effects
5. list rendering
6. image/thumbnail rendering


Mouse and hover handling

Mouse events can occur at high frequency.

Do not update application state for every pointer event unless the visual result actually changes.

Avoid unnecessary state changes such as:

self.hovered = new_value;

when new_value == self.hovered.

Keep hover effects lightweight.

Do not attach global mouse handling to the entire application when a local widget can handle the interaction.

Scrolling

Scrolling must remain cheap.

Do not perform filesystem or other expensive work merely because the scroll position changes.

If scrolling causes unexpectedly high CPU usage, inspect:

scroll event handling
widget tree size
layout cost
hover handling
image/thumbnail rendering
unnecessary state updates
repeated allocations

Do not optimize unrelated code before locating the hot path.

Widget design

Prefer simple iced widgets.

Avoid deeply nested widget trees when a simpler structure is sufficient.

Do not create custom widgets unless normal iced widgets cannot provide the required behavior.

When creating reusable widgets, keep them small and focused.

Do not create a generic abstraction for a single use case.

Prefer:

fn file_row(...)

over introducing several traits/builders/factories unless reuse justifies them.

State

Keep application state explicit.

Prefer concrete structs and enums over dynamic containers.

Example:

struct App {
    files: Vec<FileEntry>,
    selected: Option<usize>,
}

Use enums for meaningful UI/application states.

Do not duplicate state that can be derived cheaply.

Cache data when deriving it is expensive and frequently repeated.

Async and blocking work

Never block the GUI thread with expensive or blocking operations.

Filesystem operations, process execution, network access, large image decoding, and expensive computation should not run synchronously in the UI path when they can block interaction.

Use iced Task / appropriate async mechanisms where suitable.

Do not introduce async merely for trivial synchronous operations.

Environment variables

Rust 2024 requires std::env::set_var and std::env::remove_var to be used inside unsafe.

For startup-only environment configuration:

unsafe {
    std::env::set_var("EXPLORER_DPI", value);
}

Prefer setting environment variables before starting GUI/runtime threads.

If a value is only application configuration, pass it through application state instead of using an environment variable.

Do not add unnecessary unsafe.

Rust edition

Use Rust 2024 edition.

Prefer current stable Rust APIs.

Do not downgrade the edition or compiler merely to avoid fixing a small compatibility issue.

For edition migration, prefer:

cargo check
cargo fix --edition
cargo clippy
cargo test

Fix the smallest number of issues necessary.

Error handling

Do not use unwrap() or expect() on ordinary runtime conditions where failure is possible.

Use Result / Option appropriately.

unwrap() is acceptable when an invariant is genuinely guaranteed and obvious from nearby code.

Do not silently ignore errors.

For user-facing application failures, preserve enough context to diagnose the problem.

Dependencies

Avoid adding dependencies for trivial functionality.

Before adding a crate, check whether:

Rust stdlib already provides it
iced already provides it
an existing project dependency provides it

Keep dependency count low.

Do not replace a simple implementation with a large framework.

API changes

Before changing public/internal interfaces, check all call sites.

Prefer backwards-compatible local changes when possible.

Do not rename types/functions/files globally unless required.

Do not refactor unrelated code while implementing a feature.

Generated / mechanical changes

Do not reformat the entire repository for a local change.

Do not rewrite files merely to change formatting.

Keep diffs small and reviewable.

Testing

After meaningful changes, run the smallest relevant validation first:

cargo check

Then, when appropriate:

cargo test
cargo clippy

For GUI changes, also build/run the application if possible.

Do not spend tokens explaining tests that were not actually run.

Report actual errors rather than guessing.

Debugging workflow

When fixing a bug:

Reproduce or inspect the failure.
Locate the smallest responsible component.
Confirm the cause.
Make the smallest fix.
Run cargo check.
Run relevant tests.
Stop.

Do not rewrite surrounding code unless the existing structure directly causes the bug.

Performance debugging

Do not optimize based only on intuition.

When possible, compare:

before
after

under the same workload.

For GUI performance, isolate the expensive interaction first.

Useful first-level checks:

cargo run --release

and system CPU/GPU monitoring.

For suspicious rendering behavior, temporarily remove components one at a time.

Code style

Prefer straightforward Rust.

Good:

if selected {
    ...
}

Avoid unnecessary:

selected.then(...).unwrap_or_else(...)

Prefer explicit code when it is easier to read.

Use iterators when they improve clarity; do not force iterator chains into complex logic.

Avoid clever lifetime tricks.

Avoid unnecessary generic parameters.

Avoid macros unless they materially improve the code.

Comments

Comments should explain:

why something is necessary
an OS/backend limitation
a non-obvious performance constraint
an invariant that must be preserved

Do not comment obvious Rust code.

Bad:

// Set the width.
width = 800;

Good:

// Set the minimum width because the Ribbon becomes unusable below this size.
When uncertain

Do not invent iced APIs.

Check the version in Cargo.toml and use APIs compatible with that version.

Do not assume examples from another iced version apply unchanged.

If an API is uncertain, inspect the installed dependency/source or use compiler errors to determine the correct API.

Prefer existing project patterns over introducing a new pattern.

Change discipline

For every request:

understand the requested behavior
modify only what is necessary
preserve existing behavior
avoid unrelated cleanup
validate the result

A small correct patch is preferred over a large "better architecture".

Final response

After completing a coding task, report briefly:

what changed
files changed
validation performed
any remaining issue

Do not provide a long tutorial unless explicitly requested.
