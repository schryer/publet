@conformance @must-5.2
Feature: A judgement is not a verdict
  An assessment is a signer saying what they think, and an author's
  assessment of their own work is one annotation among many rather than a
  privileged field (R3). It is not input to an evaluation policy, which is
  why it may be filed about a definition, where no verdict may be cast at
  all -- and why "sound within its scope" is sayable without becoming a
  claim that the thing is true.

  Scenario: An assessment may be filed on a definition
    Given a workspace with a scoped definition
    When I assess it as "sound-in-scope"
    Then it succeeds
    And asking why shows the judgement
    And asking why still reports the class not truth-apt

  Scenario: A judgement with no stated basis is refused
    Given a workspace with a scoped definition
    When I assess it with no basis
    Then it fails
    And stderr mentions "basis"

  Scenario: An unknown assessment is refused
    Given a workspace with a scoped definition
    When I assess it as "mostly-right"
    Then it fails
    And stderr mentions "unknown assessment"

  Scenario: A judgement does not move the weights
    Given a workspace with a scoped definition
    When I assess it as "refuted"
    Then the weights are unchanged
