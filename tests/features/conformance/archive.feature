@conformance @must-13.4 @must-10.5
Feature: An archive timestamps everything it accepts
  A timestamp cannot be applied retroactively. Without one, a key
  compromised in 2040 invalidates its 2028 work; with one, it does not.

  Scenario: An unarchived store reports what it has not timestamped
    Given a store holding two objects
    When I audit it as an archive
    Then it fails
    And it names both objects

  Scenario: Archiving timestamps everything held
    Given a store holding two objects
    When I archive it
    And I audit it as an archive
    Then the audit passes
