## added requirements

###Requirement: Fresh Filter

The system MUST filter matching documents by owner.

#### Filtered response

Only matching documents for the selected owner are returned.

## MODIFIED Requirements

### requirement: Current Lookup

The system SHALL return matching documents with their source paths.

#### Existing response

A matching document and its source path are returned.

#### Missing response

An empty result is returned when no document matches.

## renamed requirements

- FROM: `### Requirement: Legacy Lookup`
- TO: `### Requirement: Current Lookup`

## REMOVED Requirements

- `### Requirement: Remove Obsolete`
