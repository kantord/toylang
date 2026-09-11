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

test("a superseded node ends the chain with no further activity check", () => {
  const root = node({ id: "root", parent: null, status: "superseded", supersededNote: "no longer relevant" })
  const path = activePath([root], [{ parent: "root", note: "should never show", since: "2026-01-01" }])
  assert.deepEqual(path, [{ kind: "node", node: root }])
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
