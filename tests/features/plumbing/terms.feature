@plumbing
Feature: Resolving a term never picks a winner
  There is no namespace and no registry. A term is whatever some
  definitional publet says it is, several publets may say different things,
  and choosing between them belongs to a viewpoint rather than to a lookup.

  Scenario: A term with one definition resolves to it
    Given a store with competing definitions
    When I look up "content address"
    Then 1 definition is reported

  Scenario: A contested term reports every definition
    Given a store with competing definitions
    When I look up "merkle tree"
    Then 2 definitions are reported
    And one of them is marked contested

  Scenario: Listing reports every term in the store
    Given a store with competing definitions
    When I list every term
    Then 3 definitions are reported

  Scenario: A term nobody has defined is not an error in the graph
    Given a store with competing definitions
    When I look up "bloom filter"
    Then it fails
    And stderr mentions "no definition here"
