---
name: segment-fixture
description: Fixture for validating markdown block segmentation.
---

# Segment Fixture

This paragraph checks that prose lands as one block.

This second paragraph is separated by a blank line.

## Code Section

```python
first = "fenced code"

second = "with an internal blank line"
```

## List Section

- first item of the fixture list

- second item after an internal blank line
- third item

Setext Heading Fixture
----------------------

> A quoted line for the quote block.
> A second quoted line.

| column | value |
| ------ | ----- |
| left   | right |

[reference-target]: https://example.com/fixture
