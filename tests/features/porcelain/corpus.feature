@porcelain @must-14.1
Feature: Project corpora build on parent corpora they are pinned to
  A corpus is a workspace whose objects form a Section 14.1 domain, built
  from named sources. A child names a parent's publet as `alias:slug`,
  never by a bare slug, and is pinned to the parent generation it built
  against. What it reaches in a parent is copied in at identical
  identifiers, so the child stands alone. Building makes unsigned drafts;
  publishing signs and exports them and advances the corpus's generation.

  Background:
    Given a parent corpus "base" declaring the publet "tool.x" at version "1.0"
    And a child corpus "proj" whose publet cites "base:tool.x#identity"

  Scenario: A child copies what it reaches and reads without its parent
    When I publish the child
    Then it succeeds
    And the child's objects include the parent's "tool.x#identity" claim
    And the child reads its publet with the parent directory moved away

  Scenario: A parent's publet is never reached by a bare name
    When the child's publet cites "tool.x" and I build the child
    Then it fails
    And stderr mentions "write `base:tool.x`"

  Scenario: A child learns its parent moved on, and upgrades when it chooses
    When I publish the child
    And the parent's "tool.x" becomes version "1.1" and the parent is rebuilt
    And I ask the child's corpus status
    Then it reports "1 generation(s) ahead"
    And it reports "base:tool.x#identity has a newer version"
    When I publish the child again
    Then it reports "membership unchanged"
    When I upgrade the child and publish it again
    Then the child's publet is published again
    And the child's generation is 1

  Scenario: An import newer than the pin is refused until the child upgrades
    When I publish the child
    And the parent declares a new publet "tool.y" and is rebuilt
    And the child's publet also cites "base:tool.y" and I build the child
    Then it fails
    And stderr mentions "pub corpus upgrade base"

  Scenario: Superseding a parent's publet proposes, unless the parent delegated
    When the child supersedes the parent's "tool.x" and I dry-run the child
    Then the superseding publet is reported "draft-proposal"
    When the parent delegates its lineage to the child's key and is rebuilt
    And I upgrade the child and dry-run it
    Then the superseding publet is reported "draft"

  Scenario: The network map spans both corpora and the external source
    When I publish the child
    And I map the child's corpus as JSON
    Then the map holds corpora "proj" and "base"
    And the map shows "proj" citing "base"
    And the map shows an external link to "https://example.org/x"

  Scenario: Rendering inside a corpus reads the corpus's own lock
    When I publish the child
    And I check the child's publet for rendering
    Then it fails
    And stderr mentions "names no render pipeline"
