@conformance @must-4.1
Feature: Canonical form is rejected, never repaired
  An object's identity is the hash of its bytes. If a decoder silently
  normalized non-canonical input, two different encodings could claim one
  identity, so section 4.1 requires rejection rather than repair.

  Scenario Outline: Non-canonical encodings are refused
    Given an object encoded with <defect>
    When I run "pub-canon" with that object on stdin
    Then the exit code is 1
    And stderr names the violated rule "<rule>"
    And stdout is empty

    Examples:
      | defect                     | rule                   |
      | a non-shortest integer     | shortest-form integers |
      | unsorted map keys          | sorted map keys        |
      | a duplicate map key        | unique map keys        |
      | an indefinite-length array | no indefinite length   |
      | a float value              | no floating point      |
      | text in NFD                | NFC normalization      |
      | a non-text map key         | text map keys          |
      | invalid UTF-8              | valid UTF-8            |
      | a CBOR tag                 | supported item types   |
      | trailing data              | single top-level item  |

  Scenario: A canonical object passes through unchanged
    Given a well-formed object
    When I run "pub-canon" with that object on stdin
    Then the exit code is 0
    And stdout is byte-identical to the input

  Scenario: There is no option that repairs non-canonical input
    Given an object encoded with a non-shortest integer
    When I run "pub-canon" with that object on stdin
    Then the exit code is 1
    And stdout is empty
