@conformance @must-11.4
Feature: Proof and replication dominate endorsement weight
  Proof settles a formal claim and independent reproduction settles an
  empirical one. Endorsement decides only what neither has settled.

  Scenario: Overwhelming endorsement cannot accept an unreplicated claim
    Given the "02-unreplicated" vector
    When I evaluate it
    Then the result is "unreplicated"
    And the result is not "accepted"

  Scenario: One independent inconsistent reproduction outweighs endorsement
    Given the "03-refuted" vector
    When I evaluate it
    Then the result is "rejected"

  Scenario: Meeting the replication floor permits acceptance
    Given the "01-chain" vector
    When I evaluate it
    Then the result is "accepted"

  Scenario: A proof is not a poll
    Given the "05-proof" vector
    When I evaluate it
    Then the result is "accepted"

  Scenario: Retraction removes positive standing whatever the evidence
    Given the "10-retracted" vector
    When I evaluate it
    Then the result is "rejected"
