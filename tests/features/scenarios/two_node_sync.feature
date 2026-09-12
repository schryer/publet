@scenario @must-14.4
Feature: Two nodes reach the same state by delta

  Background:
    Given a node serving a domain of five objects

  Scenario: A replica syncs and holds every object
    When a client synchronizes from generation 0
    Then the replica holds 10 objects
    And every stored object verifies against its identifier

  Scenario: The declared set is published as domain identifiers
    When the declared set is requested
    Then it names the served domain

  Scenario: An object fetched over the wire verifies
    When the first member is fetched by identifier
    Then it verifies against the identifier requested

  Scenario: An object the node does not hold is reported as not found
    When an object that was never published is fetched
    Then the request fails with a not-found status

  Scenario: Checkpoints are offered so a client far behind need not walk
    When the checkpoints are requested
    Then they are exponentially spaced below the head
