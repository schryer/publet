@conformance @must-13.3
Feature: An index publishes what its ranking rests on
  Requiring an operator to run particular code, or to price at cost, is not
  checkable from outside. Recomputation is checkable by everyone, so every
  result carries the policy and the snapshot it was computed over.

  Scenario: A caller must name the viewpoint
    Given a store containing the Appendix A fixture
    When I run the index without naming a policy
    Then it fails
    And it says a viewpoint must be declared

  Scenario: Every result names its policy and snapshot
    Given a store with a policy
    When I run the index under that policy
    Then every result names the policy
    And every result names the snapshot

  Scenario: The ranking is reproducible
    Given a store with a policy
    When I run the index under that policy twice
    Then both runs produce identical output
