@porcelain @must-8
Feature: A document is built from a manifest, and says how to show data
  `pub doc` reads the document's structure from one JSON file, which is
  what a build that publishes documents wants. An item's `view` names how
  to show the cited data in this document; it is presentation, like a
  gloss, and carries no claims.

  Background:
    Given a workspace with a budget claim

  Scenario: A manifest with a view is built into a document
    When I build a document from a manifest viewing the budget as "table"
    Then it succeeds
    And reading the document shows "view   table"
    And reading the document shows "gloss  Planned spend for the first phase."

  Scenario: The same data may be shown differently in another document
    When I build a document from a manifest viewing the budget as "table"
    And I build a document from a manifest viewing the budget as "bar-chart"
    Then the two documents are different objects citing the same claim

  Scenario: A view without a renderer is refused and nothing is stored
    When I build a document from a manifest whose view has no renderer
    Then it fails
    And stderr mentions "items.view.renderer"
    And nothing new is stored

  Scenario: A misspelled manifest key is refused rather than dropped
    When I build a document from a manifest with an item key "glos"
    Then it fails
    And stderr mentions "unknown item key `glos`"
