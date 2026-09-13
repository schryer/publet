@conformance @must-15
Feature: An unrecognized protocol version is rejected
  A later version may give an existing field a different meaning. Reading it
  under this version's rules would produce a confident wrong answer rather
  than an error, so partial interpretation is refused.

  Scenario: An object declaring a future version is refused
    Given an object declaring protocol version "9"
    When I run "pub-verify" with that object on stdin
    Then the exit code is 1
    And stderr mentions "unsupported protocol version"

  Scenario: An object declaring the current version is accepted
    Given an object declaring protocol version "1"
    When I run "pub-verify" with that object on stdin
    Then the exit code is 0

  Scenario: Extension fields are ignored, not rejected
    Given an object carrying an unrecognized ext field
    When I run "pub-verify" with that object on stdin
    Then the exit code is 0
