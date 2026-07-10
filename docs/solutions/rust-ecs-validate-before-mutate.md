---
title: Validate Before Mutating Atomic ECS Economy Operations
tags: [rust, ecs, data-integrity, economy]
module: game-core
problem_type: partial-mutation
---
# Validate Before Mutating Atomic ECS Economy Operations

## Problem

A transaction or recipe can pass ordinary rule checks, mutate an inventory, and then fail checked arithmetic on a later balance or output. Returning the error at that point leaves a partially applied operation.

## Solution

Use a validate-then-apply sequence:

1. Read every affected component.
2. Calculate every resulting value with checked arithmetic.
3. Return without mutation if any calculation or rule check fails.
4. Apply the already validated values to all affected components.
5. Emit the event only after successful application.

For recipe/source updates, clone the small inventory map, perform all checked transformations on the clone, and replace the component only after success. This is appropriate for the prototype's small inventories; larger simulations may use a compact transaction delta instead.

## Evidence

- Implementation: `crates/game-core/src/lib.rs`
- Regression coverage: buy overflow, sell overflow, recipe output overflow, source overflow, transaction conservation, and rejection atomicity tests
- Resolution commits: `69895f5`, `36d55ef`, `6258a3e`, `b85d26b`

## Applicability

Apply this pattern to any command advertised as atomic. Checked arithmetic alone prevents numeric corruption but does not guarantee atomicity unless all checks happen before state changes.
