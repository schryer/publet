@conformance @must-14.1
Feature: A delta cannot add, omit, or substitute anything
  Deltas are self-verifying: the client recomputes the membership root and
  compares it, so the peer that served the delta need not be trusted.

  Scenario: An honest delta applies and matches the declared root
    Given a domain at generation 0 with one member
    And a generation adding one member
    When the delta is applied
    Then the resulting root matches the generation's snapshot

  Scenario: A delta smuggling an extra member is refused
    Given a domain at generation 0 with one member
    And a generation whose added list contains a member the snapshot excludes
    When the delta is applied
    Then it fails with a root mismatch

  Scenario: A client far behind syncs in logarithmic fetches
    Given a client 300 generations behind the head
    Then the number of fetches required is at most 9
