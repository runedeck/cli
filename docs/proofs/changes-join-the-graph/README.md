---
type: proof
change: changes-join-the-graph
recorded: 2026-10-26
transcript: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
scenes:
- scenario: changes-join-the-graph#context-maps-frontmatter-to-triples/record-key-is-mapped
  kind: unproven
- scenario: changes-join-the-graph#context-maps-frontmatter-to-triples/record-key-is-unmapped
  kind: unproven
- scenario: changes-join-the-graph#context-maps-frontmatter-to-triples/context-is-absent
  kind: unproven
- scenario: changes-join-the-graph#references-resolve-by-shape/upstream-lists-three-forms
  kind: unproven
- scenario: changes-join-the-graph#lifecycle-nodes-join-the-graph/change-with-a-delta-and-a-draft
  kind: unproven
- scenario: changes-join-the-graph#lifecycle-nodes-join-the-graph/decisions-entry-is-a-title
  kind: unproven
---

# Proof: changes-join-the-graph

Every scene runs `rune graph export` from #40 "feat: add rune graph export for SHACL validation" against a copy of `tests/fixtures/changes-join-the-graph`. The fixture is a small deck with a JSON-LD context, two decision records, a change with a delta spec and a draft record, and a change that names its draft by title. No scene stays unproven.

## changes-join-the-graph#context-maps-frontmatter-to-triples/record-key-is-mapped

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/changes-join-the-graph" deck'
$ awk '/"created"/' deck/ontology/context.jsonld
    "created": {"@id": "schema:dateCreated", "@type": "xsd:date"},
$ awk '/^created:/' "deck/docs/decisions/DECK-0002 New Way.md"
created: 2026-02-19
$ sh -c "rune graph export --source deck | awk '/^@prefix/; /DECK-0002> a/,/sourcePath/'"
@prefix rune:    <https://runedeck.ai/ns#> .
@prefix dcterms: <http://purl.org/dc/terms/> .
@prefix schema: <https://schema.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
<https://runedeck.ai/id/DECK-0002> a rune:DecisionRecord ;
...
    schema:dateCreated "2026-02-19"^^xsd:date ;
...
    rune:sourcePath "docs/decisions/DECK-0002 New Way.md" .
```

## changes-join-the-graph#context-maps-frontmatter-to-triples/record-key-is-unmapped

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/changes-join-the-graph" deck'
$ awk '/internal-note/' deck/ontology/context.jsonld "deck/docs/decisions/DECK-0002 New Way.md"
internal-note: kept out of the graph
$ sh -c "rune graph export --source deck | awk '/internal-note|kept out of the graph/ { n++ } END { print n + 0, \"matches\" }'"
0 matches
```

## changes-join-the-graph#context-maps-frontmatter-to-triples/context-is-absent

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/changes-join-the-graph" deck && rm -f deck/ontology/context.jsonld'
$ sh -c "rune graph export --source deck | awk '/a rune:DecisionRecord/ && !/#adr>/,/sourcePath/'"
<https://runedeck.ai/id/DECK-0001> a rune:DecisionRecord ;
    dcterms:identifier "DECK-0001" ;
    dcterms:title "Old Way" ;
    rune:sourcePath "docs/decisions/DECK-0001 Old Way.md" .
<https://runedeck.ai/id/DECK-0002> a rune:DecisionRecord ;
    dcterms:identifier "DECK-0002" ;
    dcterms:title "New Way" ;
    dcterms:relation <https://runedeck.ai/id/DECK-0001> ;
    rune:sourcePath "docs/decisions/DECK-0002 New Way.md" .
```

## changes-join-the-graph#references-resolve-by-shape/upstream-lists-three-forms

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/changes-join-the-graph" deck'
$ awk '/^upstream:/,/obsidian/' "deck/docs/decisions/DECK-0002 New Way.md"
upstream:
    - "DECK-0001 Old Way"
    - "sample-capability#thing-is-checked"
    - "obsidian://open?vault=Atlas&file=Canvas%2FSketch"
$ sh -c "rune graph export --source deck | awk '/isBasedOn/; /> a rune:(DecisionRecord|Requirement)/ && !/#adr>/'"
<https://runedeck.ai/id/DECK-0001> a rune:DecisionRecord ;
<https://runedeck.ai/id/DECK-0002> a rune:DecisionRecord ;
    schema:isBasedOn <https://runedeck.ai/id/DECK-0001> ;
    schema:isBasedOn <https://runedeck.ai/id/sample-capability#thing-is-checked> ;
    schema:isBasedOn <obsidian://open?vault=Atlas&file=Canvas%2FSketch> ;
<https://runedeck.ai/id/sample-capability#thing-is-checked> a rune:Requirement ;
```

## changes-join-the-graph#lifecycle-nodes-join-the-graph/change-with-a-delta-and-a-draft

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/changes-join-the-graph" deck'
$ awk '/^(archived|decisions):/' deck/docs/changes/sample-change/proposal.md
archived: 2026-09-20
decisions: ["DECK-0002 New Way"]
$ sh -c "rune graph export --source deck | awk '/sample-change> a/,/sourcePath/; /sample-capability[^ ]*> a/,/(sourcePath|isPartOf)/; /sample-change#adr> a/,/sourcePath/'"
<https://runedeck.ai/id/change/sample-change> a rune:Change ;
    dcterms:identifier "sample-change" ;
    rune:archived "2026-09-20"^^xsd:date ;
    rune:decisionRecord <https://runedeck.ai/id/DECK-0002> ;
    rune:sourcePath "docs/changes/sample-change/proposal.md" .
<https://runedeck.ai/id/sample-capability> a rune:Capability ;
    dcterms:identifier "sample-capability" ;
    rune:change <https://runedeck.ai/id/change/sample-change> ;
    rune:sourcePath "docs/changes/sample-change/specs/sample-capability/spec.md" .
<https://runedeck.ai/id/sample-capability#thing-is-checked> a rune:Requirement ;
    dcterms:title "Thing is checked" ;
    dcterms:isPartOf <https://runedeck.ai/id/sample-capability> .
<https://runedeck.ai/id/sample-capability#thing-is-checked/thing-enters-the-graph> a rune:Scenario ;
    dcterms:title "Thing enters the graph" ;
    dcterms:isPartOf <https://runedeck.ai/id/sample-capability#thing-is-checked> .
<https://runedeck.ai/id/change/sample-change#adr> a rune:DecisionRecord ;
    dcterms:title "Sample draft decision" ;
    rune:status rune:proposed ;
    rune:change <https://runedeck.ai/id/change/sample-change> ;
    rune:sourcePath "docs/changes/sample-change/adr.md" .
```

## changes-join-the-graph#lifecycle-nodes-join-the-graph/decisions-entry-is-a-title

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/changes-join-the-graph" deck'
$ awk '/^decisions:/' deck/docs/changes/titled-change/proposal.md
decisions: ["Titled decision waits for its identifier"]
$ sh -c "rune graph export --source deck | awk '/titled-change> a/,/sourcePath/; /Titled(%20| )decision/'"
<https://runedeck.ai/id/change/titled-change> a rune:Change ;
    dcterms:identifier "titled-change" ;
    rune:sourcePath "docs/changes/titled-change/proposal.md" .
    dcterms:title "Titled decision waits for its identifier" ;
```
