@conformance @must-7.2 @must-10.1
Feature: Only a human principal counts toward a replication floor
  Institutions are immortal and humans are not. Institutional resources are
  welcome; the person who ran the instrument signs, with the institution
  recorded as an affiliation.

  Scenario: Reproductions from a human principal count
    Given a claim with two reproductions from human principals
    When I ask why
    Then the independent consistent count is 2

  Scenario: Reproductions from an organization do not count toward the floor
    Given a claim with two reproductions from an organization
    When I ask why
    Then the independent consistent count is 0
    And the result is "unreplicated"

  Scenario: A key must declare what it is
    Given a key object declaring no principal
    Then reading it as a key fails
