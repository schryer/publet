@conformance @must-5.8
Feature: A claim about values carries them as data
  `content` stays the one sentence saying what the values are, and `data`
  carries the values: typed columns and rows, with no floating point and no
  presentation. A table a reader would have to guess the shape of is
  refused rather than stored.

  Background:
    Given a workspace

  Scenario: A table is carried with its columns and units
    When I compose a budget claim with the data
      """
      {"columns": [{"name": "item"}, {"name": "spend", "unit": "EUR"}],
       "rows": [["seed trays", "120.00"], ["grow lights", "480.00"]]}
      """
    Then it succeeds
    And reading it shows "table, 2 column(s), 2 row(s)"
    And reading it shows "spend (EUR)"

  Scenario: A row narrower than the columns is refused and nothing is stored
    When I compose a budget claim with the data
      """
      {"columns": [{"name": "item"}, {"name": "spend", "unit": "EUR"}],
       "rows": [["seed trays"]]}
      """
    Then it fails
    And stderr mentions "rows exactly as wide as the columns"
    And nothing new is stored

  Scenario: Two columns with one name are refused
    When I compose a budget claim with the data
      """
      {"columns": [{"name": "spend"}, {"name": "spend"}], "rows": []}
      """
    Then it fails
    And stderr mentions "unique within the table"

  Scenario: A floating-point number is refused with the reason
    When I compose a budget claim with the data
      """
      {"columns": [{"name": "item"}, {"name": "spend", "unit": "EUR"}],
       "rows": [["seed trays", 120.5]]}
      """
    Then it fails
    And stderr mentions "Section 4.1"
    And stderr mentions "data.rows[0][1]"

  Scenario: Integers and empty cells are values
    When I compose a budget claim with the data
      """
      {"columns": [{"name": "item"}, {"name": "count"}],
       "rows": [["seed trays", 12], ["spare", null], ["returned", -2]]}
      """
    Then it succeeds

  Scenario: Data with no source is accepted but the author is warned
    When I compose a budget claim with the data
      """
      {"columns": [{"name": "item"}], "rows": [["seed trays"]]}
      """
    Then it succeeds
    And stderr mentions "names no --source"
