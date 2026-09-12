@plumbing @must-4.2
Feature: Content identifiers name bytes exactly

  Scenario: The identifier matches one computed independently
    Given a well-formed object
    When I run "pub-cid" with that object on stdin
    Then the exit code is 0
    And stdout is the CID this suite computed for the input

  Scenario: Identifiers are stable across runs
    Given a well-formed object
    When I run "pub-cid" with that object on stdin twice
    Then both runs produce identical output

  Scenario: A single changed byte changes the identifier
    Given a well-formed object
    And a second object differing only in its created field
    When I run "pub-cid" on each
    Then the two identifiers differ

  Scenario: Non-canonical bytes have no identifier
    Given an object encoded with a float value
    When I run "pub-cid" with that object on stdin
    Then the exit code is 1
    And stdout is empty
