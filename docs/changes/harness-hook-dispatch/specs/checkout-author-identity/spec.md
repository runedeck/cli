# checkout-author-identity

## Purpose

Who a commit made by a harness session is attributed to: the model, resolved against the roster, written into the checkout the session works in.

## ADDED Requirements

### Requirement: Identity Binds To The Checkout

The handler `author-identity` on `session.start` MUST resolve the harness and the payload's model (Claude and Codex carry `model`; a payload without it is a warning and no write) to an identity listed in the checkout's `authors.yaml`, or the deck's when the checkout has none, and write it as the checkout's jj repository author, and git author when colocated, never into a file shared between sessions. A model with no listed identity writes `Unlisted <harness> model (<id>)` with an `unlisted@` address, which the attribution check refuses at push, so the miss is loud and no listed author is borrowed.

#### Scenario: Listed model at session start

- **WHEN** Claude starts with model `claude-fable-5-1[1m]` in a checkout whose `authors.yaml` lists `Claude Fable 5.1 (claude-fable-5-1)`
- **THEN** `jj config get user.name` in that checkout prints the listed name and a commit made by the session carries it

#### Scenario: Unlisted model

- **WHEN** Codex starts with a model no `authors.yaml` lists
- **THEN** the checkout's author becomes `Unlisted codex model (<id>)`, one warning names the model, and the push gate refuses a commit by it

#### Scenario: Two sessions, two checkouts

- **WHEN** a Claude session and a Codex session start in two different checkouts
- **THEN** each checkout carries its own session's identity and neither write touches the other
