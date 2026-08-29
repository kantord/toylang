/**
 * Fetch-and-swap navigation over the static per-page build (kantord/toylang#50), so a click
 * between docs pages or examples reads as one continuous session instead of a page load per
 * link (kantord/toylang#55). Intercepts same-origin clicks, resolves the target through
 * fetchRoute's cache, and re-renders the existing React root with the result -- no new bundle to
 * fetch, no white flash, just a route swap the root already knows how to reconcile.
 *
 * Falls back to a real navigation whenever fetchRoute can't resolve the target: a `pnpm dev`
 * load (never embeds the globals fetchRoute reads), a bare section root, or a genuine 404. That
 * fallback is what keeps this additive -- every link still works if this file were deleted.
 */
import { StrictMode } from "react"
import type { Root } from "react-dom/client"

import { App } from "@/App"
import { loadRoute } from "@/lib/fetchRoute"

function internalUrl(a: HTMLAnchorElement): URL | null {
  if (a.target && a.target !== "_self") return null
  if (a.hasAttribute("download")) return null
  const url = new URL(a.href, location.href)
  return url.origin === location.origin ? url : null
}

export function installClientNav(root: Root) {
  const scrollByPath = new Map<string, number>()
  history.scrollRestoration = "manual"

  let observer: IntersectionObserver | null = null

  function watchLinksForPrefetch() {
    observer?.disconnect()
    observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue
        observer!.unobserve(entry.target)
        const url = internalUrl(entry.target as HTMLAnchorElement)
        if (url) loadRoute(url.pathname).catch(() => {})
      }
    })
    document.querySelectorAll<HTMLAnchorElement>("#root a[href]").forEach((a) => {
      if (internalUrl(a)) observer!.observe(a)
    })
  }

  async function render(pathname: string): Promise<boolean> {
    const result = await loadRoute(pathname)
    if (!result) return false
    document.title = result.title
    root.render(
      <StrictMode>
        <App key={pathname} route={result.route} />
      </StrictMode>,
    )
    requestAnimationFrame(watchLinksForPrefetch)
    return true
  }

  document.addEventListener("click", (e) => {
    if (e.defaultPrevented || e.button !== 0) return
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return
    const a = (e.target as Element).closest?.("a")
    if (!(a instanceof HTMLAnchorElement)) return
    const url = internalUrl(a)
    if (!url || url.pathname === location.pathname) return
    e.preventDefault()
    scrollByPath.set(location.pathname, window.scrollY)
    render(url.pathname).then((ok) => {
      if (!ok) {
        location.href = url.href
        return
      }
      history.pushState(null, "", url.pathname + url.hash)
      window.scrollTo(0, 0)
    })
  })

  document.addEventListener("mouseover", (e) => {
    const a = (e.target as Element).closest?.("a")
    if (!(a instanceof HTMLAnchorElement)) return
    const url = internalUrl(a)
    if (url) loadRoute(url.pathname).catch(() => {})
  })

  window.addEventListener("popstate", () => {
    render(location.pathname).then((ok) => {
      if (!ok) {
        location.reload()
        return
      }
      window.scrollTo(0, scrollByPath.get(location.pathname) ?? 0)
    })
  })

  watchLinksForPrefetch()
}
