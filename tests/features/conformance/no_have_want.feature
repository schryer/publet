@conformance @must-14.3.2
Feature: Synchronization discloses one integer
  A peer that learns which objects a reader holds learns what that reader
  has been working with. Section 14.3.2 therefore forbids have/want
  negotiation, and the constraint is on the wire, not on intent.

  Scenario: A sync request carries no object identifiers
    Given a node serving a domain of five objects
    When a client synchronizes from generation 0
    Then the requests carry the domain identifier
    And no request carries any other identifier

  Scenario: The interface has exactly one accepting route
    Given the served route table
    Then only one route accepts a body
    And that route takes a single object, not a list
