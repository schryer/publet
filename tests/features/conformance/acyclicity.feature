@conformance @must-6
Feature: Acyclic relation kinds reject cycle-closing edges
  Relations may cycle in general; mutual dispute is ordinary disagreement.
  These three express a lineage or a presupposition, which a cycle would
  make incoherent.

  Scenario Outline: A cycle in an acyclic kind is refused
    Given two publets A and B
    And a "<kind>" relation from A to B
    When a "<kind>" relation from B to A is added
    Then loading the store fails
    And stderr mentions "closes a cycle"

    Examples:
      | kind         |
      | supersedes   |
      | depends      |
      | derived-from |

  Scenario Outline: Cycles are permitted where the specification allows them
    Given two publets A and B
    And a "<kind>" relation from A to B
    When a "<kind>" relation from B to A is added
    Then loading the store succeeds

    Examples:
      | kind       |
      | disputes   |
      | supports   |
      | equivalent |
