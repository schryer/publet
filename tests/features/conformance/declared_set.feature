@conformance @must-14.6
Feature: A reduced declared set is published before service stops
  A peer reading the reduction can acquire the difference. One discovering
  it when a request fails cannot.

  Scenario: Withdrawing a set emits the reduced set
    Given a store declaring two domains
    When I withdraw one of them
    Then the reduced set is emitted
    And the withdrawn domain is absent from it

  Scenario: Withdrawing frees its members for collection
    Given a store declaring two domains
    When I withdraw one of them
    And garbage is collected
    Then the remaining domain's members are still held
