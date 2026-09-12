@porcelain @must-11.3
Feature: A standing is never reduced to a badge
  A reader told only "accepted" cannot tell whether that rests on
  replication or on agreement, and those are not the same claim.

  Background:
    Given a workspace containing the cohort claim

  Scenario: Every component of the standing is exposed
    When I ask why
    Then the affirm, deny, abstain and active weights are shown
    And the divergence factor and threshold are shown
    And the reproduction counts and the replication floor are shown

  Scenario: An unevaluated claim says so about the viewpoint, not the claim
    When I ask why
    Then the result is "undetermined"
    And the explanation says it is a fact about the viewpoint

  Scenario: Overwhelming endorsement without replication is not acceptance
    Given fifty trusted keys affirming the claim
    When I ask why
    Then the result is "unreplicated"
    And the explanation says no amount of agreement substitutes for replication
