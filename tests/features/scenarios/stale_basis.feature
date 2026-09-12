@scenario @must-14.3
Feature: A stale basis is disclosed, never a refusal
  Nothing here is overwritten, so a proposal cannot clobber a concurrent
  one. What a stale basis produces is information about what moved.

  Scenario: Superseding something that already has a successor
    Given a workspace where a claim has been superseded twice
    When I propose the second successor
    Then it succeeds
    And it reports that the lineage will branch

  Scenario: Depending on a superseded definition
    Given a workspace where a claim depends on an outdated definition
    When I propose that claim
    Then it succeeds
    And it reports the definition has been superseded

  Scenario: Disputing something already resolved
    Given a workspace where a dispute targets a resolved question
    When I propose the dispute
    Then it succeeds
    And it reports that a resolution already exists
