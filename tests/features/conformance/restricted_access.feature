@conformance @must-11.4
Feature: Concentrated access cannot manufacture established knowledge
  If nobody else is permitted to measure, the measurement cannot underpin a
  general claim however many reproductions its controller reports.

  Scenario: Restricted reproducibility caps standing at unreplicated
    Given the "04-restricted" vector
    When I evaluate it
    Then the result is "unreplicated"
    And the reported reproducibility class is "restricted"
    And the reported independent consistent reproductions are 99
