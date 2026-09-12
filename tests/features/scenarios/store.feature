@scenario @must-13.2
Feature: A node keeps what it has declared
  Within a declared set, service is unconditional and ceasing to serve
  requires a published tombstone. Outside it, a node owes nothing.

  Background:
    Given an empty store

  Scenario: Garbage collection keeps declared members and discards the rest
    Given two objects in the store
    And the first is declared as a domain member
    When garbage is collected
    Then the first object is still held
    And the second object is gone

  Scenario: A declared member cannot be removed silently
    Given two objects in the store
    And the first is declared as a domain member
    When garbage is collected twice
    Then the first object is still held

  Scenario: Objects outside every declared set are collected
    Given two objects in the store
    When garbage is collected
    Then the store is empty

  Scenario: An integrity scan passes on a healthy store
    Given two objects in the store
    When the store is scanned
    Then the scan reports no findings

  Scenario: Stored bytes are verified against their identifier on the way out
    Given two objects in the store
    When the first object is read back
    Then it verifies against the identifier it was stored under

  Scenario: A store-backed command explains why it cannot be piped
    Given two objects in the store
    When two store commands are run against the same store at once
    Then the failure explains that the store is already open
