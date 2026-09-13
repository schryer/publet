@conformance @must-5.7
Feature: A publet asserts one thing, and no two publets are merged for me
  The structural tests are advisory: a flagged publet is a valid publet,
  and the flag is a question put to its author while the fix is still
  cheap. What is not advisory is the refusal to merge: whether two
  phrasings assert the same thing is a claim, and a claim is made by
  signing it, not by a threshold inside a tool nobody can dispute.

  Scenario: A compound assertion is flagged
    Given a publet joining two assertions with a conjunction
    When I lint the store
    Then a "single-assertion" finding is reported

  Scenario: A publet opening with a pronoun is flagged
    Given a publet opening with a pronoun
    When I lint the store
    Then a "dangling-anaphora" finding is reported

  Scenario: A flagged publet is still a valid publet
    Given a publet joining two assertions with a conjunction
    When I load the store
    Then it succeeds

  Scenario Outline: Atomicity is not applied where content is not an assertion
    Given a "<class>" publet joining two clauses
    When I lint the store
    Then no finding is reported

    Examples:
      | class       |
      | procedural  |
      | attributive |
      | archival    |
      | expressive  |

  Scenario Outline: Atomicity still applies where content is the assertion
    Given a "<class>" publet joining two clauses
    When I lint the store
    Then a "single-assertion" finding is reported

    Examples:
      | class        |
      | formal       |
      | definitional |
      | normative    |

  Scenario: A method may decompose into sub-procedures
    Given a method naming sub-procedures that name further ones
    When I ask for the dependency closure of the method
    Then every sub-procedure is reached
    And no finding is reported for any of them

  Scenario: Self-containment applies whatever the class
    Given a "procedural" publet opening with a pronoun
    When I lint the store
    Then a "dangling-anaphora" finding is reported

  Scenario: A contested term used without being declared is flagged
    Given a publet using a disputed term it does not declare
    When I lint the store
    Then an "undeclared-term" finding is reported

  Scenario: Declaring the contested term clears the finding
    Given a publet using a disputed term it declares
    When I lint the store
    Then no finding is reported

  Scenario: Two near-identical publets both survive
    Given two publets whose wording differs by one word
    When I list the store
    Then 2 publets are listed

  Scenario: Two publets with identical wording both survive
    Given two publets with identical wording from different authors
    When I list the store
    Then 2 publets are listed
