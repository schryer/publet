@porcelain
Feature: Anyone may relate any two objects
  Relations are the edges, and an author writes them over objects they did
  not write (R6). What the command refuses is not an unwelcome opinion but
  an edge that would make the graph incoherent: the acyclic kinds each
  express a lineage or a presupposition, and a cycle in one is not a
  disagreement, it is nonsense.

  Scenario: A relation is authored and read back
    Given a workspace with two composed definitions
    When I relate them as "equivalent"
    Then it succeeds
    And the edge is readable from the first

  Scenario: A relation may be authored over another key's objects
    Given a workspace with two composed definitions
    When I relate them as "disputes"
    Then it succeeds

  Scenario: A cycle in an acyclic kind is refused and nothing is stored
    Given a workspace with two composed definitions
    And they are already related as "supersedes"
    When I relate them the other way as "supersedes"
    Then it fails
    And stderr mentions "closes a cycle"
    And the store is unchanged

  Scenario: Mutual disputes are ordinary disagreement, not a cycle
    Given a workspace with two composed definitions
    And they are already related as "disputes"
    When I relate them the other way as "disputes"
    Then it succeeds

  Scenario: A self-edge is refused
    Given a workspace with two composed definitions
    When I relate the first to itself as "equivalent"
    Then it fails
    And stderr mentions "self-edge"

  Scenario: An unknown relation kind is refused
    Given a workspace with two composed definitions
    When I relate them as "supercedes"
    Then it fails
    And stderr mentions "unknown relation kind"
