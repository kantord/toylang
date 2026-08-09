# Working agreement for AI agents

This repository is a language-design project. Much of it is prose and design reasoning rather
than code, so where an idea came from matters as much as whether it works. These rules exist to
keep that traceable.

Read `CLAUDE.md` as well. It governs how things are written; this file governs where they came
from and what you are allowed to do on your own.

## Agents may commit

You do not need permission to commit here. Write atomic commits, one coherent change each, and
push them yourself. The only obligation is to record provenance briefly in the message.

## Provenance vocabulary

Four categories, describing where content came from rather than who typed it.

**Human-written.** Copied verbatim from what a human wrote, or edited only superficially:
formatting, typos, wrapping. The human produced the text itself.

**Human-authored.** The content is the logical interpretation of what a human said. The test:
if the human's instruction had been pasted into the file instead of the expanded result, would
it carry the same value? If someone with sufficient expertise, human or agent, would derive
substantially this result from that instruction, it is human-authored.

This category is wider than it first looks, and that is intentional. **Anything produced by an
exploratory loop counts as human-authored**, where an agent proposes options and a human makes
the design decisions. That holds even when large stretches of the resulting text were generated
verbatim by the agent. The human decided; the agent drafted. Authored is deliberately distinct
from written, and most good agent work in this repo is human-authored and agent-written.

**Derived.** Follows from constraints already present: existing patterns in this repo, a
decision recorded earlier, or a cited external source. When the existing constraints leave
essentially one reasonable option, taking it is derived, not invented. If the constraint itself
came from a human, the result is human-authored instead.

**Agent-invented.** A real decision an agent made on its own. Something is agent-invented only
when all of these hold:

- it did not come out of a human authoring or exploration loop,
- it was not forced by existing patterns or constraints,
- it is not obvious, in the sense that there was more than one reasonable option,
- it did not come from a cited source.

Carve this down to the **minimal surface** that actually meets those tests. A section can be
human-authored overall while one specific choice inside it was purely the agent's call, and
that choice is what to name. The point is not to apportion credit. It is to make visible which
decisions no human has actually made, so a reader can tell a requirement from an accident.

External material can land in any of these categories. Consulting a source does not by itself
make the result derived, since an agent may research a source and then decide something well
beyond it. Judge honestly which happened.

## Commit message format

Keep messages brief. A subject line, then only the provenance lines that apply. Omit categories
that are empty rather than writing "none".

```
Add cardinality types to the design draft

Human-authored: the Vec/Stream split and the indexing-promise distinction
Derived: jq operator semantics (jq 1.7 manual)
Agent-invented: the `fold` block syntax

Co-Authored-By: <agent identifier>
```

A commit with no agent `Co-Authored-By` trailer is entirely human-authored and human-written.
That is the default assumption, so never add the trailer to a commit you did not contribute to.

## Correcting provenance

Provenance is a fact about how a file came to be, not a property recoverable from the file
afterwards. It has to be recorded when the commit is made or it is gone. That is why it lives
in commit messages.

Two consequences.

**Do not classify inherited files confidently.** If a file arrives already written and you do
not know its authoring history, the honest entry is that the history is unknown. Nothing in a
finished artifact distinguishes text a human wrote from text an agent wrote, and guessing
produces a false record that looks like a true one.

**If you get provenance wrong, do not rewrite history.** Add a follow-up commit that names the
bad commit and states the correction. Both entries stay in the log. A rewritten history is a
worse record than a corrected one, because it hides that the mistake was possible.

## Authority ordering

This is the practical reason the categories exist. When two parts of the repository conflict:

1. Human-written and human-authored content wins.
2. Derived content yields to human-authored content.
3. Agent-invented content has the least authority. It is a placeholder that survived, not a
   decision.

If you are changing something human-authored because agent-invented content elsewhere assumed
otherwise, you have it backwards. Change the invented part, or raise the conflict.

## Stop and ask

Flag these to a human and wait for explicit authorization. Flag before writing to files where
possible, and always before committing:

- Licensing or copyright, where the license is unclear, restrictive, or unknown
- Security, including anything touching credentials, keys, or attack surface
- Personal information, meaning anything identifying a person, from any source
- Ethical concerns, meaning anything you would want a human to weigh
- Access, meaning anything requiring credentials or permissions you were not clearly granted

Explain the specific concern rather than asking generically, and do not proceed on silence.

## Never include

Copyrighted material from other repositories. Reference and describe freely; do not copy source
text. Concepts and documented behavior are fine to describe in your own words.

Personal information or secrets about any person: names, contact details, credentials, private
paths, internal hostnames, or anything else identifying or private. This includes material
about the repository owner.

Paths or details from unrelated private repositories or systems. Design rationale should stand
on its own without them.

## Citing external material

When content draws on an external source, name it, in the commit message and in the file itself
where a reader would want it. "Derived from the jq manual" is enough; a link is better.

This matters most where the language deliberately borrows from an existing one. A reader should
be able to tell "jq does it this way" from "we decided this."
