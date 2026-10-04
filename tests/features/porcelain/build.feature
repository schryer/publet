@porcelain @must-9.1
Feature: A tree of named publet sources is built into objects and a lock
  People link by name and objects link by identifier. `pub build` reads one
  `publet.json` per publet, each referring to others by a short slug,
  publishes children before the parents that cite them, files each
  publet's Section 9.1 tag, and writes `publets.lock` mapping every slug to
  what it became. No slug enters any object: names stay withdrawable.

  Background:
    Given a workspace
    And a source tree with a tool publet and a document citing it by name

  Scenario: Names resolve to identifiers, children first
    When I build the source tree
    Then it succeeds
    And the lock names every slug in the tree
    And the document cites the tool's claim by the identifier the lock records
    And no object holds a slug
    And the tool's lineage carries its tag

  Scenario: Rebuilding unchanged sources publishes nothing
    When I build the source tree
    And I build the source tree again
    Then it succeeds
    And it reports "0 object(s) published"
    And the lock is byte-identical to the first build's

  Scenario: A changed child revises the publets that cite it
    When I build the source tree
    And I change the tool claim's content and build again
    Then it succeeds
    And the tool claim is reported "revised"
    And the document is reported "revised"
    And the document's tag is not filed again

  Scenario: Changing a tag files the new one and retracts the old
    When I build the source tree
    And I change the document's tag to "PUB-NEWTAG-10-2026" and build again
    Then it succeeds
    And the document's lineage carries only the tag "PUB-NEWTAG-10-2026"

  Scenario: A name that names nothing is refused and nothing is stored
    When I add a reference to "tool.missing" and build
    Then it fails
    And stderr mentions "tool.missing"
    And nothing new is stored

  Scenario: Publets citing each other in a cycle are refused by name
    When I make the tool cite the document and build
    Then it fails
    And stderr mentions "cycle"
    And nothing new is stored

  Scenario: A tag that is not a Section 9.1 tag is refused
    When I change the document's tag to "semantic drift" and build
    Then it fails
    And stderr mentions "<domain>-<name>-MM-YYYY"
