@porcelain @must-7.2
Feature: Build freely, publish deliberately
  In a corpus, `pub build` and `pub render` are a workshop: drafts are
  unsigned, never exported, and leave no history. `pub publish` is the one
  deliberate act: it signs and exports what is accepted, advances the
  corpus's generation, and pins the accepted rendering's lineage -- the
  document, the pipeline, the exact toolchain, and the output -- keeping
  the output as a blob until it is pruned.

  Background:
    Given a corpus with a stub tool at "1.0.0", a pipeline requiring "1.0", and a document rendered by it
    And everything in it is published

  Scenario: Drafts stay local
    When I edit the document and build twice
    Then the draft is reported "draft"
    And objects/, corpus.lock, and the generation are unchanged

  Scenario: Publishing a named publet leaves other drafts as drafts
    When I edit the document, build, edit it again, and build
    And I edit the unrelated publet and build
    And I publish the document
    Then the document's new version supersedes its previously published version
    And no intermediate draft was exported
    And the unrelated publet is still a draft
    And the generation advanced once

  Scenario: Publishing pins the accepted rendering
    When I render the document and publish it
    Then one rendering is pinned for the document
    And its output is kept in blobs/ under its identifier

  Scenario: Re-rendering identical output and publishing again files nothing
    When I render the document and publish it
    And I render the document and publish it
    Then it reports "nothing filed"
    And one rendering is pinned for the document

  Scenario: A tool upgrade within the constraint revises neither the pipeline nor the document
    When I render the document and publish it
    And the stub tool becomes "1.0.1" and its identity claim says so
    And I build
    Then the publet "render.stub" is reported "published"
    And the publet "demo-doc" is reported "published"
    And the publet "tool.stub#identity" is reported "draft"
    When I publish everything, render the document, and publish it
    Then it reports "nothing filed"

  Scenario: A machine ahead of the identity claim is refused before anything runs
    When the stub tool becomes "1.0.1" but its identity claim does not
    And I render the document
    Then it fails
    And stderr mentions "revise the identity claim"

  Scenario: An identity claim outside the step's constraint is refused
    When the stub tool becomes "2.0.0" and its identity claim says so
    And I build and render the document
    Then it fails
    And stderr mentions "requires stubtool 1.0"

  Scenario: Pruning keeps every record and removes only inactive outputs
    When I render the document and publish it
    And I edit the document, build, render it, and publish it
    And I list the corpus's renderings
    Then it reports "active"
    And it reports "held"
    When I prune the corpus's renderings
    Then it reports "pruned"
    And it reports "Re-render"
    And the first rendering's output is gone from blobs/ but its records remain

  Scenario: Revising the pipeline leaves a document citing its lineage untouched
    When I reword the pipeline's method and build
    Then the publet "render.stub#method" is reported "draft"
    And the publet "demo-doc" is reported "published"
