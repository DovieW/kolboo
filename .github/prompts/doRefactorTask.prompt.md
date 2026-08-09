---
description: "Implement one item from docs/Refactors with repo validation and safe git handoff."
agent: agent
---

# Implement Refactor TODO

1. Take an item from the refactor lists (do not skip big items) in the kolboo project and check if it was done already.

2. If it wasn't done yet, decide if this is a good change.

3. If you think it isn't a good change, ask me before implementing it.

4. Implement the TODO item. If this is a truly massive change, stop and propose a smaller breakdown before editing.

5. Remove the item completely from the refactor doc (do not mark done).

6. Run the smallest relevant format/validation command set from `AGENTS.md` and `docs/Dev Docs/TESTING.md`.

7. Do not commit or push unless I explicitly ask for that in the same invocation. If I do ask, keep the commit focused and summarize exactly what will be pushed first.

8. After you're done, as part of the summary, explain why this is a good change (if it even is).

9. Also give examples of where this code is used (either programmatically or by a user) and a scenario of what this change prevents.
