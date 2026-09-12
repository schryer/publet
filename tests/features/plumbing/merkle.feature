@plumbing @must-14.1
Feature: Membership roots and proofs

  Scenario: A root is computed from CID lines on stdin
    Given a list of four member CIDs
    When I run "pub-merkle" with them on stdin
    Then stdout is a single CID

  Scenario: The root does not depend on input order
    Given a list of four member CIDs
    When I compute the root with the list reversed
    Then both orders produce the same root

  Scenario: A present member proves inclusion
    Given a list of four member CIDs
    When I prove the first member against that list
    Then the finding is "present"
    And the proof verifies

  Scenario: An absent member proves absence
    Given a list of four member CIDs
    When I prove a member that is not in the list
    Then the finding is "absent"
    And the proof verifies

  Scenario: Listing a domain feeds the root computation
    Given a store containing the Appendix A fixture
    When I run the pipeline "pub-ls | pub-merkle"
    Then stdout is a single CID
