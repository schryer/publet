@conformance @must-8
Feature: A document composes, it does not assert
  A document contains no assertions of its own. Its glosses are
  presentational, and an author arguing in one is required by the design to
  publish a publet instead -- which drags implicit argument into the open
  where it can be cited and contested.

  Scenario: A tracking citation must record what its author read
    Given a document citing a lineage without recording the head
    When I load the store
    Then loading the store fails
    And stderr mentions "must record `at`"

  Scenario: A tracking citation recording the head is accepted
    Given a document citing a lineage and recording the head
    When I load the store
    Then loading the store succeeds

  Scenario: A fixed citation needs no head
    Given a document citing one object
    When I load the store
    Then loading the store succeeds

  Scenario: An unknown role is refused
    Given a document citing an object with role "endorse-strongly"
    When I load the store
    Then loading the store fails

  Scenario: Including a publet as a counterpoint is not endorsing it
    Given a document citing one object as a counterpoint
    When I read that document
    Then the role is shown as "counterpoint"

  Scenario: Critiques are surfaced when a document is rendered
    Given a document with a critique against it
    When I read that document
    Then the critique is shown
