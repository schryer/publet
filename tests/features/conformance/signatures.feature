@conformance @must-4.4
Feature: A signature covers a stated purpose
  Without domain separation a signature made for one purpose could be
  presented as a signature for another, and an endorsement replayed as a
  retraction.

  Scenario: The signed message is domain-separated
    Given a well-formed object
    When I ask for the message a signature would cover with purpose "endorse"
    Then the message begins with the protocol domain string
    And the purpose appears between separators

  Scenario: Two purposes over one object produce different messages
    Given a well-formed object
    When I ask for the messages for purposes "endorse" and "retract"
    Then the two messages differ

  Scenario: Separators prevent a purpose and a target from colliding
    # A NUL byte cannot traverse argv, so the input that would make a
    # purpose ambiguous is unreachable from the command line; the library
    # rejects it, and what is observable here is the property the separator
    # exists to provide.
    Given a well-formed object
    When I ask for the messages for purposes "ab" and "a"
    Then the two messages differ
    And neither is a prefix of the other beyond the separator

  Scenario: A purpose is required
    Given a well-formed object
    When I ask for a message without naming a purpose
    Then it fails
