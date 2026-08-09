# Style guide

This governs prose, comments, docs, and code shape in this repo.

The goal is not to pass as human-written. It's that the usual tells are downstream of real defects, and fixing the defect removes the tell for free. Uniform section lengths mean attention wasn't allocated. Comments that restate the code mean nothing was observed. A Roadmap heading over an empty list means the template was filled in rather than the document written. Chase the defect, not the fingerprint.

## The rule most of this reduces to

Detail should track how much the thing deserves. The hard part gets four paragraphs; the obvious part gets a line, or nothing. Asymmetry is what attention looks like on the page.

## Prose and documentation

Write what you observed, not what should be true. "Fails on inputs with a BOM" is worth more than three paragraphs correctly describing the happy path.

Stop when you're done. No closing summary, no "Key takeaways," no paragraph restating the section above it. If the document is short enough to read, it's short enough not to need a recap.

Add a heading when a reader needs to navigate past something. Six headings over 400 words is furniture, not structure. Same for horizontal rules: they separate genuinely unrelated material, which is rare inside one document.

Don't write a section you can't fill. A 200-line script doesn't need Contributing, Roadmap, Acknowledgements, or a badge row.

Bold-lead bullets (`**Term**: gloss`) are fine two or three at a time. A screen of them is either a table or a paragraph that hasn't admitted it yet. Tables are for data with two dimensions; a list of things is a list.

No emoji as section markers or status indicators. If the project already uses them, match the project.

Some phrases to cut on sight, because they consume a sentence without adding one: "It's worth noting that," "It's important to remember," "not just X, but Y," "in today's fast-paced world," "delve into," "leverage" where "use" works, and the adjective stack of robust / seamless / comprehensive / powerful applied to things whose robustness has not been measured.

## Comments

Comment the why. The what is already there in the code, in a form that can't drift out of date.

```python
# Retry with backoff: the upstream returns 502 for ~30s after a deploy.
```

not

```python
# Retry the request
```

Docstring length should track surprise, not function length. A three-line helper whose name says what it does needs nothing. A three-line helper that must be called before `connect()` or it silently no-ops needs a sentence saying so.

No banner comments dividing a file into decorated sections. If the file needs sections that badly, it needs to be two files.

## Code shape

Build for callers that exist. If nothing calls `from_dict`, don't write `from_dict` because `to_dict` exists. Symmetry is not a requirement.

Don't guard against states that can't happen. A null check on a value constructed two lines above is noise, and it trains readers to skim past the checks that matter. Where a real failure is possible, prefer failing loudly over swallowing it into a default.

Short names are fine in short scopes. `i`, `n`, `ctx`, `df` are the local dialect and everyone reads them faster than `current_row_index`. Reach for the long name when the scope is long or the meaning is genuinely non-obvious.

Match the surrounding file, including conventions this guide dislikes. Consistency with the codebase outranks consistency with this document. Don't reformat code you weren't asked to touch, and don't fold a style cleanup into a behavior change.

Don't put emoji or checkmarks in console output unless the project already does.

## Tests

Test behavior that matters, not the shape of the implementation. A test file that mirrors the source file function-for-function is usually testing that the code was typed, not that it works.

Every fixed bug gets the test that would have caught it. That test is worth more than the whole happy-path suite.

Don't assert things that cannot fail. Asserting that a constructor set the field you just passed it will never once be red.

## Uncertainty

Say what you're unsure about and what would settle it. This is only useful if it's selective: hedging everything equally carries no information, and it reads as insurance rather than honesty.

Never invent an API signature. If you can't verify a call, say the call is unverified, or check it. A plausible-looking wrong argument is the most expensive thing in this document, because it survives review. It looks exactly like knowledge.

Don't announce compliance. No "As requested," no "I've carefully ensured," no summary of your own diligence at the end of a PR description. Describe the change and why.

## Commit messages

Subject line says what changed. Body, if there is one, says why, or what was ruled out. A bulleted inventory of every file touched duplicates the diff.

## Typography

Weak signals, handled briefly because they deserve brief handling.

Plain ASCII everywhere: code, identifiers, commit messages, config, and prose. Straight quotes, `--` not an em dash, three periods not a single-character ellipsis, `->` not an arrow glyph. This is about grep, diffs, and terminals.

Applied to prose this is stricter than typography alone requires, and that is deliberate. Characters a person would not type on a keyboard are noise in a git repo.

If you find yourself reaching for `--` more than once or twice a page, the problem is usually not punctuation. A run of dashes is a paragraph that hasn't picked its sentence boundaries. Use a period, a colon, or parentheses.

## What not to do in the name of this guide

Don't manufacture the appearance of human authorship. Specifically: no deliberate typos, no invented TODO or HACK comments, no fabricated dated notes or initials, no commented-out code that was never live, no artificial inconsistency between files, and no performed uncertainty about things you're actually confident in.

All of these are false claims about how the artifact came to be, embedded in the artifact. That's worse than any stylistic tell, and it's the failure mode this guide is most likely to induce if read carelessly. Nothing here is about disguise. Every rule earns its place by making the file more useful to the next person who opens it, and if a rule stops doing that, it has no other argument in its favor.

## About this file

It follows its own rules. If it grows a summary section, a Contributing heading, or a run of twenty bold-lead bullets, that's a bug, and the fix is to edit it rather than to note the exception.

Delete anything here you disagree with. A style guide nobody follows costs more than no style guide, because it turns every review into an argument about the document instead of the code.
