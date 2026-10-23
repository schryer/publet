@conformance
Feature: A singular is one particular thing, and a fact is a value about it
  A singular is a definitional claim with `defines.sort` "singular": a
  name, the PDC code of its class, and optionally the singular it is part
  of and the H3 cell it is in. A fact table's `about` column names the
  singular each row is about, and its value columns name their quantity
  concept. What either cites, the claim presupposes.

  Background:
    Given a workspace with the Tallinn basement

  @must-5.9.1 @must-5.9.6
  Scenario: A singular names its class and what it is part of, which it presupposes
    Then reading the basement shows "class     definitional"
    And the basement presupposes the Tallinn site and the Place concept

  @must-5.9.1
  Scenario Outline: A singular of the wrong shape is refused
    When I compose a singular defining
      """
      <defines>
      """
    Then it fails
    And stderr mentions "<reason>"

    Examples:
      | defines                                                                          | reason              |
      | {"sort": "singular", "class": "(1/9)"}                                            | defines.name        |
      | {"sort": "singular", "name": "shed"}                                              | defines.class       |
      | {"sort": "singular", "name": "shed", "class": "(1/9)", "part_of": "next door"}    | defines.part_of     |
      | {"sort": "singular", "name": "shed", "class": "(1/9)", "location": {"h3": "8A089B1A2177FFF"}} | defines.location.h3 |
      | {"sort": "singular", "name": "shed", "class": "(1/9)", "location": {"h3": "8a089b1a2177ff7"}} | defines.location.h3 |
      | {"sort": "singular", "name": "shed", "class": "(1/9)", "location": {"lat": "59.4"}} | defines.location    |

  @must-5.9.6
  Scenario: A singular whose class has no concept in depends is refused
    When I compose a singular of class "(1/9)~7" with no concept for it
    Then it fails
    And stderr mentions "PDC code (1/9)~7 is named"

  @must-5.9.6
  Scenario: A singular part of something that is not a singular is refused
    When I compose a singular part of the floor area concept
    Then it fails
    And stderr mentions "does not define a singular"

  @must-5.9.6
  Scenario: A fact presupposes the singular its row is about
    Then the floor area fact presupposes the basement

  @must-5.9.6
  Scenario: An about cell that is not a CID is refused
    When I compose a fact about "the basement"
    Then it fails
    And stderr mentions "the CID of a singular, or null, in the `about` column"

  @must-5.9.6
  Scenario: An about cell naming a claim that is not a singular is refused
    When I compose a fact about the floor area concept
    Then it fails
    And stderr mentions "does not define a singular"

  @must-5.9.6
  Scenario: An about column with a unit is refused
    When I compose a fact whose about column has the unit "m2"
    Then it fails
    And stderr mentions "an `about` column with no unit, kind, measure or concept"

  @must-5.9.6
  Scenario: A column whose concept has no concept claim in depends is refused
    When I compose a fact whose column's concept is "69~9"
    Then it fails
    And stderr mentions "PDC code 69~9 is named"
