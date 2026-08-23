# Working agreement for AI agents

This repository is a language-design project, mostly prose and design reasoning rather than
code. Two things follow: how something is written is part of the work, and where an idea came
from matters as much as whether it works.

# Writing

The goal is not to pass as human-written. It's that the usual tells are downstream of real
defects, and fixing the defect removes the tell for free. Uniform section lengths mean attention
wasn't allocated. Comments that restate the code mean nothing was observed. A Roadmap heading
over an empty list means the template was filled in rather than the document written. Chase the
defect, not the fingerprint.

## The rule most of this reduces to

Detail should track how much the thing deserves. The hard part gets four paragraphs; the obvious
part gets a line, or nothing. Asymmetry is what attention looks like on the page.

## Prose and documentation

Write what you observed, not what should be true. "Fails on inputs with a BOM" is worth more
than three paragraphs correctly describing the happy path.

Stop when you're done. No closing summary, no "Key takeaways," no paragraph restating the
section above it. If the document is short enough to read, it's short enough not to need a
recap.

Name the thing, not its label. "Q8" and "option 3" tell a reader nothing and cost them a lookup;
"the vectorizability question" says what is being discussed. A short identifier is fine as the
*heading* of the thing it names, so that it can be referred to and linked, but it is not a
substitute for saying what the reference is about.

Cross-references inside a document are links. If one section points at another, the reader
should be able to follow it, which means the target is a heading and the reference is real
markdown. A reference that only names a section leaves the reader to search for it.

Add a heading when a reader needs to navigate past something. Six headings over 400 words is
furniture, not structure. Same for horizontal rules: they separate genuinely unrelated material,
which is rare inside one document.

Don't write a section you can't fill. A 200-line script doesn't need Contributing, Roadmap,
Acknowledgements, or a badge row.

Bold-lead bullets (`**Term**: gloss`) are fine two or three at a time. A screen of them is either
a table or a paragraph that hasn't admitted it yet. Tables are for data with two dimensions; a
list of things is a list.

No emoji as section markers or status indicators. If the project already uses them, match the
project.

Some phrases to cut on sight, because they consume a sentence without adding one: "It's worth
noting that," "It's important to remember," "not just X, but Y," "in today's fast-paced world,"
"delve into," "leverage" where "use" works, and the adjective stack of robust / seamless /
comprehensive / powerful applied to things whose robustness has not been measured.

## Comments

Comment the why. The what is already there in the code, in a form that can't drift out of date.

```python
# Retry with backoff: the upstream returns 502 for ~30s after a deploy.
```

not

```python
# Retry the request
```

Docstring length should track surprise, not function length. A three-line helper whose name says
what it does needs nothing. A three-line helper that must be called before `connect()` or it
silently no-ops needs a sentence saying so.

No banner comments dividing a file into decorated sections. If the file needs sections that
badly, it needs to be two files.

## Code shape

Build for callers that exist. If nothing calls `from_dict`, don't write `from_dict` because
`to_dict` exists. Symmetry is not a requirement.

Don't guard against states that can't happen. A null check on a value constructed two lines above
is noise, and it trains readers to skim past the checks that matter. Where a real failure is
possible, prefer failing loudly over swallowing it into a default.

Short names are fine in short scopes. `i`, `n`, `ctx`, `df` are the local dialect and everyone
reads them faster than `current_row_index`. Reach for the long name when the scope is long or the
meaning is genuinely non-obvious.

Match the surrounding file, including conventions this guide dislikes. Consistency with the
codebase outranks consistency with this document. Don't reformat code you weren't asked to touch,
and don't fold a style cleanup into a behavior change.

Don't put emoji or checkmarks in console output unless the project already does.

## Tests

Test behavior that matters, not the shape of the implementation. A test file that mirrors the
source file function-for-function is usually testing that the code was typed, not that it works.

Every fixed bug gets the test that would have caught it. That test is worth more than the whole
happy-path suite.

Don't assert things that cannot fail. Asserting that a constructor set the field you just passed
it will never once be red.

## Uncertainty

Say what you're unsure about and what would settle it. This is only useful if it's selective:
hedging everything equally carries no information, and it reads as insurance rather than honesty.

Never invent an API signature. If you can't verify a call, say the call is unverified, or check
it. A plausible-looking wrong argument is the most expensive thing in this document, because it
survives review. It looks exactly like knowledge.

Don't announce compliance. No "As requested," no "I've carefully ensured," no summary of your own
diligence at the end of a PR description. Describe the change and why.

## Typography

Plain ASCII everywhere: code, identifiers, commit messages, config, and prose. Straight quotes,
`--` not an em dash, three periods not a single-character ellipsis, `->` not an arrow glyph. This
is about grep, diffs, and terminals. Characters a person would not type on a keyboard are noise
in a git repo. A person's name is whatever they spell it as, and is not subject to this.

If you find yourself reaching for `--` more than once or twice a page, a run of dashes is a
paragraph that hasn't picked its sentence boundaries. Use a period, a colon, or parentheses.

## What not to do in the name of this guide

Don't manufacture the appearance of human authorship. Specifically: no deliberate typos, no
invented TODO or HACK comments, no fabricated dated notes or initials, no commented-out code that
was never live, no artificial inconsistency between files, and no performed uncertainty about
things you're actually confident in.

All of these are false claims about how the artifact came to be, embedded in the artifact. That's
worse than any stylistic tell, and it's the failure mode this guide is most likely to induce if
read carelessly.

# Provenance

Four categories, describing where content came from rather than who typed it.

**Human-written.** Copied verbatim from what a human wrote, or edited only superficially:
formatting, typos, wrapping. The human produced the text itself.

**Human-authored.** The content is the logical interpretation of what a human said. The test: if
the human's instruction had been pasted into the file instead of the expanded result, would it
carry the same value? If someone with sufficient expertise, human or agent, would derive
substantially this result from that instruction, it is human-authored.

**Anything produced by an exploratory loop counts as human-authored**, where an agent proposes
options and a human makes the design decisions. That holds even when large stretches of the
resulting text were generated verbatim by the agent.

**Derived.** Follows from constraints already present: existing patterns in this repo, a decision
recorded earlier, or a cited external source. When the existing constraints leave essentially one
reasonable option, taking it is derived, not invented. If the constraint itself came from a
human, the result is human-authored instead.

**Agent-invented.** A real decision an agent made on its own. Something is agent-invented only
when all of these hold:

- it did not come out of a human authoring or exploration loop,
- it was not forced by existing patterns or constraints,
- it is not obvious, in the sense that there was more than one reasonable option,
- it did not come from a cited source.

Carve this down to the **minimal surface** that actually meets those tests. A section can be
human-authored overall while one specific choice inside it was purely the agent's call, and that
choice is what to name. The purpose is to make visible which decisions no human has actually
made, so a reader can tell a requirement from an accident.

Consulting a source does not by itself make a result derived. An agent may research a source and
then decide something well beyond it.

## Correcting provenance

Provenance is a fact about how a file came to be, not a property recoverable from the file
afterwards. It has to be recorded when the commit is made or it is gone.

**Do not classify inherited files confidently.** If a file arrives already written and you do not
know its authoring history, the honest entry is that the history is unknown. Nothing in a
finished artifact distinguishes text a human wrote from text an agent wrote, and guessing
produces a false record that looks like a true one.

**If you get provenance wrong, do not rewrite history.** Add a follow-up commit that names the
bad commit and states the correction. Both entries stay in the log. A rewritten history is a
worse record than a corrected one, because it hides that the mistake was possible.

## Authority ordering

This is the practical reason the categories exist. When two parts of the repository conflict,
human-written and human-authored content wins, derived content yields to it, and agent-invented
content has the least authority: a placeholder that survived, not a decision.

If you are changing something human-authored because agent-invented content elsewhere assumed
otherwise, you have it backwards. Change the invented part, or raise the conflict.

# Committing

Agents may commit in this repository, and must add the `Co-Authored-By` trailer when they do.

The subject line says what changed. The body says why, or what was ruled out, plus the provenance
lines that apply. Omit categories that are empty rather than writing "none". A bulleted inventory
of every file touched duplicates the diff.

```
Add cardinality types to the design draft

Human-authored: the Vec/Stream split and the indexing-promise distinction
Derived: jq operator semantics (jq 1.7 manual)
Agent-invented: the `fold` block syntax

Co-Authored-By: <agent identifier>
```

A commit with no agent `Co-Authored-By` trailer is entirely human-authored and human-written.
That is the default assumption, so never add the trailer to a commit you did not contribute to.

## Citing external material

When content draws on an external source, name it, in the commit message and in the file itself
where a reader would want it. "Derived from the jq manual" is enough; a link is better.

This matters most where the language deliberately borrows from an existing one. A reader should
be able to tell "jq does it this way" from "we decided this."

# Limits

## Stop and ask

Flag these to a human and wait for explicit authorization. Flag before writing to files, not
after:

- Licensing or copyright, where the license is unclear, restrictive, or unknown
- Security or access: credentials, keys, attack surface, or permissions you were not clearly
  granted
- Personal information, meaning anything identifying a person, from any source
- Anything else you would want a human to weigh

Explain the specific concern rather than asking generically, and do not proceed on silence.

## Never include

Copyrighted material from other repositories. Reference and describe freely; do not copy source
text. Concepts and documented behavior are fine to describe in your own words.

Personal information or secrets about anyone, the repository owner included: names, contact
details, credentials, private paths, internal hostnames.

Paths or details from unrelated private repositories or systems. Design rationale should stand on
its own without them.

---

Delete anything here you disagree with. Every rule earns its place by making the repository more
useful to the next person who opens it, and if a rule stops doing that it has no other argument
in its favor. A guide nobody follows costs more than no guide, because it turns every review into
an argument about the document instead of the code.
