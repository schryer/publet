@conformance @must-10.3
Feature: Personhood bounds keys per person and decides nothing else
  It proves a human-issued credential underlies a key without revealing
  which human. It does not prove a human signed, and uniqueness is not
  honesty: the documented failures of modern science were committed by
  identified, credentialed, unique people.

  Scenario: Publishing never requires it
    Given a workspace
    When I compose a claim
    Then it succeeds
    And no personhood attestation was required

  Scenario: One scheme is not enough
    Given an implementation carrying one personhood scheme
    Then it does not meet the scheme minimum

  Scenario: Two schemes declaring different anonymity are both carried
    Given an implementation carrying two personhood schemes
    Then it meets the scheme minimum
    And their declared anonymity differs

  Scenario: A person holds one key per scope
    Given a verified personhood attestation in a scope
    When a second key presents the same nullifier in that scope
    Then it is refused as the same person

  Scenario: The same person is unlinkable across scopes
    Given a verified personhood attestation in a scope
    When the same person presents in a different scope
    Then it is accepted
