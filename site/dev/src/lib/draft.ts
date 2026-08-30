/**
 * localStorage drafts for anything the maintainer types into the dev app before an explicit
 * submit (kantord/toylang#46): a dead dev server or a failed POST must never cost answers
 * already given. Written on every change, cleared only once the submit has been acknowledged.
 * Every read and write is guarded -- private browsing and a full quota both throw, and losing
 * reload survival is a smaller failure than the surface not rendering at all.
 */

export function loadDraft<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    return raw ? (JSON.parse(raw) as T) : fallback
  } catch {
    return fallback
  }
}

export function saveDraft(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // Nothing to do: the surface still works, just without reload survival.
  }
}

export function clearDraft(key: string) {
  try {
    localStorage.removeItem(key)
  } catch {
    // If the write above never worked either, there is nothing to undo.
  }
}
