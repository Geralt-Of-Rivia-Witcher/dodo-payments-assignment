# AI_USAGE.md

## AI tools used

- Codex (GPT-5 based coding agent) for implementation assistance, Rust syntax guidance, and boilerplate generation.

---

## How AI was used (specific)

- This was my first time building a backend service in Rust, and I completed the assignment in a relatively short time window (~6 hours). I therefore used AI heavily for Rust-specific implementation details while focusing my own effort on system design, correctness, and failure handling decisions.

- AI assistance was primarily used for:
  - Project scaffolding (workspace structure, Docker setup, migrations wiring).
  - Writing Axum handlers, models, middleware, and async worker loops.
  - SQLx query integration and transaction wiring in Rust.
  - Test scaffolding and Markdown documentation drafting.
  - Explaining Rust compiler/lifetime/type-system errors during development.

---

## Decisions I made independently (against or beyond AI suggestions)

### 1. Strict scope control

- I intentionally kept the implementation tightly scoped to the assignment requirements and avoided adding unnecessary product features or abstractions.

- Why:
  The assignment explicitly emphasized correctness, operational reasoning, and restraint over feature count.

---

### 2. Concurrency correctness using transient `processing` invoice state

- I identified a race condition where two concurrent payment requests using different idempotency keys could potentially initiate payment simultaneously for the same invoice.

- I chose to introduce a transient invoice state transition:
  `open -> processing -> paid/open`

- Why:
  This ensured that once payment processing begins, concurrent requests are rejected before reaching the PSP call path, preventing duplicate external charges.

---

### 3. Stronger idempotency and concurrency test semantics

- I strengthened the tests beyond simple status-code assertions by explicitly validating:
  - exactly one PSP call under concurrency,
  - stable idempotent response fields,
  - replay semantics,
  - final invoice-state correctness.

- Why:
  The assignment heavily emphasized payment correctness and failure-mode handling, so I wanted the tests to verify behavioral guarantees rather than only API responses.

---

## One thing AI got wrong (and how I corrected it)

- An earlier AI-generated payment flow had ordering and concurrency issues around idempotency handling and invoice-state transitions under concurrent payment attempts.

- I corrected this by:
  - improving replay handling,
  - explicitly handling unique-key race paths,
  - introducing the transient `processing` state to serialize payment initiation safely.

- Verification:
  I validated the changes through targeted concurrency/idempotency tests and manual code-path review.

---

## Overall ownership summary

- I relied heavily on AI for Rust implementation details and syntax because I was new to the language.

- I personally drove the system-design decisions, payment-state machine behavior, concurrency strategy, idempotency semantics, failure-mode handling, and production tradeoff discussions reflected in the final implementation and DESIGN.md.
