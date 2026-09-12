@conformance @must-11.7
Feature: An evaluation can be reproduced by anyone
  Given the policy, the snapshot, and the specification, any implementation
  must produce a bit-identical standing. That is what lets settlement
  operate without granting anyone epistemic authority.

  Scenario: The same inputs produce the same bytes
    Given every committed evaluation vector
    When I evaluate each one twice
    Then both runs produce byte-identical output

  Scenario: Every weight is rendered to a fixed precision
    Given the "06-contested" vector
    When I evaluate it
    Then every weight is written to exactly six decimal places
