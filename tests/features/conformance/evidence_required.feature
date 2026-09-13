@conformance @must-5.5 @must-5.4
Feature: An empirical claim must name a method others can execute
  A measurement without a reproducible method is a report of an experience.
  It may be honest and it may be all that exists, but it is not the kind of
  thing this protocol establishes.

  Scenario: An empirical publet without a method is refused
    Given an empirical publet naming no method
    When I load the store
    Then loading the store fails
    And stderr mentions "role `method`"

  Scenario: An empirical publet naming a method is accepted
    Given an empirical publet naming a method
    When I load the store
    Then loading the store succeeds

  Scenario: Other classes carry no such requirement
    Given a "normative" publet
    When I load the store
    Then loading the store succeeds

  Scenario: A dependency cycle is refused
    Given two publets that presuppose each other
    When I ask for the dependency closure
    Then it fails
