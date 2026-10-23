Feature: pub get finds the current value of a fact by its path key
  A path key is computed from the names in the claims held: the class of
  the outermost thing and the classes it narrows, the chain of singulars
  down to the one the fact is about, and the fact's quantity concept.
  `pub get` shows the head of the fact's lineage with its value, unit,
  measure, evidence roles, and the status they imply by convention.

  Background:
    Given a workspace with the Tallinn basement

  Scenario: The owner's figure for the basement is confirmed
    When I get "place.site.tallinn.basement.floor_area"
    Then it succeeds
    And it prints "place.site.tallinn.basement.floor_area"
    And it prints "value     value 50"
    And it prints "unit      m2"
    And it prints "measure   exact value"
    And it prints "evidence  method, statement"
    And it prints "status    confirmed"
    And it prints "[local]"

  Scenario: A singular's path without a concept gives every fact about it
    When I get "place.site.tallinn.basement"
    Then it succeeds
    And it prints "place.site.tallinn.basement.floor_area"

  Scenario: A revised fact is read at the head of its lineage
    Given the floor area fact is revised to 52
    When I get "place.site.tallinn.basement.floor_area"
    Then it succeeds
    And it prints "value     value 52"
    And it does not print "value     value 50"

  Scenario: A range is an estimate
    Given an estimate of the basement's ceiling height
    When I get "place.site.tallinn.basement.ceiling_height"
    Then it succeeds
    And it prints "status    estimate"

  Scenario: A disputed figure is to be verified
    Given a dispute of the floor area fact
    When I get "place.site.tallinn.basement.floor_area"
    Then it succeeds
    And it prints "status    to-verify (1 annotation(s) leave it open"

  Scenario: A cell finds the facts about the things located in it
    When I get "place.87089b1a2ffffff"
    Then it succeeds
    And it prints "place.site.tallinn.basement.floor_area"

  Scenario: A cell elsewhere finds nothing
    When I get "place.813f7ffffffffff.floor_area"
    Then it fails
    And stderr mentions "nothing held here answers"

  Scenario: A path nothing has is answered by nothing
    When I get "place.site.tartu.basement.floor_area"
    Then it fails
    And stderr mentions "nothing held here answers"

  Scenario: A branched lineage shows every head
    Given the floor area fact is revised to 52
    And the floor area fact is revised to 48
    When I get "place.site.tallinn.basement.floor_area"
    Then it succeeds
    And it prints "lineage   branched: 2 heads"
    And it prints "value     value 52"
    And it prints "value     value 48"

  Scenario: A value not yet known is to be verified
    Given the floor area fact is revised to unknown
    When I get "place.site.tallinn.basement.floor_area"
    Then it succeeds
    And it prints "value     null: not yet known"
    And it prints "status    to-verify (no value is stated yet"

  Scenario Outline: A malformed lookup is refused
    When I run pub get with <arguments>
    Then it fails
    And stderr mentions "<reason>"

    Examples:
      | arguments                                         | reason                          |
      | no arguments                                      | usage: pub get PATH             |
      | "place..basement"                                 | a path is segments joined by dots |
      | "place.site" and "place.site"                     | one path at a time              |
      | "--json"                                          | unknown argument: --json        |
      | "place.87089b1a2ffffff.floor_area.extra"          | after an H3 cell, a path names at most a concept |

  Scenario: A class whose code has concepts of two names has no path
    Given a shed whose class has concepts named "Place" and "Area"
    When I get "place.shed"
    Then it fails
    And stderr mentions "nothing held here answers"
