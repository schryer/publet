@porcelain
Feature: Revising publishes the next version and its lineage in one step
  Revision is a new object plus `supersedes` (R1). `pub revise` does both,
  starting from the old version and applying only what changed, and does
  nothing at all when nothing changed -- so a build that re-gathers the same
  values leaves the lineage as it found it.

  Background:
    Given a workspace with a budget claim

  Scenario: New values make a new version that supersedes the old
    When I revise the budget claim with the data
      """
      {"columns": [{"name": "item"}, {"name": "spend", "unit": "EUR"}],
       "rows": [["seed trays", "150.00"]]}
      """
    Then it succeeds
    And it prints the new version and the supersedes relation
    And the relation says the new version supersedes the budget claim
    And the new version keeps the budget claim's content and scope

  Scenario: The same values make no new version
    When I revise the budget claim with its own data again
    Then it succeeds
    And it prints only the budget claim
    And stderr mentions "unchanged"
    And nothing new is stored

  Scenario: Revising the sources replaces them and keeps the rest
    When I revise the budget claim read from revision "def456"
    Then it succeeds
    And reading the new version shows "revision def456"
    And reading the new version does not show "revision abc123"

  Scenario: A document is revised from its next manifest
    Given a document viewing the budget as "table"
    When I revise the document from a manifest viewing the budget as "summary"
    Then it succeeds
    And it prints the new version and the supersedes relation
    And reading the new version shows "view   summary"

  Scenario: A document cannot be revised without a manifest
    Given a document viewing the budget as "table"
    When I revise the document with no manifest
    Then it fails
    And stderr mentions "--manifest=FILE is required"
