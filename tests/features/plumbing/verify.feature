@plumbing @must-4.2 @must-4.3
Feature: Objects are verified against the identifier they were fetched by

  Scenario: An object verifies against its own identifier
    Given a well-formed object
    When I verify it against the CID this suite computed
    Then the exit code is 0

  Scenario: An object does not verify against another object's identifier
    Given a well-formed object
    And a second object differing only in its created field
    When I verify the first against the second's CID
    Then the exit code is 1
    And stderr mentions "identifier mismatch"

  Scenario Outline: A missing required header field is refused
    Given a well-formed object with the <field> field removed
    When I run "pub-verify" with that object on stdin
    Then the exit code is 1
    And stderr mentions "missing required field"

    Examples:
      | field   |
      | pub     |
      | type    |
      | created |
      | author  |
      | body    |

  Scenario: An unrecognized top-level field is refused
    Given a well-formed object with an extra top-level field "colour"
    When I run "pub-verify" with that object on stdin
    Then the exit code is 1
    And stderr mentions "extensions belong in"
