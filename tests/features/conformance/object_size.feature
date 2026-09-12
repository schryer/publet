@conformance @must-4.5
Feature: Objects are bounded
  Content that does not fit is referenced as an opaque blob rather than
  carried, so the ceiling is a property of the format rather than advice.

  Scenario: An object at the ceiling is accepted
    Given canonical bytes of exactly 65536 bytes
    When I run "pub-canon" with that object on stdin
    Then the exit code is 0

  Scenario: An object above the ceiling is refused
    Given canonical bytes of 65537 bytes
    When I run "pub-canon" with that object on stdin
    Then the exit code is 1
    And stderr names the violated rule "object size limit"
