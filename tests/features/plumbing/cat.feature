@plumbing @must-13.2
Feature: Objects are retrieved by identifier

  Background:
    Given an empty store
    And two objects in the store

  Scenario: A stored object is returned byte for byte
    When the first object is read back
    Then stdout is byte-identical to what was stored

  Scenario: An identifier that is not held is reported as not found
    When an object that was never stored is requested
    Then the exit code is 3

  Scenario: Identifiers are accepted on stdin
    When the first identifier is piped in
    Then stdout is byte-identical to what was stored
