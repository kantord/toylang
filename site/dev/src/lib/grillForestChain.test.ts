import { strict as assert } from "node:assert"
import { test } from "node:test"

import { activePath, threadStatus, type ForestNode } from "./grillForestChain.ts"

function node(partial: Partial<ForestNode> & Pick<ForestNode, "id" | "parent" | "status">): ForestNode {
  return { title: partial.id, question: `question for ${partial.id}`, ...partial }
}

test("no root and no activity yields an empty chain", () => {
  assert.deepEqual(activePath([], []), [])
})

test("a brand-new thread's first question being drafted shows as activity, not a node", () => {
  const path = activePath([], [{ parent: null, note: "Drafting the first question...", since: "2026-01-01" }])
  assert.deepEqual(path, [{ kind: "activity", note: "Drafting the first question..." }])
})

test("a lone live root renders as one node, chain ends there", () => {
  const root = node({ id: "root", parent: null, status: "live" })
  assert.deepEqual(activePath([root], []), [{ kind: "node", node: root }])
})

test("walks through answered nodes to the trailing live question", () => {
  const root = node({ id: "root", parent: null, status: "answered" })
  const child = node({ id: "child", parent: "root", status: "live" })
  const path = activePath([root, child], [])
  assert.deepEqual(path, [
    { kind: "node", node: root },
    { kind: "node", node: child },
  ])
})

test("a superseded root ends the chain -- no sibling-of-a-root resolution, that's the deferred multi-root case", () => {
  const root = node({ id: "root", parent: null, status: "superseded", supersededNote: "no longer relevant" })
  const path = activePath([root], [{ parent: "root", note: "should never show", since: "2026-01-01" }])
  assert.deepEqual(path, [{ kind: "node", node: root }])
})

test("a superseded non-root node's replacement sibling continues the chain instead of stalling", () => {
  const root = node({ id: "root", parent: null, status: "answered" })
  const superseded = node({
    id: "child-a",
    parent: "root",
    status: "superseded",
    supersededNote: "no longer relevant, asking something else instead",
  })
  const replacement = node({ id: "child-b", parent: "root", status: "live" })
  const path = activePath([root, superseded, replacement], [])
  assert.deepEqual(path, [
    { kind: "node", node: root },
    { kind: "node", node: superseded },
    { kind: "node", node: replacement },
  ])
})

test("a chain of two retracted siblings still terminates and reaches the eventual live tail", () => {
  // Reproduces a real infinite loop: excluding only `current.id` (not every node already shown)
  // let the walk bounce backward from child-b to the already-superseded child-a forever, since
  // child-a always sorted lowest among "everyone but myself." Excluding all visited nodes fixes
  // it -- this test is the regression guard for that specific bug, not just the general shape.
  const root = node({ id: "root", parent: null, status: "answered" })
  const first = node({ id: "child-a", parent: "root", status: "superseded", supersededNote: "retracted" })
  const second = node({ id: "child-b", parent: "root", status: "superseded", supersededNote: "retracted again" })
  const third = node({ id: "child-c", parent: "root", status: "live" })
  const path = activePath([root, first, second, third], [])
  assert.deepEqual(path, [
    { kind: "node", node: root },
    { kind: "node", node: first },
    { kind: "node", node: second },
    { kind: "node", node: third },
  ])
})

test("a superseded non-root node with no replacement yet shows its own thread's activity", () => {
  const root = node({ id: "root", parent: null, status: "answered" })
  const superseded = node({
    id: "child-a",
    parent: "root",
    status: "superseded",
    supersededNote: "no longer relevant",
  })
  const activity = [{ parent: "root", note: "Drafting a replacement...", since: "2026-01-01" }]
  const path = activePath([root, superseded], activity)
  assert.deepEqual(path, [
    { kind: "node", node: root },
    { kind: "node", node: superseded },
    { kind: "activity", note: "Drafting a replacement..." },
  ])
})

test("an activity entry appends after an answered leaf with no live child yet", () => {
  const root = node({ id: "root", parent: null, status: "answered" })
  const path = activePath([root], [{ parent: "root", note: "Drafting a follow-up...", since: "2026-01-01" }])
  assert.deepEqual(path, [
    { kind: "node", node: root },
    { kind: "activity", note: "Drafting a follow-up..." },
  ])
})

test("multiple non-draft children under one node: the lowest id wins, deterministically", () => {
  const root = node({ id: "root", parent: null, status: "answered" })
  const childB = node({ id: "child-b", parent: "root", status: "live" })
  const childA = node({ id: "child-a", parent: "root", status: "live" })
  // Listed in file order b-then-a on purpose: the tie-break must not depend on array order.
  const path = activePath([root, childB, childA], [])
  assert.deepEqual(path, [
    { kind: "node", node: root },
    { kind: "node", node: childA },
  ])
})

test("multiple roots (the deferred multi-root case): the lowest id wins too, same tie-break as siblings", () => {
  const rootB = node({ id: "root-b", parent: null, status: "live" })
  const rootA = node({ id: "root-a", parent: null, status: "live" })
  const path = activePath([rootB, rootA], [])
  assert.deepEqual(path, [{ kind: "node", node: rootA }])
})

test("threadStatus: idle with nothing in flight", () => {
  const root = node({ id: "root", parent: null, status: "answered" })
  assert.equal(threadStatus({ topic: "t", activity: [], nodes: [root] }), "idle")
})

test("threadStatus: waiting on a trailing live question", () => {
  const root = node({ id: "root", parent: null, status: "live" })
  assert.equal(threadStatus({ topic: "t", activity: [], nodes: [root] }), "waiting")
})

test("threadStatus: thinking while the agent drafts a follow-up", () => {
  const root = node({ id: "root", parent: null, status: "answered" })
  const activity = [{ parent: "root", note: "Drafting...", since: "2026-01-01" }]
  assert.equal(threadStatus({ topic: "t", activity, nodes: [root] }), "thinking")
})
