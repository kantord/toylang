import { strict as assert } from "node:assert"
import { test } from "node:test"

import { FLOW, FLOW_FOR_TYPE, NEUTRAL, flowStyle } from "./flow.ts"

test("known flow values use their table entry", () => {
  assert.deepEqual(flowStyle("question"), FLOW.question)
})

test("unrecognized flow values fall back to NEUTRAL", () => {
  assert.deepEqual(flowStyle("decide"), NEUTRAL)
})

test("review maps to escalation", () => {
  assert.equal(FLOW_FOR_TYPE.review, "escalation")
}
)