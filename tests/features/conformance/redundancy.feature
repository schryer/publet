@conformance @must-7.3
Feature: A dispute adding nothing new carries no weight
  R10 bounds attrition without silencing anyone. A disputant with no new
  grounds is still published, permanent and readable; their dispute simply
  does not consume the question again. One who does produce new grounds is
  never redundant, however unpopular their position.

  Scenario: A dispute whose grounds a resolution already covers is redundant
    Given a claim disputed on grounds a sustained resolution already covers
    When I ask why
    Then the divergence factor is zero

  Scenario: A dispute on new grounds still counts
    Given a claim disputed on grounds no resolution covers
    When I ask why
    Then the divergence factor is above zero

  Scenario: A no-consensus resolution settles nothing
    Given a claim whose dispute is covered only by a no-consensus resolution
    When I ask why
    Then the divergence factor is above zero
