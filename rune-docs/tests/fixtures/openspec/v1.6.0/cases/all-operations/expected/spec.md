# Search Specification

## Purpose

Describe stored-document search.

## Requirements

<!-- Keep this note in place. -->

### requirement: Current Lookup

The system SHALL return matching documents with their source paths.

#### Existing response

A matching document and its source path are returned.

#### Missing response

An empty result is returned when no document matches.

###Requirement: Fresh Filter

The system MUST filter matching documents by owner.

#### Filtered response

Only matching documents for the selected owner are returned.

## Notes

Keep this trailing section unchanged.
