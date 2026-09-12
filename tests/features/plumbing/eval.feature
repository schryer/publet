@plumbing @must-11.5
Feature: The acceptance predicate reaches every outcome

  Scenario Outline: Each vector evaluates to its specified outcome
    Given the "<vector>" vector
    When I evaluate it
    Then the result is "<outcome>"

    Examples:
      | vector          | outcome       |
      | 01-chain        | accepted      |
      | 02-unreplicated | unreplicated  |
      | 03-refuted      | rejected      |
      | 04-restricted   | unreplicated  |
      | 05-proof        | accepted      |
      | 06-contested    | contested     |
      | 08-normative    | not-truth-apt |
      | 10-retracted    | rejected      |

  Scenario: Unvouched disputes contribute nothing to the divergence factor
    Given the "07-dispute-spam" vector
    When I evaluate it
    Then the delta is "0.000000"
    And the result is "accepted"

  Scenario: A weighted argued dispute raises the divergence factor
    Given the "06-contested" vector
    When I evaluate it
    Then the delta is greater than "0.000000"
