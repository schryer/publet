@conformance @must-11.1 @must-11.8
Feature: Standing is relative to a declared viewpoint
  There is no network-wide standing. Two readers with different trust roots
  compute different results from identical data, and both are correct.

  Scenario: A claim's outcome depends on the policy evaluating it
    Given the "02-unreplicated" vector
    And the same vector evaluated under a policy with a replication floor of zero
    Then the two evaluations disagree about the outcome

  Scenario: A policy with no roots is refused
    Given an evaluation vector whose policy declares no roots
    When I evaluate it
    Then it fails
    And it says a policy must declare at least one root

  @must-16
  Scenario: An unbounded iteration count is refused
    Given an evaluation vector whose policy asks for 5000 iterations
    When I evaluate it
    Then it fails
    And it says iterations exceed the limit
